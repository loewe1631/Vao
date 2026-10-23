#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod auto_stop;
mod connection_test;
mod dictionary_io;
use dictionary_io::{import_dictionary,export_dictionary};
mod transcription;
mod hotkeys;
mod input_feedback;
#[cfg(test)]
mod integration_tests;
mod local_capture;
mod models;
mod native_keys;
mod overlay_probe;
mod locked_target_probe;
mod permissions;
mod platform;
mod store;
mod trial;
mod voice_bar;
mod writing_probe;
#[cfg(target_os = "windows")]
mod windows_input;
#[cfg(target_os = "windows")]
mod windows_uia;
#[cfg(target_os = "windows")]
mod windows_files;
#[cfg(target_os = "windows")]
mod windows_paste;
#[cfg(target_os = "windows")]
mod windows_startup;
mod windows_input_probe;
mod windows_updater;

use input_feedback::{FlowError, InputNotice};
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use store::{ConnectionConfig, ResultRecord, Settings, Store};
use tauri::{Emitter, Manager, State};
use vao_core::{Layout, Phase, Profile, Scene, Session};

struct Runtime {
    session: Session,
    capture: Option<audio::Capture>,
    target: Value,
    markers: Vec<usize>,
    translating: bool,
    settings: Settings,
    profile: Profile,
    result: Option<ResultRecord>,
    error: String,
    detail: String,
    started: Option<Instant>,
    hotkeys: bool,
    notice: Option<InputNotice>,
    notice_serial: u64,
    answer: String,
    answer_serial: u64,
    bar_mouse_start: u32,
    auto_stopped: bool,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            session: Session::default(),
            capture: None,
            target: json!({}),
            markers: vec![],
            translating: false,
            settings: Settings::default(),
            profile: Profile::default(),
            result: None,
            error: String::new(),
            detail: String::new(),
            started: None,
            hotkeys: false,
            notice: None,
            notice_serial: 0,
            answer: String::new(),
            answer_serial: 0,
            bar_mouse_start: 0,
            auto_stopped: false,
        }
    }
}
impl Runtime {
    fn claim_finish(&mut self, id: u64) -> bool {
        self.session.id == id && self.session.phase == Phase::Listening
            && self.session.advance(id, Phase::Processing).is_ok()
    }

    #[cfg(any(target_os = "windows", test))]
    fn windows_current_input(&self) -> bool {
        !self.settings.windows_return_to_origin && self.target["selection"].as_str().unwrap_or("").is_empty()
    }
    fn recording_warning(&mut self, id: u64) -> Option<u64> {
        if self.session.id != id
            || !matches!(self.session.phase, Phase::Ready | Phase::Completed)
            || self.notice.is_some()
            || !self.answer.is_empty()
        {
            return None;
        }
        self.notice_serial += 1;
        self.notice = Some(InputNotice::RecordingInterrupted);
        Some(self.notice_serial)
    }
    fn pending_delivery(&self) -> bool {
        self.session.phase == Phase::Ready
            && self.answer.is_empty()
            && self.target["selection"].as_str().unwrap_or("").is_empty()
    }

    fn present_answer(&mut self, id: u64, text: &str) -> Result<u64, String> {
        if self.session.id != id
            || !matches!(
                self.session.phase,
                Phase::Ready | Phase::Completed | Phase::Failed
            )
        {
            return Err("stale answer".into());
        }
        if self.session.phase == Phase::Ready {
            self.session.advance(id, Phase::Completed)?;
        }
        self.answer_serial += 1;
        self.answer = text.to_owned();
        self.notice = None;
        Ok(self.answer_serial)
    }
}
struct AppState {
    runtime: Mutex<Runtime>,
    store: Mutex<Store>,
}
struct Hotkeys {
    manager: hotkeys::HotkeyManager,
    bindings: Mutex<ShortcutBindings>,
}
struct ShortcutBindings {
    keys: [hotkeys::Binding; 3],
    registered: Vec<global_hotkey::hotkey::HotKey>,
}
fn escape_key(app: &tauri::AppHandle, enabled: bool) {
    app.state::<native_keys::NativeKeys>().active(enabled);
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(keys) = h.try_state::<Hotkeys>() {
            let result = {
                let mut bindings = keys.bindings.lock().unwrap();
                let desired = bindings.keys[..if enabled { 3 } else { 2 }]
                    .iter()
                    .filter_map(hotkeys::Binding::global)
                    .collect::<Vec<_>>();
                hotkeys::replace(&keys.manager, &mut bindings.registered, &desired, || Ok(()))
            };
            if let Err(e) = result {
                h.state::<AppState>().runtime.lock().unwrap().error = e;
            }
        }
    });
}
#[tauri::command]
fn shortcut_status(app: tauri::AppHandle) -> serde_json::Value {
    if let Some(keys) = app.try_state::<Hotkeys>() {
        let b = keys.bindings.lock().unwrap();
        json!({"record":b.keys[0].to_string(),"translate":b.keys[1].to_string(),"cancel":b.keys[2].to_string(),"native_ready":app.state::<native_keys::NativeKeys>().ready(),"registered":b.registered.iter().map(|k|k.to_string()).collect::<Vec<_>>()})
    } else {
        json!({})
    }
}
#[tauri::command]
async fn save_shortcuts(
    app: tauri::AppHandle,
    shortcuts: hotkeys::Shortcuts,
) -> Result<Value, String> {
    let parsed = shortcuts.bindings()?;
    let native_error = if parsed.iter().any(|k| k.native().is_some()) {
        app.state::<native_keys::NativeKeys>().ensure(&app).err()
    } else {
        None
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let h = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| {
            let state = h.state::<AppState>();
            let mut runtime = state.runtime.lock().unwrap();
            if matches!(
                runtime.session.phase,
                Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Delivering
            ) {
                return Err(
                    "请结束或取消当前录音后再修改 / Finish or cancel the session first".into(),
                );
            }
            let keys = h
                .try_state::<Hotkeys>()
                .ok_or("快捷键服务不可用，请重启 / Restart required")?;
            let mut bindings = keys.bindings.lock().unwrap();
            if let Some(cancel) = parsed[2].global() {
                if !bindings.registered.contains(&cancel) {
                    keys.manager.register(cancel).map_err(|e| {
                        format!("取消快捷键被占用 / Cancel shortcut unavailable: {e}")
                    })?;
                    bindings.registered.push(cancel);
                    keys.manager.unregister(cancel).map_err(|e| e.to_string())?;
                    bindings.registered.retain(|k| *k != cancel);
                }
            }
            let db = state.store.lock().unwrap();
            let mut settings: Settings = db.get("settings");
            settings.shortcuts = shortcuts;
            let result = hotkeys::replace(
                &keys.manager,
                &mut bindings.registered,
                &parsed[..2]
                    .iter()
                    .filter_map(hotkeys::Binding::global)
                    .collect::<Vec<_>>(),
                || db.set("settings", &settings),
            );
            if result.is_ok() {
                h.state::<native_keys::NativeKeys>()
                    .configure(std::array::from_fn(|i| parsed[i].native()));
                bindings.keys = parsed;
                runtime.hotkeys = native_error.is_none();
                runtime.error = native_error.clone().unwrap_or_default();
            } else {
                runtime.hotkeys = hotkeys::ready(
                    &bindings.keys,
                    &bindings.registered,
                    h.state::<native_keys::NativeKeys>().ready(),
                );
            }
            result.map(|_| json!({"saved":true,"active":runtime.hotkeys}))
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "快捷键更新未完成 / Shortcut update interrupted".to_string())?
}
#[tauri::command]
fn shortcut_capture(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let r = state.runtime.lock().unwrap();
    if enabled
        && matches!(
            r.session.phase,
            Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Delivering
        )
    {
        return Err("请先结束录音 / Finish recording first".into());
    }
    drop(r);
    let native = app.state::<native_keys::NativeKeys>();
    native.suspend(enabled);
    native.capture(enabled);
    let h = app.clone();
    app.run_on_main_thread(move || {
        let _ = &h;
        local_capture::arm(enabled);
    })
    .map_err(|e| e.to_string())?;
    Ok(true)
}
fn dispatch_shortcut(h: tauri::AppHandle, action: usize) {
    if h.state::<native_keys::NativeKeys>().suspended() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if action == 2 {
            cancel(h);
            return;
        }
        if action == 1 {
            if let Err(e) = toggle_translation(h.clone()).await {
                h.state::<AppState>().runtime.lock().unwrap().error = e;
            }
            return;
        }
        let pending = {
            let state = h.state::<AppState>();
            let runtime = state.runtime.lock().unwrap();
            runtime
                .pending_delivery()
                .then(|| (runtime.session.id, runtime.target["pid"].clone()))
        };
        let res = if let Some((id, expected_pid)) = pending {
            let target = platform::target(false).unwrap_or(json!({}));
            if target["pid"] == expected_pid && target["pid"].is_number() {
                deliver_inner(&h, id)
            } else {
                toggle_recording(h.clone()).await
            }
        } else {
            toggle_recording(h.clone()).await
        };
        if let Err(e) = res {
            h.state::<AppState>().runtime.lock().unwrap().error = e;
            show_main(&h);
        }
    });
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn current(app: &tauri::AppHandle, id: u64) -> Result<(), String> {
    if app
        .state::<AppState>()
        .runtime
        .lock()
        .unwrap()
        .session
        .active(id)
    {
        Ok(())
    } else {
        Err("cancelled".into())
    }
}
fn detail(app: &tauri::AppHandle, id: u64, text: &str) {
    let state = app.state::<AppState>();
    let mut r = state.runtime.lock().unwrap();
    if r.session.active(id) {
        r.detail = text.into();
    }
}
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}
fn login_background_launch(args: &[String]) -> bool {
    #[cfg(target_os = "windows")]
    { windows_startup::background_launch(args) }
    #[cfg(not(target_os = "windows"))]
    { let _ = args; false }
}
fn show_bar(app: &tauri::AppHandle, show: bool) {
    let _ = app.emit("session-updated", ());
    if let Some(w) = app.get_webview_window("voicebar") {
        if show {
            let (display, id, expanded) = {
                let state = app.state::<AppState>();
                let r = state.runtime.lock().unwrap();
                (
                    r.target["display_id"].as_u64().unwrap_or(0) as u32,
                    r.session.id,
                    if !r.answer.is_empty(){1}else{0},
                )
            };
            let handle = app.clone();
            let window = w.clone();
            let _ = app.run_on_main_thread(move || {
                let state = handle.state::<AppState>();
                let r = state.runtime.lock().unwrap();
                let visible = r.session.id == id && (r.session.active(id) || r.notice.is_some() || !r.answer.is_empty());
                drop(r);
                if visible {
                    voice_bar::place(&window, display, expanded);
                    voice_bar::set_visible(&window, true);
                }
            });
        } else {
            let _ = app.run_on_main_thread(move || voice_bar::set_visible(&w, false));
        }
    }
}

#[tauri::command]
fn snapshot(state: State<AppState>) -> Value {
    let r = state.runtime.lock().unwrap();
    let level = r
        .capture
        .as_ref()
        .map(|c| c.level.load(Ordering::Relaxed) as f64 / 10000.)
        .unwrap_or(0.);
    let auto = r.capture.as_ref().and_then(audio::Capture::auto_stop_status);
    let countdown = auto.and_then(|(seconds, status)| status.countdown(seconds));
    let unavailable = auto.is_some_and(|(_, status)| status.unavailable);
    json!({"session":r.session,"level":level,"seconds":r.started.map(|t|t.elapsed().as_secs()).unwrap_or(0),"translating":r.translating,"detail":r.detail,"error":r.error,"result":r.result,"hotkeys":r.hotkeys,"uiLanguage":r.settings.ui_language,"translationLanguage":r.settings.translation_language,"theme":r.settings.theme,"notice":r.notice,"answer":r.answer,"selection":r.target["selection"].as_str().is_some_and(|s|!s.is_empty()),"autoStopCountdown":countdown,"autoStopUnavailable":unavailable})
}
#[tauri::command]
fn update_status(state: State<windows_updater::Updates>) -> windows_updater::Status { state.status() }
#[tauri::command]
async fn check_update(state: State<'_, windows_updater::Updates>) -> Result<windows_updater::Status, String> { state.check().await }
#[tauri::command]
async fn download_update(state: State<'_, windows_updater::Updates>) -> Result<windows_updater::Status, String> { state.download().await }
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker.state::<windows_updater::Updates>().install(|| {
            let state = worker.state::<AppState>();
            let runtime = state.runtime.lock().unwrap();
            !matches!(runtime.session.phase, Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Ready)
        })
    }).await.map_err(|_| "更新安装中断 / Update installation interrupted".to_string())??;
    // NSIS /R starts the updated installation after a successful install.
    app.exit(0);
    Ok(())
}
#[tauri::command]
fn report_frontend_state(stage: String) {
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(i) = args.iter().position(|s| s == "--smoke-test") {
        if let Some(path) = args.get(i + 1) {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{path}.trace"))
            {
                let _ = writeln!(file, "{}", json!({"stage":stage}));
            }
        }
    }
}
#[tauri::command]
async fn report_ui_ready(window: tauri::WebviewWindow, state: State<'_, AppState>) -> Result<bool, String> {
    if window.label() != "main" {
        return Ok(false);
    }
    let args = std::env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--windows-input-probe") { return Ok(false); }
    if let Some(i) = args.iter().position(|s| s == "--smoke-test") {
        if let Some(path) = args.get(i + 1) {
            #[cfg(target_os = "windows")]
            let overlay = {
                let bar = window.app_handle().get_webview_window("voicebar").ok_or("Missing overlay")?;
                let target = bar.clone();
                let (tx,rx) = tokio::sync::oneshot::channel();
                bar.run_on_main_thread(move || {let _=tx.send(platform::windows::probe_overlay(&target));}).map_err(|e| e.to_string())?;
                rx.await.map_err(|_| "Overlay probe interrupted")??
            };
            let r = state.runtime.lock().unwrap();
            #[allow(unused_mut)]
            let mut info = json!({"version":env!("VAO_RELEASE_VERSION"),"bundle":env!("VAO_BUNDLE_ID"),"webview":"main","ipc":"ready","phase":r.session.phase,"hotkeys":r.hotkeys,"size":window.inner_size().ok().map(|s|[s.width,s.height]),"captured_audio":false,"permissions":permissions::snapshot(),"trial":trial::local_status(&state.store.lock().unwrap().get::<Settings>("settings"))});
            #[cfg(target_os = "windows")]
            {
                info["overlay"] = overlay;
                info["main_visible"] = json!(window.is_visible().unwrap_or(true));
                info["main_native_handle"] = json!(window.hwnd().map(|h| h.0 as usize).unwrap_or(0));
                info["tray_ready"] = json!(window.app_handle().tray_by_id("vao-tray").is_some());
                info["login_background"] = json!(login_background_launch(&args));
            }
            std::fs::write(path, serde_json::to_vec_pretty(&info).unwrap())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(args.iter().any(|s| s == "--verify-ui"))
}
#[tauri::command]
fn bootstrap(state: State<AppState>) -> Result<Value, String> {
    let db = state.store.lock().unwrap();
    let data = json!({"settings":db.get::<Settings>("settings"),"profile":db.get::<Profile>("profile"),"history":db.history()?,"version":env!("VAO_RELEASE_VERSION"),"build":env!("VAO_BUILD_NUMBER"),"platform":std::env::consts::OS,"permissions":permissions::snapshot(),"trial":trial::local_status(&db.get::<Settings>("settings"))});
    #[cfg(target_os = "windows")]
    let data = {
        let mut data = data;
        data["startup"] = windows_startup::status().map(|s| json!(s)).unwrap_or_else(|_| json!({"available":false,"registered":false,"system_disabled":false}));
        data
    };
    Ok(data)
}

#[tauri::command]
fn login_startup_status(window: tauri::WebviewWindow) -> Result<Value, String> {
    if window.label() != "main" { return Err("Main window only".into()); }
    #[cfg(target_os = "windows")]
    { windows_startup::status().map(|s| json!(s)) }
    #[cfg(not(target_os = "windows"))]
    { Err("Windows only".into()) }
}

#[tauri::command]
fn set_login_startup(window: tauri::WebviewWindow, enabled: bool) -> Result<Value, String> {
    if window.label() != "main" { return Err("Main window only".into()); }
    #[cfg(target_os = "windows")]
    { windows_startup::set_enabled(enabled).map(|s| json!(s)) }
    #[cfg(not(target_os = "windows"))]
    { let _ = enabled; Err("Windows only".into()) }
}

#[tauri::command]
fn open_startup_settings(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" { return Err("Main window only".into()); }
    #[cfg(target_os = "windows")]
    { platform::windows::open_settings("ms-settings:startupapps") }
    #[cfg(not(target_os = "windows"))]
    { Err("Windows only".into()) }
}

fn validate_settings(s: &Settings) -> Result<(), String> {
    if !(2..=30).contains(&s.silence_seconds) {
        return Err("停顿等待时间须为 2–30 秒 / Pause delay must be 2–30 seconds".into());
    }
    for c in [&s.asr, &s.llm, &s.backup] {
        if !c.base.is_empty() {
            models::endpoint(&c.base, "models")?;
        }
        if c.model.len() > 200 {
            return Err("模型名称过长".into());
        }
    }
    if s.dictionary.len() > 100
        || s.dictionary.iter().any(|s| s.len() > 160)
        || s.translation_language.len() > 40
    {
        return Err("词典或语言设置过长".into());
    }
    if !matches!(s.language.as_str(), "auto" | "zh" | "en") {
        return Err("不支持的识别语言".into());
    }
    if !matches!(s.asr.protocol.as_str(), "openai_audio")
        || ![&s.llm, &s.backup]
            .iter()
            .all(|c| matches!(c.protocol.as_str(), "openai_chat" | "vao_refine"))
    {
        return Err("不支持的模型协议".into());
    }
    Ok(())
}
#[tauri::command]
fn save_settings(state: State<AppState>, mut settings: Settings) -> Result<(), String> {
    validate_settings(&settings)?;
    let db = state.store.lock().unwrap();
    settings.shortcuts = db.get::<Settings>("settings").shortcuts;
    db.save_settings_preserving(&settings)
}
#[tauri::command]
fn save_key(slot: String, value: String, base: String) -> Result<(), String> {
    if !["asr", "llm", "backup"].contains(&slot.as_str()) || value.len() > 4096 {
        return Err("invalid credential".into());
    }
    if value.is_empty() {
        platform::save_secret(&slot, "")
    } else {
        models::endpoint(&base, "models")?;
        platform::save_secret(
            &slot,
            &json!({"base":base.trim().trim_end_matches('/'),"key":value}).to_string(),
        )
    }
}
#[tauri::command]
fn key_status() -> Value {
    json!({"asr":platform::has_secret("asr").unwrap_or(false),"llm":platform::has_secret("llm").unwrap_or(false),"backup":platform::has_secret("backup").unwrap_or(false)})
}
#[tauri::command]
async fn fetch_models(
    slot: String,
    config: ConnectionConfig,
    key: Option<String>,
) -> Result<Vec<String>, String> {
    if !["asr", "llm", "backup"].contains(&slot.as_str()) {
        return Err("invalid slot".into());
    }
    let secret = match key {
        Some(s) => s,
        None => platform::connection_secret(&slot, &config.base)?,
    };
    models::list(&config, &secret).await
}
#[tauri::command]
async fn test_asr(
    slot: String,
    config: ConnectionConfig,
    key: Option<String>,
) -> Result<Value, String> {
    if slot != "asr" { return Err("invalid slot".into()); }
    connection_test::validate_asr(&config)?;
    let secret = match key {
        Some(s) => s,
        None => platform::connection_secret(&slot, &config.base)?,
    };
    let result = connection_test::asr(&config, &secret).await?;
    serde_json::to_value(result).map_err(|_| "连接测试结果无效".into())
}
#[tauri::command]
async fn test_llm(
    slot: String,
    config: ConnectionConfig,
    key: Option<String>,
) -> Result<Value, String> {
    if !["llm", "backup"].contains(&slot.as_str()) {
        return Err("invalid slot".into());
    }
    let secret = match key {
        Some(s) => s,
        None => platform::connection_secret(&slot, &config.base)?,
    };
    let result = models::test_connection(&config, &secret)
        .await
        .map_err(String::from)?;
    serde_json::to_value(result).map_err(|_| "连接测试结果无效".into())
}
#[tauri::command]
async fn permissions(app: tauri::AppHandle) -> Value {
    let before = permissions::snapshot();
    if before["accessibility"] == "granted" {
        let state = app.state::<AppState>();
        let can_recover = {
            let r = state.runtime.lock().unwrap();
            !r.hotkeys
                && !matches!(
                    r.session.phase,
                    Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Delivering
                )
        };
        if can_recover {
            let shortcuts = state
                .store
                .lock()
                .unwrap()
                .get::<Settings>("settings")
                .shortcuts;
            let _ = save_shortcuts(app.clone(), shortcuts).await;
        }
    }
    let mut status = before;
    status["shortcuts_ready"] = json!(app.state::<AppState>().runtime.lock().unwrap().hotkeys);
    status
}
#[tauri::command]
async fn probe_voicebar(app: tauri::AppHandle) -> Result<Value, String> {
    if !std::env::args().any(|a| a == "--verify-ui") {
        return Err("Only available in isolated native tests".into());
    }
    voice_bar::probe(
        app.get_webview_window("voicebar")
            .ok_or("Missing voice bar")?,
    )
    .await
}
#[tauri::command]
async fn probe_capture(app: tauri::AppHandle, chord: String) -> Result<(), String> {
    if !std::env::args().any(|a| a == "--verify-ui") {
        return Err("Only available in isolated native tests".into());
    }
    if ![
        "Fn",
        "Fn+Space",
        "Super+KeyC",
        "Shift+Super+Slash",
        "Super+ArrowLeft",
    ]
    .contains(&chord.as_str())
    {
        return Err("Unsupported fixture".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        if chord.starts_with("Fn") {
            local_capture::probe(0, 63, 1 << 23);
            if chord == "Fn+Space" {
                local_capture::probe(1, 49, 1 << 23);
                local_capture::probe(2, 49, 1 << 23);
            }
            local_capture::probe(0, 63, 0);
        } else {
            let flags = (1 << 20)
                | if chord.starts_with("Shift") {
                    1 << 17
                } else {
                    0
                };
            let key = if chord.ends_with("KeyC") {
                8
            } else if chord.ends_with("Slash") {
                44
            } else {
                123
            };
            local_capture::probe(0, 55, flags);
            local_capture::probe(1, key, flags);
            local_capture::probe(2, key, flags);
            local_capture::probe(0, 55, 0);
        }
        let _ = tx.send(());
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Capture probe interrupted".into())
}
#[tauri::command]
async fn request_permission(app: tauri::AppHandle, kind: String) -> Result<Value, String> {
    permissions::request(&app, &kind).await
}
#[tauri::command]
async fn trial_status() -> Result<Value, String> {
    trial::check().await
}
#[tauri::command]
async fn enroll_trial(app: tauri::AppHandle, consent: bool) -> Result<Value, String> {
    let status = trial::enroll(consent).await?;
    let state = app.state::<AppState>();
    let db = state.store.lock().unwrap();
    let mut settings = db.get::<Settings>("settings");
    settings.trial_enabled = true;
    settings.trial_consent_version = trial::CONSENT_VERSION.into();
    db.set("settings", &settings)?;
    Ok(status)
}
#[tauri::command]
async fn test_trial(state: State<'_, AppState>) -> Result<String, String> {
    if !state
        .store
        .lock()
        .unwrap()
        .get::<Settings>("settings")
        .trial_enabled
    {
        return Err("体验服务已关闭 / Trial is disabled".into());
    }
    if !trial::consented(&state.store.lock().unwrap().get::<Settings>("settings")) {
        return Err("请先同意体验服务说明 / Confirm the trial privacy notice".into());
    }
    let key = platform::secret(trial::SLOT)?;
    if key.is_empty() {
        return Err("此设备尚未开通 / Not provisioned".into());
    }
    let route = trial::resolve(&ConnectionConfig::default(), false, true, true)?;
    models::refine(
        &route.config,
        &key,
        "明天下午三点开会，不对，后天下午四点。",
        "你是语音整理器。只保留明确改口的最终内容，不添加事实。",
        None,
        &[],
    )
    .await
    .map_err(Into::into)
}
#[tauri::command]
async fn repair_permission(app: tauri::AppHandle, kind: String) -> Result<Value, String> {
    let service = match kind.as_str() {
        "accessibility" => "Accessibility",
        "input_monitoring" => "ListenEvent",
        _ => return Err("Invalid permission".into()),
    };
    if std::env::args().any(|a| a == "--verify-ui") {
        return Err("System permission resets are disabled in isolated tests".into());
    }
    if matches!(
        app.state::<AppState>()
            .runtime
            .lock()
            .unwrap()
            .session
            .phase,
        Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Delivering
    ) {
        return Err("请先完成当前录音或处理 / Finish the current recording first".into());
    }
    let result = std::process::Command::new("/usr/bin/tccutil")
        .args(["reset", service, env!("VAO_BUNDLE_ID")])
        .output()
        .map_err(|_| "无法重置授权 / Could not reset permission")?;
    if !result.status.success() {
        return Err("系统未能重新登记，请在系统设置中移除 Vao 后重新添加 / Remove and re-add Vao in System Settings".into());
    }
    permissions::request(&app, &kind).await?;
    open_permission(kind)?;
    Ok(permissions::snapshot())
}
#[tauri::command]
fn restart_for_permissions(app: tauri::AppHandle) -> Result<(), String> {
    if std::env::args().any(|a| a == "--verify-ui") {
        return Err("Restart is disabled in isolated tests".into());
    }
    if matches!(
        app.state::<AppState>()
            .runtime
            .lock()
            .unwrap()
            .session
            .phase,
        Phase::Preparing | Phase::Listening | Phase::Processing | Phase::Delivering
    ) {
        return Err("请先完成当前录音或处理 / Finish the current recording first".into());
    }
    app.request_restart();
    Ok(())
}
#[tauri::command]
fn reveal_application() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", std::env::current_exe().map_err(|e| e.to_string())?.display()))
            .spawn().map_err(|_| "Could not reveal application")?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or("Application path unavailable")?;
    if bundle.extension().and_then(|s| s.to_str()) != Some("app") {
        return Err("Not an installed app bundle".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(bundle)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err("Could not reveal application".into())
            }
        })
    }
}

#[tauri::command]
fn open_permission(kind: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let url = match kind.as_str() {
            "microphone" => "ms-settings:privacy-microphone",
            "keyboard" => "ms-settings:easeofaccess-keyboard",
            _ => return Err("Windows does not use macOS permissions".into()),
        };
        platform::windows::open_settings(url)
    }
    #[cfg(not(target_os = "windows"))]
    {
    let url = match kind.as_str() {
        "input_monitoring" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        "keyboard" => "x-apple.systempreferences:com.apple.Keyboard-Settings.extension",
        "microphone" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        _ => return Err("invalid permission".into()),
    };
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map_err(|_| "无法打开系统设置")?;
    Ok(())
    }
}

#[tauri::command]
async fn toggle_recording(app: tauri::AppHandle) -> Result<(), String> {
    let (phase, id, auto_stopped) = {
        let state = app.state::<AppState>();
        let r = state.runtime.lock().unwrap();
        (r.session.phase, r.session.id, r.auto_stopped)
    };
    if phase == Phase::Listening {
        finish(app, id, false).await
    } else if phase == Phase::Processing && auto_stopped {
        // A stop key pressed at the automatic deadline must not open Settings
        // or dispatch a second model request.
        Ok(())
    } else if matches!(
        phase,
        Phase::Processing | Phase::Preparing | Phase::Delivering
    ) {
        Err(
            "正在处理，可使用取消快捷键或取消按钮 / Busy; use your cancel shortcut or button"
                .into(),
        )
    } else {
        start(app, false).await
    }
}
async fn start(app: tauri::AppHandle, translating: bool) -> Result<(), String> {
    let (settings, profile) = {
        let state = app.state::<AppState>();
        let db = state.store.lock().unwrap();
        (db.get::<Settings>("settings"), db.get::<Profile>("profile"))
    };
    // Reserve the session before asynchronous target/microphone work, preventing double starts.
    let id = {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if app.state::<windows_updater::Updates>().installing.load(Ordering::SeqCst) {
            return Err("正在安装更新，请稍候 / Installing update; please wait".into());
        }
        let id = r.session.begin()?;
        #[cfg(target_os = "windows")]
        { windows_paste::clear(); windows_uia::invalidate(); }
        r.settings = settings.clone();
        r.profile = profile;
        r.target = json!({});
        r.markers.clear();
        if translating {
            r.markers.push(0);
        }
        r.translating = translating;
        r.error.clear();
        r.result = None;
        r.notice = None;
        r.answer.clear();
        r.bar_mouse_start = voice_bar::mouse_events();
        r.started = None;
        r.auto_stopped = false;
        r.detail = "准备麦克风 / Preparing microphone".into();
        id
    };
    let result: Result<(), FlowError> = async {
        let target = tauri::async_runtime::spawn_blocking(|| platform::target(true))
            .await
            .map_err(|_| "读取输入位置失败")??;
        #[cfg(target_os = "windows")]
        let target = {
            let mut target = target;
            if settings.windows_compatible_input {
                target["paste_transport"] = json!(true);
                if target["delivery_supported"] != true {
                    if let (Some(root),Some(pid))=(target["hwnd"].as_u64(),target["pid"].as_u64()) {
                        let candidate = tauri::async_runtime::spawn_blocking(move || {
                            let root=root as windows_sys::Win32::Foundation::HWND;
                            if !windows_input::opaque_candidate(root,pid as u32) {return Err(windows_input::UNAVAILABLE.to_owned());}
                            windows_paste::capture(root,pid as u32)
                        }).await;
                        if let Ok(Ok(input))=candidate {target.as_object_mut().unwrap().extend(input.as_object().unwrap().clone());}
                    }
                }
            }
            target
        };
        let mut target = if target["bundle"].as_str() == Some(env!("VAO_BUNDLE_ID")) {
            json!({})
        } else {
            target
        };
        let display = voice_bar::capture(&app, target["pid"].as_i64().unwrap_or(0) as i32).await;
        target["display_id"] = json!(display);
        current(&app, id)?;
        let unreadable = target["selection_error"].as_bool().unwrap_or(false)
            || (target["length"].as_u64().unwrap_or(0) > 0
                && target["selection"].as_str().unwrap_or("").is_empty());
        app.state::<AppState>().runtime.lock().unwrap().target = target;
        if unreadable {
            return Err(InputNotice::SelectionUnreadable.into());
        }
        if !audio::available() {
            return Err(InputNotice::NoMicrophone.into());
        }
        trial::routes(&settings)?;
        validate_settings(&settings)?;
        #[cfg(target_os = "macos")]
        if permissions::snapshot()["microphone"] == "not_determined" {
            permissions::request(&app, "microphone").await?;
        }
        current(&app, id)?;
        #[cfg(target_os = "macos")]
        if permissions::snapshot()["microphone"] != "granted" {
            return Err("请在设置中授予 Vao 麦克风权限 / Grant microphone access to Vao".into());
        }
        escape_key(&app, true);
        let pause = settings.auto_stop.then_some(settings.silence_seconds);
        let capture = tauri::async_runtime::spawn_blocking(move || audio::Capture::start_with_auto_stop(pause))
            .await
            .map_err(|_| FlowError::Notice(InputNotice::MicrophoneUnavailable))?
            .map_err(|_| {
                #[cfg(target_os = "windows")]
                permissions::microphone_checked(false);
                FlowError::Notice(if audio::available() {
                    InputNotice::MicrophoneUnavailable
                } else {
                    InputNotice::NoMicrophone
                })
            })?;
        #[cfg(target_os = "windows")]
        permissions::microphone_checked(true);
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if !r.session.active(id) {
            drop(capture);
            return Ok(());
        }
        r.capture = Some(capture);
        r.started = Some(Instant::now());
        r.session.advance(id, Phase::Listening)?;
        r.detail = "聆听中 / Listening".into();
        drop(r);
        show_bar(&app, true);
        if settings.auto_stop {
            start_auto_stop(&app, id);
        }
        Ok(())
    }
    .await;
    handle_flow(&app, id, result);
    Ok(())
}
#[tauri::command]
async fn toggle_translation(app: tauri::AppHandle) -> Result<bool, String> {
    let phase = app
        .state::<AppState>()
        .runtime
        .lock()
        .unwrap()
        .session
        .phase;
    if matches!(
        phase,
        Phase::Idle | Phase::Ready | Phase::Completed | Phase::Cancelled | Phase::Failed
    ) {
        start(app.clone(), true).await?;
        return Ok(app.state::<AppState>().runtime.lock().unwrap().translating);
    }
    let state = app.state::<AppState>();
    let mut r = state.runtime.lock().unwrap();
    if r.session.phase != Phase::Listening {
        return Err(
            "正在处理，请等待或取消本次录音 / Busy; wait or cancel the current recording".into(),
        );
    }
    if r.markers.len() >= 12 {
        return Err("本次翻译切换已达12次，请先结束录音".into());
    }
    let pos = r.capture.as_ref().ok_or("麦克风未就绪")?.position();
    if r.markers.last() == Some(&pos) {
        return Err("切换太快，请稍后重试".into());
    }
    r.markers.push(pos);
    r.translating = !r.translating;
    let translating = r.translating;
    drop(r);
    let _ = app.emit("session-updated", ());
    Ok(translating)
}
#[tauri::command]
fn cancel(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let capture = {
        let mut r = state.runtime.lock().unwrap();
        r.session.cancel();
        r.notice = None;
        r.answer.clear();
        r.translating = false;
        r.started = None;
        r.detail = "已取消 / Cancelled".into();
        r.capture.take()
    };
    #[cfg(target_os = "windows")]
    { windows_paste::clear(); windows_uia::invalidate(); }
    drop(capture);
    show_bar(&app, false);
    escape_key(&app, false);
}
/// Expected input problems stay beside the voice bar and never activate the main window.
fn notify_input(app: &tauri::AppHandle, id: u64, notice: InputNotice) {
    let serial = {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if !r.session.active(id) {
            return;
        }
        r.session.cancel();
        r.capture.take();
        r.started = None;
        r.translating = false;
        r.notice_serial += 1;
        r.notice = Some(notice);
        if matches!(notice,InputNotice::SelectionOutOfScope|InputNotice::SelectionScopeUnavailable) {r.result=None;r.answer.clear();}
        r.error.clear();
        r.detail.clear();
        r.notice_serial
    };
    escape_key(app, false);
    show_bar(app, true);
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let state = h.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if r.session.id == id && r.notice_serial == serial && r.notice.is_some() {
            r.notice = None;
            drop(r);
            show_bar(&h, false);
        }
    });
}
/// Delivery failures preserve the result and detailed cause without opening Vao.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn delivery_notice(app: &tauri::AppHandle, id: u64, error: &str) {
    let serial = {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if r.session.id != id { return; }
        if r.session.active(id) { let _ = r.session.advance(id, Phase::Failed); }
        r.error = error.into();
        r.notice_serial += 1;
        r.notice = Some(InputNotice::DeliveryTargetUnavailable);
        r.notice_serial
    };
    escape_key(app, false);
    show_bar(app, true);
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        let state = h.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if r.session.id == id && r.notice_serial == serial && r.notice.is_some() {
            r.notice = None;
            drop(r);
            show_bar(&h, false);
        }
    });
}
/// A partial recording remains usable, but must never look silently complete.
fn recording_warning(app: &tauri::AppHandle, id: u64) {
    let serial = app.state::<AppState>().runtime.lock().unwrap().recording_warning(id);
    let Some(serial) = serial else { return; };
    show_bar(app, true);
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        let state = h.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if r.session.id == id && r.notice_serial == serial && r.notice == Some(InputNotice::RecordingInterrupted) {
            r.notice = None;
            drop(r);
            show_bar(&h, false);
        }
    });
}
fn task_app_hints(app: &tauri::AppHandle) -> Vec<String> {
    let Ok(dir) = app.path().app_data_dir() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(dir.join("task-apps.txt")) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(40)
        .map(str::to_string)
        .collect()
}
fn handle_flow(app: &tauri::AppHandle, id: u64, result: Result<(), FlowError>) {
    match result {
        Ok(()) => {}
        Err(FlowError::Notice(notice)) => {
            // A later empty translation segment must not hide an earlier valid transcript.
            if !matches!(notice,InputNotice::SelectionOutOfScope|InputNotice::SelectionScopeUnavailable) && app
                .state::<AppState>()
                .runtime
                .lock()
                .unwrap()
                .result
                .is_some()
            {
                fail(app,id,"部分语音未识别，已保留前面识别出的文字 / Part of the audio was empty; earlier text was preserved");
            } else {
                notify_input(app, id, notice);
            }
        }
        Err(FlowError::Error(e)) if e == "cancelled" => {}
        Err(FlowError::Error(e)) => fail(app, id, &e),
    }
}
fn fail(app: &tauri::AppHandle, id: u64, error: &str) {
    let state = app.state::<AppState>();
    let mut r = state.runtime.lock().unwrap();
    if !r.session.active(id) {
        return;
    }
    let _ = r.session.advance(id, Phase::Failed);
    r.notice = None;
    r.error = error.to_owned();
    r.detail = "处理失败，转写结果如有已保留 / Failed".into();
    r.started = None;
    drop(r);
    escape_key(app, false);
    #[cfg(target_os = "windows")]
    {
        let retained = app.state::<AppState>().runtime.lock().unwrap().result.as_ref().map(|r|r.text.clone());
        let message = retained.filter(|s|!s.is_empty()).map(|text|format!("{error}\n\n已保留识别文字 / Retained transcription:\n{text}")).unwrap_or_else(||error.to_owned());
        present_answer(app, id, &message);
    }
    #[cfg(not(target_os = "windows"))]
    { show_bar(app, false); show_main(app); }
}

fn start_auto_stop(app: &tauri::AppHandle, id: u64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let due = {
                let state = app.state::<AppState>();
                let r = state.runtime.lock().unwrap();
                if r.session.id != id || r.session.phase != Phase::Listening {
                    return;
                }
                r.capture.as_ref().is_some_and(audio::Capture::auto_stop_due)
            };
            if due {
                // Recheck inside finish's session lock: voice may have resumed,
                // or manual stop/cancel may have won since the observation.
                let _ = finish(app.clone(), id, true).await;
            }
        }
    });
}

async fn finish(app: tauri::AppHandle, expected_id: u64, automatic: bool) -> Result<(), String> {
    let (id, capture, settings, profile, target, markers) = {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if automatic && !r.capture.as_ref().is_some_and(audio::Capture::auto_stop_due) {
            return Ok(());
        }
        if !r.claim_finish(expected_id) {
            return Ok(());
        }
        let id = expected_id;
        r.auto_stopped = automatic;
        r.detail = "识别中 / Transcribing".into();
        r.started = None;
        r.translating = false;
        (
            id,
            r.capture.take().ok_or("录音缺失")?,
            r.settings.clone(),
            r.profile.clone(),
            r.target.clone(),
            r.markers.clone(),
        )
    };
    let stopped = tauri::async_runtime::spawn_blocking(move || capture.stop())
        .await
        .map_err(|_| FlowError::Notice(InputNotice::MicrophoneUnavailable))
        .and_then(|r| r.map_err(|_| FlowError::Notice(InputNotice::MicrophoneUnavailable)));
    let duration=stopped.as_ref().map(|r|r.len()/audio::RATE as usize).unwrap_or(0);
    let budget=(420+duration as u64*2).min(3600);
    let outcome=tokio::time::timeout(std::time::Duration::from_secs(budget),async {
 let recording=stopped?;recording.check()?;let rate=audio::RATE;
 let (asr_route,llm_route)=trial::routes(&settings)?;
 let asr_key=trial::key(&asr_route,"asr")?;let llm_key=trial::key(&llm_route,"llm")?;
 let mut effective=settings.clone();effective.asr=asr_route.config;effective.llm=llm_route.config;
 let asr_words=settings.dictionary.clone();
 let segments=vao_core::split_segments(recording.len(),&markers)?;let mut originals=vec![];let mut outputs=vec![];let mut refinement=vec![];let mut risk_flags=if recording.warning.is_some(){vec!["audio_interrupted".into()]}else{vec![]};let mut used_scene=Scene::Chat;let mut used_layout=Layout::Paragraph;
 let selection=target["selection"].as_str().filter(|s|!s.is_empty());
 if selection.is_some()&&!markers.is_empty(){return Err("本版选区指令与局部翻译不能同时使用，请分两次操作".into());}
 for segment in segments {
  current(&app,id)?;
  if segment.end-segment.start<rate as usize/5{return Err("翻译边界片段过短，请留出短暂停顿后切换 / Translation segment too short".into());}
  let mut transcript=String::new();
  let ranges=recording.ranges(segment.start,segment.end)?;
  for (start,end) in ranges {
   current(&app,id)?;let wav=recording.wav(start,end)?;
   let part=tokio::select!{v=models::transcribe(&effective.asr,&asr_key,wav,&settings.language,&asr_words)=>match v{Ok(text)=>text,Err(e) if e.no_speech=>continue,Err(e)=>return Err(e.into())},_=cancelled(&app,id)=>return Err("cancelled".into())};
   transcript=transcription::merge_text(&transcript,&part);
   {let state=app.state::<AppState>();let mut r=state.runtime.lock().unwrap();if r.session.active(id){let retained=originals.iter().cloned().chain(std::iter::once(transcript.clone())).collect::<Vec<_>>().join("\n");r.result=Some(ResultRecord{id:0,created:now(),scene:Scene::Chat,layout:Layout::Paragraph,revision:profile.learned_revision,original:retained.clone(),text:retained,risks:vec!["unrefined".into()],app:target["name"].as_str().unwrap_or("").into(),delivery:"preview".into(),feedback:false,refinement:vec![]});}}
  }
  if transcript.trim().is_empty(){return Err(InputNotice::NoSpeech.into());}
  current(&app,id)?; originals.push(transcript.clone());
  let hints=task_app_hints(&app);let hint_refs:Vec<&str>=hints.iter().map(String::as_str).collect();let app_label=format!("{} {}",target["name"].as_str().unwrap_or(""),target["bundle"].as_str().unwrap_or(""));let semantic_transcript=vao_core::normalize_spoken_phone_numbers(&transcript);let writing=vao_core::WritingPlan::resolve(&semantic_transcript,&app_label,&hint_refs,&profile);used_scene=writing.scene;used_layout=writing.layout;
  // Retain raw transcription before refinement, including on provider failure.
  {let state=app.state::<AppState>();let mut r=state.runtime.lock().unwrap();if r.session.active(id){r.result=Some(ResultRecord{id:0,created:now(),scene:used_scene,layout:used_layout,revision:profile.learned_revision,original:originals.join("\n"),text:originals.join("\n"),risks:vec!["unrefined".into()],app:target["name"].as_str().unwrap_or("").into(),delivery:"preview".into(),feedback:false,refinement:vec![]});}}
  detail(&app,id,"思考中 / Refining");
  let request=models::WritingRequest{text:&semantic_transcript,plan:&writing,dictionary:&settings.dictionary,translation:if segment.translate{Some(&settings.translation_language)}else{None},selection,options:vao_core::WritingOptions{semantic_completion:settings.complete_book_titles}};
  let output=tokio::select!{v=refine_with_backup(&effective,&llm_key,&request)=>v?,_=cancelled(&app,id)=>return Err("cancelled".into())};
  if output.used_original{risk_flags.push("unrefined".into());} refinement.push(output.diagnostic()); let text=output.text;
  if !segment.translate&&selection.is_none(){risk_flags.extend(vao_core::risks(&semantic_transcript,&text));}
  outputs.push(text);
 }
 current(&app,id)?;
 let record=ResultRecord{id:0,created:now(),scene:used_scene,layout:used_layout,revision:profile.learned_revision,original:originals.join("\n"),text:outputs.join("\n"),risks:risk_flags,app:target["name"].as_str().unwrap_or("").into(),delivery:"preview".into(),feedback:false,refinement};
 let record={let state=app.state::<AppState>();let mut runtime=state.runtime.lock().unwrap();if !runtime.session.active(id){return Err("cancelled".into());}let record=state.store.lock().unwrap().add(record)?;runtime.session.advance(id,Phase::Ready)?;runtime.result=Some(record.clone());runtime.detail="已整理 / Ready".into();record};
 show_bar(&app,false);escape_key(&app,false);
 route_result(&app,id,&target,&record,settings.auto_input);
 if recording.warning.is_some(){recording_warning(&app,id);}
 let _=app.emit("result-ready",record.id);Ok::<(),FlowError>(())
 }).await.unwrap_or_else(|_|Err("本次处理等待超时，已停止请求 / Session timed out".into()));
    handle_flow(&app, id, outcome);
    Ok(())
}

async fn cancelled(app: &tauri::AppHandle, id: u64) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        if current(app, id).is_err() {
            return;
        }
    }
}
async fn refine_with_backup(
    s: &Settings,
    key: &str,
    request: &models::WritingRequest<'_>,
) -> Result<models::WritingOutput, models::ModelError> {
    match models::refine_writing(&s.llm, key, request).await {
        Ok(v) => Ok(v),
        Err(e) if e.retryable && !s.backup.base.is_empty() => {
            let backup_key = platform::connection_secret("backup", &s.backup.base)?;
            // The failed primary already consumed one generation attempt.
            // A backup result must not start a third attempt via repair.
            let mut output = models::refine_writing_limited(&s.backup, &backup_key, request, 1).await?;
            output.generation_attempts += 1;
            output.actual_route = format!("backup/{}", output.actual_route);
            Ok(output)
        }
        Err(e) => Err(e),
    }
}

fn route_result(
    app: &tauri::AppHandle,
    id: u64,
    target: &Value,
    record: &ResultRecord,
    auto_input: bool,
) {
    #[cfg(target_os = "windows")]
    {
        let asks = !target["selection"].as_str().unwrap_or("").is_empty()
            && vao_core::selection_asks(&record.original);
        if auto_input && !asks {
            match deliver_inner(app, id) {
                // Dispatch status stays in History; ordinary completion is quiet.
                Ok(()) => { show_bar(app, false); return; }
                Err(error) => { app.state::<AppState>().runtime.lock().unwrap().error = error; }
            }
        } else if asks {
            let _ = platform::call(json!({"action":"return_source","target":target}));
        }
        present_answer(app, id, &record.text);
    }
    #[cfg(not(target_os = "windows"))]
    {
    let has_selection = !target["selection"].as_str().unwrap_or("").is_empty();
    let has_pid = target["pid"].is_number();
    let text = &record.text;
    let spoken = &record.original;
    if has_selection {
        let asks = vao_core::selection_asks(spoken);
        let _ = platform::call(json!({"action":"copy","text":text}));
        if !asks && auto_input && has_pid {
            if let Err(e) = deliver_inner(app, id) {
                delivery_notice(app, id, &e);
                return;
            }
        } else if asks && has_pid {
            // Questions show an answer without replacing the selected source.
            if let Err(e) = platform::call(json!({"action":"return_source","target":target})) {
                app.state::<AppState>().runtime.lock().unwrap().error = e;
            }
        }
        present_answer(app, id, text);
        return;
    }
    if auto_input && has_pid {
        if let Err(e) = deliver_inner(app, id) {
            delivery_notice(app, id, &e);
        }
    } else {
        show_main(app);
    }
    }
}

fn present_answer(app: &tauri::AppHandle, id: u64, text: &str) {
    {
        let state = app.state::<AppState>();
        let mut runtime = state.runtime.lock().unwrap();
        if runtime.present_answer(id, text).is_err() { return; }
    }
    show_bar(app, true);
}
#[tauri::command]
async fn fit_voicebar(window: tauri::WebviewWindow, height: f64) -> Result<(), String> {
    if window.label() != "voicebar" { return Err("Voice bar only".into()); }
    #[cfg(target_os = "windows")]
    {
        if !height.is_finite() || !(74.0..=360.0).contains(&height) { return Err("Invalid overlay height".into()); }
        let target = window.clone();
        window.run_on_main_thread(move || { let _ = platform::windows::fit(&target, height); }).map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    let _ = height;
    Ok(())
}
#[tauri::command]
async fn drag_voicebar(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "voicebar" { return Err("Voice bar only".into()); }
    voice_bar::drag(window).await
}
#[tauri::command]
fn dismiss_answer(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "voicebar" { return Err("Voice bar only".into()); }
    let state = app.state::<AppState>();
    let mut runtime = state.runtime.lock().unwrap();
    if !runtime.answer.is_empty() {
        runtime.answer.clear();
        drop(runtime);
        show_bar(&app, false);
    }
    Ok(())
}

fn deliver_inner(app: &tauri::AppHandle, id: u64) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let (use_current, generation) = {
        let state = app.state::<AppState>();
        let r = state.runtime.lock().unwrap();
        (r.windows_current_input(), windows_paste::generation())
    };
    let (target, text, record_id, key_events, mouse_events) = {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        #[cfg(target_os = "windows")]
        let needs_origin = !use_current;
        #[cfg(not(target_os = "windows"))]
        let needs_origin = true;
        if needs_origin && !r.target["pid"].is_number() {
            return Err("没有外部输入目标，请复制结果 / Copy the result".into());
        }
        let result = r.result.as_ref().ok_or("无可输入结果")?;
        let text = result.text.clone();
        let record_id = result.id;
        r.session.claim_delivery(id)?;
        (
            r.target.clone(),
            text,
            record_id,
            hotkeys::delivery_key_events(&r.settings.shortcuts, r.markers.len())?,
            voice_bar::mouse_events().wrapping_sub(r.bar_mouse_start),
        )
    };
    #[cfg(target_os = "windows")]
    let response = if use_current {
        platform::call(json!({"action":"deliver_current","text":text,"generation":generation}))
    } else { platform::call(
        json!({"action":"deliver","target":target,"text":text,"return_to_target":true,"allowed_key_events":key_events,"allowed_mouse_events":mouse_events,"paste":target["paste_transport"]==true}),
    ) };
    #[cfg(not(target_os = "windows"))]
    let response = platform::call(json!({"action":"deliver","target":target,"text":text,"return_to_target":true,"allowed_key_events":key_events,"allowed_mouse_events":mouse_events}));
    let state = app.state::<AppState>();
    let mut r = state.runtime.lock().unwrap();
    match response {
        Ok(response) => {
            r.session.advance(id, Phase::Completed)?;
            if response["verified"]==false {
                r.error.clear();
                r.detail="已派发粘贴，接收未核验；结果已保存在历史 / Paste dispatched; receipt unverified. Result saved in History.".into();
                if let Some(record)=r.result.as_mut(){record.delivery="paste_unconfirmed".into();let _=state.store.lock().unwrap().update(record);}
                return Ok(());
            }
            r.detail = "已发送输入 / Paste sent".into();
            if let Some(record) = r.result.as_mut() {
                record.delivery = "paste_sent".into();
                let _ = state.store.lock().unwrap().update(record);
            }
            Ok(())
        }
        Err(e) => {
            let _ = r.session.advance(id, Phase::Failed);
            r.error = e.clone();
            let _ = record_id;
            Err(e)
        }
    }
}
#[tauri::command]
fn copy_result(state: State<AppState>, id: i64) -> Result<(), String> {
    let text = if id > 0 {
        state.store.lock().unwrap().result(id)?.text
    } else {
        state
            .runtime
            .lock()
            .unwrap()
            .result
            .as_ref()
            .ok_or("无结果")?
            .text
            .clone()
    };
    platform::call(json!({"action":"copy","text":text}))?;
    let mut r = state.runtime.lock().unwrap();
    if r.session.phase == Phase::Ready && r.result.as_ref().map(|v| v.id) == Some(id) {
        r.session.cancel();
    }
    Ok(())
}
#[tauri::command]
fn feedback(
    state: State<AppState>,
    id: i64,
    layout: Layout,
    remember: bool,
) -> Result<bool, String> {
    state.store.lock().unwrap().feedback(id, layout, remember)
}
#[tauri::command]
fn profile_action(
    state: State<AppState>,
    action: String,
    value: Option<bool>,
) -> Result<(), String> {
    let db = state.store.lock().unwrap();
    let mut p: Profile = db.get("profile");
    match action.as_str() {
        "learning" => {
            p.set_learning(value.unwrap_or(false));
        }
        "personalization" => {
            p.set_personalization(value.unwrap_or(false));
        }
        "undo" => {
            p.undo();
        }
        "forget" => p.forget(),
        _ => return Err("invalid profile action".into()),
    }
    db.set("profile", &p)
}
#[tauri::command]
fn clear_history(state: State<AppState>) -> Result<(), String> {
    state.store.lock().unwrap().clear_history()?;
    let mut r = state.runtime.lock().unwrap();
    r.result = None;
    if r.session.phase == Phase::Ready {
        r.session.cancel();
    }
    Ok(())
}

fn verification_output() -> Result<std::path::PathBuf, String> {
    let args = std::env::args().collect::<Vec<_>>();
    if !args.iter().any(|a| a == "--verify-ui") {
        return Err("Diagnostic only".into());
    }
    let i = args
        .iter()
        .position(|a| a == "--smoke-test")
        .ok_or("Missing diagnostic path")?;
    Ok(
        std::path::Path::new(args.get(i + 1).ok_or("Missing diagnostic path")?)
            .parent()
            .ok_or("Invalid path")?
            .to_path_buf(),
    )
}
#[tauri::command]
async fn probe_selection(app: tauri::AppHandle) -> Result<Value, String> {
    let dir = verification_output()?.join("selection-fixtures");
    platform::call(json!({"action":"probe_clipboard","mode":"begin"}))?;
    let mut pids = vec![];
    let outcome=async {
        let mut checks=vec![];
        for label in ["A","B","C"] {
            let mut cmd=std::process::Command::new("/usr/bin/open");
            cmd.arg("-a").arg(dir.join(format!("SelectionFixture{label}.app")));
            if label=="C"{cmd.args(["--args","--cursor"]);}
            if !cmd.status().map_err(|e|e.to_string())?.success(){return Err("Fixture launch failed".into());}
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
            let target=platform::target(true)?;
            if let Some(pid)=target["pid"].as_i64(){if target["bundle"].as_str().is_some_and(|b|b.starts_with("app.vaovao.fixture")){pids.push(pid);}}
            let expected_bundle=format!("app.vaovao.fixture{}",label.to_lowercase());
            let selected=target["selection"].as_str().unwrap_or("");
            checks.push(json!({"name":format!("live_target_{label}"),"passed":target["bundle"]==expected_bundle}));
            checks.push(json!({"name":format!("selection_{label}"),"passed":if label=="C"{selected.is_empty()}else{selected=="合成选区：明天发送报告。"}}));
            let copy=platform::call(json!({"action":"probe_copy_selection"}))?;
            checks.push(json!({"name":format!("copy_fallback_{label}"),"passed":if label=="C"{copy["text"]==""}else{copy["text"]=="合成选区：明天发送报告。"}}));
            checks.push(json!({"name":format!("clipboard_formats_preserved_{label}"),"passed":platform::call(json!({"action":"probe_clipboard","mode":"check"}))?["ok"]==true}));
        }
        Ok::<Value,String>(json!({"checks":checks}))
    }.await;
    let _ = platform::call(json!({"action":"probe_clipboard","mode":"restore"}));
    for pid in pids {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    show_main(&app);
    outcome
}
#[tauri::command]
async fn probe_composer(app: tauri::AppHandle) -> Result<Value, String> {
    verification_output()?;
    let process = std::process::Command::new("/usr/bin/pgrep")
        .args(["-x", "WeChat"])
        .output()
        .map_err(|e| e.to_string())?;
    let pid = String::from_utf8_lossy(&process.stdout)
        .lines()
        .next()
        .and_then(|s| s.parse::<i64>().ok());
    let mut inspected = json!({"available":false});
    if let Some(pid) = pid {
        let _ = std::process::Command::new("/usr/bin/open")
            .args(["-b", "com.tencent.xinWeChat"])
            .status();
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        inspected = platform::call(json!({"action":"inspect_app","pid":pid}))?;
    }
    let dir = verification_output()?.join("selection-fixtures");
    let mut checks = vec![];
    let mut pids = vec![];
    platform::call(json!({"action":"probe_clipboard","mode":"begin"}))?;
    let outcome=async {
      for label in ["Composer","Opaque","Ambiguous"] {
        let status=std::process::Command::new("/usr/bin/open").arg("-a").arg(dir.join(format!("SelectionFixture{label}.app"))).status().map_err(|e|e.to_string())?;
        if !status.success(){return Err("Composer fixture launch failed".into());}
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let target=platform::target(true)?;
        let bundle=format!("app.vaovao.fixture{}",label.to_lowercase());
        if target["bundle"]!=bundle{return Err("Wrong fixture application".into());}
        pids.push(target["pid"].as_i64().ok_or("Missing fixture PID")?);
        checks.push(json!({"name":format!("composer_discovery_{label}"),"passed":if label=="Composer"{target["target_mode"]=="composer"}else if label=="Opaque"{target["target_mode"]=="window_paste"}else{target["captureToken"].is_null()}}));
        if label=="Composer" || label=="Opaque" {
          let delivery=platform::call(json!({"action":"deliver","target":target,"text":"合成自动回填。","allowed_key_events":0}));
          checks.push(json!({"name":format!("composer_paste_sent_{label}"),"passed":delivery.is_ok()}));
          let received=platform::call(json!({"action":"probe_fixture","mode":"read"}))?;
          checks.push(json!({"name":format!("composer_receives_actual_text_{label}"),"passed":received["text"]=="合成自动回填。"}));
          let target=platform::target(true)?;
          platform::call(json!({"action":"probe_fixture","mode":"key"}))?;
          let rejected=platform::call(json!({"action":"deliver","target":target,"text":"不可输入","allowed_key_events":0}));
          checks.push(json!({"name":format!("composer_rejects_intervening_key_{label}"),"passed":rejected.is_err()}));
          let target=platform::target(true)?;
          platform::call(json!({"action":"probe_fixture","mode":"mouse"}))?;
          let rejected=platform::call(json!({"action":"deliver","target":target,"text":"不可输入","allowed_key_events":0}));
          checks.push(json!({"name":format!("composer_rejects_intervening_mouse_{label}"),"passed":rejected.is_err()}));
          let target=platform::target(true)?;
          platform::call(json!({"action":"probe_fixture","mode":"mouse"}))?;
          let delivery=platform::call(json!({"action":"deliver","target":target,"text":"已登记的声浪操作。","allowed_key_events":0,"allowed_mouse_events":1}));
          checks.push(json!({"name":format!("composer_accepts_exact_overlay_mouse_count_{label}"),"passed":delivery.is_ok()}));
          let received=platform::call(json!({"action":"probe_fixture","mode":"read"}))?;
          checks.push(json!({"name":format!("composer_receives_text_after_allowed_mouse_{label}"),"passed":received["text"].as_str().is_some_and(|s|s.contains("已登记的声浪操作。"))}));
          let target=platform::target(true)?;
          for _ in 0..2 { platform::call(json!({"action":"probe_fixture","mode":"mouse"}))?; }
          let rejected=platform::call(json!({"action":"deliver","target":target,"text":"不可输入","allowed_key_events":0,"allowed_mouse_events":1}));
          checks.push(json!({"name":format!("composer_rejects_additional_unaccounted_mouse_{label}"),"passed":rejected.is_err()}));
          let count=platform::call(json!({"action":"probe_fixture","mode":"fn_chord_count"}))?;
          checks.push(json!({"name":format!("native_fn_chord_has_no_delivered_keydown_{label}"),"passed":count["delta"]==0}));
          let target=platform::target(true)?;
          show_main(&app);tokio::time::sleep(std::time::Duration::from_millis(150)).await;
          let rejected=platform::call(json!({"action":"deliver","target":target,"text":"不可输入","allowed_key_events":0}));
          checks.push(json!({"name":format!("composer_rejects_changed_app_{label}"),"passed":rejected.is_err()}));
        }
        checks.push(json!({"name":format!("composer_clipboard_restored_{label}"),"passed":platform::call(json!({"action":"probe_clipboard","mode":"check"}))?["ok"]==true}));
      }
      Ok::<(),String>(())
    }.await;
    let _ = platform::call(json!({"action":"probe_clipboard","mode":"restore"}));
    for pid in pids {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    show_main(&app);
    outcome?;
    Ok(json!({"checks":checks,"wechat_structure_only":inspected}))
}
#[tauri::command]
async fn probe_input_feedback(app: tauri::AppHandle) -> Result<Value, String> {
    verification_output()?;
    let mut checks = vec![];
    for (name, notice) in [
        ("empty_audio", InputNotice::NoAudio),
        ("no_device", InputNotice::NoMicrophone),
    ] {
        let main = app.get_webview_window("main").ok_or("Missing window")?;
        main.hide().map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let mut target = platform::target(false)?;
        let pid = target["pid"].as_i64().unwrap_or(0);
        target["display_id"] = json!(voice_bar::capture(&app, pid as i32).await);
        let id = {
            let state = app.state::<AppState>();
            let mut r = state.runtime.lock().unwrap();
            let id = r.session.begin()?;
            r.target = target;
            r.result = None;
            id
        };
        let error = if notice == InputNotice::NoAudio {
            input_feedback::check_audio(&vec![0.; 16000], 16000).unwrap_err()
        } else {
            notice
        };
        handle_flow(&app, id, Err(error.into()));
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let bar = app.get_webview_window("voicebar").ok_or("Missing bar")?;
        checks.push(json!({"name":format!("{name}_bubble_visible"),"passed":voice_bar::is_visible(bar.clone()).await.unwrap_or(false)}));
        checks.push(json!({"name":format!("{name}_main_stays_hidden"),"passed":!main.is_visible().unwrap_or(true)}));
        checks.push(json!({"name":format!("{name}_focus_preserved"),"passed":platform::target(false)?["pid"].as_i64()==Some(pid)}));
        tokio::time::sleep(std::time::Duration::from_millis(4000)).await;
        checks.push(json!({"name":format!("{name}_bubble_auto_hides"),"passed":!voice_bar::is_visible(bar.clone()).await.unwrap_or(true)}));
    }
    show_main(&app);
    Ok(json!({"checks":checks}))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let build_info = json!({"version":env!("VAO_RELEASE_VERSION"),"build":env!("VAO_BUILD_NUMBER"),"bundle":env!("VAO_BUNDLE_ID"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH});
    if args.iter().any(|a| a == "--version") { println!("{build_info}"); return; }
    if let Some(index) = args.iter().position(|a| a == "--build-info") {
        let Some(path) = args.get(index + 1) else { std::process::exit(2); };
        if std::fs::write(path, build_info.to_string()).is_err() { std::process::exit(3); }
        return;
    }
    if let Some(index) = args.iter().position(|a| a == "--verify-silence-fixture") {
        let seconds = args.iter().position(|a| a == "--silence-seconds")
            .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(5);
        match args.get(index + 1).ok_or("Expected a PCM16 fixture path".to_string())
            .and_then(|p| auto_stop::verify_fixture(p, seconds)) {
            Ok(report) => { println!("{report}"); return; }
            Err(error) => { eprintln!("{error}"); std::process::exit(2); }
        }
    }
    if let Some(index) = args.iter().position(|a| a == "--verify-writing-fixtures") {
        if let Err(e)=writing_probe::validate_cli(&args[1..]) {eprintln!("{e}");std::process::exit(2);}
        let argument = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
        let Some(path) = args.get(index + 1) else { eprintln!("Expected a JSON fixture path"); std::process::exit(2); };
        let Some(database) = argument("--writing-database") else { eprintln!("Expected --writing-database path (opened read-only)"); std::process::exit(2); };
        let Some(route) = argument("--writing-route") else { eprintln!("Expected --writing-route custom/trial; this sends fixture text to that model"); std::process::exit(2); };
        let profile = argument("--writing-profile").map(String::as_str).unwrap_or("fresh");
        let books=match argument("--writing-completion").or_else(||argument("--writing-books")).map(String::as_str) {
            None=>None,Some("on")=>Some(true),Some("off")=>Some(false),
            _=>{eprintln!("Expected --writing-completion on/off");std::process::exit(2);}
        };
        let options=writing_probe::ReplayOptions{books,
            connection:argument("--writing-connection").map(std::path::Path::new),
            credential_slot:argument("--writing-credential-slot").map(String::as_str),
            generation_budget:match argument("--writing-budget") {
                None=>None,Some(s)=>match s.parse::<usize>() {
                    Ok(n) if (1..=80).contains(&n)=>Some(n),
                    _=>{eprintln!("Expected --writing-budget 1-80");std::process::exit(2);}
                }
            }};
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Writing replay runtime");
        match runtime.block_on(writing_probe::replay(std::path::Path::new(path), std::path::Path::new(database), route, profile, options)) {
            Ok(report) => { println!("{report}"); return; }
            Err(e) => { eprintln!("{e}"); std::process::exit(3); }
        }
    }
    if args.iter().any(|a| a == "--activate-trial") {
        if !args.iter().any(|a| a == "--accept-trial-privacy") {
            eprintln!("Read PRIVACY.md; activation requires --accept-trial-privacy");
            std::process::exit(2);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Activation runtime");
        match runtime.block_on(trial::enroll(true)) {
            Ok(status) => println!("{}", status),
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(3);
            }
        }
        return;
    }
    if let Some(index) = args.iter().position(|a| a == "--verify-trial-fixture") {
        let Some(path) = args.get(index + 1) else {
            eprintln!("Expected a WAV fixture path");
            std::process::exit(2);
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Diagnostic runtime");
        match runtime.block_on(trial::verify_fixture(path)) {
            Ok(report) => {
                println!("{}", report);
                return;
            }
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(3);
            }
        }
    }
    if std::env::args().any(|a| a == "--provision-trial") {
        use std::io::Read;
        let mut token = String::new();
        std::io::stdin()
            .take(1024)
            .read_to_string(&mut token)
            .expect("Read credential from stdin");
        let token = token.trim();
        if !token.starts_with("vao_device_") || token.len() > 160 {
            eprintln!("Invalid device credential");
            std::process::exit(2);
        }
        match platform::save_secret(trial::SLOT, token) {
            Ok(()) => {
                println!("Device trial credential installed");
                return;
            }
            Err(_) => {
                eprintln!("Could not install credential in Keychain");
                std::process::exit(3);
            }
        }
    }

    let builder = tauri::Builder::default()
        .manage(windows_updater::Updates::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !login_background_launch(&args) { show_main(app); }
        }))
        .setup(|app| {
            #[cfg(target_os = "windows")]
            if let Some(main) = app.get_webview_window("main") { main.set_title("Vao")?; }
            let args: Vec<String> = std::env::args().collect();
            // UI regression tests must never mutate the user's preferences/history.
            let path = if args.iter().any(|a| a == "--verify-ui" || (cfg!(target_os = "windows") && (a == "--windows-smoke" || a == "--windows-input-probe"))) {
                let index = args.iter().position(|a| a == "--smoke-test").ok_or("--verify-ui requires --smoke-test output")?;
                let output = args.get(index + 1).ok_or("Missing smoke output path")?;
                std::path::Path::new(output).parent().ok_or("Invalid smoke output path")?.join("isolated-native-state")
            } else { app.path().app_data_dir()? };
            std::fs::create_dir_all(&path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            }
            let db = Store::open(&path.join("vao.sqlite3")).map_err(std::io::Error::other)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    path.join("vao.sqlite3"),
                    std::fs::Permissions::from_mode(0o600),
                )?;
            }
            app.manage(AppState {
                runtime: Mutex::new(Runtime::default()),
                store: Mutex::new(db),
            });
            use tauri::menu::{Menu, MenuItem};
            let open =
                MenuItem::with_id(app, "open", "打开 Vao / Open", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, "cancel", "取消录音 / Cancel", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 / Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &stop, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::with_id("vao-tray")
                .tooltip("Vao")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "cancel" => cancel(app.clone()),
                    "quit" => {
                        cancel(app.clone());
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if let Some(bar) = app.get_webview_window("voicebar") {
                voice_bar::initialize(&bar).map_err(std::io::Error::other)?;
            }
            app.manage(native_keys::NativeKeys::default());
            local_capture::install(app.handle());
            let handle = app.handle().clone();
            let registration = (|| {
                let manager = hotkeys::HotkeyManager::new().map_err(|e| e.to_string())?;
                let config=app.state::<AppState>().store.lock().unwrap().get::<Settings>("settings").shortcuts;
                let parsed=config.bindings()?;
                let mut registered=vec![];
                let startup=hotkeys::replace(&manager,&mut registered,&parsed[..2].iter().filter_map(hotkeys::Binding::global).collect::<Vec<_>>(),||Ok(()));
                let native=app.state::<native_keys::NativeKeys>();
                native.configure(std::array::from_fn(|i|parsed[i].native()));
                let native_startup=if parsed.iter().any(|k|k.native().is_some()){native.ensure(&handle)}else{Ok(())};
                app.manage(Hotkeys{manager,bindings:Mutex::new(ShortcutBindings{keys:parsed,registered})});
                let app_handle = handle.clone();
                global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(move|event:global_hotkey::GlobalHotKeyEvent|{
                    if event.state!=global_hotkey::HotKeyState::Pressed{return;}
                    let h=app_handle.clone();let key=event.id;
                    let action={let keys=h.state::<Hotkeys>();let b=keys.bindings.lock().unwrap();if !b.registered.iter().any(|k|k.id()==key){None}else{b.keys.iter().position(|k|k.global().is_some_and(|k|k.id()==key))}};
                    if let Some(action)=action{
                       let chord={let k=h.state::<Hotkeys>();let b=k.bindings.lock().unwrap();b.keys[action].to_string()};
                       if !h.state::<native_keys::NativeKeys>().capture_global(&h,chord){dispatch_shortcut(h,action);}
                    }
                }));
                startup.and(native_startup)
            })();
            {
                let state = app.state::<AppState>();
                let mut r = state.runtime.lock().unwrap();
                r.hotkeys = registration.is_ok();
                if let Err(e) = registration {
                    r.error = format!("快捷键尚未生效 / Shortcuts inactive: {e}");
                }
            }
            #[cfg(target_os = "windows")]
            if args.iter().any(|arg| arg == "--windows-input-probe") {
                if let Some(main) = app.get_webview_window("main") {
                    let mut url = main.url()?;
                    url.set_path("/windows-input-fixture.html");
                    main.navigate(url)?;
                    main.set_focus()?;
                }
            }
            Ok(())
        })
        .on_page_load(|window,payload|{
            report_frontend_state(format!("page:{}:{:?}",window.label(),payload.event()));
            if matches!(payload.event(),tauri::webview::PageLoadEvent::Finished){let _=window.eval("window.__TAURI_INTERNALS__.invoke('report_frontend_state',{stage:JSON.stringify({html:document.documentElement.outerHTML.length,api:typeof window.__TAURI__,heading:document.querySelector('h1')?.textContent,app:document.getElementById('app')?.childElementCount})})");}
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::Focused(false) = event {local_capture::arm(false);window.state::<native_keys::NativeKeys>().capture(false);window.state::<native_keys::NativeKeys>().suspend(false);}
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            update_status,check_update,download_update,install_update,
            login_startup_status,set_login_startup,open_startup_settings,
            report_ui_ready,report_frontend_state,
            windows_input_probe::probe_windows_input,
            snapshot,
            fit_voicebar,drag_voicebar,dismiss_answer,locked_target_probe::probe_locked_target,overlay_probe::probe_overlay,overlay_probe::probe_overlay_event,
            bootstrap,
            save_settings,import_dictionary,export_dictionary,save_shortcuts,shortcut_status,shortcut_capture,
            save_key,
            key_status,
            fetch_models,
            test_llm,
            test_asr,
            permissions,request_permission,trial_status,enroll_trial,test_trial,probe_capture,probe_voicebar,
            open_permission,repair_permission,restart_for_permissions,reveal_application,probe_selection,probe_input_feedback,probe_composer,
            toggle_recording,
            toggle_translation,
            cancel,
            copy_result,
            feedback,
            profile_action,
            clear_history
        ]);
    let context = tauri::generate_context!();
    #[cfg(target_os = "windows")]
    let context = {
        let mut context = context;
        if login_background_launch(&args) {
            if let Some(main) = context.config_mut().app.windows.iter_mut().find(|w| w.label == "main") {
                main.visible = false;
                main.focus = false;
            }
        }
        context
    };
    builder
        .build(context)
        .expect("Vao startup failed")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app);
            }
            let _ = (app, event);
        });
}
