//! Protocol-specific diagnostics using a fixed synthetic phrase, never the microphone.
use crate::{audio, models, store::ConnectionConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::time::{Duration, Instant};

pub fn validate_asr(config: &ConnectionConfig) -> Result<(), String> {
    if !matches!(config.protocol.as_str(), "openai_audio") {
        return Err("请选择支持的语音协议 / Select a supported speech protocol".into());
    }
    if config.model.trim().is_empty() {
        return Err("请先选择语音模型 / Select a speech model first".into());
    }
    models::endpoint(&config.base, "audio/transcriptions")?;
    Ok(())
}

fn sample() -> Result<(String, audio::Recording), String> {
    #[derive(serde::Deserialize)]
    struct Sample {
        input: String,
        sample_rate: u32,
        pcm16_base64: String,
    }
    let sample: Sample = serde_json::from_str(include_str!("asr_test_audio.json"))
        .map_err(|_| "内置测试音频无效 / Invalid test audio")?;
    let bytes = STANDARD
        .decode(sample.pcm16_base64)
        .map_err(|_| "内置测试音频无效 / Invalid test audio")?;
    if sample.sample_rate != audio::RATE || bytes.is_empty() || bytes.len() % 2 != 0 {
        return Err("内置测试音频格式无效 / Invalid test audio format".into());
    }
    let pcm: Vec<i16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| i16::from_le_bytes([v[0], v[1]]))
        .collect();
    Ok((sample.input, audio::Recording::from_pcm(&pcm)?))
}

pub async fn asr(config: &ConnectionConfig, key: &str) -> Result<models::ConnectionTest, String> {
    validate_asr(config)?;
    let (input, recording) = sample()?;
    let started = Instant::now();
    let output = tokio::time::timeout(Duration::from_secs(40), async {
        models::transcribe(config, key, recording.wav(0, recording.len())?, "en", &[])
            .await.map_err(String::from)
    })
    .await
    .map_err(|_| "语音测试超时，请检查服务状态 / Speech test timed out")??;
    if output.trim().is_empty() {
        return Err("服务未返回有效转写 / No final transcript returned".into());
    }
    Ok(models::ConnectionTest {
        input,
        output,
        latency_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        model: config.model.clone(),
        protocol: config.protocol.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalid_speech_config_fails_before_network() {
        for config in [
            ConnectionConfig {
                base: "http://127.0.0.1:1".into(),
                model: "test".into(),
                protocol: "openai_chat".into(),
            },
            ConnectionConfig {
                base: "http://127.0.0.1:1".into(),
                model: " ".into(),
                protocol: "openai_audio".into(),
            },
            ConnectionConfig {
                base: "https://secret@example.com".into(),
                model: "test".into(),
                protocol: "openai_realtime".into(),
            },
        ] {
            assert!(asr(&config, "").await.is_err());
        }
    }

    #[test]
    fn built_in_speech_is_short_non_silent_and_known() {
        let (phrase, recording) = sample().unwrap();
        assert_eq!(phrase, "This is a speech recognition test.");
        assert_eq!(recording.len(), 37611);
        assert!(recording.check().is_ok());
        assert!(recording
            .read(0, recording.len())
            .unwrap()
            .iter()
            .any(|s| s.abs() > 0.05));
        let wav = recording.wav(0, recording.len()).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

}
