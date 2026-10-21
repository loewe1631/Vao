//! Verified, one-shot insertion selected by control capability, not app name.
//! Full field contents live only in this bounded in-memory snapshot, never JSON,
//! history, diagnostics or model requests. Unknown controls fall back to copying.
use serde_json::{json, Value};
use std::sync::Mutex;
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HWND},
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    UI::{
        Controls::{EM_GETPASSWORDCHAR, EM_GETSEL, EM_REPLACESEL},
        Input::KeyboardAndMouse::IsWindowEnabled,
        WindowsAndMessaging::{
            GetAncestor, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW,
            GetWindowThreadProcessId, IsChild, IsIconic, IsWindow, SendMessageTimeoutW,
            SetForegroundWindow, ShowWindow, ES_PASSWORD, ES_READONLY, GA_ROOT, GUITHREADINFO,
            GWL_STYLE, SMTO_ABORTIFHUNG, SMTO_BLOCK, SW_RESTORE, WM_GETTEXT, WM_GETTEXTLENGTH,
        },
    },
};

pub(super) const LIMIT: usize = 256 * 1024; // UTF-16 code units, single active recording only.
pub(super) const UNAVAILABLE: &str = "无法验证原输入框或光标，结果已保留供复制 / Original input or caret could not be verified; copy the result";
static TARGET: Mutex<Option<Target>> = Mutex::new(None);

struct Target {
    token: String,
    root: usize,
    field: usize,
    pid: u32,
    created: u64,
    class: String,
    text: Vec<u16>,
    start: usize,
    end: usize,
}

fn message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Result<usize, String> {
    let mut result = 0;
    if unsafe {
        SendMessageTimeoutW(
            hwnd,
            msg,
            wparam,
            lparam,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            1_000,
            &mut result,
        )
    } == 0
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(result)
}
pub(super) fn process_created(pid: u32) -> Result<u64, String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(UNAVAILABLE.into());
        }
        let [mut created, mut exited, mut kernel, mut user] = [FILETIME::default(); 4];
        let ok = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user);
        CloseHandle(process);
        if ok == 0 {
            return Err(UNAVAILABLE.into());
        }
        Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}
fn class(hwnd: HWND) -> String {
    let mut name = [0u16; 128];
    let length = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32) };
    String::from_utf16_lossy(&name[..length.max(0) as usize])
}
fn supported(class: &str) -> bool {
    ["edit", "richedit20w", "richedit50w", "richeditd2dpt"]
        .contains(&class.to_ascii_lowercase().as_str())
}
fn focused(root: HWND, pid: u32) -> Result<HWND, String> {
    unsafe {
        let mut actual_pid = 0;
        let thread = GetWindowThreadProcessId(root, &mut actual_pid);
        if thread == 0 || actual_pid != pid {
            return Err(UNAVAILABLE.into());
        }
        let mut gui: GUITHREADINFO = std::mem::zeroed();
        gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread, &mut gui) == 0 || gui.hwndFocus.is_null() {
            return Err(UNAVAILABLE.into());
        }
        let field = gui.hwndFocus;
        GetWindowThreadProcessId(field, &mut actual_pid);
        if actual_pid != pid
            || (field != root && IsChild(root, field) == 0)
            || GetAncestor(field, GA_ROOT) != root
        {
            return Err(UNAVAILABLE.into());
        }
        Ok(field)
    }
}
fn editable(field: HWND, name: &str) -> Result<(), String> {
    if !supported(name) || unsafe { IsWindow(field) } == 0 || unsafe { IsWindowEnabled(field) } == 0
    {
        return Err(UNAVAILABLE.into());
    }
    let style = unsafe { GetWindowLongPtrW(field, GWL_STYLE) } as u32;
    if style & (ES_PASSWORD as u32 | ES_READONLY as u32) != 0
        || message(field, EM_GETPASSWORDCHAR, 0, 0)? != 0
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(())
}
// Current-focus paste does not require reading the field, but known native
// password/readonly/disabled controls still have to refuse before any write.
pub fn paste_allowed(root: HWND, field: HWND) -> Result<(), String> {
    // Embedded renderers can own the focused child HWND in another process.
    // Its ancestry, lifetime and focus are checked by the paste lease; do not
    // apply the same-process restriction used by native text snapshots here.
    if unsafe { GetForegroundWindow() } != root
        || unsafe { IsWindow(field) } == 0
        || unsafe { GetAncestor(field, GA_ROOT) } != root
        || unsafe { IsWindowEnabled(field) } == 0
    {
        return Err(UNAVAILABLE.into());
    }
    let name = class(field);
    if supported(&name) {
        editable(field, &name)?;
    }
    Ok(())
}
fn contents(field: HWND, name: &str) -> Result<(Vec<u16>, usize, usize), String> {
    editable(field, name)?;
    let length = message(field, WM_GETTEXTLENGTH, 0, 0)?;
    if length > LIMIT {
        return Err(UNAVAILABLE.into());
    }
    let mut text = vec![0u16; length + 1];
    let read = message(field, WM_GETTEXT, text.len(), text.as_mut_ptr() as isize)?;
    if read > length || read != message(field, WM_GETTEXTLENGTH, 0, 0)? {
        return Err(UNAVAILABLE.into());
    }
    text.truncate(read);
    // RichEdit selection offsets count paragraph CRs, while WM_GETTEXT may return CRLF.
    if !name.eq_ignore_ascii_case("edit") {
        text = rich_text(&text);
    }
    let mut start = 0u32;
    let mut end = 0u32;
    message(
        field,
        EM_GETSEL,
        &mut start as *mut u32 as usize,
        &mut end as *mut u32 as isize,
    )?;
    if start > end || end as usize > text.len() {
        return Err(UNAVAILABLE.into());
    }
    Ok((text, start as usize, end as usize))
}
fn rich_text(text: &[u16]) -> Vec<u16> {
    let mut result = Vec::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let ch = text[index];
        result.push(if ch == 10 { 13 } else { ch });
        if ch == 13 && text.get(index + 1) == Some(&10) {
            index += 1;
        }
        index += 1;
    }
    result
}

pub fn capture(root: HWND, pid: u32) -> Result<Value, String> {
    crate::windows_paste::clear();
    let epoch = crate::windows_uia::invalidate();
    capture_native(root, pid).or_else(|error| {
        // A recognized native control's refusal (protected state, size,
        // inconsistent text/caret or timeout) is final. Do not reinterpret that
        // refusal through another provider; UIA covers other control technology.
        if focused(root, pid)
            .ok()
            .is_some_and(|field| supported(&class(field)))
        {
            Err(error)
        } else {
            crate::windows_uia::capture(root, pid, epoch)
        }
    })
}

fn capture_native(root: HWND, pid: u32) -> Result<Value, String> {
    let mut saved = TARGET.lock().map_err(|_| UNAVAILABLE)?;
    *saved = None; // A new recording always invalidates the previous token.
    let field = focused(root, pid)?;
    let name = class(field);
    let (text, start, end) = contents(field, &name)?;
    if unsafe { GetForegroundWindow() } != root || focused(root, pid)? != field {
        return Err(UNAVAILABLE.into());
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| UNAVAILABLE)?;
    let token: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    String::from_utf16(&text[..start]).map_err(|_| UNAVAILABLE)?;
    String::from_utf16(&text[end..]).map_err(|_| UNAVAILABLE)?;
    let selection = String::from_utf16(&text[start..end]).map_err(|_| UNAVAILABLE)?;
    let result = json!({"input_token":token,"input_adapter":"win32","delivery_supported":true,"selection":selection,"length":end-start});
    *saved = Some(Target {
        token,
        root: root as usize,
        field: field as usize,
        pid,
        created: process_created(pid)?,
        class: name,
        text,
        start,
        end,
    });
    Ok(result)
}

impl Target {
    fn verify(&self) -> Result<(), String> {
        let root = self.root as HWND;
        let field = self.field as HWND;
        let mut root_pid = 0;
        let mut field_pid = 0;
        unsafe {
            GetWindowThreadProcessId(root, &mut root_pid);
            GetWindowThreadProcessId(field, &mut field_pid);
        }
        if unsafe { IsWindow(root) } == 0
            || root_pid != self.pid
            || field_pid != self.pid
            || unsafe { GetAncestor(field, GA_ROOT) } != root
            || class(field) != self.class
            || process_created(self.pid)? != self.created
        {
            return Err(UNAVAILABLE.into());
        }
        let (text, start, end) = contents(field, &self.class)?;
        if text != self.text || start != self.start || end != self.end {
            return Err(UNAVAILABLE.into());
        }
        Ok(())
    }
    fn restore(&self) -> Result<(), String> {
        self.verify()
            .map_err(|_| format!("{UNAVAILABLE} (snapshot changed)"))?;
        let root = self.root as HWND;
        let field = self.field as HWND;
        // An inactive GUI thread may report no focus. A known different field
        // is a conflict; otherwise the exact field must be confirmed after the
        // system accepts activation. Never set another process's child focus.
        if focused(root, self.pid).is_ok_and(|current| current != field) {
            return Err(format!("{UNAVAILABLE} (focused field changed)"));
        }
        unsafe {
            if IsIconic(root) != 0 {
                ShowWindow(root, SW_RESTORE);
            }
            if GetForegroundWindow() != root && SetForegroundWindow(root) == 0 {
                return Err(format!("{UNAVAILABLE} (Windows denied activation)"));
            }
        }
        // Cross-thread foreground activation can finish after SetForegroundWindow
        // returns. Wait a bounded interval, without synthetic input or focus tricks.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            if unsafe { GetForegroundWindow() } == root
                && focused(root, self.pid).ok() == Some(field)
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!("{UNAVAILABLE} (activation not confirmed)"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.verify()
    }
    fn verify_focused(&self) -> Result<(), String> {
        self.verify()?;
        if unsafe { GetForegroundWindow() } != self.root as HWND
            || focused(self.root as HWND, self.pid)? != self.field as HWND
        {
            return Err(UNAVAILABLE.into());
        }
        Ok(())
    }
}

pub fn deliver(request: &Value, write: bool) -> Result<Value, String> {
    if matches!(
        request["target"]["input_adapter"].as_str(),
        Some("compat_paste" | "current_paste")
    ) {
        return crate::windows_paste::deliver(request, write);
    }
    if matches!(
        request["target"]["input_adapter"].as_str(),
        Some("uia_value" | "uia_unicode")
    ) {
        return crate::windows_uia::deliver(request.clone(), write);
    }
    let mut saved = TARGET.lock().map_err(|_| UNAVAILABLE)?;
    let token = request["target"]["input_token"]
        .as_str()
        .ok_or(UNAVAILABLE)?;
    if saved.as_ref().is_none_or(|target| target.token != token) {
        return Err(UNAVAILABLE.into());
    }
    // Consume before any focus/write operation: a timeout/partial write cannot be retried.
    let target = saved.take().ok_or(UNAVAILABLE)?;
    let text = request["text"].as_str().unwrap_or("");
    if write && (text.is_empty() || text.contains('\0') || text.encode_utf16().count() > LIMIT) {
        return Err(UNAVAILABLE.into());
    }
    target.restore()?;
    if !write {
        return Ok(json!({"ok":true}));
    }
    let inserted: Vec<_> = text.encode_utf16().collect();
    let inserted = if target.class.eq_ignore_ascii_case("edit") {
        // Standard multiline Edit needs CRLF; single-line fields reject multiline input.
        let normalized = String::from_utf16(&inserted)
            .map_err(|_| UNAVAILABLE)?
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let style = unsafe { GetWindowLongPtrW(target.field as HWND, GWL_STYLE) } as u32;
        if normalized.contains('\n')
            && style & windows_sys::Win32::UI::WindowsAndMessaging::ES_MULTILINE as u32 == 0
        {
            return Err(UNAVAILABLE.into());
        }
        normalized
            .replace('\n', "\r\n")
            .encode_utf16()
            .collect::<Vec<_>>()
    } else {
        rich_text(&inserted)
    };
    let expected = [
        &target.text[..target.start],
        &inserted,
        &target.text[target.end..],
    ]
    .concat();
    if expected.len() > LIMIT {
        return Err(UNAVAILABLE.into());
    }
    let terminated: Vec<_> = inserted.iter().copied().chain(Some(0)).collect();
    target.verify_focused()?;
    // Directly address the verified control. No clipboard, keyboard events or Enter.
    message(
        target.field as HWND,
        EM_REPLACESEL,
        1,
        terminated.as_ptr() as isize,
    )?;
    let (received, start, end) = contents(target.field as HWND, &target.class)?;
    if received != expected || start != target.start + inserted.len() || end != start {
        return Err("输入框未确认完整接收，请检查原文后复制，避免重复 / Receipt not confirmed; inspect the original field before copying to avoid duplication".into());
    }
    Ok(json!({"ok":true,"verified":true}))
}

pub fn opaque_candidate(root: HWND, pid: u32) -> bool {
    focused(root, pid).is_ok_and(|field| !supported(&class(field)))
}
pub fn context_value(text: &[u16], start: usize, end: usize) -> Result<Value, String> {
    if start != end || end > text.len() {
        return Err(UNAVAILABLE.into());
    }
    let before = String::from_utf16(&text[..start]).map_err(|_| UNAVAILABLE)?;
    let after = String::from_utf16(&text[end..]).map_err(|_| UNAVAILABLE)?;
    let before: String = before
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let after: String = after.chars().take(500).collect();
    Ok(json!({"version":1,"before_cursor":before,"after_cursor":after}))
}
pub fn context(target: &Value) -> Result<Value, String> {
    let token = target["input_token"].as_str().ok_or(UNAVAILABLE)?;
    if matches!(
        target["input_adapter"].as_str(),
        Some("uia_value" | "uia_unicode")
    ) {
        return crate::windows_uia::context(token);
    }
    let saved = TARGET.lock().map_err(|_| UNAVAILABLE)?;
    let saved = saved
        .as_ref()
        .filter(|t| t.token == token)
        .ok_or(UNAVAILABLE)?;
    saved.verify()?;
    context_value(&saved.text, saved.start, saved.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        process::{Child, Command},
        time::{Duration, Instant},
    };
    use windows_sys::{
        core::w,
        Win32::{
            Foundation::{LPARAM, LRESULT, WPARAM},
            System::{
                DataExchange::GetClipboardSequenceNumber,
                LibraryLoader::{GetModuleHandleW, LoadLibraryW},
            },
            UI::{
                Controls::EM_SETSEL,
                Input::KeyboardAndMouse::{EnableWindow, SetFocus},
                WindowsAndMessaging::*,
            },
        },
    };

    struct Fixture {
        child: Child,
        path: PathBuf,
        handles: Value,
    }
    impl Fixture {
        fn start() -> Self {
            let path =
                std::env::temp_dir().join(format!("vao-edit-fixture-{}.json", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "windows_input::tests::fixture_host",
                    "--nocapture",
                ])
                .env("VAO_EDIT_FIXTURE", &path)
                .env("VAO_EDIT_PARENT", std::process::id().to_string())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            let handles = loop {
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(value) = serde_json::from_slice(&bytes) {
                        break value;
                    }
                }
                assert!(Instant::now() < deadline, "fixture did not start");
                std::thread::sleep(Duration::from_millis(20));
            };
            Self {
                child,
                path,
                handles,
            }
        }
        fn hwnd(&self, name: &str) -> HWND {
            self.handles[name].as_u64().unwrap() as usize as HWND
        }
        fn focus(&self, name: &str) {
            unsafe {
                // Model the user choosing the next application. The foreground
                // fixture grants this test driver activation rights; a background
                // target process must not be expected to activate itself after
                // another application has received focus.
                SetForegroundWindow(self.hwnd("root"));
                PostMessageW(self.hwnd("root"), WM_APP, self.hwnd(name) as usize, 0);
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if unsafe { GetForegroundWindow() } == self.hwnd("root")
                    && focused(self.hwnd("root"), self.child.id()).ok() == Some(self.hwnd(name))
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "fixture focus {name}: root={}, foreground={}, field={}, focus={:?}",
                    self.hwnd("root") as usize,
                    unsafe { GetForegroundWindow() } as usize,
                    self.hwnd(name) as usize,
                    focused(self.hwnd("root"), self.child.id()).map(|hwnd| hwnd as usize)
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        fn text(&self, name: &str, value: &str) {
            let wide: Vec<_> = value.encode_utf16().chain(Some(0)).collect();
            let started = Instant::now();
            println!(
                "Native fixture: seed {name}, {} UTF16 units",
                wide.len() - 1
            );
            let mut result = 0;
            // First-time font initialization on a fresh runner can exceed the
            // production read budget. This only seeds the synthetic fixture.
            let ok = unsafe {
                SendMessageTimeoutW(
                    self.hwnd(name),
                    WM_SETTEXT,
                    0,
                    wide.as_ptr() as isize,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    5_000,
                    &mut result,
                )
            };
            assert_ne!(
                ok,
                0,
                "fixture WM_SETTEXT {name}: units={}, elapsed={:?}, class={}, window={}, process={:?}, error={}",
                wide.len() - 1,
                started.elapsed(),
                class(self.hwnd(name)),
                unsafe { IsWindow(self.hwnd(name)) },
                process_created(self.child.id()),
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            );
            assert_ne!(result, 0, "fixture WM_SETTEXT rejected {name}");
        }
        fn select(&self, name: &str, start: usize, end: usize) {
            message(self.hwnd(name), EM_SETSEL, start, end as isize).unwrap();
        }
        fn target(&self, name: &str) -> Value {
            self.focus(name);
            capture(self.hwnd("root"), self.child.id()).unwrap_or_else(|error| {
                let hwnd = self.hwnd(name);
                let mut start = 0u32;
                let mut end = 0u32;
                let selection = message(hwnd, EM_GETSEL, &mut start as *mut u32 as usize, &mut end as *mut u32 as isize);
                panic!("capture {name}: {error}; class={}, style={:#x}, password={:?}, length={:?}, selection={selection:?}/{start}/{end}, process={:?}, focused={:?}, contents={:?}",
                    class(hwnd), unsafe{GetWindowLongPtrW(hwnd,GWL_STYLE)}, message(hwnd,EM_GETPASSWORDCHAR,0,0), message(hwnd,WM_GETTEXTLENGTH,0,0), process_created(self.child.id()), focused(self.hwnd("root"),self.child.id()).map(|h|h as usize), contents(hwnd,&class(hwnd)).map(|(text,start,end)|(text.len(),start,end)));
            })
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_file(&self.path);
        }
    }
    unsafe extern "system" fn fixture_window(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_SETFOCUS {
            // Real editor hosts restore their own last focused child on activation.
            // A bare STATIC root does not; model that application behavior here,
            // inside the target process. Production never sets a foreign child focus.
            let field = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as HWND;
            if !field.is_null() && IsWindowEnabled(field) != 0 {
                SetFocus(field);
            }
            return 0;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
    #[test]
    #[ignore = "only spawned by the cross-process native test"]
    fn fixture_host() {
        let Ok(path) = std::env::var("VAO_EDIT_FIXTURE") else {
            return;
        };
        unsafe {
            assert!(!LoadLibraryW(w!("Msftedit.dll")).is_null());
            let mut window_class: WNDCLASSW = std::mem::zeroed();
            window_class.lpfnWndProc = Some(fixture_window);
            window_class.hInstance = GetModuleHandleW(std::ptr::null());
            window_class.lpszClassName = w!("VaoSyntheticInputTest");
            assert_ne!(RegisterClassW(&window_class), 0);
            let root = CreateWindowExW(
                0,
                window_class.lpszClassName,
                w!("Vao synthetic input test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                30,
                30,
                600,
                400,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                window_class.hInstance,
                std::ptr::null(),
            );
            assert!(!root.is_null());
            let make = |class, style, y| {
                CreateWindowExW(
                    0,
                    class,
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
                    10,
                    y,
                    500,
                    45,
                    root,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            let edit = make(w!("EDIT"), ES_MULTILINE as u32, 10);
            let rich = make(w!("RICHEDIT50W"), ES_MULTILINE as u32, 70);
            let next = make(w!("EDIT"), 0, 130);
            let password = make(w!("EDIT"), ES_PASSWORD as u32, 190);
            let readonly = make(w!("EDIT"), ES_READONLY as u32, 250);
            let unknown = make(w!("STATIC"), 0, 310);
            assert!(!edit.is_null() && !rich.is_null() && !next.is_null());
            SetWindowLongPtrW(root, GWLP_USERDATA, edit as isize);
            SetForegroundWindow(root);
            SetFocus(edit);
            AllowSetForegroundWindow(std::env::var("VAO_EDIT_PARENT").unwrap().parse().unwrap());
            std::fs::write(path,json!({"root":root as usize,"edit":edit as usize,"rich":rich as usize,"next":next as usize,"password":password as usize,"readonly":readonly as usize,"unknown":unknown as usize}).to_string()).unwrap();
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_APP {
                    SetWindowLongPtrW(root, GWLP_USERDATA, msg.wParam as isize);
                    SetForegroundWindow(root);
                    SetFocus(msg.wParam as HWND);
                } else {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
    #[test]
    fn native_cross_process_receipt_conflicts_and_selection() {
        let mut f = Fixture::start();
        let sequence = unsafe { GetClipboardSequenceNumber() };
        for name in ["edit", "rich"] {
            println!("Native fixture: {name} initial capture and replacement");
            f.text(name, "前😀后");
            f.select(name, 1, 3);
            let target = f.target(name);
            assert_eq!(target["selection"], "😀");
            assert!(target.get("text").is_none());
            let request = json!({"target":target,"text":"中文\nsecond 😀"});
            assert_eq!(deliver(&request, true).unwrap()["verified"], true);
            let value = contents(f.hwnd(name), &class(f.hwnd(name))).unwrap().0;
            let expected = if name == "edit" {
                "前中文\r\nsecond 😀后"
            } else {
                "前中文\rsecond 😀后"
            };
            assert_eq!(String::from_utf16(&value).unwrap(), expected);
            assert!(deliver(&request, true).is_err(), "one-shot token");
            // A second insertion after multiline content checks RichEdit offset conversion.
            let target = f.target(name);
            assert!(deliver(&json!({"target":target,"text":"!"}), true).is_ok());
            f.text(name, "baseline");
            f.select(name, 3, 3);
            let target = f.target(name);
            f.text(name, "changed");
            assert!(deliver(&json!({"target":target,"text":"NO"}), true).is_err());
            assert_eq!(
                String::from_utf16(&contents(f.hwnd(name), &class(f.hwnd(name))).unwrap().0)
                    .unwrap(),
                "changed"
            );
            f.text(name, "baseline");
            f.select(name, 3, 3);
            let target = f.target(name);
            f.select(name, 1, 1);
            assert!(
                deliver(&json!({"target":target,"text":"NO"}), true).is_err(),
                "caret conflict"
            );
            let target = f.target(name);
            f.focus("next");
            assert!(
                deliver(&json!({"target":target,"text":"NO"}), true).is_err(),
                "other field"
            );
            f.focus(name);
            f.select(name, 0, 4);
            let target = f.target(name);
            let original = contents(f.hwnd(name), &class(f.hwnd(name))).unwrap();
            assert!(deliver(&json!({"target":target}), false).is_ok());
            assert_eq!(
                contents(f.hwnd(name), &class(f.hwnd(name))).unwrap(),
                original,
                "question preserves selection"
            );
        }
        for name in ["password", "readonly", "unknown"] {
            println!("Native fixture: reject {name}");
            f.focus(name);
            assert!(
                capture(f.hwnd("root"), f.child.id()).is_err(),
                "reject {name}"
            );
        }
        f.text("next", "unchanged");
        f.select("next", 0, 0);
        let target = f.target("next");
        assert!(
            deliver(&json!({"target":target,"text":"line1\nline2"}), true).is_err(),
            "single line protects against newline"
        );
        // Exercise the content bound, not the EDIT renderer's slow layout
        // of one 256K unbroken line. Keep the real oversized field, with short
        // paragraphs, and prove it exceeds the limit before testing rejection.
        let paragraph = "x".repeat(63) + "\r\n";
        f.text("edit", &paragraph.repeat(LIMIT / paragraph.len() + 1));
        assert!(message(f.hwnd("edit"), WM_GETTEXTLENGTH, 0, 0).unwrap() > LIMIT);
        f.focus("edit");
        assert!(
            capture(f.hwnd("root"), f.child.id()).is_err(),
            "oversized field"
        );
        f.text("edit", "baseline");
        f.select("edit", 0, 0);
        let target = f.target("edit");
        assert!(
            deliver(&json!({"target":target,"text":"\0"}), true).is_err(),
            "NUL rejected"
        );
        let old = f.target("edit");
        let _new = f.target("edit");
        assert!(
            deliver(&json!({"target":old,"text":"NO"}), true).is_err(),
            "stale token"
        );
        let target = f.target("edit");
        unsafe {
            EnableWindow(f.hwnd("edit"), 0);
        }
        assert!(
            deliver(&json!({"target":target,"text":"NO"}), true).is_err(),
            "disabled"
        );
        unsafe {
            EnableWindow(f.hwnd("edit"), 1);
        }
        // Another process becomes foreground; permitted restoration returns to the captured control.
        let target = f.target("edit");
        let other = Fixture::start();
        other.focus("edit");
        let restored = deliver(&json!({"target":target,"text":" restored"}), true);
        assert!(
            restored.is_ok(),
            "restore original app: {restored:?}; foreground={}, expected root={}, focus={:?}, expected field={}",
            unsafe { GetForegroundWindow() } as usize,
            f.hwnd("root") as usize,
            focused(f.hwnd("root"), f.child.id()).map(|hwnd| hwnd as usize),
            f.hwnd("edit") as usize,
        );
        // New recordings must select their own process, including A -> B -> A.
        f.text("edit", "A");
        f.select("edit", 1, 1);
        let first = f.target("edit");
        other.text("edit", "B");
        other.select("edit", 1, 1);
        let second = other.target("edit");
        assert!(deliver(&json!({"target":first,"text":"NO"}), true).is_err());
        assert!(deliver(&json!({"target":second,"text":"2"}), true).is_ok());
        assert_eq!(contents(f.hwnd("edit"), "edit").unwrap().0, vec![65]);
        let third = f.target("edit");
        assert!(deliver(&json!({"target":third,"text":"1"}), true).is_ok());
        assert_eq!(contents(f.hwnd("edit"), "edit").unwrap().0, vec![65, 49]);
        assert_eq!(
            contents(other.hwnd("edit"), "edit").unwrap().0,
            vec![66, 50]
        );
        let previous = f.target("edit");
        f.focus("unknown");
        assert!(capture(f.hwnd("root"), f.child.id()).is_err());
        assert!(deliver(&json!({"target":previous,"text":"NO"}), true).is_err());

        // Exercise UIA against an actual foreign process as well as WebView2.
        f.text("next", "前😀后");
        f.select("next", 1, 3);
        f.focus("next");
        let epoch = crate::windows_uia::invalidate();
        let uia = crate::windows_uia::capture(f.hwnd("root"), f.child.id(), epoch)
            .expect("foreign Edit UIA capture");
        assert_eq!(uia["selection"], "😀");
        other.focus("edit");
        assert!(
            deliver(&json!({"target":uia,"text":"新"}), true).is_ok(),
            "UIA restores original process"
        );
        assert_eq!(
            String::from_utf16(&contents(f.hwnd("next"), "edit").unwrap().0).unwrap(),
            "前新后"
        );
        f.select("next", 0, 0);
        let epoch = crate::windows_uia::invalidate();
        let uia = crate::windows_uia::capture(f.hwnd("root"), f.child.id(), epoch).unwrap();
        f.select("next", 1, 1);
        assert!(
            deliver(&json!({"target":uia,"text":"NO"}), true).is_err(),
            "UIA caret conflict"
        );
        drop(other);
        let target = f.target("edit");
        f.child.kill().unwrap();
        f.child.wait().unwrap();
        assert!(
            deliver(&json!({"target":target,"text":"NO"}), true).is_err(),
            "closed process"
        );
        assert_eq!(
            unsafe { GetClipboardSequenceNumber() },
            sequence,
            "insertion does not touch clipboard"
        );
    }
}
