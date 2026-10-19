use serde_json::{json, Value};
#[cfg(target_os = "windows")]
static MICROPHONE_CHECKED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "windows")]
pub fn microphone_checked(available: bool) {
    MICROPHONE_CHECKED.store(available, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn vao_permission_identity() -> *mut std::ffi::c_char;
    fn vao_free_permission_identity(value: *mut std::ffi::c_char);
    fn vao_microphone_status() -> i32;
    fn vao_request_microphone(done: extern "C" fn(i32));
    fn vao_accessibility_status() -> i32;
    fn vao_request_accessibility();
    fn vao_input_status() -> i32;
    fn vao_request_input() -> i32;
}
#[cfg(target_os = "macos")]
static MIC_REPLY: std::sync::OnceLock<
    std::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>,
> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
extern "C" fn microphone_reply(value: i32) {
    if let Some(tx) = MIC_REPLY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .take()
    {
        let _ = tx.send(value == 1);
    }
}
pub fn snapshot() -> Value {
    #[cfg(target_os = "macos")]
    {
        let identity = unsafe {
            let ptr = vao_permission_identity();
            if ptr.is_null() {
                json!({})
            } else {
                let parsed =
                    serde_json::from_slice::<Value>(std::ffi::CStr::from_ptr(ptr).to_bytes())
                        .unwrap_or_else(|_| json!({}));
                vao_free_permission_identity(ptr);
                parsed
            }
        };
        let mic = unsafe { vao_microphone_status() };
        let executable = std::env::current_exe().ok();
        let application_path = executable.as_deref().and_then(|p| {
            p.ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
        });
        json!({"microphone":match mic{0=>"not_determined",1=>"restricted",2=>"denied",3=>"granted",_=>"unknown"},"accessibility":if unsafe{vao_accessibility_status()}==1{"granted"}else{"not_granted"},"input_monitoring":if unsafe{vao_input_status()}==1{"granted"}else{"not_granted"},"application":"Vao","bundle":identity["bundle"],"expected_bundle":env!("VAO_BUNDLE_ID"),"build":env!("VAO_BUILD_NUMBER"),"identity":identity,"application_path":application_path,"executable":executable})
    }
    #[cfg(target_os = "windows")]
    {
        let ready = MICROPHONE_CHECKED.load(std::sync::atomic::Ordering::Relaxed)
            && crate::audio::available();
        json!({"microphone":if ready {"granted"} else {"not_determined"},
               "accessibility":"not_supported","input_monitoring":"not_required",
               "application":"Vao Windows Preview","bundle":env!("VAO_BUNDLE_ID"),
               "build":env!("VAO_BUILD_NUMBER"),"executable":std::env::current_exe().ok(),
               "delivery_supported":false,"microphone_status_source":"stream_probe"})
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        json!({"microphone":"unknown","accessibility":"unknown","input_monitoring":"unknown"})
    }
}
pub async fn request(app: &tauri::AppHandle, kind: &str) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        match kind {
            "microphone" => {
                let status = unsafe { vao_microphone_status() };
                if status == 0 {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    {
                        let mut pending = MIC_REPLY
                            .get_or_init(|| std::sync::Mutex::new(None))
                            .lock()
                            .unwrap();
                        if pending.is_some() {
                            return Err("权限请求已打开 / Permission request already open".into());
                        }
                        *pending = Some(tx);
                    }
                    unsafe { vao_request_microphone(microphone_reply) };
                    // Completion is driven by the macOS permission prompt, not a fabricated local toggle.
                    rx.await
                        .map_err(|_| "麦克风权限请求中断 / Permission request interrupted")?;
                }
            }
            "accessibility" | "input_monitoring" => {
                let accessibility = kind == "accessibility";
                let (tx, rx) = tokio::sync::oneshot::channel();
                // Interactive system requests belong to the application's UI thread.
                // A request returning does not mean the user has granted access.
                app.run_on_main_thread(move || {
                    if accessibility {
                        unsafe { vao_request_accessibility() };
                    } else {
                        unsafe { vao_request_input() };
                    }
                    let _ = tx.send(());
                })
                .map_err(|e| e.to_string())?;
                rx.await
                    .map_err(|_| "权限请求中断 / Permission request interrupted")?;
            }
            _ => return Err("Invalid permission".into()),
        }
        Ok(snapshot())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        if kind != "microphone" {
            return Err("Windows does not use macOS permissions".into());
        }
        // Explicit open/start/close checks actual WASAPI access. Samples stay in
        // memory and are dropped. This does not change OS privacy settings.
        let result =
            tauri::async_runtime::spawn_blocking(|| crate::audio::Capture::start().map(drop))
                .await
                .map_err(|_| "Microphone check interrupted")?;
        microphone_checked(result.is_ok());
        result.map_err(|_| "无法访问麦克风，请检查 Windows 麦克风与桌面应用访问 / Check Windows microphone privacy and desktop app access")?;
        Ok(snapshot())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, kind);
        Err("Permission adapter is not implemented on this platform".into())
    }
}
