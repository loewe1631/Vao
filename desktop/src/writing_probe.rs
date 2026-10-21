//! Explicit fixed-text replay. Never records audio, inserts text or saves feedback.
use crate::{
    models,
    store::{ConnectionConfig, Settings, Store},
    trial,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use vao_core::{Profile, WritingPlan};

#[derive(Deserialize)]
struct Fixture {
    id: String,
    text: String,
    app: String,
    #[serde(default)]
    selection: Option<String>,
    #[serde(default)]
    translation: Option<String>,
}

pub async fn replay(
    path: &Path,
    database: &Path,
    route: &str,
    profile_mode: &str,
) -> Result<Value, String> {
    if !matches!(route, "custom" | "trial") || !matches!(profile_mode, "fresh" | "current") {
        return Err("Expected route custom/trial and profile fresh/current".into());
    }
    if std::fs::metadata(path)
        .map_err(|_| "Fixture not found")?
        .len()
        > 131072
    {
        return Err("Fixture file too large".into());
    }
    let fixtures: Vec<Fixture> =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "Could not read fixtures")?)
            .map_err(|_| "Invalid fixtures JSON")?;
    if fixtures.is_empty()
        || fixtures.len() > 40
        || fixtures
            .iter()
            .any(|f| f.text.len() > 20000 || f.app.len() > 512)
    {
        return Err("Expected 1-40 bounded text fixtures".into());
    }
    // Open read-only: do not migrate, prune history or overwrite normal settings.
    let snapshot = Store::open_read_only(database)?;
    let settings: Settings = snapshot.get("settings");
    let profile: Profile = if profile_mode == "current" {
        snapshot.get("profile")
    } else {
        Profile::default()
    };
    let dictionary = if profile_mode == "current" {
        settings.dictionary.clone()
    } else {
        vec![]
    };
    let connection = if route == "trial" {
        ConnectionConfig::default()
    } else {
        settings.llm.clone()
    };
    if route == "custom" && connection.base.is_empty() {
        return Err("No custom LLM configured".into());
    }
    let route = trial::resolve(
        &connection,
        false,
        settings.trial_enabled,
        crate::platform::has_secret(trial::SLOT)?,
    )?;
    if route.trial && !trial::consented(&settings) {
        return Err("Trial privacy consent is required".into());
    }
    let key = trial::key(&route, "llm")?;
    let mut reports = vec![];
    for fixture in fixtures {
        let plan = WritingPlan::resolve(&fixture.text, &fixture.app, &[], &profile);
        let request = models::WritingRequest {
            text: &fixture.text,
            plan: &plan,
            dictionary: &dictionary,
            translation: fixture.translation.as_deref(),
            selection: fixture.selection.as_deref(),
            context:None,
        };
        let started = std::time::Instant::now();
        let output = models::refine_writing(&route.config, &key, &request).await;
        let mut report = json!({"id":fixture.id,"source":fixture.text,"app":fixture.app,"plan":plan,"latency_ms":started.elapsed().as_millis()});
        match output {
            Ok(value) => {
                report["output"] = json!(value.text);
                report["repair_attempted"] = json!(value.repair_attempted);
                report["used_original"] = json!(value.used_original);
            }
            Err(e) => report["error"] = json!(e.message),
        }
        reports.push(report);
    }
    Ok(
        json!({"policy_version":vao_core::POLICY_VERSION,"route":if route.trial {"trial"}else{"custom"},"model":route.config.model,"profile":profile_mode,"results":reports}),
    )
}
