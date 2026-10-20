//! Windows target, clipboard and overlay adapters.
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
        "deliver" => crate::windows_input::deliver(&request, true),
        "return_source" => crate::windows_input::deliver(&request, false),
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
        let mut result = json!({"pid":pid,"hwnd":hwnd as usize,"app":app,"name":app,"bundle":app,
                  "selection":"","length":0,"delivery_supported":false});
        if let Ok(input) = crate::windows_input::capture(hwnd, pid) {
            result
                .as_object_mut()
                .unwrap()
                .extend(input.as_object().unwrap().clone());
        }
        Ok(result)
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

static OVERLAY_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(74);
const FRAME_BITS: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WS_CAPTION
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_THICKFRAME
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_SYSMENU
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_MINIMIZEBOX
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_MAXIMIZEBOX;
const EDGE_BITS: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_WINDOWEDGE
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE
    | windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_DLGMODALFRAME;

unsafe extern "system" fn overlay_window(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    _data: usize,
) -> isize {
    use windows_sys::Win32::UI::{Shell::*, WindowsAndMessaging::*};
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as isize;
    }
    if msg == WM_STYLECHANGING && lparam != 0 {
        let result = DefSubclassProc(hwnd, msg, wparam, lparam);
        let styles = &mut *(lparam as *mut STYLESTRUCT);
        match wparam as i32 {
            GWL_STYLE => styles.styleNew &= !FRAME_BITS,
            GWL_EXSTYLE => {
                styles.styleNew = (styles.styleNew | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)
                    & !(EDGE_BITS | WS_EX_APPWINDOW);
            }
            _ => {}
        }
        return result;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(overlay_window), id);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

pub fn initialize_overlay(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows_sys::Win32::{Graphics::Dwm::*, UI::WindowsAndMessaging::*};
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
    // Keep later framework style updates from restoring a native caption or focus.
    if unsafe { windows_sys::Win32::UI::Shell::SetWindowSubclass(hwnd, Some(overlay_window), 1, 0) }
        == 0
    {
        return Err("Could not protect overlay window styles".into());
    }
    window.set_shadow(false).map_err(|e| e.to_string())?;
    window.set_focusable(false).map_err(|e| e.to_string())?;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(FRAME_BITS as isize));
        let extended = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (extended | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize)
                & !((EDGE_BITS | WS_EX_APPWINDOW) as isize),
        );
        let policy = DWMNCRP_DISABLED;
        // Disabling non-client rendering removes the outline around transparent space.
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY as u32,
            &policy as *const _ as _,
            std::mem::size_of_val(&policy) as u32,
        );
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct DragOrigin {
    hwnd: usize,
    cursor: windows_sys::Win32::Foundation::POINT,
    rect: windows_sys::Win32::Foundation::RECT,
}

fn move_drag(
    origin: DragOrigin,
    cursor: windows_sys::Win32::Foundation::POINT,
    held: bool,
) -> Result<bool, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if !held {
        return Ok(false);
    }
    let dx = i64::from(cursor.x) - i64::from(origin.cursor.x);
    let dy = i64::from(cursor.y) - i64::from(origin.cursor.y);
    if dx.abs() < 4 && dy.abs() < 4 {
        return Ok(true);
    }
    let x = (i64::from(origin.rect.left) + dx).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y = (i64::from(origin.rect.top) + dy).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    if unsafe {
        SetWindowPos(
            origin.hwnd as HWND,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
    } == 0
    {
        return Err("Could not move overlay".into());
    }
    Ok(true)
}

fn drag_tick(origin: DragOrigin) -> Result<bool, String> {
    use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};
    if unsafe { IsWindowVisible(origin.hwnd as HWND) } == 0 {
        return Ok(false);
    }
    let held = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } < 0;
    let mut cursor = windows_sys::Win32::Foundation::POINT::default();
    if held && unsafe { GetCursorPos(&mut cursor) } == 0 {
        return Err("Mouse position unavailable".into());
    }
    move_drag(origin, cursor, held)
}

pub async fn drag_overlay(window: tauri::WebviewWindow) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
    static DRAGGING: AtomicBool = AtomicBool::new(false);
    if DRAGGING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    struct ResetDrag;
    impl Drop for ResetDrag {
        fn drop(&mut self) {
            DRAGGING.store(false, Ordering::Release);
        }
    }
    let _reset = ResetDrag;
    let mut origin = DragOrigin {
        hwnd: window.hwnd().map_err(|e| e.to_string())?.0 as usize,
        cursor: POINT::default(),
        rect: RECT::default(),
    };
    if unsafe { GetCursorPos(&mut origin.cursor) } == 0
        || unsafe { GetWindowRect(origin.hwnd as HWND, &mut origin.rect) } == 0
    {
        return Err("Overlay position unavailable".into());
    }
    // Do not enter Windows' non-client move loop: it can show a native ghost
    // caption, particularly when the async request arrives after mouse release.
    // Only sample the physical button/cursor while this drag is active.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let (tx, rx) = tokio::sync::oneshot::channel();
        window
            .run_on_main_thread(move || {
                let _ = tx.send(drag_tick(origin));
            })
            .map_err(|e| e.to_string())?;
        if !rx.await.map_err(|_| "Overlay drag interrupted")??
            || std::time::Instant::now() >= deadline
        {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(16)).await;
    }
}
fn overlay_area(
    hwnd: HWND,
    captured: bool,
) -> Result<(windows_sys::Win32::Foundation::RECT, f64), String> {
    unsafe {
        let saved = MONITOR.load(std::sync::atomic::Ordering::Relaxed) as *mut std::ffi::c_void;
        let monitor = if captured && !saved.is_null() {
            saved
        } else {
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
        };
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err("Display unavailable".into());
        }
        let mut x = 96;
        let mut y = 96;
        let status = windows_sys::Win32::UI::HiDpi::GetDpiForMonitor(
            monitor,
            windows_sys::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
            &mut x,
            &mut y,
        );
        Ok((info.rcWork, if status == 0 { x as f64 / 96.0 } else { 1.0 }))
    }
}
pub fn place(window: &tauri::WebviewWindow, _expanded: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE};
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
    let (area, scale) = overlay_area(hwnd, true)?;
    let width = (380.0 * scale).round() as i32;
    let height = OVERLAY_HEIGHT.load(std::sync::atomic::Ordering::Relaxed) as f64;
    let (x, y, width, height) = geometry(area, scale, width, height, None);
    if unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE) } == 0 {
        return Err("Overlay placement failed".into());
    }
    Ok(())
}
fn geometry(
    area: windows_sys::Win32::Foundation::RECT,
    scale: f64,
    width: i32,
    height: f64,
    anchor: Option<(i32, i32)>,
) -> (i32, i32, i32, i32) {
    let width = width.min(area.right - area.left).max(1);
    let height = ((height.clamp(74.0, 360.0) * scale).round() as i32)
        .min(area.bottom - area.top)
        .max(1);
    let (left, bottom) = anchor.unwrap_or((
        area.left + (area.right - area.left - width) / 2,
        area.bottom - (28.0 * scale).round() as i32,
    ));
    (
        left.clamp(area.left, area.right - width),
        (bottom - height).clamp(area.top, area.bottom - height),
        width,
        height,
    )
}
pub fn fit(window: &tauri::WebviewWindow, height: f64) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE,
    };
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
    let (area, scale) = overlay_area(hwnd, false)?;
    let mut rect = windows_sys::Win32::Foundation::RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return Err("Overlay unavailable".into());
    }
    let (x, y, width, pixels) = geometry(
        area,
        scale,
        rect.right - rect.left,
        height,
        Some((rect.left, rect.bottom)),
    );
    OVERLAY_HEIGHT.store(height.round() as u32, std::sync::atomic::Ordering::Relaxed);
    if rect.bottom - rect.top != pixels
        && unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, pixels, SWP_NOACTIVATE) } == 0
    {
        return Err("Overlay sizing failed".into());
    }
    Ok(())
}

pub fn probe_overlay(window: &tauri::WebviewWindow) -> Result<Value, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
    let mut before = windows_sys::Win32::Foundation::RECT::default();
    let mut after = before;
    let foreground = unsafe { GetForegroundWindow() };
    place(window, false)?;
    unsafe {
        GetWindowRect(hwnd, &mut before);
    }
    fit(window, 120.0)?;
    unsafe {
        GetWindowRect(hwnd, &mut after);
    }
    // Exercise the persistent style guard against a later framework restyle.
    unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            GetWindowLongPtrW(hwnd, GWL_STYLE) | FRAME_BITS as isize,
        );
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | EDGE_BITS as isize)
                & !(WS_EX_NOACTIVATE as isize),
        );
    }
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let extended = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let mouse_activate = unsafe {
        SendMessageW(
            hwnd,
            WM_MOUSEACTIVATE,
            0,
            (HTCLIENT | (WM_LBUTTONDOWN << 16)) as isize,
        )
    };
    let origin = DragOrigin {
        hwnd: hwnd as usize,
        cursor: windows_sys::Win32::Foundation::POINT { x: 100, y: 100 },
        rect: after,
    };
    // Synthetic coordinates are applied only to this isolated probe window;
    // no physical cursor/button or other application's input is modified.
    for _ in 0..3 {
        move_drag(origin, origin.cursor, true)?;
        move_drag(origin, origin.cursor, false)?;
    }
    let mut clicked = after;
    unsafe {
        GetWindowRect(hwnd, &mut clicked);
    }
    move_drag(
        origin,
        windows_sys::Win32::Foundation::POINT { x: 132, y: 116 },
        true,
    )?;
    let mut dragged = after;
    unsafe {
        GetWindowRect(hwnd, &mut dragged);
    }
    let released = !move_drag(
        origin,
        windows_sys::Win32::Foundation::POINT { x: 300, y: 300 },
        false,
    )?;
    let mut stopped = after;
    unsafe {
        GetWindowRect(hwnd, &mut stopped);
    }
    let position = |rect: &windows_sys::Win32::Foundation::RECT| {
        (rect.left, rect.top, rect.right, rect.bottom)
    };
    let result = json!({"frame_free":style & FRAME_BITS==0,
        "no_activate":extended & WS_EX_NOACTIVATE!=0,
        "no_edge":extended & EDGE_BITS==0,
        "bottom_retained":before.bottom==after.bottom && before.left==after.left,
        "focus_preserved":foreground==unsafe{GetForegroundWindow()},
        "content_height_changed":before.bottom-before.top!=after.bottom-after.top,
        "style_guarded":style & FRAME_BITS==0 && extended & EDGE_BITS==0 && extended & WS_EX_NOACTIVATE!=0,
        "click_no_activate":mouse_activate==MA_NOACTIVATE as isize && foreground==unsafe{GetForegroundWindow()},
        "click_preserves_position":position(&clicked)==position(&after),
        "drag_moves_without_frame":dragged.left==after.left+32 && dragged.top==after.top+16 && unsafe{GetWindowLongPtrW(hwnd,GWL_STYLE)} as u32 & FRAME_BITS==0,
        "release_stops_drag":released && position(&dragged)==position(&stopped)});
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            after.left,
            after.top,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
    fit(window, 74.0)?;
    Ok(result)
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
    fn content_fit_retains_drag_anchor_and_clamps_negative_dpi_work_area() {
        let area = windows_sys::Win32::Foundation::RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        assert_eq!(
            geometry(area, 1.5, 570, 100.0, Some((-1600, 700))),
            (-1600, 550, 570, 150)
        );
        assert_eq!(
            geometry(area, 1.5, 570, 360.0, Some((200, 1000))),
            (-570, 340, 570, 540)
        );
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
