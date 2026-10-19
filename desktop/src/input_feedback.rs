use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputNotice {
    NoAudio,
    TooShort,
    NoSpeech,
    NoMicrophone,
    MicrophoneUnavailable,
    SelectionUnreadable,
    DeliveryTargetUnavailable,
}

pub enum FlowError {
    Notice(InputNotice),
    Error(String),
}
impl From<String> for FlowError {
    fn from(value: String) -> Self {
        Self::Error(value)
    }
}
impl From<&str> for FlowError {
    fn from(value: &str) -> Self {
        Self::Error(value.into())
    }
}
impl From<InputNotice> for FlowError {
    fn from(value: InputNotice) -> Self {
        Self::Notice(value)
    }
}
impl From<crate::models::ModelError> for FlowError {
    fn from(value: crate::models::ModelError) -> Self {
        if value.no_speech {
            Self::Notice(InputNotice::NoSpeech)
        } else {
            Self::Error(value.message)
        }
    }
}

/// Only reject definite silence. Quiet speech is left to ASR; no aggressive VAD cutoff.
pub fn check_audio(samples: &[f32], rate: u32) -> Result<(), InputNotice> {
    if samples.is_empty() || samples.iter().all(|s| !s.is_finite() || s.abs() < 0.0001) {
        return Err(InputNotice::NoAudio);
    }
    if samples.len() < rate as usize / 4 {
        return Err(InputNotice::TooShort);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_silent_short_and_quiet_audio_are_distinct() {
        assert_eq!(check_audio(&[], 16000), Err(InputNotice::NoAudio));
        assert_eq!(
            check_audio(&vec![0.; 16000], 16000),
            Err(InputNotice::NoAudio)
        );
        assert_eq!(
            check_audio(&vec![0.01; 1000], 16000),
            Err(InputNotice::TooShort)
        );
        assert!(check_audio(&vec![0.0002; 16000], 16000).is_ok());
    }
}
