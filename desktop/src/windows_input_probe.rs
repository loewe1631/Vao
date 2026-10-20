//! Synthetic WebView2 input receipt checks, enabled only by an explicit CLI probe.
use serde_json::Value;
#[tauri::command]
pub async fn probe_windows_input(
    window: tauri::WebviewWindow,
    action: String,
    request: Option<Value>,
) -> Result<Value, String> {
    if window.label() != "main" || !std::env::args().any(|arg| arg == "--windows-input-probe") {
        return Err("Input fixture mode is not enabled".into());
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        let root = window.hwnd().map_err(|e| e.to_string())?.0 as usize;
        match action.as_str() {
            "capture" => tauri::async_runtime::spawn_blocking(move || {
                crate::windows_input::capture(root as _, unsafe { GetCurrentProcessId() })
            })
            .await
            .map_err(|_| "Input fixture interrupted")?,
            "deliver" | "restore" => {
                let request = request.ok_or("Missing synthetic request")?;
                let write = action == "deliver";
                tauri::async_runtime::spawn_blocking(move || {
                    crate::windows_input::deliver(&request, write)
                })
                .await
                .map_err(|_| "Input fixture interrupted")?
            }
            "report" => {
                let args = std::env::args().collect::<Vec<_>>();
                let index = args
                    .iter()
                    .position(|arg| arg == "--smoke-test")
                    .ok_or("Missing fixture output")?;
                let path = args.get(index + 1).ok_or("Missing fixture output")?;
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(&request.ok_or("Missing synthetic report")?)
                        .map_err(|_| "Invalid report")?,
                )
                .map_err(|e| e.to_string())?;
                Ok(serde_json::json!({"ok":true}))
            }
            _ => Err("Unknown input fixture operation".into()),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, action, request);
        Err("Windows-only input fixture".into())
    }
}
