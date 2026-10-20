//! Capability-based UI Automation for plain Edit controls (Win32, XAML, web).
//! All COM objects and transient field text remain on one MTA worker. No app
//! allowlist, keyboard injection, clipboard, window titles or background scan.
use crate::windows_input::{process_created, LIMIT, UNAVAILABLE};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, OnceLock,
    },
    time::{Duration, Instant},
};
use windows::{
    core::{Interface, BSTR},
    Win32::{
        Foundation::HWND as ComHwnd,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_MULTITHREADED,
        },
        UI::Accessibility::*,
    },
};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    },
};

static EPOCH: AtomicU64 = AtomicU64::new(0);
static WORKER: OnceLock<Option<mpsc::SyncSender<Command>>> = OnceLock::new();
const BUDGET: Duration = Duration::from_secs(5);
const RECEIPT: &str = "输入框未确认完整接收，请检查原文后复制，避免重复 / Receipt not confirmed; inspect the original field before copying to avoid duplication";

enum Action {
    Clear,
    Capture { root: usize, pid: u32 },
    Deliver { request: Value, write: bool },
}
struct Command {
    epoch: u64,
    deadline: Instant,
    action: Action,
    reply: mpsc::SyncSender<Result<Value, String>>,
}
impl Command {
    fn current(&self) -> Result<(), String> {
        if EPOCH.load(Ordering::Acquire) != self.epoch || Instant::now() >= self.deadline {
            return Err(UNAVAILABLE.into());
        }
        Ok(())
    }
}
#[track_caller]
fn unavailable<T>(value: windows::core::Result<T>) -> Result<T, String> {
    // Provider errors can contain arbitrary data; never expose their description.
    let caller = std::panic::Location::caller();
    value.map_err(|error| {
        diagnostic(json!({"uia_hresult":error.code().0,"line":caller.line()}));
        UNAVAILABLE.to_owned()
    })
}
// Metadata only, in explicit synthetic probes/tests. Never pass field contents,
// selections, names or provider error descriptions to this helper.
fn diagnostic(metadata: Value) {
    if cfg!(test) {
        eprintln!("UIA {metadata}");
    }
    let args = std::env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--windows-input-probe") {
        if let Some(path) = args
            .iter()
            .position(|arg| arg == "--smoke-test")
            .and_then(|index| args.get(index + 1))
        {
            use std::io::Write;
            if let Ok(mut trace) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{path}.trace"))
            {
                let _ = writeln!(trace, "{metadata}");
            }
        }
    }
}
pub fn invalidate() -> u64 {
    let epoch = EPOCH.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    if let Some(Some(worker)) = WORKER.get() {
        let (reply, _) = mpsc::sync_channel(1);
        let _ = worker.try_send(Command {
            epoch,
            deadline: Instant::now() + BUDGET,
            action: Action::Clear,
            reply,
        });
    }
    epoch
}
fn request(epoch: u64, action: Action) -> Result<Value, String> {
    let worker = WORKER
        .get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<Command>(4);
            std::thread::Builder::new()
                .name("vao-input-uia".into())
                .spawn(move || {
                    unsafe {
                        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                            return;
                        }
                        struct Apartment;
                        impl Drop for Apartment {
                            fn drop(&mut self) {
                                unsafe {
                                    CoUninitialize();
                                }
                            }
                        }
                        let _apartment = Apartment;
                        let Ok(client) = CoCreateInstance::<_, IUIAutomation2>(
                            &CUIAutomation8,
                            None,
                            CLSCTX_INPROC_SERVER,
                        ) else {
                            return;
                        };
                        if client.SetConnectionTimeout(750).is_err()
                            || client.SetTransactionTimeout(1_000).is_err()
                        {
                            return;
                        }
                        let Ok(automation) = client.cast::<IUIAutomation>() else {
                            return;
                        };
                        let mut target: Option<Target> = None;
                        while let Ok(command) = rx.recv() {
                            if target
                                .as_ref()
                                .is_some_and(|t| t.epoch != EPOCH.load(Ordering::Acquire))
                            {
                                target = None;
                            }
                            let result = command.current().and_then(|_| match &command.action {
                                Action::Clear => {
                                    target = None;
                                    Ok(json!({"ok":true}))
                                }
                                Action::Capture { root, pid } => {
                                    target = None;
                                    Target::capture(&automation, *root, *pid, &command).map(
                                        |saved| {
                                            let result = saved.result();
                                            target = Some(saved);
                                            result
                                        },
                                    )
                                }
                                Action::Deliver { request, write } => {
                                    if target.as_ref().is_none_or(|saved| {
                                        request["target"]["input_token"].as_str()
                                            != Some(saved.token.as_str())
                                    }) {
                                        Err(UNAVAILABLE.into())
                                    } else {
                                        // Consume before restoring focus or writing. Never retry a possible partial write.
                                        target.take().unwrap().deliver(
                                            &automation,
                                            request,
                                            *write,
                                            &command,
                                        )
                                    }
                                }
                            });
                            let _ = command.reply.send(result);
                        }
                    }
                })
                .ok()
                .map(|_| tx)
        })
        .as_ref()
        .ok_or(UNAVAILABLE)?;
    let (reply, rx) = mpsc::sync_channel(1);
    worker
        .try_send(Command {
            epoch,
            deadline: Instant::now() + BUDGET,
            action,
            reply,
        })
        .map_err(|_| UNAVAILABLE)?;
    rx.recv_timeout(BUDGET + Duration::from_millis(100))
        .map_err(|_| UNAVAILABLE)?
}
pub fn capture(root: HWND, pid: u32, epoch: u64) -> Result<Value, String> {
    diagnostic(json!({"uia_stage":"request_capture"}));
    request(
        epoch,
        Action::Capture {
            root: root as usize,
            pid,
        },
    )
}
pub fn deliver(request_value: Value, write: bool) -> Result<Value, String> {
    request(
        EPOCH.load(Ordering::Acquire),
        Action::Deliver {
            request: request_value,
            write,
        },
    )
}

#[derive(PartialEq, Eq)]
struct Snapshot {
    text: Vec<u16>,
    start: usize,
    end: usize,
}
struct Target {
    epoch: u64,
    token: String,
    root: usize,
    pid: u32,
    created: u64,
    element_pid: u32,
    element_created: u64,
    element: IUIAutomationElement,
    snapshot: Snapshot,
}
fn canonical(text: &[u16]) -> Vec<u16> {
    let mut result = Vec::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let ch = text[index];
        result.push(if ch == 13 { 10 } else { ch });
        if ch == 13 && text.get(index + 1) == Some(&10) {
            index += 1;
        }
        index += 1;
    }
    result
}
fn document_matches(document: &[u16], value: &[u16]) -> bool {
    document == value
        || (document.len() == value.len() + 1
            && document.starts_with(value)
            && document.last() == Some(&10))
}
unsafe fn patterns(
    element: &IUIAutomationElement,
) -> Result<(IUIAutomationValuePattern, IUIAutomationTextPattern), String> {
    if unavailable(element.CurrentControlType())? != UIA_EditControlTypeId
        || unavailable(element.CurrentIsPassword())?.as_bool()
        || !unavailable(element.CurrentIsEnabled())?.as_bool()
        || !unavailable(element.CurrentIsKeyboardFocusable())?.as_bool()
    {
        return Err(UNAVAILABLE.into());
    }
    let value: IUIAutomationValuePattern =
        unavailable(element.GetCurrentPatternAs(UIA_ValuePatternId))?;
    let text: IUIAutomationTextPattern =
        unavailable(element.GetCurrentPatternAs(UIA_TextPatternId))?;
    let readonly = unavailable(value.CurrentIsReadOnly())?.as_bool();
    let selection = unavailable(text.SupportedTextSelection())?;
    if readonly || selection != SupportedTextSelection_Single {
        diagnostic(
            json!({"uia_stage":"patterns_rejected","readonly":readonly,"selection":selection.0}),
        );
        return Err(UNAVAILABLE.into());
    }
    Ok((value, text))
}
unsafe fn value(pattern: &IUIAutomationValuePattern) -> Result<Vec<u16>, String> {
    let value = unavailable(pattern.CurrentValue())?;
    diagnostic(json!({"uia_stage":"value_length","utf16_units":value.len()}));
    if value.len() > LIMIT {
        return Err(UNAVAILABLE.into());
    }
    let text = canonical(&value);
    String::from_utf16(&text).map_err(|_| UNAVAILABLE)?;
    if text.contains(&0) {
        return Err(UNAVAILABLE.into());
    }
    Ok(text)
}
unsafe fn range_text(range: &IUIAutomationTextRange) -> Result<Vec<u16>, String> {
    let text = unavailable(range.GetText((LIMIT + 2) as i32))?;
    if text.len() > LIMIT + 1 {
        return Err(UNAVAILABLE.into());
    }
    Ok(text.to_vec())
}
unsafe fn range_owned_by(
    automation: &IUIAutomation,
    range: &IUIAutomationTextRange,
    element: &IUIAutomationElement,
    command: &Command,
) -> Result<bool, String> {
    let walker = unavailable(automation.RawViewWalker())?;
    let mut enclosing = unavailable(range.GetEnclosingElement())?;
    for _ in 0..16 {
        command.current()?;
        if unavailable(automation.CompareElements(&enclosing, element))?.as_bool() {
            return Ok(true);
        }
        match walker.GetParentElement(&enclosing) {
            Ok(parent) => enclosing = parent,
            Err(_) => return Ok(false),
        }
    }
    Ok(false)
}
unsafe fn snapshot(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    command: &Command,
) -> Result<Snapshot, String> {
    diagnostic(json!({"uia_stage":"snapshot_patterns"}));
    let (value_pattern, text_pattern) = patterns(element)?;
    let text = value(&value_pattern)?;
    let document = unavailable(text_pattern.DocumentRange())?;
    let document_text = canonical(&range_text(&document)?);
    if !document_matches(&document_text, &text) {
        diagnostic(json!({"uia_stage":"snapshot_document_mismatch"}));
        return Err(UNAVAILABLE.into());
    }
    let selection = unavailable(text_pattern.GetSelection())?;
    let count = unavailable(selection.Length())?;
    if count != 1 {
        diagnostic(json!({"uia_stage":"selection_count_rejected","count":count}));
        return Err(UNAVAILABLE.into());
    }
    let selection = unavailable(selection.GetElement(0))?;
    let from_start = unavailable(selection.CompareEndpoints(
        TextPatternRangeEndpoint_Start,
        &document,
        TextPatternRangeEndpoint_Start,
    ))?;
    let to_end = unavailable(selection.CompareEndpoints(
        TextPatternRangeEndpoint_End,
        &document,
        TextPatternRangeEndpoint_End,
    ))?;
    let order = unavailable(selection.CompareEndpoints(
        TextPatternRangeEndpoint_Start,
        &selection,
        TextPatternRangeEndpoint_End,
    ))?;
    // Some providers anchor a collapsed end caret on the Edit container, while
    // DocumentRange ends on its last text child. CompareEndpoints then places
    // that caret after DocumentRange. Accept this representation only if the
    // range still belongs to this exact Edit and both prefixes below prove the
    // complete field boundary; a selection in another field remains invalid.
    let end_caret = from_start >= 0
        && to_end > 0
        && order == 0
        && range_text(&selection)?.is_empty()
        && range_owned_by(automation, &selection, element, command)?;
    if from_start < 0 || (to_end > 0 && !end_caret) || order > 0 {
        diagnostic(
            json!({"uia_stage":"selection_bounds_rejected","from_start":from_start,"to_end":to_end,"order":order}),
        );
        return Err(UNAVAILABLE.into());
    }
    let prefix = unavailable(document.Clone())?;
    unavailable(prefix.MoveEndpointByRange(
        TextPatternRangeEndpoint_End,
        &selection,
        TextPatternRangeEndpoint_Start,
    ))?;
    let before = canonical(&range_text(&prefix)?);
    let prefix_end = unavailable(document.Clone())?;
    unavailable(prefix_end.MoveEndpointByRange(
        TextPatternRangeEndpoint_End,
        &selection,
        TextPatternRangeEndpoint_End,
    ))?;
    let through = canonical(&range_text(&prefix_end)?);
    let valid_prefixes = if end_caret {
        diagnostic(json!({"uia_stage":"owned_end_caret"}));
        document_matches(&before, &text) && document_matches(&through, &text)
    } else {
        document_text.starts_with(&before)
            && document_text.starts_with(&through)
            && before.len() <= through.len()
    };
    if !valid_prefixes {
        diagnostic(
            json!({"uia_stage":"selection_prefix_rejected","before":before.len(),"through":through.len(),"document":document_text.len(),"value":text.len()}),
        );
        return Err(UNAVAILABLE.into());
    }
    let start = before.len().min(text.len());
    let end = through.len().min(text.len());
    String::from_utf16(&text[..start]).map_err(|_| UNAVAILABLE)?;
    String::from_utf16(&text[start..end]).map_err(|_| UNAVAILABLE)?;
    String::from_utf16(&text[end..]).map_err(|_| UNAVAILABLE)?;
    // Reject inconsistent reads if the provider or user changed the field mid-capture.
    if value(&value_pattern)? != text {
        diagnostic(json!({"uia_stage":"snapshot_value_changed"}));
        return Err(UNAVAILABLE.into());
    }
    Ok(Snapshot { text, start, end })
}
unsafe fn within(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    root: usize,
    command: &Command,
) -> Result<(), String> {
    let parent = unavailable(automation.ElementFromHandle(ComHwnd(root as *mut _)))?;
    let walker = unavailable(automation.RawViewWalker())?;
    let mut node = element.clone();
    for _ in 0..64 {
        command.current()?;
        if unavailable(automation.CompareElements(&node, &parent))?.as_bool() {
            return Ok(());
        }
        node = unavailable(walker.GetParentElement(&node))?;
    }
    Err(UNAVAILABLE.into())
}
impl Target {
    unsafe fn capture(
        automation: &IUIAutomation,
        root: usize,
        pid: u32,
        command: &Command,
    ) -> Result<Self, String> {
        diagnostic(json!({"uia_stage":"capture_worker"}));
        let created = process_created(pid)?;
        // Chromium may initialize accessibility when first queried. Only accept an
        // actually focused Edit; never guess a field by enumerating other editors.
        for _ in 0..3 {
            command.current()?;
            if GetForegroundWindow() != root as HWND {
                diagnostic(json!({"uia_stage":"capture_foreground_changed"}));
                return Err(UNAVAILABLE.into());
            }
            let element = unavailable(automation.GetFocusedElement())?;
            diagnostic(
                json!({"uia_stage":"focused_element","control_type":unavailable(element.CurrentControlType())?.0,"focused":unavailable(element.CurrentHasKeyboardFocus())?.as_bool(),"pid":unavailable(element.CurrentProcessId())?}),
            );
            if unavailable(element.CurrentHasKeyboardFocus())?.as_bool() {
                within(automation, &element, root, command)?;
                if let Ok(snapshot) = snapshot(automation, &element, command) {
                    let element_pid = unavailable(element.CurrentProcessId())? as u32;
                    let saved = Self {
                        epoch: command.epoch,
                        token: nonce()?,
                        root,
                        pid,
                        created,
                        element_pid,
                        element_created: process_created(element_pid)?,
                        element,
                        snapshot,
                    };
                    saved.verify_focused(automation, command)?;
                    return Ok(saved);
                }
            }
            let _root = unavailable(automation.ElementFromHandle(ComHwnd(root as *mut _)))?;
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(UNAVAILABLE.into())
    }
    fn result(&self) -> Value {
        json!({"input_token":self.token,"input_adapter":"uia_value","delivery_supported":true,
            "selection":String::from_utf16_lossy(&self.snapshot.text[self.snapshot.start..self.snapshot.end]),
            "length":self.snapshot.end-self.snapshot.start})
    }
    unsafe fn identity(&self, automation: &IUIAutomation, command: &Command) -> Result<(), String> {
        command.current()?;
        let mut pid = 0;
        GetWindowThreadProcessId(self.root as HWND, &mut pid);
        if IsWindow(self.root as HWND) == 0
            || pid != self.pid
            || process_created(pid)? != self.created
            || unavailable(self.element.CurrentProcessId())? as u32 != self.element_pid
            || process_created(self.element_pid)? != self.element_created
        {
            return Err(UNAVAILABLE.into());
        }
        within(automation, &self.element, self.root, command)
    }
    unsafe fn verify_focused(
        &self,
        automation: &IUIAutomation,
        command: &Command,
    ) -> Result<(), String> {
        self.verify_focus_identity(automation, command)?;
        if snapshot(automation, &self.element, command)? != self.snapshot {
            return Err(UNAVAILABLE.into());
        }
        command.current()
    }
    unsafe fn verify_focus_identity(
        &self,
        automation: &IUIAutomation,
        command: &Command,
    ) -> Result<(), String> {
        self.identity(automation, command)?;
        if GetForegroundWindow() != self.root as HWND
            || !unavailable(self.element.CurrentHasKeyboardFocus())?.as_bool()
            || !unavailable(
                automation
                    .CompareElements(&unavailable(automation.GetFocusedElement())?, &self.element),
            )?
            .as_bool()
        {
            return Err(UNAVAILABLE.into());
        }
        command.current()
    }
    unsafe fn deliver(
        self,
        automation: &IUIAutomation,
        request: &Value,
        write: bool,
        command: &Command,
    ) -> Result<Value, String> {
        let inserted: Vec<u16> = request["text"]
            .as_str()
            .unwrap_or("")
            .encode_utf16()
            .collect();
        if write && (inserted.is_empty() || inserted.contains(&0) || inserted.len() > LIMIT) {
            return Err(UNAVAILABLE.into());
        }
        self.identity(automation, command)?;
        let (value_pattern, _) = patterns(&self.element)?;
        if value(&value_pattern)? != self.snapshot.text {
            return Err(UNAVAILABLE.into());
        }
        if IsIconic(self.root as HWND) != 0 {
            ShowWindow(self.root as HWND, SW_RESTORE);
        }
        command.current()?;
        if GetForegroundWindow() != self.root as HWND && SetForegroundWindow(self.root as HWND) == 0
        {
            return Err(UNAVAILABLE.into());
        }
        let activate_deadline = Instant::now() + Duration::from_millis(500);
        loop {
            command.current()?;
            if GetForegroundWindow() == self.root as HWND
                && unavailable(self.element.CurrentHasKeyboardFocus())?.as_bool()
                && unavailable(
                    automation.CompareElements(
                        &unavailable(automation.GetFocusedElement())?,
                        &self.element,
                    ),
                )?
                .as_bool()
            {
                break;
            }
            if Instant::now() >= activate_deadline {
                return Err(UNAVAILABLE.into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.verify_focused(automation, command)?;
        if !write {
            return Ok(json!({"ok":true}));
        }
        let inserted = canonical(&inserted);
        // ValuePattern has no portable multiline capability flag. A line break
        // could be silently stripped by a single-line editor. Until a richer
        // provider contract is established, preserve multiline results for copy.
        if inserted.contains(&10) {
            return Err(UNAVAILABLE.into());
        }
        let expected = [
            &self.snapshot.text[..self.snapshot.start],
            &inserted,
            &self.snapshot.text[self.snapshot.end..],
        ]
        .concat();
        if expected.len() > LIMIT {
            return Err(UNAVAILABLE.into());
        }
        let expected_string = String::from_utf16(&expected).map_err(|_| UNAVAILABLE)?;
        self.verify_focused(automation, command)?;
        // Plain ValuePattern Edit controls only: direct provider operation, no
        // paste shortcut/Enter and no document formatting replacement fallback.
        diagnostic(json!({"uia_stage":"set_value_once"}));
        unavailable(value_pattern.SetValue(&BSTR::from(expected_string.as_str())))
            .map_err(|_| RECEIPT)?;
        let caret = self.snapshot.start + inserted.len();
        let receipt: Result<(), String> = (|| {
            // Providers such as Chromium dispatch the action to another process.
            // S_OK acknowledges dispatch, not a refreshed accessibility tree.
            // Poll bounded reads; never repeat SetValue or Select on a timeout.
            diagnostic(json!({"uia_stage":"await_value"}));
            let value_deadline = Instant::now() + Duration::from_millis(750);
            loop {
                self.verify_focus_identity(automation, command)?;
                let received = value(&value_pattern)?;
                if received == expected {
                    break;
                }
                if received != self.snapshot.text || Instant::now() >= value_deadline {
                    return Err(RECEIPT.into());
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            self.verify_focus_identity(automation, command)?;
            diagnostic(json!({"uia_stage":"select_caret_once"}));
            select_caret(&self.element, &expected, caret)?;
            diagnostic(json!({"uia_stage":"await_caret_receipt"}));
            let caret_deadline = Instant::now() + Duration::from_millis(750);
            loop {
                self.verify_focus_identity(automation, command)?;
                if let Ok(received) = snapshot(automation, &self.element, command) {
                    if received.text != expected {
                        return Err(RECEIPT.into());
                    }
                    if received.start == caret && received.end == caret {
                        return Ok(());
                    }
                }
                if Instant::now() >= caret_deadline {
                    return Err(RECEIPT.into());
                }
                std::thread::sleep(Duration::from_millis(15));
            }
        })();
        receipt.map_err(|_| RECEIPT)?;
        Ok(json!({"ok":true,"verified":true,"input_adapter":"uia_value"}))
    }
}
fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| UNAVAILABLE)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
// Translate a canonical UTF-16 prefix length back to the provider's CR/CRLF
// representation. Avoid TextUnit_Character offsets, which vary for emoji/marks.
fn raw_prefix_length(raw: &[u16], wanted: usize) -> Option<usize> {
    let (mut raw_index, mut count) = (0, 0);
    while count < wanted {
        let ch = *raw.get(raw_index)?;
        raw_index += 1;
        if ch == 13 && raw.get(raw_index) == Some(&10) {
            raw_index += 1;
        }
        count += 1;
    }
    Some(raw_index)
}
unsafe fn select_caret(
    element: &IUIAutomationElement,
    expected: &[u16],
    caret: usize,
) -> Result<(), String> {
    let (_, text) = patterns(element)?;
    let document = unavailable(text.DocumentRange())?;
    let raw = range_text(&document)?;
    if !document_matches(&canonical(&raw), expected) {
        return Err(RECEIPT.into());
    }
    let prefix_length = raw_prefix_length(&raw, caret).ok_or(RECEIPT)?;
    let range = if prefix_length == 0 {
        let range = unavailable(document.Clone())?;
        unavailable(range.MoveEndpointByRange(
            TextPatternRangeEndpoint_End,
            &document,
            TextPatternRangeEndpoint_Start,
        ))?;
        range
    } else {
        let prefix = String::from_utf16(&raw[..prefix_length]).map_err(|_| RECEIPT)?;
        let range = unavailable(document.FindText(&BSTR::from(prefix.as_str()), false, false))?;
        if unavailable(range.CompareEndpoints(
            TextPatternRangeEndpoint_Start,
            &document,
            TextPatternRangeEndpoint_Start,
        ))? != 0
            || range_text(&range)? != raw[..prefix_length]
        {
            return Err(RECEIPT.into());
        }
        unavailable(range.MoveEndpointByRange(
            TextPatternRangeEndpoint_Start,
            &range,
            TextPatternRangeEndpoint_End,
        ))?;
        range
    };
    unavailable(range.Select())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_newlines_and_utf16_prefixes_are_exact() {
        let raw: Vec<u16> = "前😀e\u{301}\r\n后\r尾".encode_utf16().collect();
        let text = canonical(&raw);
        assert_eq!(String::from_utf16(&text).unwrap(), "前😀e\u{301}\n后\n尾");
        for index in 0..=text.len() {
            let raw_index = raw_prefix_length(&raw, index).unwrap();
            assert_eq!(canonical(&raw[..raw_index]), text[..index]);
        }
        assert!(raw_prefix_length(&raw, text.len() + 1).is_none());
        assert!(document_matches(&[1, 2, 10], &[1, 2]));
        assert!(!document_matches(&[1, 2, 10, 10], &[1, 2]));
    }
}
