use crate::store::ConnectionConfig;
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(serde::Serialize)]
pub struct ConnectionTest {
    pub input: String,
    pub output: String,
    pub latency_ms: u64,
    pub model: String,
}
pub async fn test_connection(
    config: &ConnectionConfig,
    key: &str,
) -> Result<ConnectionTest, ModelError> {
    let input = "明天开会，不对，后天开会。";
    let started = std::time::Instant::now();
    let output = refine(
        config,
        key,
        input,
        "你是语音转写整理工具。保留最终改口，只输出整理文本。",
        None,
        &[],
    )
    .await?;
    Ok(ConnectionTest {
        input: input.into(),
        output,
        latency_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        model: config.model.clone(),
    })
}

pub struct ModelError {
    pub message: String,
    pub retryable: bool,
    pub no_speech: bool,
}
impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl From<String> for ModelError {
    fn from(message: String) -> Self {
        Self {
            message,
            retryable: false,
            no_speech: false,
        }
    }
}
pub fn endpoint(base: &str, path: &str) -> Result<String, String> {
    let mut u =
        url::Url::parse(base.trim()).map_err(|_| "请输入完整服务地址 / Invalid service URL")?;
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("服务地址不能包含凭据、查询或片段".into());
    }
    let local = matches!(
        u.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if u.scheme() != "https" && !(u.scheme() == "http" && local) {
        return Err("远程服务必须使用HTTPS；HTTP仅限本机".into());
    }
    let p = u.path().trim_end_matches('/');
    let p = p
        .strip_suffix("/audio/transcriptions")
        .or_else(|| p.strip_suffix("/chat/completions"))
        .or_else(|| p.strip_suffix("/models"))
        .unwrap_or(p);
    let prefix = if p.is_empty() { "/v1" } else { p };
    let full = format!("{prefix}/{path}");
    u.set_path(&full);
    Ok(u.into())
}
pub fn client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| "网络初始化失败".into())
}
pub(crate) async fn read(response: reqwest::Response) -> Result<Value, ModelError> {
    let status = response.status();
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ModelError {
            message: "读取服务响应失败".into(),
            retryable: true,
            no_speech: false,
        })?;
        if bytes.len() + chunk.len() > 524288 {
            return Err("服务响应超过限制".to_string().into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let body = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
        return Err(response_error(status, &body));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "服务未返回有效JSON；请检查协议设置".to_string().into())
}
fn response_error(status: StatusCode, body: &Value) -> ModelError {
    let code = body["error"]["code"]
        .as_str()
        .or(body["detail"]["code"].as_str())
        .unwrap_or("");
    let hint = match code {
        "empty_transcript" => "未识别到有效语音，请检查麦克风输入后重新录音 / No speech recognized",
        "audio_too_short" => "语音太短，请说完一句话后再结束 / Audio too short",
        "audio_too_long" => {
            "体验语音每段最多120秒，请缩短或使用自有模型 / Trial audio exceeds 120 seconds"
        }
        "invalid_audio" => "服务无法解码本次音频，请重新录音 / Could not decode audio",
        "unsupported_language" => {
            "体验语音当前支持中文、英文和自动识别 / Unsupported trial language"
        }
        "context_limit" => "文字超过体验模型的上下文限制，请分段输入 / Context limit exceeded",
        "device_daily_limit" => {
            "本设备今日体验额度已用完，请明天重试或配置自有模型 / Device trial quota exhausted"
        }
        "test_daily_limit" => {
            "今日共享体验额度已用完，请稍后重试或使用自有模型 / Shared trial quota exhausted"
        }
        "service_busy" => "体验服务当前繁忙，请稍后重试 / Trial service busy",
        "unauthorized" => {
            "模型凭据无效或已到期，请检查连接或设备开通状态 / Credential invalid or expired"
        }
        _ if status == StatusCode::UNAUTHORIZED => {
            "凭据未通过验证，请检查模型连接 / Credential rejected"
        }
        _ => "模型服务返回错误，请检查连接或稍后重试 / Model service error",
    };
    ModelError {
        message: format!("{hint}（HTTP {}）", status.as_u16()),
        retryable: status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error(),
        no_speech: matches!(code, "empty_transcript" | "audio_too_short"),
    }
}
fn transport(_: reqwest::Error) -> ModelError {
    ModelError {
        message: "模型连接失败或超时，请检查地址与网络 / Connection failed or timed out".into(),
        retryable: true,
        no_speech: false,
    }
}
fn auth(request: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    }
}
pub async fn list(config: &ConnectionConfig, key: &str) -> Result<Vec<String>, String> {
    let url = endpoint(&config.base, "models")?;
    let v = read(
        auth(client()?.get(url), key)
            .send()
            .await
            .map_err(transport)?,
    )
    .await?;
    let mut ids = v["data"]
        .as_array()
        .ok_or("模型列表不符合OpenAI格式")?
        .iter()
        .filter_map(|x| x["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Err("服务未返回模型；可在高级设置手动填写".into());
    }
    Ok(ids)
}
impl From<ModelError> for String {
    fn from(e: ModelError) -> String {
        e.message
    }
}
pub async fn transcribe(
    config: &ConnectionConfig,
    key: &str,
    wav: Vec<u8>,
    language: &str,
    words: &[String],
) -> Result<String, ModelError> {
    if config.model.trim().is_empty() {
        return Err("请先选择语音模型".to_string().into());
    }
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("voice.wav")
        .mime_str("audio/wav")
        .map_err(|_| "音频编码失败".to_string())?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", config.model.clone())
        .text("response_format", "json");
    if language != "auto" {
        form = form.text("language", language.to_string());
    }
    if !words.is_empty() {
        form = form.text("prompt", words.join(", "));
    }
    let response = auth(
        client()?.post(endpoint(&config.base, "audio/transcriptions")?),
        key,
    )
    .multipart(form)
    .send()
    .await
    .map_err(transport)?;
    let value = read(response).await?;
    let text = value["text"]
        .as_str()
        .ok_or("语音服务缺少text字段".to_string())?
        .trim();
    if text.is_empty() {
        return Err(ModelError {
            message: "未识别到语音，请重新录音".into(),
            retryable: false,
            no_speech: true,
        });
    }
    if text.len() > 20000 {
        return Err("转写文本超过本版单次限制，请分段录音".to_string().into());
    }
    Ok(text.to_owned())
}
pub async fn refine(
    config: &ConnectionConfig,
    key: &str,
    text: &str,
    prompt: &str,
    selection: Option<&str>,
    preferences: &[String],
) -> Result<String, ModelError> {
    if config.model.trim().is_empty() {
        return Err("请先选择文字模型".to_string().into());
    }
    let (url, body) = if config.protocol == "vao_refine" {
        if preferences.iter().any(|p| p.starts_with("翻译目标：")) {
            return Err("Vao整理协议不支持局部翻译，请使用OpenAI文字模型连接"
                .to_string()
                .into());
        }
        if selection.is_some() {
            return Err("体验整理协议暂不支持选区指令；请选择自有OpenAI文字连接"
                .to_string()
                .into());
        }
        (
            endpoint(&config.base, "refine")?,
            json!({"text":text,"preferences":preferences}),
        )
    } else {
        let content = if let Some(s) = selection {
            serde_json::to_string(&json!({"selected_text":s,"voice_instruction":text})).unwrap()
        } else {
            // Keep speech as data so providers do not treat a dictated question as chat.
            serde_json::to_string(&json!({"operation":"dictation_cleanup","transcript":text}))
                .unwrap()
        };
        (
            endpoint(&config.base, "chat/completions")?,
            json!({"model":config.model,"messages":[{"role":"system","content":prompt},{"role":"user","content":content}],"stream":false}),
        )
    };
    let v = read(
        auth(client()?.post(url), key)
            .json(&body)
            .send()
            .await
            .map_err(transport)?,
    )
    .await?;
    let output = if config.protocol == "vao_refine" {
        v["text"].as_str()
    } else {
        v["choices"][0]["message"]["content"].as_str()
    }
    .ok_or("文字服务缺少最终文本，请检查协议与模型".to_string())?
    .trim();
    if output.is_empty() {
        return Err("文字模型返回空内容".to_string().into());
    }
    if output.len() > 64000 {
        return Err("文字模型输出超过限制".to_string().into());
    }
    Ok(output.to_string())
}

pub struct WritingRequest<'a> {
    pub text: &'a str,
    pub plan: &'a vao_core::WritingPlan,
    pub dictionary: &'a [String],
    pub translation: Option<&'a str>,
    pub selection: Option<&'a str>,
}

pub struct WritingOutput {
    pub text: String,
    pub repair_attempted: bool,
    pub used_original: bool,
}

pub async fn refine_writing(
    config: &ConnectionConfig,
    key: &str,
    request: &WritingRequest<'_>,
) -> Result<WritingOutput, ModelError> {
    let prompt = request
        .plan
        .prompt(request.dictionary, request.translation, request.selection);
    let preferences = request.plan.preferences(request.translation);
    let output = refine(
        config,
        key,
        request.text,
        &prompt,
        request.selection,
        &preferences,
    )
    .await?;
    // These operations intentionally change source text/language. Do not apply
    // dictation preservation or list cleanup to them.
    if request.translation.is_some() || request.selection.is_some() {
        return Ok(WritingOutput {
            text: output,
            repair_attempted: false,
            used_original: false,
        });
    }
    let output = vao_core::normalize_writing_output(request.text, &output);
    let issues = request.plan.output_issues(request.text, &output);
    if issues.is_empty() {
        return Ok(WritingOutput {
            text: output,
            repair_attempted: false,
            used_original: false,
        });
    }
    let note = "上一轮未通过日期、引用、取消约束或明确多步骤排版的检查。重新整理原始transcript，逐项保留时间、引用和引用后的全部说明以及不执行的步骤。补回日期不能牺牲分项，日期可放在清单前。不得把示例里的信息写入结果。只输出完整最终文本。";
    let repair_prompt = format!(
        "{prompt}\n{note}\n{}",
        request.plan.repair_layout_instruction()
    );
    let mut repair_preferences = preferences;
    let room = 800_usize.saturating_sub(
        repair_preferences
            .iter()
            .map(|p| p.chars().count())
            .sum::<usize>(),
    );
    repair_preferences.push(note.chars().take(room).collect());
    let repaired = refine(
        config,
        key,
        request.text,
        &repair_prompt,
        None,
        &repair_preferences,
    )
    .await?;
    let repaired = vao_core::normalize_writing_output(request.text, &repaired);
    let used_original = !request
        .plan
        .output_issues(request.text, &repaired)
        .is_empty();
    Ok(WritingOutput {
        text: if used_original {
            request.text.into()
        } else {
            repaired
        },
        repair_attempted: true,
        used_original,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_codes_are_actionable_without_echoing_provider_payloads() {
        let e = response_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &json!({"error":{"code":"empty_transcript"}}),
        );
        assert!(e.message.contains("未识别到有效语音"));
        assert!(!e.retryable);
        let e = response_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &json!({"error":{"code":"unknown","message":"synthetic-sensitive-body"}}),
        );
        assert!(e.retryable);
        assert!(!e.message.contains("synthetic-sensitive-body"));
    }
    #[test]
    fn safe_url_and_prefix() {
        assert_eq!(
            endpoint("https://example.com/v1/", "models").unwrap(),
            "https://example.com/v1/models"
        );
        assert_eq!(
            endpoint("http://127.0.0.1:8000", "models").unwrap(),
            "http://127.0.0.1:8000/v1/models"
        );
        assert!(endpoint("http://example.com", "models").is_err());
        assert!(endpoint("https://a:b@example.com", "models").is_err());
        assert!(endpoint("https://example.com?key=x", "models").is_err());
    }
}
