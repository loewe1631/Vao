// Isolated native regression harness. These commands reject normal app launches.
use crate::{platform, voice_bar, AppState, Phase};
use serde_json::{json, Value};
use tauri::Manager;

#[tauri::command]
pub async fn probe_overlay_event(window: tauri::WebviewWindow, x: f64, y: f64, kind: i32) -> Result<(), String> {
    crate::verification_output()?;
    if window.label() != "voicebar" || !x.is_finite() || !y.is_finite() {
        return Err("Isolated voice overlay only".into());
    }
    voice_bar::probe_event(window, x, y, kind).await
}

async fn pause() { tokio::time::sleep(std::time::Duration::from_millis(750)).await; }

async fn gesture(bar: &tauri::WebviewWindow, selector: &str, kind: i32) -> Result<(), String> {
    let selector = serde_json::to_string(selector).map_err(|e| e.to_string())?;
    bar.eval(format!("(()=>{{const r=document.querySelector({selector}).getBoundingClientRect();window.__TAURI__.core.invoke('probe_overlay_event',{{x:r.x+r.width/2,y:r.y+r.height/2,kind:{kind}}}).catch(e=>window.__TAURI__.core.invoke('report_frontend_state',{{stage:'overlay-event-error:'+String(e)}}));}})()"))
        .map_err(|e| e.to_string())?;
    pause().await;
    Ok(())
}

fn check(checks: &mut Vec<Value>, name: impl Into<String>, passed: bool) {
    checks.push(json!({"name":name.into(),"passed":passed}));
}

async fn focus_check(app: &tauri::AppHandle, checks: &mut Vec<Value>, label: &str, expected: &Value) -> Result<(), String> {
    let main = app.get_webview_window("main").ok_or("Missing main")?;
    let bar = app.get_webview_window("voicebar").ok_or("Missing bar")?;
    let current = platform::target(false)?;
    let native = voice_bar::inspect(bar).await?;
    crate::report_frontend_state(json!({"kind":"overlay-focus-evidence","name":label,"native":native,"expected_pid":expected["pid"],"actual_pid":current["pid"],"main_visible":main.is_visible().unwrap_or(true)}).to_string());
    check(checks, format!("{label}_original_app_frontmost"), current["pid"] == expected["pid"]);
    check(checks, format!("{label}_main_stays_hidden"), !main.is_visible().unwrap_or(true));
    check(checks, format!("{label}_panel_never_key_or_main"), native["key"] == false && native["main"] == false && native["owner_visible"] == false && native["nonactivating_panel"] == true);
    Ok(())
}

#[tauri::command]
pub async fn probe_overlay(app: tauri::AppHandle) -> Result<Value, String> {
    let directory = crate::verification_output()?.join("selection-fixtures");
    let main = app.get_webview_window("main").ok_or("Missing main")?;
    let bar = app.get_webview_window("voicebar").ok_or("Missing bar")?;
    let mut checks = vec![];
    let mut pids = vec![];
    voice_bar::probe_positions(&app, true).await?;
    platform::call(json!({"action":"probe_clipboard","mode":"begin"}))?;
    let result = async {
        // The preceding composer suite has just terminated these same fixture apps.
        // Let LaunchServices finish that transition before requesting new instances.
        pause().await;
        for label in ["Composer", "Opaque", "A"] {
            main.hide().map_err(|e| e.to_string())?;
            let launch = std::process::Command::new("/usr/bin/open").args(["-n", "-a"])
                .arg(directory.join(format!("SelectionFixture{label}.app")))
                .output().map_err(|e| e.to_string())?;
            if !launch.status.success() {
                return Err(format!("Could not launch overlay fixture: {}", String::from_utf8_lossy(&launch.stderr)));
            }
            tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
            let target = platform::target(true)?;
            if target["bundle"] != format!("app.vaovao.fixture{}", label.to_lowercase()) {
                return Err("Wrong overlay test app".into());
            }
            pids.push(target["pid"].as_i64().ok_or("Missing fixture PID")?);
            let display = voice_bar::capture(&app, target["pid"].as_i64().unwrap() as i32).await;
            let start = voice_bar::mouse_events();
            let id = {
                let state = app.state::<AppState>();
                let mut r = state.runtime.lock().unwrap();
                let id = r.session.begin()?;
                r.target = target.clone(); r.target["display_id"] = json!(display);
                r.bar_mouse_start = start; r.answer.clear(); r.notice = None;
                r.session.advance(id, Phase::Listening)?;
                id
            };
            crate::show_bar(&app, true);
            pause().await;
            focus_check(&app, &mut checks, &format!("{label}_show"), &target).await?;
            gesture(&bar, ".voicebar", 0).await?;
            focus_check(&app, &mut checks, &format!("{label}_wave_click"), &target).await?;
            check(&mut checks, format!("{label}_native_click_registered"), voice_bar::mouse_events().wrapping_sub(start) == 1);
            let before = voice_bar::inspect(bar.clone()).await?;
            gesture(&bar, ".voicebar", 1).await?;
            let after = voice_bar::inspect(bar.clone()).await?;
            check(&mut checks, format!("{label}_wave_actually_moved"), before["x"] != after["x"] || before["y"] != after["y"]);
            focus_check(&app, &mut checks, &format!("{label}_wave_drag"), &target).await?;
            let allowed = voice_bar::mouse_events().wrapping_sub(start);
            check(&mut checks, format!("{label}_two_physical_clicks_counted_once"), allowed == 2);
            let text = format!("{label}浮层操作后回填成功。");
            if label != "A" {
                let sent = platform::call(json!({"action":"deliver","target":target,"text":text,"allowed_key_events":0,"allowed_mouse_events":allowed}));
                check(&mut checks, format!("{label}_delivery_after_drag"), sent.is_ok());
                let received = platform::call(json!({"action":"probe_fixture","mode":"read"}))?;
                check(&mut checks, format!("{label}_input_received_actual_text"), received["text"].as_str().is_some_and(|v| v.contains(&text)));
            }
            // A still has a real selected passage; answer clicks must preserve it.
            let selected = platform::target(true)?;
            {
                let state = app.state::<AppState>(); let mut r = state.runtime.lock().unwrap();
                r.session.advance(id, Phase::Processing)?;
                r.session.advance(id, Phase::Ready)?;
            }
            crate::present_answer(&app, id, &"合成追问结果，保留原应用和选区。\n".repeat(100));
            pause().await;
            let before = voice_bar::inspect(bar.clone()).await?;
            gesture(&bar, ".answer-handle", 1).await?;
            let after = voice_bar::inspect(bar.clone()).await?;
            check(&mut checks, format!("{label}_answer_actually_moved"), before["x"] != after["x"] || before["y"] != after["y"]);
            focus_check(&app, &mut checks, &format!("{label}_answer_drag"), &selected).await?;
            gesture(&bar, "#bar-message", 0).await?;
            focus_check(&app, &mut checks, &format!("{label}_answer_body_click"), &selected).await?;
            gesture(&bar, "#bar-message", 2).await?;
            focus_check(&app, &mut checks, &format!("{label}_answer_scroll"), &selected).await?;
            let current = platform::target(true)?;
            check(&mut checks, format!("{label}_answer_keeps_external_selection"), current["selection"] == selected["selection"] && current["location"] == selected["location"] && current["length"] == selected["length"]);
            gesture(&bar, "#bar-dismiss", 0).await?;
            focus_check(&app, &mut checks, &format!("{label}_answer_close"), &selected).await?;
            check(&mut checks, format!("{label}_close_hides_answer"), !voice_bar::is_visible(bar.clone()).await? && app.state::<AppState>().runtime.lock().unwrap().answer.is_empty());
            // An actual external click must remain disallowed even after overlay clicks.
            if label != "A" {
                let target = platform::target(true)?;
                let baseline = voice_bar::mouse_events();
                platform::call(json!({"action":"probe_fixture","mode":"mouse"}))?;
                let rejected = platform::call(json!({"action":"deliver","target":target,"text":"不可输入","allowed_key_events":0,"allowed_mouse_events":voice_bar::mouse_events().wrapping_sub(baseline)}));
                check(&mut checks, format!("{label}_external_click_still_rejected"), rejected.is_err());
            }
        }
        Ok::<(), String>(())
    }.await;
    crate::show_bar(&app, false);
    let _ = voice_bar::probe_positions(&app, false).await;
    {
        let state = app.state::<AppState>(); let mut r = state.runtime.lock().unwrap();
        r.session.cancel(); r.answer.clear();
    }
    let _ = platform::call(json!({"action":"probe_clipboard","mode":"restore"}));
    for pid in pids { let _ = std::process::Command::new("/bin/kill").args(["-TERM", &pid.to_string()]).status(); }
    crate::show_main(&app);
    result?;
    Ok(json!({"checks":checks}))
}
