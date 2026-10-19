//! First Windows candidate: target metadata and explicit clipboard copy.
//! Automatic insertion remains closed until a verifiable UIA target exists.
use serde_json::{json, Value};
use std::sync::Mutex;
use windows_sys::Win32::{
    Foundation::{CloseHandle, GlobalFree, HWND},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        Threading::{
            GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, IsWindow},
};

static CLIPBOARD: Mutex<()> = Mutex::new(());

pub fn open_settings(url: &str) -> Result<(), String> {
    use windows_sys::{
        core::w,
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    if ![
        "ms-settings:privacy-microphone",
        "ms-settings:easeofaccess-keyboard",
    ]
    .contains(&url)
    {
        return Err("Invalid Windows settings URI".into());
    }
    let uri = url.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let status = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            w!("open"),
            uri.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if status as isize <= 32 {
        Err("无法打开 Windows 设置 / Could not open Windows Settings".into())
    } else {
        Ok(())
    }
}

pub fn call(request: Value) -> Result<Value, String> {
    match request["action"].as_str().unwrap_or("") {
        "target" => target(),
        "copy" => {
            copy(request["text"].as_str().ok_or("Missing clipboard text")?)?;
            Ok(json!({"ok":true}))
        }
        "deliver" | "return_source" => Err("Windows 原位回填尚未启用，结果已保留，请从历史复制 / Windows input delivery is pending; copy the saved result".into()),
        _ => Err("Windows adapter does not support this operation".into()),
    }
}

fn target() -> Result<Value, String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return Ok(json!({}));
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == GetCurrentProcessId() {
            return Ok(json!({}));
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        let mut buffer = vec![0u16; 32768];
        let mut size = buffer.len() as u32;
        let app = if !process.is_null() {
            let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size);
            CloseHandle(process);
            if ok != 0 {
                let path = String::from_utf16_lossy(&buffer[..size as usize]);
                path.rsplit('\\').next().unwrap_or("").to_owned()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        // Do not read the window title, arbitrary input contents or clipboard.
        Ok(
            json!({"pid":pid,"hwnd":hwnd as usize,"app":app,"name":app,"bundle":app,
                  "selection":"","length":0,"delivery_supported":false}),
        )
    }
}

fn copy(text: &str) -> Result<(), String> {
    if text.len() > 512 * 1024 || text.contains('\0') {
        return Err("Invalid clipboard text".into());
    }
    let _guard = CLIPBOARD.lock().map_err(|_| "Clipboard unavailable")?;
    let wide = text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if memory.is_null() {
            return Err("Clipboard allocation failed".into());
        }
        let ptr = GlobalLock(memory);
        if ptr.is_null() {
            GlobalFree(memory);
            return Err("Clipboard allocation failed".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr.cast::<u16>(), wide.len());
        GlobalUnlock(memory);
        // A real owner window is needed; use a hidden message-only window for
        // clipboard operations rather than OpenClipboard(NULL) + EmptyClipboard.
        let owner = match clipboard_owner() {
            Ok(owner) => owner,
            Err(error) => {
                GlobalFree(memory);
                return Err(error);
            }
        };
        if OpenClipboard(owner) == 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(owner);
            GlobalFree(memory);
            return Err("剪贴板正被占用，请重试 / Clipboard busy".into());
        }
        let ok = EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null();
        CloseClipboard();
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(owner);
        if !ok {
            GlobalFree(memory);
            return Err("无法写入剪贴板 / Clipboard write failed".into());
        }
    }
    Ok(())
}

unsafe fn clipboard_owner() -> Result<HWND, String> {
    use windows_sys::{
        core::w,
        Win32::UI::WindowsAndMessaging::{CreateWindowExW, HWND_MESSAGE},
    };
    let hwnd = CreateWindowExW(
        0,
        w!("STATIC"),
        w!("Vao Clipboard"),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null(),
    );
    if hwnd.is_null() {
        Err("Clipboard owner unavailable".into())
    } else {
        Ok(hwnd)
    }
}

pub fn monitor(pid: u32) -> u32 {
    // HMONITOR is pointer-sized: retain it locally and expose an opaque u32 token.
    // The first candidate keeps just the most recent recording's monitor.
    MONITOR.store(0, std::sync::atomic::Ordering::Relaxed);
    unsafe {
        let window = GetForegroundWindow();
        let mut foreground_pid = 0;
        GetWindowThreadProcessId(window, &mut foreground_pid);
        if foreground_pid != pid {
            return 0;
        }
        MONITOR.store(
            MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) as usize,
            std::sync::atomic::Ordering::Relaxed,
        );
        1
    }
}

static MONITOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn place(window: &tauri::WebviewWindow, expanded: bool) -> Result<(), String> {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
        let mut monitor =
            MONITOR.load(std::sync::atomic::Ordering::Relaxed) as *mut std::ffi::c_void;
        if monitor.is_null() {
            monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err("Display unavailable".into());
        }
        // Work area is physical pixels; scale the WebView's logical dimensions.
        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let dpi_status = windows_sys::Win32::UI::HiDpi::GetDpiForMonitor(
            monitor,
            windows_sys::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
            &mut dpi_x,
            &mut dpi_y,
        );
        let scale = if dpi_status == 0 {
            dpi_x as f64 / 96.0
        } else {
            window.scale_factor().unwrap_or(1.0)
        };
        let area = info.rcWork;
        let width = ((380.0 * scale) as i32).min(area.right - area.left);
        let height =
            ((if expanded { 360.0 } else { 148.0 } * scale) as i32).min(area.bottom - area.top);
        let x = area.left + (area.right - area.left - width) / 2;
        let y = (area.bottom - height - (28.0 * scale) as i32).max(area.top);
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_targets_cannot_trigger_automatic_input() {
        for action in ["deliver", "return_source"] {
            assert!(call(json!({"action":action,"target":{"pid":1},"text":"synthetic"})).is_err());
        }
    }

    #[test]
    fn settings_uri_is_allowlisted() {
        assert!(open_settings("https://settings.example/").is_err());
    }

    #[test]
    fn windows_credentials_round_trip_and_delete() {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let slot = format!("synthetic-test-{:x?}", nonce);
        assert!(!crate::platform::has_secret(&slot).unwrap());
        crate::platform::save_secret(&slot, "synthetic credential fixture").unwrap();
        let result = crate::platform::secret(&slot);
        let exists = crate::platform::has_secret(&slot);
        let cleanup = crate::platform::save_secret(&slot, "");
        cleanup.unwrap();
        assert_eq!(result.unwrap(), "synthetic credential fixture");
        assert!(exists.unwrap());
        assert!(!crate::platform::has_secret(&slot).unwrap());
    }
}
