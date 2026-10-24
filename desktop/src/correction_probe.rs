//! Synthetic native WKWebView/NSPanel checks. Never available on a normal launch.
use crate::{correction_runtime, voice_bar, AppState, Phase};
use serde_json::{json, Value};
use tauri::Manager;
use vao_core::corrections::{Book, Pair};

#[tauri::command]
pub async fn probe_corrections(app: tauri::AppHandle) -> Result<Value, String> {
    crate::verification_output()?;
    let main = app.get_webview_window("main").ok_or("Missing main")?;
    let bar = app
        .get_webview_window("voicebar")
        .ok_or("Missing overlay")?;
    let mut checks = vec![];
    for mode in ["replace", "keep", "cancel"] {
        let id = {
            let state = app.state::<AppState>();
            let mut r = state.runtime.lock().unwrap();
            r.session.cancel();
            r.pending_correction = None;
            r.correction_resolved = None;
            r.correction_book = Book::default();
            r.correction_book.remember(&[Pair {
                wrong: "Tablets".into(),
                correct: "typeless".into(),
            }])?;
            let id = r.session.begin()?;
            r.session.advance(id, Phase::Listening)?;
            r.session.advance(id, Phase::Processing)?;
            r.target = json!({});
            r.settings.auto_input = false;
            r.notice = None;
            r.answer.clear();
            let record=serde_json::from_value(json!({"id":0,"created":crate::now(),"scene":"chat","layout":"paragraph","revision":0,"original":"测试Tablets，取件码007417。","text":"测试Tablets，取件码007417。","risks":[],"app":"Synthetic correction check","delivery":"preview","feedback":false})).map_err(|e|e.to_string())?;
            let record = state.store.lock().unwrap().add(record)?;
            r.result = Some(record.clone());
            correction_runtime::prepare(&mut r, &record, true)?;
            id
        };
        main.hide().map_err(|e| e.to_string())?;
        crate::show_bar(&app, true);
        tokio::time::sleep(std::time::Duration::from_millis(1400)).await;
        let panel = voice_bar::inspect(bar.clone()).await?;
        checks.push(json!({"name":format!("{mode}_native_nonactivating_panel"),"passed":panel["nonactivating_panel"]==true&&panel["key"]==false&&panel["main"]==false}));
        bar.eval(format!(r#"(()=>{{const p=document.querySelector('.correction-review');const b=document.querySelector('[data-correction-cancel]');window.__TAURI__.core.invoke('report_frontend_state',{{stage:JSON.stringify({{kind:'correction-native-dom',mode:{},rendered:!!p,waveHidden:document.querySelector('.voicebar').classList.contains('hidden'),actionsFit:!!b&&b.getBoundingClientRect().bottom<=innerHeight}})}});}})()"#,serde_json::to_string(mode).unwrap())).map_err(|e|e.to_string())?;
        let selector = match mode {
            "replace" => "[data-correction-apply]",
            "keep" => "[data-correction-keep]",
            _ => "[data-correction-cancel]",
        };
        // DOM click tests WKWebView -> real IPC -> real DB. It is not a physical mouse test.
        bar.eval(format!(
            "document.querySelector({}).click()",
            serde_json::to_string(selector).unwrap()
        ))
        .map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        {
            let state = app.state::<AppState>();
            let r = state.runtime.lock().unwrap();
            let db = state.store.lock().unwrap();
            let record = r.result.as_ref().ok_or("Missing synthetic result")?;
            let expected = if mode == "replace" {
                "测试typeless，取件码007417。"
            } else {
                "测试Tablets，取件码007417。"
            };
            checks.push(json!({"name":format!("{mode}_actual_database_result"),"passed":db.effective_text(record)?==expected}));
            checks.push(json!({"name":format!("{mode}_original_is_immutable"),"passed":record.text=="测试Tablets，取件码007417。"&&record.original==record.text}));
            checks.push(json!({"name":format!("{mode}_phase_and_prompt_cleared"),"passed":r.session.id==id&&r.pending_correction.is_none()&&r.session.phase==if mode=="cancel"{Phase::Cancelled}else{Phase::Ready}}));
        }
    }
    crate::cancel(app.clone());
    crate::show_main(&app);
    // Avoid touching clipboard, microphone, credentials or external applications.
    Ok(json!({"checks":checks,"physical_click":false,"audio":false,"external_insertion":false}))
}
