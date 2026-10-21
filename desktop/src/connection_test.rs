//! Protocol-specific diagnostics using a fixed synthetic phrase, never the microphone.
use crate::{audio, models, realtime, store::ConnectionConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::time::{Duration, Instant};

pub fn validate_asr(config: &ConnectionConfig) -> Result<(), String> {
    if !matches!(config.protocol.as_str(), "openai_audio" | "openai_realtime") {
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
        match config.protocol.as_str() {
            "openai_audio" => {
                models::transcribe(config, key, recording.wav(0, recording.len())?, "en", &[])
                    .await
                    .map_err(String::from)
            }
            "openai_realtime" => {
                let (stop, receiver) = tokio::sync::oneshot::channel();
                let _ = stop.send(recording.len());
                // Keep the adapter's error. Diagnostic tests must never use file fallback.
                realtime::run(config, key, "en", &[], recording, receiver, |_| {}).await
            }
            _ => unreachable!(),
        }
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
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::Message;

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

    #[tokio::test]
    async fn speech_realtime_test_requires_negotiation_and_final_transcript() {
        for outcome in ["complete", "rejected", "empty", "format", "closed"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let config = ConnectionConfig {
                base: format!("http://{}/v1", listener.local_addr().unwrap()),
                model: "synthetic-realtime".into(),
                protocol: "openai_realtime".into(),
            };
            let task = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
                let setup: Value =
                    serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(
                    setup["session"]["audio"]["input"]["transcription"]["model"],
                    "synthetic-realtime"
                );
                if outcome == "rejected" {
                    ws.send(Message::Text(json!({"type":"error","error":{"message":"synthetic-private-provider-detail"}}).to_string().into())).await.unwrap();
                } else if outcome != "closed" {
                    let mut session = setup["session"].clone();
                    if outcome == "format" {
                        session["audio"]["input"]["format"]["rate"] = json!(16000);
                    }
                    ws.send(Message::Text(
                        json!({"type":"session.updated","session":session})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                    if outcome != "format" {
                        let mut bytes = 0;
                        while let Some(Ok(Message::Text(text))) = ws.next().await {
                            let event: Value = serde_json::from_str(&text).unwrap();
                            if event["type"] == "input_audio_buffer.append" {
                                bytes += STANDARD
                                    .decode(event["audio"].as_str().unwrap())
                                    .unwrap()
                                    .len();
                            }
                            if event["type"] == "input_audio_buffer.commit" {
                                assert_eq!(bytes, 112832);
                                ws.send(Message::Text(json!({"type":"input_audio_buffer.committed","item_id":"test-final"}).to_string().into())).await.unwrap();
                                let transcript = if outcome == "empty" {
                                    " "
                                } else {
                                    "This is a speech recognition test."
                                };
                                ws.send(Message::Text(json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"test-final","transcript":transcript}).to_string().into())).await.unwrap();
                                break;
                            }
                        }
                    }
                }
                drop(ws);
                // No HTTP request or second connection may disguise a failed realtime test.
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), listener.accept())
                        .await
                        .is_err()
                );
            });
            let result = asr(&config, "").await;
            if outcome == "complete" {
                let result = result.unwrap();
                assert_eq!(result.protocol, "openai_realtime");
                assert_eq!(result.input, result.output);
                assert_eq!(result.model, config.model);
            } else {
                let error = result.err().unwrap();
                assert!(!error.contains("synthetic-private-provider-detail"));
            }
            task.await.unwrap();
        }
    }
}
