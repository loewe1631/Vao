use serde_json::Value;
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn vao_initialize_voice_bar(window: *mut std::ffi::c_void) -> i32;
    fn vao_set_voice_bar_visible(window: *mut std::ffi::c_void, visible: i32);
    fn vao_voice_bar_visible(window: *mut std::ffi::c_void) -> i32;
    fn vao_voice_bar_mouse_events() -> u32;
    fn vao_inspect_voice_bar(window: *mut std::ffi::c_void) -> *mut std::ffi::c_char;
    fn vao_probe_voice_bar_event(window: *mut std::ffi::c_void, x: f64, y: f64, kind: i32) -> i32;
    fn vao_probe_voice_bar_positions(begin: i32);
    fn vao_capture_input_display(pid: i32) -> u32;
    fn vao_place_voice_bar(window: *mut std::ffi::c_void, display: u32, expanded: i32);
    fn vao_drag_voice_bar(window: *mut std::ffi::c_void) -> i32;
    fn vao_probe_voice_bar(window: *mut std::ffi::c_void) -> *mut std::ffi::c_char;
    fn vao_free_display_probe(value: *mut std::ffi::c_char);
    #[cfg(test)]
    fn vao_test_display_geometry() -> u32;
}
// Window-host operations run on the AppKit thread; only the atomic counter is
// read by the processing worker.
pub fn initialize(window: &tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let ptr = window.ns_window().map_err(|e| e.to_string())?;
        if unsafe { vao_initialize_voice_bar(ptr) } == 0 {
            return Err("Could not initialize the voice overlay".into());
        }
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::initialize_overlay(window)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    window.set_focusable(false).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn set_visible(window: &tauri::WebviewWindow, visible: bool) {
    #[cfg(target_os = "macos")]
    if let Ok(ptr) = window.ns_window() {
        unsafe { vao_set_voice_bar_visible(ptr, i32::from(visible)) };
    }
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE, SW_HIDE};
        unsafe { ShowWindow(hwnd.0 as _, if visible {SW_SHOWNOACTIVATE} else {SW_HIDE}); }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { let _ = if visible { window.show() } else { window.hide() }; }
}
pub async fn is_visible(window: tauri::WebviewWindow) -> Result<bool, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let visible = target.ns_window().is_ok_and(|ptr| unsafe { vao_voice_bar_visible(ptr) != 0 });
        #[cfg(not(target_os = "macos"))]
        let visible = target.is_visible().unwrap_or(false);
        let _ = tx.send(visible);
    }).map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Overlay visibility check interrupted".into())
}
pub fn mouse_events() -> u32 {
    #[cfg(target_os = "macos")]
    { unsafe { vao_voice_bar_mouse_events() } }
    #[cfg(not(target_os = "macos"))]
    { 0 }
}
pub async fn inspect(window: tauri::WebviewWindow) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let result = (|| {
            let ptr = target.ns_window().map_err(|e| e.to_string())?;
            let raw = unsafe { vao_inspect_voice_bar(ptr) };
            if raw.is_null() { return Err("Overlay inspection failed".into()); }
            let parsed = serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(raw) }.to_bytes())
                .map_err(|e| e.to_string());
            unsafe { vao_free_display_probe(raw) };
            parsed
        })();
        #[cfg(not(target_os = "macos"))]
        let result = { let _ = target; Err("Platform not supported".into()) };
        let _ = tx.send(result);
    }).map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Overlay inspection interrupted".to_string())?
}
pub async fn probe_event(window: tauri::WebviewWindow, x: f64, y: f64, kind: i32) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let sent = target.ns_window().is_ok_and(|ptr| unsafe { vao_probe_voice_bar_event(ptr, x, y, kind) != 0 });
        #[cfg(not(target_os = "macos"))]
        let sent = { let _ = (target, x, y, kind); false };
        let _ = tx.send(sent);
    }).map_err(|e| e.to_string())?;
    if rx.await.unwrap_or(false) { Ok(()) } else { Err("Could not post overlay test event".into()) }
}
pub async fn probe_positions(app: &tauri::AppHandle, begin: bool) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    let _ = begin;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        unsafe { vao_probe_voice_bar_positions(i32::from(begin)) };
        let _ = tx.send(());
    }).map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Overlay position restore interrupted".into())
}
pub async fn capture(app: &tauri::AppHandle, pid: i32) -> u32 {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if app
            .run_on_main_thread(move || {
                let _ = tx.send(unsafe { vao_capture_input_display(pid) });
            })
            .is_err()
        {
            return 0;
        }
        rx.await.unwrap_or(0)
    }
    #[cfg(target_os = "windows")]
    { let _ = app; crate::platform::windows::monitor(pid as u32) }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { let _ = (app, pid); 0 }
}
pub fn place(window: &tauri::WebviewWindow, display: u32, expanded: i32) {
    #[cfg(target_os = "macos")]
    if let Ok(ptr) = window.ns_window() {
        unsafe { vao_place_voice_bar(ptr, display, expanded) }
    }
    #[cfg(target_os = "windows")]
    { let _ = display; let _ = crate::platform::windows::place(window, expanded != 0); }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (window, display, expanded);
}
#[cfg(target_os = "windows")]
pub async fn drag(window: tauri::WebviewWindow) -> Result<(), String> {
    crate::platform::windows::drag_overlay(window).await
}
#[cfg(not(target_os = "windows"))]
pub async fn drag(window: tauri::WebviewWindow) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let dragged = target.ns_window().is_ok_and(|ptr| unsafe { vao_drag_voice_bar(ptr) != 0 });
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let dragged = { let _ = target; false };
        let _ = tx.send(dragged);
    }).map_err(|e| e.to_string())?;
    if rx.await.unwrap_or(false) { Ok(()) } else { Err("No active mouse drag".into()) }
}
pub async fn probe(window: tauri::WebviewWindow) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            #[cfg(target_os = "macos")]
            let result = (|| {
                let ptr = target.ns_window().map_err(|e| e.to_string())?;
                let json = unsafe { vao_probe_voice_bar(ptr) };
                if json.is_null() {
                    return Err("Display probe failed".into());
                }
                let result =
                    serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(json) }.to_bytes())
                        .map_err(|e| e.to_string());
                unsafe { vao_free_display_probe(json) };
                result
            })();
            #[cfg(not(target_os = "macos"))]
            let result = { let _ = target; Err("Platform not supported".into()) };
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "Display probe interrupted".to_string())?
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn horizontal_vertical_negative_origins_and_spanning_windows() {
        assert_eq!(unsafe { super::vao_test_display_geometry() }, 0);
    }
}
