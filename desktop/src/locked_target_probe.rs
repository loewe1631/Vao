//! WindowServer integration against isolated synthetic windows, never real chats.
use crate::{platform, store::ResultRecord, AppState};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::Manager;
use vao_core::{Layout, Phase, Scene};

struct Fixture {
    control: PathBuf,
    sequence: u64,
}
impl Fixture {
    async fn command(&mut self, action: &str, field: &str) -> Result<Value, String> {
        self.sequence += 1;
        std::fs::write(
            &self.control,
            json!({"sequence":self.sequence,"action":action,"field":field}).to_string(),
        )
        .map_err(|e| e.to_string())?;
        let response = self.control.with_extension("json.response");
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            if let Ok(data) = std::fs::read(&response) {
                if let Ok(v) = serde_json::from_slice::<Value>(&data) {
                    if v["sequence"] == self.sequence {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        return Ok(v);
                    }
                }
            }
        }
        Err(format!("Fixture timed out: {action} {field}"))
    }
}
async fn launch(dir: &Path, label: &str) -> Result<Fixture, String> {
    let control = dir.join(format!("control-{label}.json"));
    let _ = std::fs::remove_file(&control);
    let _ = std::fs::remove_file(control.with_extension("json.response"));
    let output = std::process::Command::new("/usr/bin/open")
        .args(["-n", "-a"])
        .arg(dir.join(format!("SelectionFixture{label}.app")))
        .arg("--args")
        .arg("--control")
        .arg(&control)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    Ok(Fixture {
        control,
        sequence: 0,
    })
}
fn check(checks: &mut Vec<Value>, name: impl Into<String>, passed: bool) {
    let name = name.into();
    crate::report_frontend_state(
        json!({"kind":"locked-progress","name":name,"passed":passed}).to_string(),
    );
    checks.push(json!({"name":name,"passed":passed}));
}
fn ready(
    app: &tauri::AppHandle,
    target: &Value,
    source: &str,
    text: &str,
) -> Result<(u64, ResultRecord), String> {
    let state = app.state::<AppState>();
    let mut r = state.runtime.lock().unwrap();
    r.session.cancel();
    let id = r.session.begin()?;
    r.session.advance(id, Phase::Listening)?;
    r.session.advance(id, Phase::Processing)?;
    let record = state.store.lock().unwrap().add(ResultRecord {
        id: 0,
        created: crate::now(),
        scene: Scene::Tasks,
        layout: Layout::Paragraph,
        revision: 0,
        original: source.into(),
        text: text.into(),
        risks: vao_core::risks(source, text),
        app: "Isolated fixture".into(),
        delivery: "preview".into(),
        feedback: false,
        refinement: vec![],
    })?;
    r.target = target.clone();
    r.result = Some(record.clone());
    r.error.clear();
    r.answer.clear();
    r.notice = None;
    r.bar_mouse_start = crate::voice_bar::mouse_events();
    r.session.advance(id, Phase::Ready)?;
    Ok((id, record))
}

async fn configured_intent_checks(dir: &Path, checks: &mut Vec<Value>) -> Result<(), String> {
    let file = dir
        .parent()
        .ok_or("Missing diagnostic directory")?
        .join("configured-llm.json");
    if !file.exists() {
        return Ok(());
    }
    // Explicit local diagnostic file only. It holds configuration, never credentials.
    let config: crate::store::ConnectionConfig =
        serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    crate::report_frontend_state(
        json!({"kind":"configured-model-progress","stage":"credential"}).to_string(),
    );
    let key = platform::connection_secret("llm", &config.base)?;
    let mut outputs = vec![];
    for (label,source,scene,selection,translation) in [
        ("gemini_question","如果我修改了会议时间，是不是需要重新发送通知呢？",Scene::Tasks,None,None),
        ("chat_question","你觉得这个方案有什么问题？能不能再检查一下？",Scene::Chat,None,None),
        ("english_question","Should I send another notice after changing the meeting time?",Scene::Tasks,None,None),
        ("selection_answer","会议是什么时候？",Scene::Tasks,Some("会议定在周五下午三点。"),None),
        ("translated_question","我们明天还能见面吗？",Scene::Chat,None,Some("English")),
    ] {
        let prompt=vao_core::system_prompt(scene,Layout::Paragraph,&[],translation,selection);
        let output=crate::models::refine(&config,&key,source,&prompt,selection,&[]).await.map_err(|e|e.message)?;
        let text=output.trim();
        let answer=["是的","当然","可以，","你可以","不可以","以下是","Yes,","Yes.","Certainly","You can"].iter().any(|s|text.starts_with(s));
        let passed=match label {
            "selection_answer" => text.contains("周五")&&(text.contains('三')||text.contains('3'))&&!text.contains('？'),
            "english_question"|"translated_question" => text.contains('?')&&!answer,
            _ => ["是不是","是否","吗","能不能"].iter().any(|s|text.contains(s))&&!answer&&text.chars().count()<source.chars().count()*2,
        };
        check(checks,format!("configured_model_{label}"),passed);
        outputs.push(json!({"case":label,"source":source,"selection":selection,"output":output,"passed":passed}));
        std::fs::write(dir.parent().unwrap().join("configured-intent-results.json"),serde_json::to_vec_pretty(&outputs).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    }
    std::fs::write(
        dir.parent().unwrap().join("configured-intent-results.json"),
        serde_json::to_vec_pretty(&outputs).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn probe_locked_target(app: tauri::AppHandle) -> Result<Value, String> {
    let dir = crate::verification_output()?.join("selection-fixtures");
    let main = app.get_webview_window("main").ok_or("Missing main")?;
    let mut checks = vec![];
    let mut pids = vec![];
    platform::call(json!({"action":"probe_clipboard","mode":"begin"}))?;
    let result = async {
        let mut original = launch(&dir, "Locked").await?;
        pids.push(
            original.command("reset", "A1").await?["pid"]
                .as_i64()
                .ok_or("Missing PID")?,
        );
        let mut browser = launch(&dir, "Browser").await?;
        pids.push(
            browser.command("reset", "R1").await?["pid"]
                .as_i64()
                .ok_or("Missing PID")?,
        );
        for mode in ["browser", "window", "field", "minimized", "risk"] {
            main.hide().map_err(|e| e.to_string())?;
            crate::show_bar(&app, false);
            original.command("reset", "A1").await?;
            let target = platform::target(true)?;
            check(
                &mut checks,
                format!("locked_{mode}_captures_A1"),
                target["bundle"] == "app.vaovao.fixturelocked"
                    && target["identifier"] == "A1"
                    && target["offset"] == 7,
            );
            let (source, text) = if mode == "risk" {
                ("预算100元", "预算200元")
            } else {
                ("记录浏览资料后的结论。", "回填结论。")
            };
            let (id, record) = ready(&app, &target, source, text)?;
            if mode == "risk" {
                check(
                    &mut checks,
                    "nonempty_diagnostics_fixture",
                    !record.risks.is_empty(),
                );
            }
            match mode {
                "window" => {
                    original.command("focus", "B1").await?;
                }
                "field" => {
                    original.command("focus", "A2").await?;
                }
                "minimized" => {
                    original.command("minimize", "A1").await?;
                    browser.command("focus", "R1").await?;
                }
                _ => {
                    browser.command("reset", "R1").await?;
                    let events = platform::call(json!({"action":"probe_locked_interaction"}))?;
                    check(
                        &mut checks,
                        format!("locked_{mode}_real_research_events"),
                        events["mouse"].as_u64().unwrap_or(0) > 0
                            && events["keys"].as_u64().unwrap_or(0) > 0,
                    );
                }
            }
            let research = browser.command("read", "R1").await?;
            crate::route_result(&app, id, &target, &record, true);
            let received = original.command("read", "A1").await?;
            check(
                &mut checks,
                format!("locked_{mode}_inserts_at_original_caret"),
                received["fields"]["A1"]["text"] == format!("prefix {text}suffix"),
            );
            check(
                &mut checks,
                format!("locked_{mode}_other_fields_untouched"),
                received["fields"]["A2"]["text"] == "A2 seed"
                    && received["fields"]["B1"]["text"] == "B1 seed",
            );
            check(
                &mut checks,
                format!("locked_{mode}_original_window_frontmost"),
                platform::target(false)?["pid"] == target["pid"] && received["key_window"] == "A",
            );
            check(
                &mut checks,
                format!("locked_{mode}_browser_unchanged"),
                browser.command("read", "R1").await?["fields"] == research["fields"],
            );
            let main_hidden = !main.is_visible().unwrap_or(true);
            let r = app.state::<AppState>();
            let r = r.runtime.lock().unwrap();
            check(
                &mut checks,
                format!("locked_{mode}_completed_no_main"),
                r.session.phase == Phase::Completed && main_hidden,
            );
            check(
                &mut checks,
                format!("locked_{mode}_delivery_recorded"),
                r.result
                    .as_ref()
                    .is_some_and(|r| r.delivery == "paste_sent"),
            );
        }
        for mode in ["change", "caret", "close", "exit"] {
            main.hide().map_err(|e| e.to_string())?;
            crate::show_bar(&app, false);
            original.command("reset", "A1").await?;
            let target = platform::target(true)?;
            let (id, record) = ready(&app, &target, "保留文字。", "不可误填的结果。")?;
            if mode == "exit" {
                let status = std::process::Command::new("/bin/kill")
                    .args(["-TERM", &pids[0].to_string()])
                    .status()
                    .map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err("Could not close fixture".into());
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            } else {
                original.command(mode, "A1").await?;
            }
            browser.command("reset", "R1").await?;
            let before = browser.command("read", "R1").await?;
            crate::route_result(&app, id, &target, &record, true);
            check(
                &mut checks,
                format!("locked_{mode}_no_browser_paste"),
                browser.command("read", "R1").await?["fields"] == before["fields"],
            );
            check(
                &mut checks,
                format!("locked_{mode}_main_hidden"),
                !main.is_visible().unwrap_or(true),
            );
            {
                let state = app.state::<AppState>();
                let r = state.runtime.lock().unwrap();
                check(
                    &mut checks,
                    format!("locked_{mode}_preserved_with_notice"),
                    r.session.phase == Phase::Failed
                        && r.result.as_ref().is_some_and(|r| r.text == record.text)
                        && r.notice == Some(crate::InputNotice::DeliveryTargetUnavailable)
                        && !r.error.is_empty(),
                );
            }
        }
        original = launch(&dir, "Locked").await?;
        pids.push(
            original.command("reset", "A1").await?["pid"]
                .as_i64()
                .ok_or("Missing PID")?,
        );
        crate::show_bar(&app, false);
        main.hide().map_err(|e| e.to_string())?;
        original.command("select", "A1").await?;
        let target = platform::target(true)?;
        check(
            &mut checks,
            "locked_question_selected_source",
            target["selection"] == "prefix",
        );
        let (id, record) = ready(&app, &target, "这段话是什么意思？", "这是选中文字的解释。")?;
        browser.command("focus", "R1").await?;
        crate::route_result(&app, id, &target, &record, true);
        let answer_state = original.command("read", "A1").await?;
        check(
            &mut checks,
            "locked_question_returns_source_without_replacing",
            answer_state["fields"]["A1"]["text"] == "prefix suffix"
                && platform::target(false)?["pid"] == target["pid"],
        );
        let main_hidden = !main.is_visible().unwrap_or(true);
        {
            let state = app.state::<AppState>();
            let r = state.runtime.lock().unwrap();
            check(
                &mut checks,
                "locked_question_shows_answer_no_main",
                r.answer == record.text && r.session.phase == Phase::Completed && main_hidden,
            );
        }
        crate::show_bar(&app, false);
        original.command("reset", "A1").await?;
        let target = platform::target(true)?;
        let (id, record) = ready(&app, &target, "预览模式。", "仅供预览。")?;
        browser.command("focus", "R1").await?;
        crate::route_result(&app, id, &target, &record, false);
        check(
            &mut checks,
            "explicit_preview_preserved",
            original.command("read", "A1").await?["fields"]["A1"]["text"] == "prefix suffix"
                && main.is_visible().unwrap_or(false),
        );

        main.hide().map_err(|e| e.to_string())?;
        original.command("reset", "A1").await?;
        original.command("select", "A1").await?;
        let target = platform::target(true)?;
        let (id, record) = ready(&app, &target, "改成英文标题。", "Heading")?;
        browser.command("focus", "R1").await?;
        crate::route_result(&app, id, &target, &record, true);
        check(
            &mut checks,
            "locked_selection_edit_replaces_original_only",
            original.command("read", "A1").await?["fields"]["A1"]["text"] == "Heading suffix",
        );
        let duplicate = crate::deliver_inner(&app, id);
        check(
            &mut checks,
            "locked_duplicate_delivery_rejected",
            duplicate.is_err()
                && original.command("read", "A1").await?["fields"]["A1"]["text"]
                    == "Heading suffix",
        );
        crate::show_bar(&app, false);
        main.hide().map_err(|e| e.to_string())?;
        let launched = std::process::Command::new("/usr/bin/open")
            .args(["-n", "-a"])
            .arg(dir.join("SelectionFixtureOpaque.app"))
            .output()
            .map_err(|e| e.to_string())?;
        if !launched.status.success() {
            return Err("Opaque fixture launch failed".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        let target = platform::target(true)?;
        if target["bundle"] != "app.vaovao.fixtureopaque" {
            return Err("Wrong opaque fixture".into());
        }
        pids.push(target["pid"].as_i64().ok_or("Missing opaque PID")?);
        let (id, record) = ready(&app, &target, "不透明目标。", "不能误填。")?;
        browser.command("reset", "R1").await?;
        crate::route_result(&app, id, &target, &record, true);
        let main_hidden = !main.is_visible().unwrap_or(true);
        {
            let state = app.state::<AppState>();
            let r = state.runtime.lock().unwrap();
            check(
                &mut checks,
                "opaque_cross_app_keeps_foreground_protection",
                r.session.phase == Phase::Failed
                    && r.notice == Some(crate::InputNotice::DeliveryTargetUnavailable)
                    && main_hidden,
            );
        }
        check(
            &mut checks,
            "opaque_cross_app_does_not_paste_to_browser",
            browser.command("read", "R1").await?["fields"]["R1"]["text"] == "R1 seed",
        );
        configured_intent_checks(&dir, &mut checks).await?;
        Ok::<(), String>(())
    }
    .await;
    crate::show_bar(&app, false);
    {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        r.session.cancel();
        r.notice = None;
        r.answer.clear();
        r.error.clear();
    }
    let _ = platform::call(json!({"action":"probe_clipboard","mode":"restore"}));
    for pid in pids {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .output();
    }
    crate::show_main(&app);
    crate::report_frontend_state(
        json!({"kind":"locked-target-evidence","checks":checks,"error":result.as_ref().err()})
            .to_string(),
    );
    result?;
    Ok(json!({"checks":checks}))
}
