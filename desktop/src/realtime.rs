//! Public OpenAI Realtime transcription contract. No provider-private protocol.
use crate::{audio, models, store::ConnectionConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::HeaderValue,
    protocol::{Message, WebSocketConfig},
};

pub struct Handle {
    pub stop: oneshot::Sender<usize>,
    pub result: oneshot::Receiver<Result<String, String>>,
}
pub async fn finish(handle: Handle, end: usize) -> Option<String> {
    let _ = handle.stop.send(end);
    tokio::time::timeout(Duration::from_secs(12), handle.result)
        .await
        .ok()?
        .ok()?
        .ok()
        .filter(|s| !s.trim().is_empty())
}
#[derive(Default)]
struct Transcript {
    pending: Option<usize>,
    order: BTreeMap<usize, String>,
    partial: HashMap<String, String>,
    final_text: HashMap<String, String>,
}
impl Transcript {
    fn event(&mut self, v: &Value) -> Result<(), String> {
        let kind = v["type"].as_str().unwrap_or("");
        if matches!(
            kind,
            "error" | "conversation.item.input_audio_transcription.failed"
        ) {
            return Err("实时转写服务返回错误 / Realtime transcription failed".into());
        }
        let id = v["item_id"].as_str().unwrap_or("");
        if id.len() > 200 {
            return Err("Invalid realtime item".into());
        }
        match kind {
            "input_audio_buffer.committed" => {
                if self.order.values().any(|item| item == id) {
                    return Ok(());
                }
                let end = self.pending.take().ok_or("Unexpected realtime commit")?;
                if id.is_empty() {
                    return Err("Missing realtime item".into());
                }
                self.order.insert(end, id.to_owned());
            }
            "conversation.item.input_audio_transcription.delta" => {
                if id.is_empty() || self.final_text.contains_key(id) {
                    return Ok(());
                }
                if self.partial.len() > 4 && !self.partial.contains_key(id) {
                    return Err("Too many realtime items".into());
                }
                let text = self.partial.entry(id.into()).or_default();
                text.push_str(v["delta"].as_str().unwrap_or(""));
                if text.len() > 64000 {
                    return Err("Realtime text too long".into());
                }
            }
            "conversation.item.input_audio_transcription.completed" => {
                let text = v["transcript"]
                    .as_str()
                    .ok_or("Missing realtime transcript")?;
                if id.is_empty() || text.len() > 64000 {
                    return Err("Invalid realtime transcript".into());
                }
                if let Some(old) = self.final_text.get(id) {
                    if old != text {
                        return Err("Conflicting realtime final".into());
                    }
                    return Ok(());
                }
                if self.final_text.len() >= 500 {
                    return Err("Realtime session requires final file verification".into());
                }
                self.final_text.insert(id.into(), text.into());
                self.partial.remove(id);
            }
            _ => {}
        }
        Ok(())
    }
    fn waiting(&self) -> bool {
        self.pending.is_some()
            || self
                .order
                .values()
                .any(|id| !self.final_text.contains_key(id))
    }
    fn text(&self) -> String {
        self.order
            .values()
            .filter_map(|id| self.final_text.get(id))
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn preview(&self) -> String {
        let mut s = self.text();
        for p in self.partial.values() {
            s.push_str(p);
        }
        s.chars()
            .rev()
            .take(400)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }
}
fn pcm24(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 3);
    for i in 0..samples.len() * 3 / 2 {
        let a = i * 2 / 3;
        let f = (i * 2 % 3) as f32 / 3.;
        let x = samples[a] * (1. - f) + samples.get(a + 1).copied().unwrap_or(samples[a]) * f;
        out.extend_from_slice(&((x.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    out
}
pub async fn run(
    config: &ConnectionConfig,
    key: &str,
    language: &str,
    words: &[String],
    recording: audio::Recording,
    mut stop: oneshot::Receiver<usize>,
    mut preview: impl FnMut(String),
) -> Result<String, String> {
    let mut url = url::Url::parse(&models::endpoint(&config.base, "realtime")?)
        .map_err(|_| "Invalid realtime endpoint")?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme)
        .map_err(|_| "Invalid realtime scheme")?;
    url.set_query(Some("intent=transcription"));
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| "实时服务地址无效")?;
    if !key.is_empty() {
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|_| "Invalid realtime credential")?,
        );
    }
    let mut limits = WebSocketConfig::default();
    limits.max_message_size = Some(524288);
    limits.max_frame_size = Some(524288);
    let (mut ws, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async_with_config(request, Some(limits), false),
    )
    .await
    .map_err(|_| "实时服务连接超时")?
    .map_err(|_| "实时服务不可用；请检查协议支持 / Realtime connection unavailable")?;
    let mut transcription = json!({"model":config.model});
    if language != "auto" {
        transcription["language"] = json!(language);
    }
    if !words.is_empty() {
        transcription["prompt"] = json!(words.join(", "));
    }
    let session = json!({"type":"session.update","session":{"type":"transcription","audio":{"input":{"format":{"type":"audio/pcm","rate":24000},"transcription":transcription,"turn_detection":null}}}});
    ws.send(Message::Text(session.to_string().into()))
        .await
        .map_err(|_| "Realtime setup failed")?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = ws.next().await {
            if let Message::Text(text) = event.map_err(|_| "Realtime setup failed")? {
                let v: Value = serde_json::from_str(&text).map_err(|_| "Invalid realtime event")?;
                if v["type"] == "error" {
                    return Err("实时配置被拒绝，请确认服务与模型支持 OpenAI Realtime 转写 / Realtime configuration rejected; verify service and model support");
                }
                if v["type"] == "session.updated" {
                    let input = &v["session"]["audio"]["input"];
                    if input["format"]["type"] != "audio/pcm"
                        || input["format"]["rate"] != 24000
                        || !input["turn_detection"].is_null()
                    {
                        return Err("Realtime audio format was not accepted");
                    }
                    return Ok(());
                }
            }
        }
        Err("Realtime connection closed")
    })
    .await
    .map_err(|_| "Realtime setup timeout")??;
    let mut state = Transcript::default();
    let mut sent = 0;
    let mut committed = 0;
    let mut end = None;
    let mut stopping = None;
    let mut pending_since = None;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            result=&mut stop, if end.is_none() => {end=Some(result.map_err(|_|"Realtime cancelled")?);stopping=Some(Instant::now());},
            event=ws.next()=> {
                let message=event.ok_or("Realtime closed before completion")?.map_err(|_|"Realtime receive failed")?;
                match message {
                    Message::Text(text)=> {let value:Value=serde_json::from_str(&text).map_err(|_|"Invalid realtime JSON")?;state.event(&value)?;preview(state.preview());},
                    Message::Close(_)=>return Err("Realtime closed before completion".into()),
                    Message::Ping(bytes)=>{ws.send(Message::Pong(bytes)).await.map_err(|_|"Realtime keepalive failed")?;},
                    _=>{}
                }
            },
            _=tick.tick()=>{}
        }
        if stopping.is_some_and(|t: Instant| t.elapsed() > Duration::from_secs(10))
            || pending_since.is_some_and(|t: Instant| t.elapsed() > Duration::from_secs(25))
        {
            return Err("Realtime final timeout".into());
        }
        let available = end.unwrap_or_else(|| recording.len());
        if available.saturating_sub(sent) > audio::RATE as usize * 20 {
            return Err("Realtime upload fell behind".into());
        }
        if available > sent && (available - sent >= 8000 || end.is_some()) {
            let to = (sent + 8000).min(available);
            let input = recording.read(sent, to)?;
            let bytes = pcm24(&input);
            let append = json!({"type":"input_audio_buffer.append","audio":STANDARD.encode(bytes)});
            tokio::time::timeout(
                Duration::from_secs(4),
                ws.send(Message::Text(append.to_string().into())),
            )
            .await
            .map_err(|_| "Realtime send timeout")?
            .map_err(|_| "Realtime send failed")?;
            sent = to;
        }
        if !state.waiting() {
            pending_since = None;
        }
        if !state.waiting()
            && sent > committed
            && (sent - committed >= audio::RATE as usize * 8 || end == Some(sent))
        {
            if sent - committed < 1600 {
                let pad = vec![0u8; (1600 - (sent - committed)) * 3];
                ws.send(Message::Text(
                    json!({"type":"input_audio_buffer.append","audio":STANDARD.encode(pad)})
                        .to_string()
                        .into(),
                ))
                .await
                .map_err(|_| "Realtime final padding failed")?;
            }
            state.pending = Some(sent);
            pending_since = Some(Instant::now());
            committed = sent;
            ws.send(Message::Text(
                json!({"type":"input_audio_buffer.commit"})
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|_| "Realtime commit failed")?;
        }
        if end == Some(sent) && committed == sent && !state.waiting() && !state.order.is_empty() {
            let text = state.text();
            return if text.trim().is_empty() {
                Err("Realtime recognized no speech".into())
            } else {
                Ok(text)
            };
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordering_duplicates_and_unmatched_turns_are_checked() {
        let mut t = Transcript {
            pending: Some(100),
            ..Default::default()
        };
        t.event(&json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"b","transcript":"second"})).unwrap();
        assert!(t.waiting());
        t.event(&json!({"type":"input_audio_buffer.committed","item_id":"a"}))
            .unwrap();
        t.event(&json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"a","transcript":"first"})).unwrap();
        t.pending = Some(200);
        t.event(&json!({"type":"input_audio_buffer.committed","item_id":"b"}))
            .unwrap();
        assert_eq!(t.text(), "first\nsecond");
        assert!(!t.waiting());
        t.event(&json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"a","transcript":"first"})).unwrap();
        assert_eq!(t.text(), "first\nsecond");
        assert!(t
            .event(&json!({"type":"conversation.item.input_audio_transcription.failed"}))
            .is_err());
        assert_eq!(pcm24(&vec![0.; 16000]).len(), 48000);
    }
    // tungstenite requires its concrete HTTP response as the handshake error type.
    #[allow(clippy::result_large_err)]
    #[tokio::test]
    async fn websocket_contract_covers_final_flush_and_negotiation_failure() {
        for valid in [true, false] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let config = ConnectionConfig {
                base: format!("http://{}/v1", listener.local_addr().unwrap()),
                model: "synthetic-transcriber".into(),
                protocol: "openai_realtime".into(),
            };
            let task = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut ws=tokio_tungstenite::accept_hdr_async(socket,|req:&tokio_tungstenite::tungstenite::handshake::server::Request,res:tokio_tungstenite::tungstenite::handshake::server::Response|{
                    assert_eq!(req.uri().to_string(),"/v1/realtime?intent=transcription");
                    assert_eq!(req.headers()["authorization"],"Bearer synthetic-ws-key");Ok(res)
                }).await.unwrap();
                let setup: Value =
                    serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(
                    setup["session"]["audio"]["input"]["transcription"]["model"],
                    "synthetic-transcriber"
                );
                let mut session = setup["session"].clone();
                if !valid {
                    session["audio"]["input"]["format"]["rate"] = json!(16000);
                }
                ws.send(Message::Text(
                    json!({"type":"session.updated","session":session})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                if !valid {
                    return 0;
                }
                let mut bytes = 0;
                while let Some(Ok(message)) = ws.next().await {
                    if let Message::Text(text) = message {
                        let event: Value = serde_json::from_str(&text).unwrap();
                        if event["type"] == "input_audio_buffer.append" {
                            bytes += STANDARD
                                .decode(event["audio"].as_str().unwrap())
                                .unwrap()
                                .len();
                        }
                        if event["type"] == "input_audio_buffer.commit" {
                            ws.send(Message::Text(
                                json!({"type":"input_audio_buffer.committed","item_id":"final"})
                                    .to_string()
                                    .into(),
                            ))
                            .await
                            .unwrap();
                            ws.send(Message::Text(json!({"type":"conversation.item.input_audio_transcription.delta","item_id":"final","delta":"临时"}).to_string().into())).await.unwrap();
                            ws.send(Message::Text(json!({"type":"conversation.item.input_audio_transcription.completed","item_id":"final","transcript":"最后一句完整。"}).to_string().into())).await.unwrap();
                            return bytes;
                        }
                    }
                }
                bytes
            });
            let recording = audio::Recording::fixture(&vec![123; audio::RATE as usize * 2]);
            let (stop_tx, stop_rx) = oneshot::channel();
            stop_tx.send(recording.len()).unwrap();
            let mut previews = vec![];
            let result = run(
                &config,
                "synthetic-ws-key",
                "zh",
                &[],
                recording,
                stop_rx,
                |t| previews.push(t),
            )
            .await;
            let bytes = task.await.unwrap();
            if valid {
                assert_eq!(result.unwrap(), "最后一句完整。");
                assert_eq!(bytes, 96000);
                assert!(previews.iter().any(|s| s.contains("临时")));
            } else {
                assert!(result.is_err());
            }
        }
    }
}
