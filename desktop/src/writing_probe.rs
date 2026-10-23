//! Explicit fixed-text replay. Never records audio, inserts text or saves feedback.
use crate::{models, store::{ConnectionConfig, Settings, Store}, trial};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use vao_core::{Layout, Profile, Scene, WritingOptions, WritingPlan};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    id: String,
    text: String,
    app: String,
    #[serde(default)]
    selection: Option<String>,
    #[serde(default)]
    translation: Option<String>,
    #[serde(default, rename="scene")]
    _scene: Option<Value>,
    #[serde(default, rename="preset")]
    _preset: Option<Value>,
    #[serde(default, rename="layout")]
    _layout: Option<Value>,
    #[serde(default, rename="expect")]
    _expect: Option<Value>,
    #[serde(default, rename="reject")]
    _reject: Option<Value>,
    #[serde(default, rename="numbered_items")]
    _numbered_items: Option<Value>,
    #[serde(default, rename="single_paragraph")]
    _single_paragraph: Option<Value>,
    #[serde(default, rename="check")]
    _check: Option<Value>,
}

#[derive(Default)]
pub struct ReplayOptions<'a> {
    pub books: Option<bool>,
    pub connection: Option<&'a Path>,
    pub credential_slot: Option<&'a str>,
    pub generation_budget: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TestConnection { base: String, model: String, protocol: String }

fn read_fixtures(path: &Path) -> Result<Vec<Fixture>, String> {
    if std::fs::metadata(path).map_err(|_| "Fixture not found")?.len() > 131072 {
        return Err("Fixture file too large".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).map_err(|_| "Could not read fixtures")?)
        .map_err(|_| "Invalid fixtures JSON or unknown fixture field")?;
    let mut ids = std::collections::HashSet::new();
    if fixtures.is_empty() || fixtures.len() > 40 || fixtures.iter().any(|f| {
        f.id.is_empty() || f.id.len()>128 || !ids.insert(f.id.clone()) || f.text.trim().is_empty()
            || f.text.len()>20000 || f.app.len()>512
            || f.selection.as_ref().is_some_and(|s|s.len()>20000)
            || f.translation.as_ref().is_some_and(|s|s.is_empty()||s.len()>40)
    }) { return Err("Expected 1-40 unique bounded text fixtures".into()); }
    Ok(fixtures)
}

pub fn validate_cli(args: &[String]) -> Result<(), String> {
    let allowed = ["--verify-writing-fixtures","--writing-database","--writing-route",
        "--writing-profile","--writing-books","--writing-connection","--writing-credential-slot","--writing-budget","--writing-completion"];
    let mut seen = std::collections::HashSet::new();
    for pair in args.chunks(2) {
        if pair.len()!=2 || !allowed.contains(&pair[0].as_str()) || pair[1].starts_with("--")
            || !seen.insert(pair[0].as_str()) { return Err("Unknown, duplicate or missing writing replay argument".into()); }
    }
    if seen.contains("--writing-books") && seen.contains("--writing-completion") {return Err("Use one completion option".into());}
    Ok(())
}

pub async fn replay(path: &Path, database: &Path, route_name: &str, profile_mode: &str, options: ReplayOptions<'_>) -> Result<Value, String> {
    if !matches!(route_name,"custom"|"trial") || !matches!(profile_mode,"fresh"|"current"|"paragraph"|"numbered") {
        return Err("Expected route custom/trial and profile fresh/current/paragraph/numbered".into());
    }
    let fixtures = read_fixtures(path)?;
    let snapshot = Store::open_read_only(database)?;
    let settings: Settings = snapshot.get("settings");
    let mut profile: Profile = if profile_mode=="current" {snapshot.get("profile")} else {Profile::default()};
    if matches!(profile_mode,"paragraph"|"numbered") {
        let layout=if profile_mode=="paragraph" {Layout::Paragraph} else {Layout::Numbered};
        for (i,scene) in [Scene::Chat,Scene::Tasks,Scene::Technical].into_iter().enumerate() {
            profile.feedback(i as i64,scene,layout,true)?;
        }
    }
    let dictionary = if profile_mode=="current" {settings.dictionary.clone()} else {vec![]};
    let mut connection = if route_name=="trial" {ConnectionConfig::default()} else {settings.llm.clone()};
    if let Some(file) = options.connection {
        if route_name!="custom" {return Err("Test connection overrides require custom route".into());}
        if std::fs::metadata(file).map_err(|_|"Test connection missing")?.len()>8192 {return Err("Test connection too large".into());}
        let c:TestConnection=serde_json::from_slice(&std::fs::read(file).map_err(|_|"Test connection unreadable")?)
            .map_err(|_|"Invalid test connection; use only base/model/protocol, never credentials")?;
        connection=ConnectionConfig{base:c.base,model:c.model,protocol:c.protocol};
    }
    if route_name=="custom" && connection.base.is_empty() {return Err("No custom LLM configured".into());}
    let route=trial::resolve(&connection,false,settings.trial_enabled,
        if route_name=="trial" {crate::platform::has_secret(trial::SLOT)?} else {false})?;
    if route.trial && !trial::consented(&settings) {return Err("Trial privacy consent is required".into());}
    let slot=options.credential_slot.unwrap_or("llm");
    if !matches!(slot,"llm"|"backup") {return Err("Expected credential slot llm/backup".into());}
    let key=trial::key(&route,slot)?;
    let requested=options.books.unwrap_or(settings.complete_book_titles);
    let mut reports=vec![];
    let generation_budget=options.generation_budget.unwrap_or(fixtures.len()*2).min(fixtures.len()*2);
    if generation_budget==0 {return Err("Expected positive generation budget".into());}
    let fixture_count=fixtures.len();
    let mut generations=0;
    let mut accounted=0;
    let mut consecutive_errors=0;
    for fixture in fixtures {
        if accounted>=generation_budget {break;}
        let available=(generation_budget-accounted).min(2);
        let semantic_text=vao_core::normalize_spoken_phone_numbers(&fixture.text);
        let plan=WritingPlan::resolve(&semantic_text,&fixture.app,&[],&profile);
        let request=models::WritingRequest{text:&semantic_text,plan:&plan,dictionary:&dictionary,
            translation:fixture.translation.as_deref(),selection:fixture.selection.as_deref(),
            options:WritingOptions{semantic_completion:requested}};
        let started=std::time::Instant::now();
        let output=models::refine_writing_limited(&route.config,&key,&request,available).await;
        let mut report=json!({"id":fixture.id,"source":fixture.text,"semantic_text":semantic_text,"app":fixture.app,"plan":plan,
            "operation":if fixture.selection.is_some(){"selection_action"}else if fixture.translation.is_some(){"translation"}else{"dictation_cleanup"},
            "latency_ms":started.elapsed().as_millis(),"completion_requested":requested});
        match output {
            Ok(value)=>{
                consecutive_errors=0;
                generations+=value.generation_attempts;
                accounted+=value.generation_attempts;
                report["output"]=json!(value.text);
                report["repair_attempted"]=json!(value.repair_attempted);
                report["used_original"]=json!(value.used_original);
                report["trace"]=serde_json::to_value(value).unwrap();
            }
            Err(error)=>{
                consecutive_errors+=1;
                report["error"]=json!("model_request_failed");
                report["retryable"]=json!(error.retryable);
                let status=error.message.split("HTTP ").nth(1)
                    .map(|tail|tail.chars().take_while(char::is_ascii_digit).collect::<String>())
                    .and_then(|n|n.parse::<u16>().ok()).filter(|n|(100..=599).contains(n));
                report["http_status"]=json!(status);
                report["attempts_upper_bound"]=json!(available);
                accounted+=available;
            }
        }
        reports.push(report);
        if consecutive_errors>=3 {break;}
    }
    Ok(json!({"policy_version":vao_core::POLICY_VERSION,"route":route_name,"model":route.config.model,
        "protocol":route.config.protocol,"profile":profile_mode,"semantic_completion":requested,
        "generation_budget":generation_budget,"successful_case_generation_attempts":generations,
        "accounted_attempts_upper_bound":accounted,"unrun_cases":fixture_count-reports.len(),"network_fallback":false,
        "version":env!("VAO_RELEASE_VERSION"),"build":env!("VAO_BUILD_NUMBER"),
        "stopped_after_consecutive_errors":consecutive_errors>=3,
        "knowledge_source":if requested {"model knowledge inference; no retrieval"}else{"disabled"},
        "microphone":false,"inserted_text":false,"wrote_settings_or_feedback":false,"results":reports}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_options_and_fixtures_are_explicit() {
        assert!(validate_cli(&["--writing-books".into(),"on".into(),"--unknown".into(),"x".into()]).is_err());
        assert!(validate_cli(&["--writing-books".into(),"on".into(),"--writing-books".into(),"off".into()]).is_err());
        assert!(serde_json::from_str::<Fixture>(r#"{"id":"a","text":"test","app":"Notes","complete_book_titles":true}"#).is_err());
        assert!(serde_json::from_str::<Fixture>(r#"{"id":"a","text":"test","app":"Notes","selection":"selected","translation":"English"}"#).is_ok());
        assert!(serde_json::from_str::<TestConnection>(r#"{"base":"https://example.com","model":"test","protocol":"openai_chat","key":"not-allowed"}"#).is_err());
    }
}
