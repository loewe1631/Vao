//! Prepare Electron's interface on foreground activation, before the user's click.
//! No target fields or content are read by this activation path.
use tauri::Manager;

fn eligible(auto_input: bool, phase: vao_core::Phase) -> bool {
    auto_input && matches!(phase, vao_core::Phase::Idle | vao_core::Phase::Completed | vao_core::Phase::Cancelled | vao_core::Phase::Failed)
}

#[cfg(target_os = "macos")]
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
unsafe extern "C" { fn vao_install_input_activation(callback: extern "C" fn(i32)); }

#[cfg(target_os = "macos")]
extern "C" fn activated(pid: i32) {
    let Some(app) = APP.get().cloned() else { return; };
    if pid <= 0 { return; }
    tauri::async_runtime::spawn(async move {
        for delay in [0, 150, 350] {
            if delay > 0 { tokio::time::sleep(std::time::Duration::from_millis(delay)).await; }
            let worker = app.clone();
            let reply = tauri::async_runtime::spawn_blocking(move || {
                let state = worker.state::<crate::AppState>();
                let runtime = state.runtime.lock().unwrap();
                let auto_input = state.store.lock().unwrap().get::<crate::store::Settings>("settings").auto_input;
                if !eligible(auto_input, runtime.session.phase) { return Ok(serde_json::json!({"obsolete":true})); }
                // Serialize with session.begin(): never mutate the target app's
                // interface after recording/target acquisition has started.
                crate::platform::call(serde_json::json!({"action":"prepare_input_tree","pid":pid}))
            }).await;
            match reply {
                Ok(Ok(value)) if value["prepared"] == true || value["obsolete"] == true => break,
                Ok(Ok(_)) => (),
                _ => break,
            }
        }
    });
}

pub fn install(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    if APP.set(app.clone()).is_ok() { unsafe { vao_install_input_activation(activated); } }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    use super::eligible;
    use vao_core::Phase;
    #[test]
    fn preparing_another_app_never_interferes_with_live_or_pending_input() {
        for phase in [Phase::Preparing, Phase::Listening, Phase::Processing, Phase::AwaitingCorrection, Phase::Delivering, Phase::Ready] {
            assert!(!eligible(true, phase));
        }
        for phase in [Phase::Idle, Phase::Completed, Phase::Cancelled, Phase::Failed] {
            assert!(eligible(true, phase));
            assert!(!eligible(false, phase));
        }
    }
}
