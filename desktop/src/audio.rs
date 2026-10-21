#[cfg(not(windows))]
use std::fs::OpenOptions;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(unix)]
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::{
    fs::File,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
};

pub const RATE: u32 = 16000;
pub fn available() -> bool {
    cpal::default_host().default_input_device().is_some()
}

// An unlinked, owner-only session file keeps long recordings out of an unbounded
// RAM buffer. It has no pathname during capture and closes on cancel/drop.
struct Spool {
    file: File,
    #[cfg(windows)]
    io: Mutex<()>,
    len: AtomicUsize,
    signal: AtomicBool,
}
impl Spool {
    fn new() -> Result<Self, String> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| "无法准备录音缓冲")?;
        let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!(".vao-audio-{name}"));
        #[cfg(not(windows))]
        let mut options = OpenOptions::new();
        #[cfg(not(windows))]
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        #[cfg(not(windows))]
        let file = options
            .open(&path)
            .map_err(|_| "无法准备临时录音缓冲 / Audio buffer unavailable")?;
        #[cfg(not(windows))]
        std::fs::remove_file(&path).map_err(|_| "无法保护临时录音缓冲")?;
        #[cfg(windows)]
        let file = crate::windows_files::private_audio(&path)?;
        Ok(Self {
            file,
            #[cfg(windows)]
            io: Mutex::new(()),
            len: AtomicUsize::new(0),
            signal: AtomicBool::new(false),
        })
    }
    fn append(&self, pcm: &[i16]) -> Result<(), String> {
        let pos = self.len.load(Ordering::Acquire);
        let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.write_at(&bytes, (pos as u64) * 2)
            .map_err(|_| "录音缓冲写入失败，已保留此前录音 / Audio buffer interrupted")?;
        if pcm.iter().any(|s| s.unsigned_abs() >= 4) {
            self.signal.store(true, Ordering::Relaxed);
        }
        self.len.store(pos + pcm.len(), Ordering::Release);
        Ok(())
    }
    fn read(&self, start: usize, end: usize) -> Result<Vec<f32>, String> {
        let len = self.len.load(Ordering::Acquire);
        if start > end || end > len || end - start > RATE as usize * 120 {
            return Err("无效的音频片段 / Invalid audio range".into());
        }
        let mut bytes = vec![0; (end - start) * 2];
        self.read_at(&mut bytes, start as u64 * 2)
            .map_err(|_| "无法读取本次录音")?;
        Ok(bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| i16::from_le_bytes([v[0], v[1]]) as f32 / 32767.)
            .collect())
    }
    fn write_at(&self, bytes: &[u8], offset: u64) -> std::io::Result<()> {
        #[cfg(unix)]
        { self.file.write_all_at(bytes, offset) }
        #[cfg(windows)]
        {
            use std::io::{Seek, SeekFrom, Write};
            let _guard = self.io.lock().unwrap();
            let mut file = &self.file;
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(bytes)
        }
    }
    fn read_at(&self, bytes: &mut [u8], offset: u64) -> std::io::Result<()> {
        #[cfg(unix)]
        { self.file.read_exact_at(bytes, offset) }
        #[cfg(windows)]
        {
            use std::io::{Read, Seek, SeekFrom};
            let _guard = self.io.lock().unwrap();
            let mut file = &self.file;
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(bytes)
        }
    }
}
#[derive(Clone)]
pub struct Recording {
    spool: Arc<Spool>,
    pub warning: Option<String>,
}
impl Recording {
    pub(crate) fn from_pcm(samples: &[i16]) -> Result<Self, String> {
        let spool = Arc::new(Spool::new()?);
        spool.append(samples)?;
        Ok(Self { spool, warning: None })
    }
    #[cfg(test)]
    pub(crate) fn fixture(samples: &[i16]) -> Self {
        Self::from_pcm(samples).unwrap()
    }

    pub fn len(&self) -> usize {
        self.spool.len.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), crate::input_feedback::InputNotice> {
        use crate::input_feedback::InputNotice;
        if !self.spool.signal.load(Ordering::Relaxed) {
            Err(InputNotice::NoAudio)
        } else if self.len() < RATE as usize / 4 {
            Err(InputNotice::TooShort)
        } else {
            Ok(())
        }
    }
    pub fn read(&self, start: usize, end: usize) -> Result<Vec<f32>, String> {
        self.spool.read(start, end)
    }
    pub fn wav(&self, start: usize, end: usize) -> Result<Vec<u8>, String> {
        Ok(wav(&self.read(start, end)?, RATE))
    }
    // Prefer a quiet boundary within the last four seconds. A short overlap and
    // text deduplication cover a word that crosses a hard boundary.
    pub fn ranges(&self, start: usize, end: usize) -> Result<Vec<(usize, usize)>, String> {
        let mut out = vec![];
        let mut at = start;
        let max = RATE as usize * 90;
        while at < end {
            let mut next = (at + max).min(end);
            if next < end {
                let search = next - RATE as usize * 4;
                let pcm = self.read(search, next)?;
                let quiet = RATE as usize / 5;
                if let Some((index, _)) = pcm
                    .chunks(quiet)
                    .enumerate()
                    .rev()
                    .find(|(_, chunk)| chunk.iter().all(|v| v.abs() < 0.008))
                {
                    next = search + index * quiet + quiet;
                }
            }
            out.push((at, next));
            if next == end {
                break;
            }
            at = next - RATE as usize / 4;
        }
        Ok(out)
    }
}
pub struct Capture {
    recording: Recording,
    pub level: Arc<AtomicU32>,
    stop: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
    writer: Option<std::thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}
impl Capture {
    pub fn start() -> Result<Self, String> {
        let spool = Arc::new(Spool::new()?);
        let level = Arc::new(AtomicU32::new(0));
        let error = Arc::new(Mutex::new(None));
        let (audio_tx, audio_rx) = mpsc::sync_channel::<Vec<i16>>(64);
        let output = spool.clone();
        let writer_error = error.clone();
        let writer = std::thread::spawn(move || {
            while let Ok(pcm) = audio_rx.recv() {
                if let Err(e) = output.append(&pcm) {
                    *writer_error.lock().unwrap() = Some(e);
                    break;
                }
            }
        });
        let (stop, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let l = level.clone();
        let e = error.clone();
        let thread = std::thread::spawn(move || {
            let setup = (|| {
                let device = cpal::default_host()
                    .default_input_device()
                    .ok_or("未找到麦克风 / No microphone")?;
                let supported = device
                    .default_input_config()
                    .map_err(|_| "无法读取麦克风配置")?;
                let rate = supported.sample_rate().0;
                let channels = supported.channels() as usize;
                let config = supported.config();
                let stream = match supported.sample_format() {
                    cpal::SampleFormat::F32 => {
                        stream::<f32>(&device, &config, channels, rate, audio_tx, l, e)?
                    }
                    cpal::SampleFormat::I16 => {
                        stream::<i16>(&device, &config, channels, rate, audio_tx, l, e)?
                    }
                    cpal::SampleFormat::U16 => {
                        stream::<u16>(&device, &config, channels, rate, audio_tx, l, e)?
                    }
                    cpal::SampleFormat::I32 => {
                        stream::<i32>(&device, &config, channels, rate, audio_tx, l, e)?
                    }
                    cpal::SampleFormat::F64 => {
                        stream::<f64>(&device, &config, channels, rate, audio_tx, l, e)?
                    }
                    _ => return Err("不支持的麦克风格式".into()),
                };
                stream
                    .play()
                    .map_err(|_| "麦克风启动失败，请检查系统权限")?;
                Ok::<_, String>(stream)
            })();
            match setup {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "麦克风准备超时，请重试")??;
        Ok(Self {
            recording: Recording {
                spool,
                warning: None,
            },
            level,
            stop,
            thread: Some(thread),
            writer: Some(writer),
            error,
        })
    }
    pub fn position(&self) -> usize {
        self.recording.len()
    }
    pub fn reader(&self) -> Recording {
        self.recording.clone()
    }
    pub fn stop(mut self) -> Result<Recording, String> {
        self.close();
        let error = self.error.lock().unwrap().clone();
        if self.recording.len() == 0 {
            if let Some(e) = error {
                return Err(e);
            }
        }
        let mut recording = self.recording.clone();
        recording.warning = error;
        Ok(recording)
    }
    fn close(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.writer.take() {
            let _ = t.join();
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.close();
    }
}
struct Resampler {
    index: usize,
    next: f64,
    previous: f32,
    step: f64,
}
impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            index: 0,
            next: 0.,
            previous: 0.,
            step: rate as f64 / RATE as f64,
        }
    }
    fn push(&mut self, value: f32, out: &mut Vec<i16>) {
        while self.next <= self.index as f64 {
            let t = (self.next - (self.index as f64 - 1.)).clamp(0., 1.) as f32;
            let v = if self.index == 0 {
                value
            } else {
                self.previous * (1. - t) + value * t
            };
            out.push((v.clamp(-1., 1.) * 32767.) as i16);
            self.next += self.step;
        }
        self.previous = value;
        self.index += 1;
    }
}
fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rate: u32,
    tx: mpsc::SyncSender<Vec<i16>>,
    level: Arc<AtomicU32>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let err = error.clone();
    let mut resampler = Resampler::new(rate);
    let mut failed = false;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if failed {
                    return;
                }
                let mut pcm = Vec::with_capacity(data.len() / channels);
                let mut peak = 0f32;
                for frame in data.chunks(channels) {
                    let v = frame
                        .iter()
                        .map(|x| cpal::Sample::to_sample::<f32>(*x))
                        .sum::<f32>()
                        / channels as f32;
                    peak = peak.max(v.abs());
                    resampler.push(if v.is_finite() { v } else { 0. }, &mut pcm);
                }
                level.store((peak.min(1.) * 10000.) as u32, Ordering::Relaxed);
                if tx.try_send(pcm).is_err() {
                    failed = true;
                    *error.lock().unwrap() =
                        Some("录音缓冲繁忙，已保留此前录音 / Audio buffer interrupted".into());
                }
            },
            move |_| {
                *err.lock().unwrap() =
                    Some("麦克风连接中断，已保留此前录音 / Microphone disconnected".into());
            },
            None,
        )
        .map_err(|_| "无法打开麦克风，请检查设备与权限".into())
}
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let count = (samples.len() as u64 * 16000 / rate as u64) as usize;
    let mut out = Vec::with_capacity(44 + count * 2);
    out.extend(b"RIFF");
    out.extend((36 + count as u32 * 2).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(16000u32.to_le_bytes());
    out.extend(32000u32.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((count as u32 * 2).to_le_bytes());
    for i in 0..count {
        let pos = i as f64 * rate as f64 / 16000.;
        let a = pos as usize;
        let f = (pos - a as f64) as f32;
        let v = samples.get(a).copied().unwrap_or(0.) * (1. - f)
            + samples.get(a + 1).copied().unwrap_or(0.) * f;
        out.extend(((v.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resampling_header_and_length() {
        let w = wav(&vec![0.; 48000], 48000);
        assert_eq!(w.len(), 32044);
        assert_eq!(&w[..4], b"RIFF");
    }
    #[test]
    fn long_audio_is_retained_and_final_ranges_cover_it() {
        let spool = Arc::new(Spool::new().unwrap());
        for second in 0..131 {
            let mut pcm = vec![1000; RATE as usize];
            if second == 130 {
                pcm[15999] = 12345;
            }
            spool.append(&pcm).unwrap();
        }
        let r = Recording {
            spool,
            warning: None,
        };
        assert_eq!(r.len(), 131 * RATE as usize);
        assert!(r.check().is_ok());
        let ranges = r.ranges(0, r.len()).unwrap();
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges.last().unwrap().1, r.len());
        assert!(ranges.windows(2).all(|p| p[0].1 >= p[1].0));
        assert!(ranges.iter().all(|(a, b)| b - a <= 90 * RATE as usize));
        assert!((r.read(r.len() - 1, r.len()).unwrap()[0] - 12345. / 32767.).abs() < 0.0001);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let m = r.spool.file.metadata().unwrap();
            assert_eq!(m.nlink(), 0);
            assert_eq!(m.mode() & 0o777, 0o600);
        }
    }
    #[test]
    fn resampler_is_continuous_across_callbacks() {
        let mut r = Resampler::new(48000);
        let mut pcm = vec![];
        for _ in 0..48000 {
            r.push(0.2, &mut pcm);
        }
        assert_eq!(pcm.len(), 16000);
        assert!(pcm.iter().all(|v| (*v - 6553).abs() <= 1));
    }
}
