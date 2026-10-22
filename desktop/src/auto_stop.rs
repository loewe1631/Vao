//! Local voice activity detection; it never transcribes or uploads audio.
use webrtc_vad::{SampleRate, Vad, VadMode};

const FRAME_SAMPLES: usize = 320; // 20 ms at the capture pipeline's 16 kHz.
const FRAME_MS: u64 = 20;

#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    pub heard_speech: bool,
    pub silence_ms: u64,
    pub unavailable: bool,
}
impl Status {
    pub fn due(self, seconds: u8) -> bool {
        self.heard_speech && !self.unavailable && self.silence_ms >= seconds as u64 * 1000
    }
    pub fn countdown(self, seconds: u8) -> Option<u64> {
        if !self.heard_speech || self.unavailable {
            return None;
        }
        let remaining = (seconds as u64 * 1000).saturating_sub(self.silence_ms);
        (remaining <= 2000).then_some(remaining.div_ceil(1000).max(1))
    }
}

#[derive(Default)]
struct PauseGate {
    recent: u16,
    status: Status,
}
impl PauseGate {
    fn push(&mut self, voice: bool) {
        // Require 160 ms of voice within 240 ms before arming. A single
        // impulsive sound must not start the stop timer on an empty recording.
        self.recent = ((self.recent << 1) | u16::from(voice)) & 0x0fff;
        if self.recent.count_ones() >= 8 {
            self.status.heard_speech = true;
        }
        if voice {
            self.status.silence_ms = 0;
        } else if self.status.heard_speech {
            self.status.silence_ms = self.status.silence_ms.saturating_add(FRAME_MS);
        }
    }
}

pub struct Monitor {
    vad: Vad,
    frame: Vec<i16>,
    gate: PauseGate,
}
impl Monitor {
    pub fn new() -> Self {
        Self {
            vad: Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive),
            frame: Vec::with_capacity(FRAME_SAMPLES),
            gate: PauseGate::default(),
        }
    }
    pub fn push(&mut self, pcm: &[i16]) -> Status {
        if self.gate.status.unavailable {
            return self.gate.status;
        }
        for &sample in pcm {
            self.frame.push(sample);
            if self.frame.len() == FRAME_SAMPLES {
                match self.vad.is_voice_segment(&self.frame) {
                    Ok(voice) => self.gate.push(voice),
                    Err(()) => self.gate.status.unavailable = true,
                }
                self.frame.clear();
            }
        }
        self.gate.status
    }
}

/// Opt-in offline diagnostic using synthetic PCM, without microphone or model access.
pub fn verify_fixture(path: &str, seconds: u8) -> Result<serde_json::Value, String> {
    if !(2..=30).contains(&seconds) {
        return Err("Expected a silence delay of 2–30 seconds".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() % 2 != 0 || bytes.len() > 16_000 * 2 * 120 {
        return Err("Expected up to 120 seconds of mono 16 kHz signed PCM16".into());
    }
    let samples: Vec<i16> = bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
    let mut monitor = Monitor::new();
    let mut first_speech_ms = None;
    let mut first_stop_ms = None;
    let mut frames = 0;
    let mut status = Status::default();
    for frame in samples.as_chunks::<FRAME_SAMPLES>().0 {
        frames += 1;
        status = monitor.push(frame);
        if status.heard_speech && first_speech_ms.is_none() {
            first_speech_ms = Some(frames * FRAME_MS);
        }
        if status.due(seconds) && first_stop_ms.is_none() {
            first_stop_ms = Some(frames * FRAME_MS);
        }
    }
    Ok(serde_json::json!({"input_ms":samples.len() as u64 / 16,"silence_seconds":seconds,
        "first_speech_ms":first_speech_ms,"first_stop_ms":first_stop_ms,"unavailable":status.unavailable,
        "microphone":false,"model_requests":0}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn advance(g: &mut PauseGate, voice: bool, frames: usize) {
        for _ in 0..frames { g.push(voice); }
    }
    #[test]
    fn silence_before_first_speech_never_finishes() {
        let mut g = PauseGate::default();
        advance(&mut g, false, 2000);
        assert!(!g.status.due(2));
        assert_eq!(g.status.countdown(5), None);
        advance(&mut g, true, 4);
        advance(&mut g, false, 1500);
        assert!(!g.status.heard_speech);
    }
    #[test]
    fn pause_deadline_and_countdown_follow_audio_time() {
        let mut g = PauseGate::default();
        advance(&mut g, true, 12);
        advance(&mut g, false, 149);
        assert_eq!(g.status.countdown(5), None);
        advance(&mut g, false, 1);
        assert_eq!(g.status.countdown(5), Some(2));
        advance(&mut g, false, 50);
        assert_eq!(g.status.countdown(5), Some(1));
        advance(&mut g, false, 49);
        assert!(!g.status.due(5));
        advance(&mut g, false, 1);
        assert!(g.status.due(5));
        assert!(!g.status.due(30));
    }
    #[test]
    fn resumed_speech_resets_pause_and_each_monitor_starts_clean() {
        let mut g = PauseGate::default();
        advance(&mut g, true, 12);
        advance(&mut g, false, 240);
        g.push(true);
        assert_eq!(g.status.countdown(5), None);
        advance(&mut g, false, 249);
        assert!(!g.status.due(5));
        advance(&mut g, false, 1);
        assert!(g.status.due(5));
        assert!(!PauseGate::default().status.due(5));
    }
    #[test]
    fn actual_vad_accepts_arbitrary_buffer_boundaries_and_quiet_audio() {
        let mut monitor = Monitor::new();
        for _ in 0..1000 {
            let status = monitor.push(&[0; 317]);
            assert!(!status.heard_speech);
            assert!(!status.unavailable);
        }
        assert!(monitor.frame.len() < FRAME_SAMPLES);
        assert!(!Status {heard_speech:true,silence_ms:30_000,unavailable:true}.due(2));
    }
}
