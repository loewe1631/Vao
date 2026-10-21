//! One-shot paste transport. A focus lease is never a text receipt.
use crate::windows_input::{process_created, UNAVAILABLE};
use serde_json::{json, Value};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, GlobalFree, SetLastError, HWND},
    System::{DataExchange::*, Memory::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

struct Lease {
    token: String,
    generation: u64,
    root: usize,
    field: usize,
    pid: u32,
    created: u64,
    expires: Instant,
    focus_token: Option<String>,
}
static TARGET: Mutex<Option<Lease>> = Mutex::new(None);
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const CLIPBOARD_LIMIT: usize = 16 * 1024 * 1024;

pub fn clear() {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    *TARGET.lock().unwrap() = None;
}
pub fn generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::Acquire)
}
// Capture at completion, without restoring a window or reading its contents.
// The generation was reserved with the session, before any asynchronous work.
pub fn capture_current(expected: u64, owned_probe: bool) -> Result<Value, String> {
    let root = unsafe { GetForegroundWindow() };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(root, &mut pid);
    }
    if root.is_null()
        || (!owned_probe
            && pid == unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() })
        || expected != generation()
    {
        return Err("当前没有可用的外部输入位置，请复制历史结果 / No external input focus; copy the saved result".into());
    }
    crate::windows_input::paste_allowed(root, pid)?;
    let field = unsafe { focus(root, pid)? };
    let identity = crate::windows_uia::paste_focus(root, None)?;
    let focus_token = identity["token"].as_str().ok_or(UNAVAILABLE)?.to_owned();
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| UNAVAILABLE)?;
    let token = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let saved = Lease {
        token: token.clone(),
        generation: expected,
        root: root as usize,
        field: field as usize,
        pid,
        created: process_created(pid)?,
        expires: Instant::now() + Duration::from_secs(5),
        focus_token: Some(focus_token),
    };
    saved.verify()?;
    let mut target = TARGET.lock().unwrap();
    if expected != generation() {
        return Err(UNAVAILABLE.into());
    }
    *target = Some(saved);
    Ok(
        json!({"input_token":token,"input_adapter":"current_paste","pid":pid,"hwnd":root as usize,"delivery_supported":true,"receipt_supported":false}),
    )
}
pub fn deliver_current(request: &Value) -> Result<Value, String> {
    let target = capture_current(request["generation"].as_u64().ok_or(UNAVAILABLE)?, false)?;
    deliver(&json!({"target":target,"text":request["text"]}), true)
}
unsafe fn focus(root: HWND, pid: u32) -> Result<HWND, String> {
    let mut actual = 0;
    let thread = GetWindowThreadProcessId(root, &mut actual);
    let mut info: GUITHREADINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if actual != pid
        || thread == 0
        || GetGUIThreadInfo(thread, &mut info) == 0
        || info.hwndFocus.is_null()
        || GetAncestor(info.hwndFocus, GA_ROOT) != root
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(info.hwndFocus)
}
pub fn capture(root: HWND, pid: u32) -> Result<Value, String> {
    let generation = GENERATION.load(std::sync::atomic::Ordering::Acquire);
    // The UIA worker checks password/readonly and rejects an Edit refused by the
    // strict snapshot path. Unknown non-Edit panes require explicit user opt-in.
    crate::windows_uia::compatibility_allowed(root, pid)?;
    let field = unsafe { focus(root, pid)? };
    if unsafe { GetForegroundWindow() } != root {
        return Err(UNAVAILABLE.into());
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| UNAVAILABLE)?;
    let token = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut target = TARGET.lock().unwrap();
    if generation != GENERATION.load(std::sync::atomic::Ordering::Acquire) {
        return Err(UNAVAILABLE.into());
    }
    *target = Some(Lease {
        token: token.clone(),
        generation,
        root: root as usize,
        field: field as usize,
        pid,
        created: process_created(pid)?,
        expires: Instant::now() + Duration::from_secs(3600),
        focus_token: None,
    });
    Ok(
        json!({"input_token":token,"input_adapter":"compat_paste","delivery_supported":true,"receipt_supported":false,"selection":"","length":0}),
    )
}
impl Lease {
    fn verify(&self) -> Result<(), String> {
        if self.generation != GENERATION.load(std::sync::atomic::Ordering::Acquire) {
            return Err(UNAVAILABLE.into());
        }
        let root = self.root as HWND;
        let mut actual = 0;
        unsafe {
            GetWindowThreadProcessId(root, &mut actual);
        }
        if Instant::now() >= self.expires
            || unsafe { IsWindow(root) } == 0
            || actual != self.pid
            || process_created(actual)? != self.created
            || unsafe { GetForegroundWindow() } != root
            || unsafe { focus(root, self.pid)? } as usize != self.field
        {
            return Err(UNAVAILABLE.into());
        }
        if let Some(token) = &self.focus_token {
            crate::windows_input::paste_allowed(root, self.pid)?;
            crate::windows_uia::paste_focus(root, Some(token))?;
            if self.generation != generation()
                || unsafe { GetForegroundWindow() } != root
                || unsafe { focus(root, self.pid)? } as usize != self.field
            {
                return Err(UNAVAILABLE.into());
            }
        }
        Ok(())
    }
}
pub fn deliver(request: &Value, write: bool) -> Result<Value, String> {
    let saved = {
        let mut lease = TARGET.lock().unwrap();
        if lease
            .as_ref()
            .is_none_or(|l| request["target"]["input_token"].as_str() != Some(l.token.as_str()))
        {
            return Err(UNAVAILABLE.into());
        }
        lease.take().unwrap()
    };
    saved.verify()?;
    if saved.focus_token.is_none() {
        crate::windows_uia::compatibility_allowed(saved.root as HWND, saved.pid)?;
    }
    if !write {
        return Ok(json!({"ok":true}));
    }
    let text = request["text"].as_str().ok_or(UNAVAILABLE)?;
    let _transaction = dispatch(text, || saved.verify())?;
    // No readback is available. Keep the inserted text on the clipboard for
    // delayed paste consumers; restore would race without a host receipt.
    Ok(
        json!({"ok":true,"verified":false,"input_adapter":if saved.focus_token.is_some(){"current_paste"}else{"compat_paste"}}),
    )
}

fn keys_released() -> bool {
    (1..255).all(|key| unsafe { GetAsyncKeyState(key) } as u16 & 0x8000 == 0)
}
fn events() -> [INPUT; 4] {
    [
        (0x11, 0),
        (0x56, 0),
        (0x56, KEYEVENTF_KEYUP),
        (0x11, KEYEVENTF_KEYUP),
    ]
    .map(|(key, flags)| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0x56414f,
            },
        },
    })
}

struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
unsafe fn open(owner: HWND) -> Result<Open, String> {
    if OpenClipboard(owner) == 0 {
        Err("剪贴板正被占用，结果已保留 / Clipboard busy; result retained".into())
    } else {
        Ok(Open)
    }
}
unsafe fn put(format: u32, data: &[u8]) -> Result<(), String> {
    let memory = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if memory.is_null() {
        return Err("剪贴板内存不足".into());
    }
    let ptr = GlobalLock(memory);
    if ptr.is_null() {
        GlobalFree(memory);
        return Err("剪贴板内存不可用".into());
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.cast(), data.len());
    GlobalUnlock(memory);
    if SetClipboardData(format, memory).is_null() {
        GlobalFree(memory);
        return Err("无法写入剪贴板".into());
    }
    Ok(())
}
unsafe fn snapshot() -> Result<Vec<(u32, Vec<u8>)>, String> {
    let mut out = vec![];
    let mut format = 0;
    let mut size = 0;
    loop {
        SetLastError(0);
        format = EnumClipboardFormats(format);
        if format == 0 {
            if GetLastError() != 0 {
                return Err("无法备份剪贴板".into());
            }
            break;
        }
        // Only memory-backed standard/registered formats. GDI/metafile/private
        // object handles cannot safely be copied as bytes; refuse before writes.
        if !matches!(format, 1 | 7 | 8 | 13 | 15 | 16 | 17 | 0xc000..=0xffff) || out.len() >= 64 {
            return Err("剪贴板格式无法安全备份，结果已保留，请复制后手动粘贴 / Clipboard format unsupported; copy the result".into());
        }
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return Err("剪贴板格式暂不可读取".into());
        }
        let bytes = GlobalSize(handle);
        size += bytes;
        if bytes == 0 || size > CLIPBOARD_LIMIT {
            return Err("剪贴板备份超过限制，结果已保留".into());
        }
        let pointer = GlobalLock(handle);
        if pointer.is_null() {
            return Err("剪贴板无法备份".into());
        }
        let data = std::slice::from_raw_parts(pointer.cast::<u8>(), bytes).to_vec();
        GlobalUnlock(handle);
        out.push((format, data));
    }
    Ok(out)
}

unsafe fn matches_data(format: u32, expected: &[u8]) -> bool {
    let handle = GetClipboardData(format);
    if handle.is_null() || GlobalSize(handle) < expected.len() {
        return false;
    }
    let pointer = GlobalLock(handle);
    if pointer.is_null() {
        return false;
    }
    let matches = std::slice::from_raw_parts(pointer.cast::<u8>(), expected.len()) == expected;
    GlobalUnlock(handle);
    matches
}

pub struct Transaction {
    sequence: u32,
    marker: u32,
    nonce: Vec<u8>,
    backup: Vec<(u32, Vec<u8>)>,
}
impl Transaction {
    unsafe fn owns_open(&self) -> bool {
        GetClipboardSequenceNumber() == self.sequence && matches_data(self.marker, &self.nonce)
    }
    fn current(&self) -> bool {
        let Ok(_guard) = crate::platform::windows::CLIPBOARD.lock() else {
            return false;
        };
        unsafe {
            let Ok(owner) = crate::platform::windows::clipboard_owner() else {
                return false;
            };
            let owner = Window(owner);
            let Ok(_open) = open(owner.0) else {
                return false;
            };
            self.owns_open()
        }
    }
    /// Called only after a reliable target receipt. Verify ownership again while
    /// holding the clipboard. Never overwrite a later user copy.
    pub fn restore(&self) -> bool {
        let Ok(_guard) = crate::platform::windows::CLIPBOARD.lock() else {
            return false;
        };
        unsafe {
            let Ok(owner) = crate::platform::windows::clipboard_owner() else {
                return false;
            };
            let owner = Window(owner);
            let Ok(_open) = open(owner.0) else {
                return false;
            };
            if !self.owns_open() || EmptyClipboard() == 0 {
                return false;
            }
            self.backup
                .iter()
                .all(|(format, data)| put(*format, data).is_ok())
        }
    }
}

pub fn dispatch(
    text: &str,
    verify: impl Fn() -> Result<(), String>,
) -> Result<Transaction, String> {
    if text.is_empty()
        || text.len() > 512 * 1024
        || text
            .chars()
            .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t')))
    {
        return Err("输入文字无效".into());
    }
    let deadline = Instant::now() + Duration::from_millis(750);
    while !keys_released() {
        if Instant::now() >= deadline {
            return Err("请松开按键，结果已保留".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    verify()?;
    let transaction = prepare(text, &verify)?;
    if let Err(error) = verify() {
        transaction.restore();
        return Err(error);
    }
    if !keys_released() {
        transaction.restore();
        return Err("按键状态变化，结果已保留".into());
    }
    if !transaction.current() {
        return Err(
            "剪贴板已变化，未派发粘贴，结果已保留 / Clipboard changed; paste not dispatched".into(),
        );
    }
    let input = events();
    let queued = unsafe { SendInput(4, input.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    // Partial queue acceptance can mean a partial key chord; no re-dispatch or
    // clipboard restoration that could feed a delayed host the wrong contents.
    if queued != 4 {
        // Release only keys this partial chord pressed; never issue another V.
        let cleanup: Vec<INPUT> = if queued >= 2 {
            input[2..].to_vec()
        } else if queued == 1 {
            vec![input[3]]
        } else {
            vec![]
        };
        if !cleanup.is_empty() {
            unsafe {
                SendInput(
                    cleanup.len() as u32,
                    cleanup.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                );
            }
        }
        return Err("粘贴接收未确认，请检查原框后复制；不会自动重试 / Paste not confirmed; inspect before copying".into());
    }
    Ok(transaction)
}

fn prepare(text: &str, verify: impl Fn() -> Result<(), String>) -> Result<Transaction, String> {
    let transaction = {
        let _guard = crate::platform::windows::CLIPBOARD
            .lock()
            .map_err(|_| UNAVAILABLE)?;
        unsafe {
            let owner = Window(crate::platform::windows::clipboard_owner()?);
            let _open = open(owner.0)?;
            let backup = snapshot()?;
            let marker_name: Vec<_> = "Vao.Input.Transaction\0".encode_utf16().collect();
            let marker = RegisterClipboardFormatW(marker_name.as_ptr());
            if marker == 0 {
                return Err(UNAVAILABLE.into());
            }
            let mut nonce = vec![0u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| UNAVAILABLE)?;
            verify()?;
            if EmptyClipboard() == 0 {
                return Err("剪贴板无法准备".into());
            }
            let bytes: Vec<_> = text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            let write = put(13, &bytes).and_then(|_| put(marker, &nonce));
            if let Err(error) = write {
                // No paste has occurred. Best effort restore while still owning
                // clipboard access; never dispatch after setup failed.
                EmptyClipboard();
                for (format, data) in &backup {
                    let _ = put(*format, data);
                }
                return Err(error);
            }
            // Windows may synthesize text formats and advance the sequence on
            // CloseClipboard. Seal after that write-close, while re-opening to
            // verify the original owner, marker and exact Unicode payload.
            drop(_open);
            let _seal = open(owner.0)?;
            if GetClipboardOwner() != owner.0
                || !matches_data(marker, &nonce)
                || !matches_data(13, &bytes)
            {
                return Err("剪贴板已变化，未派发粘贴，结果已保留 / Clipboard changed; paste not dispatched".into());
            }
            Transaction {
                sequence: GetClipboardSequenceNumber(),
                marker,
                nonce,
                backup,
            }
        }
    };
    Ok(transaction)
}

// Only reached through the explicit synthetic input probe; never exposes user
// clipboard contents to the WebView. All returned values are comparisons.
pub fn fixture_clipboard(action: &str) -> Result<Value, String> {
    const BASE: &str = "Vao synthetic clipboard baseline";
    const USER: &str = "Vao synthetic later user copy";
    fn seed(text: &str) -> Result<(), String> {
        let _guard = crate::platform::windows::CLIPBOARD
            .lock()
            .map_err(|_| UNAVAILABLE)?;
        unsafe {
            let owner = Window(crate::platform::windows::clipboard_owner()?);
            let _open = open(owner.0)?;
            if EmptyClipboard() == 0 {
                return Err(UNAVAILABLE.into());
            }
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            put(13, &bytes)?;
            let name: Vec<u16> = "HTML Format\0".encode_utf16().collect();
            let format = RegisterClipboardFormatW(name.as_ptr());
            if format == 0 {
                return Err(UNAVAILABLE.into());
            }
            put(format, b"Vao synthetic HTML format\0")
        }
    }
    fn matches(text: &str, html: bool) -> Result<bool, String> {
        let _guard = crate::platform::windows::CLIPBOARD
            .lock()
            .map_err(|_| UNAVAILABLE)?;
        unsafe {
            let owner = Window(crate::platform::windows::clipboard_owner()?);
            let _open = open(owner.0)?;
            let saved = snapshot()?;
            let bytes: Vec<u8> = text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            let name: Vec<u16> = "HTML Format\0".encode_utf16().collect();
            let format = RegisterClipboardFormatW(name.as_ptr());
            Ok(saved.iter().any(|(f, b)| *f == 13 && b.starts_with(&bytes))
                && (!html
                    || saved.iter().any(|(f, b)| {
                        *f == format && b.starts_with(b"Vao synthetic HTML format\0")
                    })))
        }
    }
    match action {
        "seed" => {
            seed(BASE)?;
            Ok(json!({"ok":true}))
        }
        "status" => {
            Ok(json!({"baseline":matches(BASE, true)?,"pending":matches("未确认🙂", false)?}))
        }
        "race" => {
            seed(BASE)?;
            let t = prepare("Vao synthetic pending paste", || Ok(()))?;
            let restored = t.restore() && matches(BASE, true)?;
            let t = prepare("Vao synthetic pending paste", || Ok(()))?;
            seed(USER)?;
            let later_copy_preserved = !t.restore() && matches(USER, true)?;
            let mut t = prepare("Vao synthetic pending paste", || Ok(()))?;
            t.nonce[0] ^= 1;
            let wrong_marker_rejected = !t.restore();
            seed(BASE)?;
            Ok(
                json!({"restored":restored,"later_copy_preserved":later_copy_preserved,"wrong_marker_rejected":wrong_marker_rejected}),
            )
        }
        _ => Err("Unknown synthetic clipboard operation".into()),
    }
}

// A self-owned opaque Win32 host exercises the window-only lease without
// attaching to an external application or assuming a text/selection provider.
static FIXTURE_WINDOW: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
static FIXTURE_PASTES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static FIXTURE_TEXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static FIXTURE_ENTERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn fixture_window(action: &str) -> Result<Value, String> {
    use std::sync::atomic::Ordering;
    unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
        if msg == WM_KEYDOWN && w == 0x0d {
            FIXTURE_ENTERS.fetch_add(1, Ordering::SeqCst);
        }
        if msg == WM_KEYDOWN && w == 0x56 && GetKeyState(0x11) as u16 & 0x8000 != 0 {
            FIXTURE_PASTES.fetch_add(1, Ordering::SeqCst);
            if let Ok(_open) = open(hwnd) {
                let handle = GetClipboardData(13);
                if !handle.is_null() && GlobalSize(handle) >= 12 {
                    let ptr = GlobalLock(handle);
                    if !ptr.is_null() {
                        let expected: Vec<u16> = "不透明🙂\0".encode_utf16().collect();
                        FIXTURE_TEXT.store(
                            std::slice::from_raw_parts(ptr.cast::<u16>(), expected.len())
                                == expected,
                            Ordering::SeqCst,
                        );
                        GlobalUnlock(handle);
                    }
                }
            }
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
    let root = *FIXTURE_WINDOW.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || unsafe {
            let instance =
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let name: Vec<u16> = "VaoSyntheticOpaqueHost\0".encode_utf16().collect();
            let mut class: WNDCLASSW = std::mem::zeroed();
            class.hInstance = instance;
            class.lpszClassName = name.as_ptr();
            class.lpfnWndProc = Some(procedure);
            if RegisterClassW(&class) == 0 {
                let _ = tx.send(0usize);
                return;
            }
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                name.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                80,
                80,
                420,
                200,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if !hwnd.is_null() {
                SetForegroundWindow(hwnd);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
            }
            let _ = tx.send(hwnd as usize);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap_or(0)
    }) as HWND;
    if root.is_null() {
        return Err("Opaque synthetic host unavailable".into());
    }
    match action {
        "capture" => {
            let pid = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() };
            let deadline = Instant::now() + Duration::from_secs(2);
            while unsafe { GetForegroundWindow() } != root
                || unsafe { focus(root, pid) }.ok() != Some(root)
            {
                if Instant::now() >= deadline {
                    return Err("Opaque synthetic focus unavailable".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            clear();
            crate::windows_uia::invalidate();
            capture(root, pid)
        }
        "status" => Ok(
            json!({"pastes":FIXTURE_PASTES.load(Ordering::SeqCst),"text_matches":FIXTURE_TEXT.load(Ordering::SeqCst),"enters":FIXTURE_ENTERS.load(Ordering::SeqCst)}),
        ),
        "hide" => {
            unsafe {
                ShowWindow(root, SW_HIDE);
            }
            Ok(json!({"ok":true}))
        }
        _ => Err("Unknown opaque synthetic operation".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chord_contains_no_enter_and_exactly_one_paste() {
        let e = events();
        assert_eq!(e.len(), 4);
        unsafe {
            assert_eq!(e[0].Anonymous.ki.wVk, 0x11);
            assert_eq!(e[1].Anonymous.ki.wVk, 0x56);
            assert_eq!(e[2].Anonymous.ki.dwFlags, KEYEVENTF_KEYUP);
            assert_eq!(e[3].Anonymous.ki.wVk, 0x11);
        }
    }
    #[test]
    fn invalid_input_never_touches_clipboard_or_keys() {
        for text in ["", "bad\0text", "bad\u{1b}text"] {
            assert!(dispatch(text, || panic!("must reject before touching target")).is_err());
        }
    }
}
