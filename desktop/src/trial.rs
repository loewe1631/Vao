use crate::{
    models, platform,
    store::{ConnectionConfig, Settings},
};
use serde_json::{json, Value};
pub const BASE: &str = "https://test.vaovao.app/v1";
pub const SLOT: &str = "trial_device";
pub const CONSENT_VERSION: &str = "2026-10-18";
const INSTALL_SLOT: &str = "trial_installation";
static ENROLLMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn new_installation() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "无法创建设备凭据 / Secure randomness unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn consented(settings: &Settings) -> bool {
    settings.trial_consent_version == CONSENT_VERSION
}

// A random installation secret is kept in Keychain. No hardware identifier,
// private speech, shared model key or administrator credential is sent here.
pub async fn enroll(consent: bool) -> Result<Value, String> {
    if !consent {
        return Err(
            "请先同意体验服务的数据处理说明 / Confirm the trial privacy notice first".into(),
        );
    }
    let _guard = ENROLLMENT.lock().await;
    if platform::has_secret(SLOT)? {
        return check().await;
    }
    let mut installation = platform::secret(INSTALL_SLOT)?;
    if installation.is_empty() {
        installation = new_installation()?;
        platform::save_secret(INSTALL_SLOT, &installation)?;
    }
    let response = models::client()?
        .post(format!("{BASE}/enroll"))
        .json(&json!({"installation_key":installation,"consent_version":CONSENT_VERSION}))
        .send()
        .await
        .map_err(|_| {
            "开通连接失败，请稍后重试；也可配置自有模型 / Enrollment connection failed".to_owned()
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            429 => "共享服务繁忙，请稍后再试，或配置自有模型 / Shared service busy; retry later or connect your models".into(),
            403 => "此安装的服务凭据已停用，请配置自有模型 / Service credential disabled; connect your models".into(),
            _ => format!("体验开通失败（HTTP {}），请稍后重试 / Enrollment failed", status.as_u16()),
        });
    }
    let value = models::read(response).await.map_err(String::from)?;
    let token = value["token"]
        .as_str()
        .filter(|s| s.starts_with("vao_device_") && s.len() >= 40 && s.len() <= 160)
        .ok_or("开通凭据无效 / Invalid device credential")?;
    if !value["device"].is_object() {
        return Err("开通响应缺少设备状态 / Missing enrollment status".into());
    }
    platform::save_secret(SLOT, token)?;
    // Never return the credential to the WebView, logs or release reports.
    Ok(json!({"device":value["device"],"max_audio_seconds":value["max_audio_seconds"],"enrolled":true}))
}
#[derive(Clone)]
pub struct Route {
    pub config: ConnectionConfig,
    pub trial: bool,
}
pub fn resolve(
    custom: &ConnectionConfig,
    asr: bool,
    enabled: bool,
    provisioned: bool,
) -> Result<Route, String> {
    if !custom.base.trim().is_empty() || !custom.model.trim().is_empty() {
        if custom.base.trim().is_empty() || custom.model.trim().is_empty() {
            return Err("自有连接未填写完整，请补全或清空自有连接 / Complete or clear the custom connection".into());
        }
        models::endpoint(&custom.base, "models")?;
        return Ok(Route {
            config: custom.clone(),
            trial: false,
        });
    }
    if !enabled {
        return Err(
            "体验服务已关闭，请启用体验或配置自有模型 / Enable trial or connect your models".into(),
        );
    }
    if !provisioned {
        return Err(
            "此设备尚未开通体验服务，请配置自有模型 / Trial not provisioned on this device".into(),
        );
    }
    Ok(Route {
        config: ConnectionConfig {
            base: BASE.into(),
            model: if asr { "vao-asr" } else { "vao-refine" }.into(),
            protocol: if asr { "openai_audio" } else { "openai_chat" }.into(),
        },
        trial: true,
    })
}
pub fn routes(settings: &Settings) -> Result<(Route, Route), String> {
    let provisioned = platform::has_secret(SLOT).unwrap_or(false);
    let routes = (
        resolve(&settings.asr, true, settings.trial_enabled, provisioned)?,
        resolve(&settings.llm, false, settings.trial_enabled, provisioned)?,
    );
    if (routes.0.trial || routes.1.trial) && !consented(settings) {
        return Err("请先在模型连接页确认体验服务说明并开通，或配置自有模型 / Activate the trial or connect your models first".into());
    }
    Ok(routes)
}
pub fn key(route: &Route, slot: &str) -> Result<String, String> {
    if route.trial {
        platform::secret(SLOT)
    } else {
        platform::connection_secret(slot, &route.config.base)
    }
}
pub fn local_status(settings: &Settings) -> Value {
    let provisioned = platform::has_secret(SLOT).unwrap_or(false);
    let asr = resolve(&settings.asr, true, settings.trial_enabled, provisioned);
    let llm = resolve(&settings.llm, false, settings.trial_enabled, provisioned);
    let ready = asr.as_ref().is_ok_and(|r| !r.trial || consented(settings))
        && llm.as_ref().is_ok_and(|r| !r.trial || consented(settings));
    json!({"provisioned":provisioned,"consented":consented(settings),"enabled":settings.trial_enabled,"asr_source":if settings.asr.base.is_empty()&&settings.asr.model.is_empty(){"trial"}else{"custom"},"llm_source":if settings.llm.base.is_empty()&&settings.llm.model.is_empty(){"trial"}else{"custom"},"ready":ready})
}
pub async fn check() -> Result<Value, String> {
    let key = platform::secret(SLOT)?;
    if key.is_empty() {
        return Err("设备尚未开通体验服务 / Device is not provisioned".into());
    }
    let response = models::client()?
        .get(format!("{BASE}/experience"))
        .bearer_auth(key)
        .send()
        .await
        .map_err(|_| "体验服务连接失败 / Trial connection failed")?;
    if !response.status().is_success() {
        return Err(format!(
            "体验服务 HTTP {}，请检查开通状态或稍后重试 / Trial unavailable",
            response.status().as_u16()
        ));
    }
    models::read(response).await.map_err(String::from)
}
// Explicit diagnostic entry point: uses a supplied fixture, never captures a microphone.
pub async fn verify_fixture(path: &str) -> Result<Value, String> {
    let wav = std::fs::read(path).map_err(|_| "Could not read fixture")?;
    if wav.len() > 12 * 1024 * 1024 || wav.get(..4) != Some(b"RIFF") {
        return Err("Expected a WAV fixture under 12 MiB".into());
    }
    let settings = Settings {
        trial_consent_version: CONSENT_VERSION.into(),
        ..Settings::default()
    };
    let (asr, llm) = routes(&settings)?;
    let asr_key = key(&asr, "asr")?;
    let llm_key = key(&llm, "llm")?;
    let start = std::time::Instant::now();
    let transcript = models::transcribe(&asr.config, &asr_key, wav, "zh", &[]).await?;
    let asr_ms = start.elapsed().as_millis();
    let prompt = vao_core::system_prompt(
        vao_core::Scene::Chat,
        vao_core::Layout::Paragraph,
        &[],
        None,
        None,
    );
    let text = models::refine(&llm.config, &llm_key, &transcript, &prompt, None, &[]).await?;
    let dictation_ms = start.elapsed().as_millis();
    let correction = models::refine(
        &llm.config,
        &llm_key,
        "明天下午三点开会，不对，后天下午四点。",
        &prompt,
        None,
        &[],
    )
    .await?;
    let translation_prompt = vao_core::system_prompt(
        vao_core::Scene::Chat,
        vao_core::Layout::Paragraph,
        &[],
        Some("English"),
        None,
    );
    let translation = models::refine(
        &llm.config,
        &llm_key,
        "明天下午四点见。",
        &translation_prompt,
        None,
        &[],
    )
    .await?;
    let selection = Some("请明天发送报告。");
    let selection_prompt = vao_core::system_prompt(
        vao_core::Scene::Chat,
        vao_core::Layout::Paragraph,
        &[],
        None,
        selection,
    );
    let selected = models::refine(
        &llm.config,
        &llm_key,
        "把明天改成后天，其他不变。",
        &selection_prompt,
        selection,
        &[],
    )
    .await?;
    let status = check().await?;
    Ok(
        json!({"kind":"installed client adapters over HTTPS with public audio fixture; no microphone or delivery", "transcript":transcript,"text":text,"asr_ms":asr_ms,"dictation_ms":dictation_ms,"correction":correction,"translation":translation,"selection":selected,"status":status}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_defaults_require_current_consent_and_keep_private_data_empty() {
        let mut settings = Settings::default();
        assert!(!consented(&settings));
        assert!(!settings.onboarding_complete && settings.auto_input);
        assert!(settings.asr.base.is_empty() && settings.llm.base.is_empty());
        assert!(settings.dictionary.is_empty());
        settings.trial_consent_version = "outdated".into();
        assert!(!consented(&settings));
        settings.trial_consent_version = CONSENT_VERSION.into();
        assert!(consented(&settings));
        let a = new_installation().unwrap();
        let b = new_installation().unwrap();
        assert!(a.len() == 64 && a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
    #[test]
    fn independent_routes_and_no_partial_fallback() {
        let s = Settings::default();
        assert!(resolve(&s.asr, true, true, true).unwrap().trial);
        assert_eq!(
            resolve(&s.llm, false, true, true).unwrap().config.protocol,
            "openai_chat"
        );
        let c = ConnectionConfig {
            base: "http://127.0.0.1:8000/v1".into(),
            model: "own".into(),
            protocol: "openai_audio".into(),
        };
        assert!(!resolve(&c, true, false, false).unwrap().trial);
        let incomplete = ConnectionConfig {
            base: c.base,
            ..Default::default()
        };
        assert!(resolve(&incomplete, true, true, true).is_err());
        assert!(resolve(&s.asr, true, false, true).is_err());
        assert!(resolve(&s.llm, false, true, false).is_err());
    }
}
