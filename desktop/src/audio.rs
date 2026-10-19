use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc, Arc, Mutex,
};

pub fn available() -> bool {
    cpal::default_host().default_input_device().is_some()
}

pub struct Capture {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub rate: u32,
    pub level: Arc<AtomicU32>,
    pub error: Arc<Mutex<Option<String>>>,
    stop: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Capture {
    pub fn start() -> Result<Self, String> {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let level = Arc::new(AtomicU32::new(0));
        let error = Arc::new(Mutex::new(None));
        let (stop, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (s, l, e) = (samples.clone(), level.clone(), error.clone());
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
                        stream::<f32>(&device, &config, channels, rate, s, l, e)?
                    }
                    cpal::SampleFormat::I16 => {
                        stream::<i16>(&device, &config, channels, rate, s, l, e)?
                    }
                    cpal::SampleFormat::U16 => {
                        stream::<u16>(&device, &config, channels, rate, s, l, e)?
                    }
                    cpal::SampleFormat::I32 => {
                        stream::<i32>(&device, &config, channels, rate, s, l, e)?
                    }
                    cpal::SampleFormat::F64 => {
                        stream::<f64>(&device, &config, channels, rate, s, l, e)?
                    }
                    _ => return Err("不支持的麦克风格式".into()),
                };
                stream
                    .play()
                    .map_err(|_| "麦克风启动失败，请检查系统权限")?;
                Ok::<_, String>((stream, rate))
            })();
            match setup {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    let _ = rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        });
        let rate = ready_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "麦克风准备超时，请重试")??;
        Ok(Self {
            samples,
            rate,
            level,
            error,
            stop,
            thread: Some(thread),
        })
    }
    pub fn position(&self) -> usize {
        self.samples.lock().unwrap().len()
    }
    pub fn stop(mut self) -> Result<(Vec<f32>, u32), String> {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(e) = self.error.lock().unwrap().clone() {
            return Err(e);
        }
        let data = self.samples.lock().unwrap().clone();
        Ok((data, self.rate))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rate: u32,
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let err = error.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut out = samples.lock().unwrap();
                let mut peak = 0f32;
                for frame in data.chunks(channels) {
                    let v = frame
                        .iter()
                        .map(|x| cpal::Sample::to_sample::<f32>(*x))
                        .sum::<f32>()
                        / channels as f32;
                    peak = peak.max(v.abs());
                    if out.len() < rate as usize * 120 {
                        out.push(v);
                    } else {
                        *error.lock().unwrap() = Some("录音超过120秒，请分段输入".into());
                        break;
                    }
                }
                level.store((peak.min(1.) * 10000.) as u32, Ordering::Relaxed);
            },
            move |_| {
                *err.lock().unwrap() = Some("麦克风连接中断，请重新录音".into());
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
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 16000);
    }
}
