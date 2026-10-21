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
            "mode" => Ok(
                serde_json::json!({"unicode_preview":crate::windows_uia::unicode_preview(),"paste_preview":std::env::args().any(|a|a=="--windows-paste-preview")}),
            ),
            "invalidate" => {
                crate::windows_paste::clear();
                crate::windows_uia::invalidate();
                Ok(serde_json::json!({"ok":true}))
            }
            "opaque_capture" | "opaque_status" | "opaque_hide" => {
                let operation = action.trim_start_matches("opaque_").to_owned();
                let hidden = operation == "hide";
                let result = tauri::async_runtime::spawn_blocking(move || {
                    crate::windows_paste::fixture_window(&operation)
                })
                .await
                .map_err(|_| "Opaque synthetic host interrupted")?;
                if hidden {
                    window
                        .set_focus()
                        .map_err(|_| "Synthetic focus restore unavailable")?;
                }
                result
            }
            "context" => {
                let target = request.ok_or("Missing synthetic context target")?;
                tauri::async_runtime::spawn_blocking(move || crate::windows_input::context(&target))
                    .await
                    .map_err(|_| "Synthetic context fixture interrupted")?
            }
            "cover" => {
                use tauri::Manager;
                let cover =
                    if let Some(cover) = window.app_handle().get_webview_window("input-cover") {
                        cover
                    } else {
                        tauri::WebviewWindowBuilder::new(
                            window.app_handle(),
                            "input-cover",
                            tauri::WebviewUrl::App("windows-input-cover.html".into()),
                        )
                        .title("Synthetic other window")
                        .inner_size(420.0, 180.0)
                        .build()
                        .map_err(|_| "Owned cover fixture unavailable")?
                    };
                cover
                    .set_focus()
                    .map_err(|_| "Owned cover focus unavailable")?;
                Ok(serde_json::json!({"ok":true}))
            }
            "clipboard_seed" | "clipboard_status" | "clipboard_race" => {
                let operation = action.trim_start_matches("clipboard_").to_owned();
                tauri::async_runtime::spawn_blocking(move || {
                    crate::windows_paste::fixture_clipboard(&operation)
                })
                .await
                .map_err(|_| "Synthetic clipboard fixture interrupted")?
            }
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
