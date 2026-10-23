//! Local voice activity detection; it never transcribes or uploads audio.
use webrtc_vad::{SampleRate, Vad, VadMode};

const FRAME_SAMPLES: usize = 320; // 20 ms at the capture pipeline's 16 kHz.
const FRAME_MS: u64 = 20;
const COUNTDOWN_PAUSE_MS: u64 = 500; // Display only; the stop deadline is unchanged.

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
        if !self.heard_speech || self.unavailable || self.silence_ms < COUNTDOWN_PAUSE_MS {
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
    vad: Option<Vad>,
    frame: Vec<i16>,
    gate: PauseGate,
}
impl Monitor {
    pub fn new() -> Self {
        // The binding panics if its native detector cannot be initialized.
        // A detector failure must never kill the recording writer or submit.
        let vad = std::panic::catch_unwind(|| {
            Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive)
        }).ok();
        let gate = PauseGate {
            status: Status { unavailable: vad.is_none(), ..Default::default() },
            ..Default::default()
        };
        Self {
            vad,
            frame: Vec::with_capacity(FRAME_SAMPLES),
            gate,
        }
    }
    fn classify_frame(&mut self) {
        let decision = self.vad.as_mut().and_then(|vad| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                vad.is_voice_segment(&self.frame)
            })).ok().and_then(Result::ok)
        });
        match decision {
            Some(voice) => self.gate.push(voice),
            None => self.gate.status.unavailable = true,
        }
    }
    pub fn push(&mut self, pcm: &[i16]) -> Status {
        if self.gate.status.unavailable {
            return self.gate.status;
        }
        for &sample in pcm {
            self.frame.push(sample);
            if self.frame.len() == FRAME_SAMPLES {
                self.classify_frame();
                self.frame.clear();
                if self.gate.status.unavailable { break; }
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
    fn minimum_delay_hides_countdown_during_speech_and_restarts_after_resume() {
        let mut g = PauseGate::default();
        advance(&mut g, true, 12);
        assert_eq!(g.status.countdown(2), None);
        advance(&mut g, false, 24);
        assert_eq!(g.status.countdown(2), None);
        g.push(false);
        assert_eq!(g.status.countdown(2), Some(2));
        advance(&mut g, false, 66);
        g.push(true);
        assert_eq!(g.status.countdown(2), None);
        assert!(!g.status.due(2));
        advance(&mut g, false, 99);
        assert!(!g.status.due(2));
        g.push(false);
        assert!(g.status.due(2));
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
    #[test]
    fn detector_error_is_sticky_and_cannot_submit_even_after_prior_speech() {
        let mut monitor = Monitor::new();
        advance(&mut monitor.gate, true, 12);
        advance(&mut monitor.gate, false, 1500);
        assert!(monitor.gate.status.due(5));
        // Exercise an actual native invalid-frame return, not a mocked status.
        monitor.frame = vec![0; 17];
        monitor.classify_frame();
        let status = monitor.push(&[0; 320]);
        assert!(status.unavailable);
        assert!(!status.due(5));
        assert_eq!(status.countdown(5), None);
        monitor.vad = None;
        monitor.classify_frame();
        assert!(!monitor.gate.status.due(2));
    }
    #[test]
    fn public_synthetic_speech_arms_real_vad_and_resuming_it_resets_deadline() {
        use base64::Engine;
        // Public connection-test fixture; no microphone, user recording or network.
        let fixture: serde_json::Value = serde_json::from_str(include_str!("asr_test_audio.json")).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(fixture["pcm16_base64"].as_str().unwrap()).unwrap();
        let pcm: Vec<i16> = bytes.as_chunks::<2>().0.iter()
            .map(|b| i16::from_le_bytes(*b)).collect();
        let mut monitor = Monitor::new();
        let status = monitor.push(&pcm);
        assert!(status.heard_speech && !status.unavailable);
        let mut stopped_at = None;
        for frame in 1..=300 {
            if monitor.push(&[0; FRAME_SAMPLES]).due(5) && stopped_at.is_none() {
                stopped_at = Some(frame * FRAME_MS);
            }
        }
        assert!(stopped_at.is_some_and(|ms| (20..=5500).contains(&ms)));
        let status = monitor.push(&pcm);
        assert!(!status.due(5));
        assert_eq!(status.countdown(5), None);
        assert!(status.heard_speech && !status.unavailable);
    }
}
