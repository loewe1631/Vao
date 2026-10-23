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
    pub protocol: String,
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
        protocol: config.protocol.clone(),
    })
}

pub struct ModelError {
    pub message: String,
    pub retryable: bool,
    pub no_speech: bool,
    pub scope_notice: Option<crate::input_feedback::InputNotice>,
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
            scope_notice: None,
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
        .or_else(|| p.strip_suffix("/refine"))
        .or_else(|| p.strip_suffix("/models"))
        .unwrap_or(p);
    // An explicit endpoint at the root must stay at the root. Only a bare
    // service origin receives the default /v1 prefix.
    let prefix = if p.is_empty() && u.path().trim_end_matches('/').is_empty() {
        "/v1"
    } else {
        p
    };
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
            scope_notice: None,
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
        "selection_out_of_scope" => "请围绕选中文字提出问题，或说明希望如何处理这段文字。",
        "selection_scope_unavailable" => "暂时无法确认划词请求范围，请稍后重试。",
        "empty_transcript" => "未识别到有效语音，请检查麦克风输入后重新录音 / No speech recognized",
        "audio_too_short" => "语音太短，请说完一句话后再结束 / Audio too short",
        "audio_too_long" => {
            "语音服务拒绝了本次音频片段，请检查接口的片段长度支持 / Audio slice exceeds provider capacity"
        }
        "invalid_audio" => "服务无法解码本次音频，请重新录音 / Could not decode audio",
        "unsupported_language" => {
            "体验语音当前支持中文、英文和自动识别 / Unsupported trial language"
        }
        "asr_context_limit" => "词典提示超出语音模型的运行 token 预算，请精简词条或切换连接 / Vocabulary exceeds ASR runtime token budget",
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
        retryable: !matches!(code,"selection_out_of_scope"|"selection_scope_unavailable") && (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()),
        no_speech: matches!(code, "empty_transcript" | "audio_too_short"),
        scope_notice: match code {
            "selection_out_of_scope" => Some(crate::input_feedback::InputNotice::SelectionOutOfScope),
            "selection_scope_unavailable" => Some(crate::input_feedback::InputNotice::SelectionScopeUnavailable),
            _ => None,
        },
    }
}
fn transport(_: reqwest::Error) -> ModelError {
    ModelError {
        message: "模型连接失败或超时，请检查地址与网络 / Connection failed or timed out".into(),
        retryable: true,
        no_speech: false,
        scope_notice: None,
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
            scope_notice: None,
        });
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
    refine_context(config,key,text,prompt,selection,preferences,RefinementExtras::default()).await
}
#[derive(Default)]
struct RefinementExtras {
    metadata:Option<Value>,
}
async fn refine_context(
    config:&ConnectionConfig,key:&str,text:&str,prompt:&str,selection:Option<&str>,preferences:&[String],extras:RefinementExtras,
)->Result<String,ModelError>{
    let RefinementExtras{metadata}=extras;
    if config.model.trim().is_empty() {
        return Err("请先选择文字模型".to_string().into());
    }
    let (url, body) = if config.protocol == "vao_refine" {
        if preferences.iter().any(|p| p.starts_with("翻译目标：")) {
            return Err("专用 /refine 接口不支持翻译，请选择 OpenAI Chat Completions / The dedicated /refine API does not support translation; choose OpenAI Chat Completions"
                .to_string()
                .into());
        }
        if selection.is_some() {
            return Err("专用 /refine 接口不支持划词指令，请选择 OpenAI Chat Completions / The dedicated /refine API does not support selection commands; choose OpenAI Chat Completions"
                .to_string()
                .into());
        }
        (
            endpoint(&config.base, "refine")?,
            json!({"text":text,"preferences":preferences}),
        )
    } else {
        if let Some(selected) = selection {
            // The default endpoint validates independently. Custom connections use
            // the same policy before generation; no provider key leaves this route.
            if !default_gateway(config) { validate_selection(config,key,selected,text).await?; }
        }
        let mut content = if let Some(selected) = selection {
            json!({"operation":"selection_action","selected_text":selected,"voice_instruction":text})
        } else {
            json!({"operation":"dictation_cleanup","transcript":text})
        };
        if let Some(meta)=metadata.as_ref() {
            if selection.is_none() && !meta["translation"].is_null() {content["operation"]=json!("translation");}
            if meta.get("descriptions").is_some() {content["operation"]=json!("completion_review");}
            content["writing"]=meta.clone();
        }
        if content["operation"] == "dictation_cleanup" && semantic_supported(config) {
            let clues = vao_core::spelling_evidence(text);
            if !clues.is_empty() {content["spelling_evidence"] = json!(clues);}
        }
        let content=serde_json::to_string(&content).unwrap();
        (
            endpoint(&config.base, "chat/completions")?,
            json!({"model":config.model,"messages":[{"role":"system","content":prompt},{"role":"user","content":content}],"stream":false}),
        )
    };
    let response = auth(client()?.post(url), key)
            .json(&body)
            .send()
            .await
            .map_err(transport)?;
    if config.protocol == "vao_refine"
        && matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED)
    {
        return Err(format!("服务不支持当前 /refine 请求（HTTP {}）。普通自有文字模型请选择 OpenAI Chat Completions；此选项仅用于实现 /refine 的旧服务。 / The service rejected /refine. For standard custom text models, choose OpenAI Chat Completions.",response.status().as_u16()).into());
    }
    let v = read(response).await?;
    let output = if config.protocol == "vao_refine" {
        v["text"].as_str()
    } else {
        v["choices"][0]["message"]["content"].as_str()
    }
    .ok_or_else(|| if config.protocol == "vao_refine" {
        "专用 /refine 接口须返回 JSON text 字段。普通自有文字模型请选择 OpenAI Chat Completions。 / The dedicated /refine API must return a JSON text field; choose OpenAI Chat Completions for standard custom models.".to_string()
    } else {
        "文字服务缺少 choices[0].message.content，请检查 OpenAI Chat Completions 协议与模型 / Missing Chat Completions content; check the protocol and model".to_string()
    })?
    .trim();
    if output.is_empty() {
        return Err("文字模型返回空内容".to_string().into());
    }
    if selection.is_some() && output=="请围绕选中文字提出问题，或说明希望如何处理这段文字。" {return Err(scope_failure(true));}
    Ok(output.to_string())
}

fn default_gateway(config: &ConnectionConfig) -> bool {
    endpoint(&config.base,"chat/completions").is_ok_and(|u|u==format!("{}/chat/completions",crate::trial::BASE))
}
pub fn semantic_supported(config: &ConnectionConfig) -> bool {
    config.protocol == "openai_chat"
}
fn scope_failure(denied: bool) -> ModelError {
    use crate::input_feedback::InputNotice;
    ModelError { message:if denied {"请围绕选中文字提出问题，或说明希望如何处理这段文字。"}else{"暂时无法确认划词请求范围，请稍后重试。"}.into(),retryable:false,no_speech:false,
        scope_notice:Some(if denied {InputNotice::SelectionOutOfScope}else{InputNotice::SelectionScopeUnavailable}) }
}
fn parse_scope(text: &str) -> Result<bool,ModelError> {
    let text=text.trim();
    let text=text.strip_prefix("```json").or_else(||text.strip_prefix("```"))
        .and_then(|s|s.trim().strip_suffix("```" )).unwrap_or(text).trim();
    let v:Value=serde_json::from_str(text).map_err(|_|scope_failure(false))?;
    let obj=v.as_object().filter(|o|o.len()==1).ok_or_else(||scope_failure(false))?;
    obj.get("allowed").and_then(Value::as_bool).ok_or_else(||scope_failure(false))
}
async fn validate_selection(config:&ConnectionConfig,key:&str,selected:&str,instruction:&str)->Result<(),ModelError> {
    let response=auth(client()?.post(endpoint(&config.base,"chat/completions")?),key)
        .json(&json!({"model":config.model,"messages":[{"role":"system","content":vao_core::scope_prompt()},
            {"role":"user","content":serde_json::to_string(&json!({"selected_text":selected,"voice_instruction":instruction})).unwrap()}],"stream":false,"temperature":0,"max_tokens":32}))
        .timeout(Duration::from_secs(60)).send().await.map_err(|_|scope_failure(false))?;
    let v=read(response).await.map_err(|_|scope_failure(false))?;
    if v["choices"][0]["finish_reason"].as_str().is_some_and(|s|s!="stop") {return Err(scope_failure(false));}
    let allowed=parse_scope(v["choices"][0]["message"]["content"].as_str().ok_or_else(||scope_failure(false))?)?;
    if allowed {Ok(())}else{Err(scope_failure(true))}
}

pub struct WritingRequest<'a> {
    pub text: &'a str,
    pub plan: &'a vao_core::WritingPlan,
    pub dictionary: &'a [String],
    pub translation: Option<&'a str>,
    pub selection: Option<&'a str>,
    pub options: vao_core::WritingOptions,
}

#[derive(serde::Serialize)]
pub struct WritingOutput {
    pub text: String,
    pub repair_attempted: bool,
    pub used_original: bool,
    pub policy_version: &'static str,
    pub semantic_effective: bool,
    pub completion_requested: bool,
    pub completion_effective: bool,
    pub actual_route: String,
    pub actual_model: String,
    pub actual_protocol: String,
    pub generation_attempts: usize,
    pub first_output: String,
    pub repair_output: Option<String>,
    pub first_issues: Vec<&'static str>,
    pub final_issues: Vec<&'static str>,
    pub completion_review: String,
    pub completion_applied: usize,
    pub completion_issues: Vec<&'static str>,
}

impl WritingOutput {
    pub fn diagnostic(&self) -> crate::store::RefinementDiagnostic {
        crate::store::RefinementDiagnostic {
            policy_version:self.policy_version.into(),route:self.actual_route.clone(),model:self.actual_model.clone(),protocol:self.actual_protocol.clone(),
            semantic_effective:self.semantic_effective,completion_requested:self.completion_requested,completion_effective:self.completion_effective,
            generation_attempts:self.generation_attempts,repair_attempted:self.repair_attempted,used_original:self.used_original,
            first_issues:self.first_issues.iter().map(|s|(*s).into()).collect(),
            final_issues:self.final_issues.iter().map(|s|(*s).into()).collect(),
            completion_review:self.completion_review.clone(),completion_applied:self.completion_applied,
            completion_issues:self.completion_issues.iter().map(|s|(*s).into()).collect(),
        }
    }
}

pub async fn refine_writing(
    config: &ConnectionConfig,
    key: &str,
    request: &WritingRequest<'_>,
) -> Result<WritingOutput, ModelError> {
    refine_writing_limited(config, key, request, 2).await
}

pub async fn refine_writing_limited(
    config: &ConnectionConfig, key: &str, request: &WritingRequest<'_>, max_generations: usize,
) -> Result<WritingOutput, ModelError> {
    if !(1..=2).contains(&max_generations) { return Err("Invalid generation budget".to_owned().into()); }
    let ordinary = request.translation.is_none() && request.selection.is_none();
    let enhanced = ordinary && semantic_supported(config);
    let options = vao_core::WritingOptions {semantic_completion: enhanced && request.options.semantic_completion
        && vao_core::semantic_completion_allowed(request.text)};
    let fallback_text = || {
        let body = if options.semantic_completion {
            vao_core::completion::consume_preserve_description_instruction(request.text, request.text)
        } else { request.text.to_owned() };
        vao_core::normalize_writing_output(request.text, &body)
    };
    let prompt = request.plan.prompt_with_options(request.dictionary, request.translation, request.selection, options);
    let preferences = request.plan.preferences(request.translation);
    let output = refine_context(
        config,
        key,
        request.text,
        &prompt,
        request.selection,
        &preferences,
        RefinementExtras {
        metadata:Some(writing_metadata(request, options, false)),},
    )
    .await?;
    // These operations intentionally change source text/language. Do not apply
    // dictation preservation or list cleanup to them.
    let decoded = if options.semantic_completion {
        vao_core::completion::completion_draft(request.text, &output).map(|draft|draft.text)
    } else { Ok(output.clone()) };
    let decode_issue = decoded.as_ref().err().copied();
    let mut result = WritingOutput {
        text: if ordinary {vao_core::normalize_writing_output(request.text, decoded.as_deref().unwrap_or(request.text))} else {output.clone()},
        first_output: output, repair_output: None, repair_attempted:false,used_original:false,
        policy_version:vao_core::POLICY_VERSION,semantic_effective:enhanced,
        completion_requested:request.options.semantic_completion, completion_effective:options.semantic_completion,
        actual_route:if default_gateway(config) {"trial"} else if config.protocol=="vao_refine" {"legacy_refine"} else {"custom_chat"}.into(),
        actual_model:config.model.clone(), actual_protocol:config.protocol.clone(),generation_attempts:1,
        first_issues:vec![], final_issues:vec![],
        completion_review:"not_needed".into(),completion_applied:0,completion_issues:vec![],
    };
    if !ordinary { return Ok(result); }
    // The draft must contain no new title. Only the separately reviewed proposal
    // may introduce a name; validate the preserved body before replacing spans.
    result.first_issues = output_issues(request, &result.text, enhanced, options.semantic_completion);
    if let Some(issue) = decode_issue { result.first_issues.push(issue); }
    if result.first_issues.is_empty() {
        if options.semantic_completion {
            review_completion(config, key, request, &mut result, max_generations).await;
        }
        return Ok(result);
    }
    if max_generations == 1 {
        result.final_issues = result.first_issues.clone();
        result.used_original = true;
        result.text = fallback_text();
        return Ok(result);
    }
    let note = vao_core::WRITING_REPAIR;
    let mut repair_prompt = format!(
        "{prompt}\n{note}\n本轮检查发现：{:?}。按原文及上述输出接口修正这些问题。\n{}",
        result.first_issues,
        request.plan.repair_layout_instruction()
    );
    if options.semantic_completion {
        repair_prompt.push_str("\n本次输出接口保持text与annotations的JSON对象，不改为纯文本。正文只含整理后的口述，推测名称只填annotations；作者简称保持原样。description必须同时逐字出现在原文和正文中。");
    }
    let mut repair_preferences = preferences;
    let room = 800_usize.saturating_sub(
        repair_preferences
            .iter()
            .map(|p| p.chars().count())
            .sum::<usize>(),
    );
    repair_preferences.push(note.chars().take(room).collect());
    let repaired = refine_context(
        config,
        key,
        request.text,
        &repair_prompt,
        None,
        &repair_preferences,
        RefinementExtras {
        metadata:Some(writing_metadata(request, options, true)),},
    )
    .await.map_err(|mut error| {
        // A failed repair has exhausted the two-generation budget. Do not
        // then retry the whole operation on a backup with a fresh budget.
        error.retryable = false;
        error
    })?;
    let repaired_draft = if options.semantic_completion {
        vao_core::completion::completion_draft(request.text, &repaired).ok()
    } else { None };
    let decoded = if let Some(draft) = repaired_draft.as_ref() {
        Ok(draft.text.clone())
    } else if options.semantic_completion {
        Err("invalid_annotation_output")
    } else { Ok(repaired.clone()) };
    let decode_issue = decoded.as_ref().err().copied();
    let repaired_text = vao_core::normalize_writing_output(request.text, decoded.as_deref().unwrap_or(request.text));
    result.final_issues = output_issues(request, &repaired_text, enhanced, options.semantic_completion);
    if let Some(issue) = decode_issue { result.final_issues.push(issue); }
    let unreviewed_candidate = options.semantic_completion
        && repaired_draft.as_ref().is_some_and(|draft| !draft.annotations.is_empty());
    result.used_original = !result.final_issues.is_empty() || unreviewed_candidate;
    result.repair_output = Some(repaired.clone());
    result.text = if result.used_original {
            fallback_text()
        } else {
            repaired_text
        };
    result.repair_attempted = true;
    result.generation_attempts = 2;
    if unreviewed_candidate {
        // Repair consumed the second generation. Do not deliver a body whose
        // candidate has not passed independent review; preserve the transcript.
        result.completion_review = "budget_exhausted".into();
        result.completion_issues.push("completion_not_reviewed");
        result.text = fallback_text();
    }
    Ok(result)
}

async fn review_completion(
    config: &ConnectionConfig, key: &str, request: &WritingRequest<'_>,
    result: &mut WritingOutput, max_generations: usize,
) {
    let Ok(draft) = vao_core::completion::completion_draft(request.text, &result.first_output) else { return; };
    if draft.annotations.is_empty() { return; }
    if result.generation_attempts >= max_generations {
        result.completion_review = "budget_exhausted".into();
        result.completion_issues.push("completion_not_reviewed");
        return;
    }
    // Do not send candidate names, prior response, scene examples or ambient
    // context. The second pass independently identifies the same source spans.
    let descriptions = draft.annotations.iter().map(|a|a.description.as_str()).collect::<Vec<_>>();
    result.generation_attempts += 1;
    let reviewed = refine_context(config,key,request.text,vao_core::completion::REVIEW_PROMPT,None,&[],
        RefinementExtras {metadata:Some(json!({"policy_version":vao_core::POLICY_VERSION,"descriptions":descriptions}))}).await;
    match reviewed {
        Ok(review) => match vao_core::completion::apply_reviewed_completion(request.text,&result.text,&draft,&review) {
            Ok(completed) => {
                result.completion_review = if completed.withheld == 0 {"agreed"} else {"withheld"}.into();
                result.completion_applied = completed.applied;
                if completed.withheld > 0 { result.completion_issues.push("completion_ambiguous_or_disagreed"); }
                result.text = completed.text;
            }
            Err(issue) => { result.completion_review = "invalid".into();result.completion_issues.push(issue); }
        },
        Err(_) => {
            // A failed knowledge pass must not discard usable cleanup or trigger
            // another provider request. No provider body is saved in history.
            result.completion_review = "unavailable".into();
            result.completion_issues.push("completion_review_unavailable");
        }
    }
}

fn writing_metadata(request: &WritingRequest<'_>, options: vao_core::WritingOptions, repair: bool) -> Value {
    let mut metadata=json!({"preset":request.plan.preset,"layout":request.plan.layout,
        "dictionary":request.dictionary,"translation":request.translation,"repair":repair});
    if request.translation.is_none() && request.selection.is_none() {
        metadata["policy_version"]=json!(vao_core::POLICY_VERSION);
        metadata["semantic_completion"]=json!(options.semantic_completion);
        metadata["layout_source"]=json!(request.plan.layout_source);
        metadata["single_paragraph"]=json!(request.plan.single_paragraph);
    }
    metadata
}

fn output_issues(request: &WritingRequest<'_>, output: &str, enhanced: bool, completion: bool) -> Vec<&'static str> {
    let mut issues = request.plan.output_issues(request.text, output);
    // Explicit spelling is a deterministic editing instruction and must be
    // checked for the built-in service as well as custom semantic routes.
    for clue in vao_core::spelling_evidence(request.text) {
        if let Some(expected) = clue["expected_greeting_name"].as_str() {
            let greeting=output.split_once("你好").or_else(||output.split_once("您好"))
                .map(|(before,_)|before.trim_matches(|c:char|c.is_whitespace()||"：:，,。.!！".contains(c)));
            if greeting != Some(expected) { issues.push("explicit_greeting_spelling_missing"); }
        }
    }
    // Completion may add only the missing name/term. Descriptive clues remain
    // part of the user's intended record even after a title is supplied.
    for clue in completion_description_clues(request.text) {
        if longest_shared_run(&clue, output) < 3 {
            issues.push("completion_description_missing");
        }
    }
    let cleaned_source = vao_core::normalize_writing_output(request.text, request.text);
    if cleaned_source != request.text {
        // Removing hesitations must not erase the meaningful lead-in. A later
        // explicit correction may legitimately replace it, so defer there.
        let lead = request.text.split(['，', ',', '。']).next().unwrap_or("");
        if lead.chars().count() >= 6 && longest_shared_run(lead, output) < 3
            && !["不对", "改成", "改为", "不是"].iter().any(|s| request.text.contains(s)) {
            issues.push("dictation_leading_clause_missing");
        }
    }
    if enhanced {
        if request.text.contains("不是") && ["另一本","另一部","另一个"].iter().any(|s|request.text.contains(s))
            && !["不是","并非","不叫","不是指"].iter().any(|s|output.contains(s)) {
            issues.push("entity_exclusion_missing");
        }
        // Preserve explicit Latin product/technical identifiers in an entity
        // description. An inferred organization name must not erase its clues.
        if ["公司","机构","软件","工具","产品"].iter().any(|s|request.text.contains(s))
            && !["改成","改为","不是","原样"].iter().any(|s|request.text.contains(s)) {
            for term in request.text.split(|c:char|!c.is_ascii_alphanumeric()) {
                if term.len()>=2 && term.chars().any(|c|c.is_ascii_uppercase())
                    && !output.to_lowercase().contains(&term.to_lowercase()) {
                    issues.push("explicit_entity_identifier_missing");
                }
            }
        }
        if completion {
            for marker in ["作者", "导演", "演员", "创办人", "年份", "出生于"] {
                if output.contains(marker) && !request.text.contains(marker) {
                    issues.push("completion_added_unsupported_fact");
                }
            }
            // A model sometimes appends an inferred author such as
            // “安迪·威尔” beside a completed title. A middle-dot name that
            // was absent from the spoken clues is an unsupported side fact.
            for token in output.split(|c: char| c.is_whitespace() || "，。；：:、()（）[]【】".contains(c)) {
                if token.contains('·') && !request.text.replace('·', "").contains(&token.replace('·', "")) {
                    issues.push("completion_added_unsupported_fact");
                }
            }
        }
    }
    // Draft bodies never contain inferred titles, including when completion is
    // enabled. A separate reviewed proposal is the only title addition path.
    // This narrow invariant does not claim to determine literary truth.
    if enhanced {
        let mut rest = output;
        while let Some((_, tail)) = rest.split_once('《') {
            let Some((title, after)) = tail.split_once('》') else { break; };
            let canonical = |text: &str| text.chars().filter(|c|c.is_alphanumeric()).map(|c| match c {
                '〇'|'零'=>'0','一'=>'1','二'=>'2','三'=>'3','四'=>'4','五'=>'5','六'=>'6','七'=>'7','八'=>'8','九'=>'9',_=>c
            }).collect::<String>();
            if !title.is_empty() && !canonical(request.text).contains(&canonical(title)) { issues.push("unrequested_work_title"); }
            rest = after;
        }
    }
    issues.sort();
    issues.dedup();
    issues
}

fn completion_description_clues(source: &str) -> Vec<String> {
    fn without_titles(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut in_title = false;
        for c in text.chars() {
            match c {
                '《' => in_title = true,
                '》' => in_title = false,
                _ if !in_title => out.push(c),
                _ => {}
            }
        }
        out
    }
    source
        .split(['。', '！', '!', '？', '?', '；', ';', '\n'])
        .flat_map(|sentence| sentence.split(['，', ',', '：', ':']))
        .map(without_titles)
        .map(|clause| clause.trim().to_owned())
        .filter(|clause| {
            clause.chars().count() >= 6
                && !recording_lead_in(clause)
                // Repeated standalone hesitation words are not entity clues.
                && !clause.chars().filter(|c| !c.is_whitespace() && !"、，,".contains(*c))
                    .collect::<String>().replace("那个", "").replace("这个", "")
                    .replace(['嗯', '呃', '额'], "").is_empty()
                && ["那本", "那部", "那首", "那个", "一本", "一部", "小说", "电影", "剧集", "歌曲"]
                    .iter()
                    .any(|marker| clause.contains(marker))
        })
        .collect()
}

fn recording_lead_in(clause: &str) -> bool {
    // A consumed list-introduction is not a missing entity description. Match
    // only a bounded recording instruction; actual descriptive words still
    // have to survive the independent source-span checks.
    let clause=clause.trim_start_matches(['请', '先']);
    let Some(tail)=["帮我记录", "帮我记下", "帮我记", "帮我列一下", "帮我列", "记录", "记下", "记", "列出", "列一下"]
        .iter().find_map(|prefix|clause.strip_prefix(prefix)) else {return false;};
    let tail=tail.trim_start_matches(|c:char|c.is_ascii_digit()||"一二两三四五六七八九十几些本部首个种".contains(c));
    let tail=["想看的", "想读的", "要看的", "要读的"].iter().find_map(|prefix|tail.strip_prefix(prefix)).unwrap_or(tail);
    ["书", "书籍", "小说", "电影", "动画片", "电视剧", "歌曲", "作品", "清单"].contains(&tail)
}

fn longest_shared_run(clue: &str, output: &str) -> usize {
    let chars = clue.chars().collect::<Vec<_>>();
    let mut longest = 0;
    for start in 0..chars.len() {
        for end in (start + 1)..=chars.len() {
            let candidate = chars[start..end].iter().collect::<String>();
            if candidate.chars().count() > longest && output.contains(&candidate) {
                longest = candidate.chars().count();
            }
        }
    }
    longest
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hesitation_cleanup_is_not_a_missing_work_description() {
        assert!(completion_description_clues("测试一下，那个、那个、那个，这个，我想试一下这个功能。").is_empty());
        assert_eq!(completion_description_clues("记下那个戴帽子的演员出演的电影。").len(), 1);
    }
    #[test]
    fn consumed_recording_lead_in_is_not_a_missing_description() {
        let clues=completion_description_clues("帮我记两部电影，一部讲小孩遇到两个窃贼；另一部讲玩具的故事。");
        assert_eq!(clues.len(),2);
        assert!(recording_lead_in("帮我记几本想看的书"));
        assert!(!recording_lead_in("帮我记那部讲宇航员的电影"));
        assert!(!recording_lead_in("帮我记一部电影明天去看"));
    }
    #[test]
    fn name_punctuation_is_not_a_new_person_but_expanding_a_name_is() {
        let text="帮我记录诺拉斯通主演的那部电影。";
        let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&vao_core::Profile::default());
        let request=WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,
            options:vao_core::WritingOptions{semantic_completion:true}};
        assert!(!output_issues(&request,"帮我记录诺拉·斯通主演的那部电影。",true,true).contains(&"completion_added_unsupported_fact"));
        assert!(output_issues(&request,"帮我记录诺拉·斯通·额外姓名主演的那部电影。",true,true).contains(&"completion_added_unsupported_fact"));
    }
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
        for (base, path, expected) in [
            ("https://example.com/refine", "refine", "https://example.com/refine"),
            ("https://example.com/v1/refine/", "refine", "https://example.com/v1/refine"),
            ("https://example.com/custom/refine", "refine", "https://example.com/custom/refine"),
            ("https://example.com/chat/completions", "chat/completions", "https://example.com/chat/completions"),
            ("https://example.com/audio/transcriptions", "audio/transcriptions", "https://example.com/audio/transcriptions"),
        ] {
            assert_eq!(endpoint(base, path).unwrap(), expected);
        }
        assert!(endpoint("http://example.com", "models").is_err());
        assert!(endpoint("https://a:b@example.com", "models").is_err());
        assert!(endpoint("https://example.com?key=x", "models").is_err());
    }
}
