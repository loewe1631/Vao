// Independent, explicitly armed synthetic input experiment. Not a Vao fallback.
// Interactive mode never reads field text, names, titles, clipboard or audio.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <objbase.h>
#include <UIAutomation.h>
#include <wrl/client.h>
#include <algorithm>
#include <chrono>
#include <cwctype>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#ifndef VAO_LAB_COMMIT
#define VAO_LAB_COMMIT "uncommitted"
#endif

using Microsoft::WRL::ComPtr;
constexpr wchar_t kText[] = L"Vao \u901a\u7528\u8f93\u5165\u6d4b\u8bd5 \U0001f642 A7";
constexpr UINT kFocus = WM_APP + 1;
constexpr UINT kState = WM_APP + 2;
constexpr UINT kEnterCount = WM_APP + 3;
constexpr UINT kRestoreMode = WM_APP + 4;
constexpr wchar_t kFixture[] = L"VaoSyntheticInputFixture";

std::string utf8(const std::wstring& value) {
    if (value.empty()) return {};
    const int n = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(),
        static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
    if (!n) return "invalid_utf16";
    std::string result(static_cast<size_t>(n), '\0');
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(),
        static_cast<int>(value.size()), result.data(), n, nullptr, nullptr);
    return result;
}
std::string quoted(const std::string& value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char c : value) {
        if (c == '"' || c == '\\') out << '\\' << static_cast<char>(c);
        else if (c >= 32) out << static_cast<char>(c);
        else out << '?';
    }
    out << '"';
    return out.str();
}
std::wstring window_class(HWND window) {
    wchar_t buffer[256]{};
    const int n = GetClassNameW(window, buffer, 256);
    return {buffer, static_cast<size_t>(std::max(n, 0))};
}
ULONGLONG created(DWORD pid) {
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) throw std::runtime_error("process_unavailable");
    FILETIME start{}, end{}, kernel{}, user{};
    const BOOL ok = GetProcessTimes(process, &start, &end, &kernel, &user);
    CloseHandle(process);
    if (!ok) throw std::runtime_error("process_identity_unavailable");
    return (static_cast<ULONGLONG>(start.dwHighDateTime) << 32) | start.dwLowDateTime;
}
DWORD integrity(DWORD pid) {
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) throw std::runtime_error("integrity_process_unavailable");
    HANDLE token = nullptr;
    const BOOL opened = OpenProcessToken(process, TOKEN_QUERY, &token);
    CloseHandle(process);
    if (!opened) throw std::runtime_error("integrity_token_unavailable");
    DWORD size = 0;
    GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &size);
    std::vector<BYTE> data(size);
    const BOOL ok = GetTokenInformation(token, TokenIntegrityLevel, data.data(), size, &size);
    CloseHandle(token);
    if (!ok) throw std::runtime_error("integrity_level_unavailable");
    const PSID sid = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(data.data())->Label.Sid;
    return *GetSidSubAuthority(sid, static_cast<DWORD>(*GetSidSubAuthorityCount(sid) - 1));
}
std::wstring process_name(DWORD pid) {
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) return {};
    wchar_t buffer[32768]{};
    DWORD size = 32768;
    const BOOL ok = QueryFullProcessImageNameW(process, 0, buffer, &size);
    CloseHandle(process);
    return ok ? std::filesystem::path(std::wstring(buffer, size)).filename().wstring() : L"";
}
GUITHREADINFO gui_state(HWND root) {
    GUITHREADINFO info{};
    info.cbSize = sizeof(info);
    const DWORD thread = GetWindowThreadProcessId(root, nullptr);
    if (!thread || !GetGUIThreadInfo(thread, &info))
        throw std::runtime_error("native_focus_query_failed_" + std::to_string(GetLastError()));
    if (info.hwndFocus && GetAncestor(info.hwndFocus, GA_ROOT) != root)
        throw std::runtime_error("focus_outside_root");
    if (info.flags & (GUI_INMENUMODE | GUI_INMOVESIZE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE))
        throw std::runtime_error("menu_or_move_active");
    return info;
}
GUITHREADINFO gui(HWND root) {
    const auto info = gui_state(root);
    if (!info.hwndFocus) throw std::runtime_error("native_focus_null");
    return info;
}
bool keys_up() {
    // Do not release, reset or synthesize the user's physical keys.
    for (int key = 1; key < 255; ++key)
        if (GetAsyncKeyState(key) & 0x8000) return false;
    return true;
}
bool protected_native(HWND field) {
    auto name = window_class(field);
    std::transform(name.begin(), name.end(), name.begin(), [](wchar_t ch) {
        return static_cast<wchar_t>(std::towlower(ch));
    });
    if (name == L"edit" || name.find(L"richedit") == 0) {
        const LONG_PTR style = GetWindowLongPtrW(field, GWL_STYLE);
        return (style & (ES_PASSWORD | ES_READONLY)) != 0;
    }
    return false;
}

struct Target {
    HWND root = nullptr;
    DWORD pid = 0;
    ULONGLONG process_created = 0;
    GUITHREADINFO focus{};
    std::wstring root_class, field_class, app;
    ComPtr<IUIAutomationElement> element;
    bool uia_focused = false, uia_text = false, uia_value = false;
    CONTROLTYPEID control_type = 0;
    std::chrono::steady_clock::time_point time;
};
Target capture(IUIAutomation* automation) {
    Target target;
    target.root = GetForegroundWindow();
    if (!target.root || target.root == GetShellWindow() || target.root == GetDesktopWindow())
        throw std::runtime_error("no_target_window");
    GetWindowThreadProcessId(target.root, &target.pid);
    target.process_created = created(target.pid);
    if (integrity(target.pid) > integrity(GetCurrentProcessId()))
        throw std::runtime_error("higher_integrity_target_rejected");
    target.focus = gui(target.root);
    if (!IsWindowEnabled(target.focus.hwndFocus) || protected_native(target.focus.hwndFocus))
        throw std::runtime_error("disabled_or_protected_native_field");
    target.root_class = window_class(target.root);
    target.field_class = window_class(target.focus.hwndFocus);
    target.app = process_name(target.pid);
    // Capability/identity metadata only. Never call CurrentValue, GetText or Name.
    if (automation) {
        if (FAILED(automation->GetFocusedElement(&target.element)) || !target.element)
            throw std::runtime_error("uia_focused_element_unavailable");
        BOOL password = FALSE, enabled = FALSE, focused = FALSE;
        if (FAILED(target.element->get_CurrentIsPassword(&password)) || password ||
            FAILED(target.element->get_CurrentIsEnabled(&enabled)) || !enabled)
            throw std::runtime_error("protected_or_unavailable_uia_element");
        if (SUCCEEDED(target.element->get_CurrentHasKeyboardFocus(&focused)))
            target.uia_focused = focused != FALSE;
        target.element->get_CurrentControlType(&target.control_type);
        ComPtr<IUIAutomationTextPattern> text;
        // UIA can return S_OK with a null interface for an unsupported pattern.
        // HRESULT alone is neither a capability check nor permission to dereference.
        target.uia_text = SUCCEEDED(target.element->GetCurrentPatternAs(UIA_TextPatternId, IID_PPV_ARGS(&text))) && text;
        ComPtr<IUIAutomationValuePattern> value;
        target.uia_value = SUCCEEDED(target.element->GetCurrentPatternAs(UIA_ValuePatternId, IID_PPV_ARGS(&value))) && value;
        if (target.uia_value) {
            BOOL readonly = TRUE;
            if (FAILED(value->get_CurrentIsReadOnly(&readonly)) || readonly)
                throw std::runtime_error("readonly_uia_element");
        }
    }
    target.time = std::chrono::steady_clock::now();
    if (GetForegroundWindow() != target.root || gui(target.root).hwndFocus != target.focus.hwndFocus)
        throw std::runtime_error("focus_changed_during_capture");
    return target;
}
void identity(const Target& target) {
    DWORD pid = 0;
    GetWindowThreadProcessId(target.root, &pid);
    if (!IsWindow(target.root) || pid != target.pid || created(pid) != target.process_created ||
        window_class(target.root) != target.root_class || !IsWindow(target.focus.hwndFocus) ||
        window_class(target.focus.hwndFocus) != target.field_class ||
        !IsWindowEnabled(target.focus.hwndFocus) || protected_native(target.focus.hwndFocus))
        throw std::runtime_error("target_invalidated");
    if (integrity(pid) > integrity(GetCurrentProcessId()))
        throw std::runtime_error("target_integrity_changed");
    if (std::chrono::steady_clock::now() - target.time > std::chrono::seconds(60))
        throw std::runtime_error("target_expired");
}
void verify_focus(const Target& target, IUIAutomation* automation) {
    identity(target);
    const auto now = gui(target.root);
    if (now.hwndFocus != target.focus.hwndFocus)
        throw std::runtime_error("native_focused_field_changed");
    if (target.focus.hwndCaret && (now.hwndCaret != target.focus.hwndCaret ||
        !EqualRect(&now.rcCaret, &target.focus.rcCaret)))
        throw std::runtime_error("native_caret_changed");
    if (automation && target.element && target.uia_focused) {
        ComPtr<IUIAutomationElement> current;
        BOOL same = FALSE, password = TRUE, enabled = FALSE;
        if (FAILED(automation->GetFocusedElement(&current)) || !current ||
            FAILED(automation->CompareElements(target.element.Get(), current.Get(), &same)) || !same ||
            FAILED(current->get_CurrentIsPassword(&password)) || password ||
            FAILED(current->get_CurrentIsEnabled(&enabled)) || !enabled)
            throw std::runtime_error("uia_focused_element_changed");
    }
}
struct RestoreTrace {
    ULONGLONG elapsed_ms = 0;
    unsigned null_focus_samples = 0;
    bool uia_request_attempted = false;
    HRESULT uia_request_result = E_PENDING;
    bool original_focus_confirmed = false;
    std::string stage = "not_started";
};
std::string restore_metadata(const RestoreTrace& trace) {
    std::ostringstream out;
    out << "\"restore_stage\":" << quoted(trace.stage)
        << ",\"focus_wait_ms\":" << trace.elapsed_ms
        << ",\"null_focus_samples\":" << trace.null_focus_samples
        << ",\"uia_focus_request_attempted\":" << (trace.uia_request_attempted ? "true" : "false")
        << ",\"uia_focus_request_hresult\":" << trace.uia_request_result
        << ",\"original_focus_confirmed\":" << (trace.original_focus_confirmed ? "true" : "false");
    return out.str();
}
void await_original_focus(const Target& target, IUIAutomation* automation, RestoreTrace& trace) {
    const ULONGLONG start = GetTickCount64();
    trace.stage = "waiting_for_original_focus";
    for (;;) {
        trace.elapsed_ms = GetTickCount64() - start;
        identity(target);
        if (GetForegroundWindow() != target.root)
            throw std::runtime_error("foreground_changed_during_focus_restore");
        if (!keys_up()) throw std::runtime_error("physical_input_during_focus_restore");
        const auto now = gui_state(target.root);
        if (now.hwndFocus) {
            // Never wait out a known different field or replace its focus.
            if (now.hwndFocus != target.focus.hwndFocus)
                throw std::runtime_error("native_focused_field_changed");
            verify_focus(target, automation);
            trace.original_focus_confirmed = true;
            trace.stage = "original_focus_confirmed";
            return;
        }
        ++trace.null_focus_samples;
        if (trace.elapsed_ms >= 1500)
            throw std::runtime_error("native_focus_restore_timeout");
        // Some apps restore their child asynchronously; others need the
        // standard UIA focus request. Request only the retained original
        // element, once, while the original window has no native focus.
        if (trace.elapsed_ms >= 250 && !trace.uia_request_attempted &&
            automation && target.element && target.uia_focused) {
            BOOL password = TRUE, enabled = FALSE;
            if (FAILED(target.element->get_CurrentIsPassword(&password)) || password ||
                FAILED(target.element->get_CurrentIsEnabled(&enabled)) || !enabled)
                throw std::runtime_error("saved_uia_element_unavailable_for_focus");
            identity(target);
            if (GetForegroundWindow() != target.root || !keys_up())
                throw std::runtime_error("input_changed_before_focus_request");
            const auto before_request = gui_state(target.root);
            if (before_request.hwndFocus) continue;
            trace.uia_request_attempted = true;
            trace.uia_request_result = target.element->SetFocus();
        }
        Sleep(20);
    }
}
UINT submit(const Target& target, IUIAutomation* automation, RestoreTrace* diagnostics = nullptr) {
    RestoreTrace local;
    auto& trace = diagnostics ? *diagnostics : local;
    trace.stage = "validating_target";
    identity(target);
    if (!keys_up()) throw std::runtime_error("physical_key_held");
    // Never choose another child, attach input queues or click coordinates.
    GUITHREADINFO inactive{};
    inactive.cbSize = sizeof(inactive);
    if (!GetGUIThreadInfo(GetWindowThreadProcessId(target.root, nullptr), &inactive))
        throw std::runtime_error("saved_native_focus_query_failed");
    // Inactive threads can legitimately have no focus. A known different child
    // is a conflict; after activation the original child/caret must match.
    if (inactive.hwndFocus && inactive.hwndFocus != target.focus.hwndFocus)
        throw std::runtime_error("saved_native_focus_changed");
    trace.stage = "activating_original_window";
    if (IsIconic(target.root)) ShowWindow(target.root, SW_RESTORE);
    if (GetForegroundWindow() != target.root && !SetForegroundWindow(target.root))
        throw std::runtime_error("windows_denied_activation");
    for (int i = 0; i < 50 && GetForegroundWindow() != target.root; ++i) Sleep(10);
    if (GetForegroundWindow() != target.root) throw std::runtime_error("activation_unconfirmed");
    await_original_focus(target, automation, trace);
    std::vector<INPUT> input;
    for (const wchar_t ch : std::wstring(kText)) {
        INPUT down{};
        down.type = INPUT_KEYBOARD;
        down.ki.wScan = ch;
        down.ki.dwFlags = KEYEVENTF_UNICODE;
        input.push_back(down);
        down.ki.dwFlags |= KEYEVENTF_KEYUP;
        input.push_back(down);
    }
    if (!keys_up() || GetForegroundWindow() != target.root)
        throw std::runtime_error("input_changed_before_dispatch");
    verify_focus(target, automation);
    // One batch, no Enter, paste, shortcuts or retries. Return value is queue
    // acceptance only; the interactive report must never call it a receipt.
    trace.stage = "dispatching_once";
    const UINT sent = SendInput(static_cast<UINT>(input.size()), input.data(), sizeof(INPUT));
    trace.stage = "dispatched_once";
    return sent;
}
std::string metadata(const Target& target) {
    const char* level = target.uia_focused && target.uia_text ? "uia_element_without_text_snapshot" :
        target.focus.hwndCaret ? "win32_caret_geometry_only" : "native_window_only";
    std::ostringstream out;
    out << "\"app\":" << quoted(utf8(target.app)) << ",\"pid\":" << target.pid
        << ",\"root_class\":" << quoted(utf8(target.root_class))
        << ",\"focus_class\":" << quoted(utf8(target.field_class))
        << ",\"uia_control_type\":" << target.control_type
        << ",\"uia_focused\":" << (target.uia_focused ? "true" : "false")
        << ",\"uia_text_pattern\":" << (target.uia_text ? "true" : "false")
        << ",\"uia_value_pattern\":" << (target.uia_value ? "true" : "false")
        << ",\"has_native_caret\":" << (target.focus.hwndCaret ? "true" : "false")
        << ",\"identity_level\":\"" << level << "\",\"content_conflict_protection\":false";
    return out.str();
}
void write_report(const std::filesystem::path& path, const std::string& data) {
    if (path.has_parent_path()) std::filesystem::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out << data << '\n';
    if (!out) throw std::runtime_error("report_write_failed");
}

// Synthetic child host is the only place this executable ever reads text.
WNDPROC edit_proc = nullptr;
LONG enter_count = 0;
DWORD fixture_controller = 0;
DWORD fixture_focus_delay = 0;
int fixture_restore_id = 0;
void restore_fixture_child(HWND hwnd) {
    HWND last = fixture_restore_id ? GetDlgItem(hwnd, fixture_restore_id) :
        reinterpret_cast<HWND>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    if (!last) last = GetDlgItem(hwnd, 101);
    if (last && IsWindowEnabled(last)) SetFocus(last);
}
LRESULT CALLBACK fixture_edit(HWND hwnd, UINT msg, WPARAM w, LPARAM l) {
    if (msg == WM_SETFOCUS)
        SetWindowLongPtrW(GetParent(hwnd), GWLP_USERDATA, reinterpret_cast<LONG_PTR>(hwnd));
    if ((msg == WM_CHAR && (w == '\r' || w == '\n')) || (msg == WM_KEYDOWN && w == VK_RETURN))
        ++enter_count;
    return CallWindowProcW(edit_proc, hwnd, msg, w, l);
}
LRESULT CALLBACK fixture_window(HWND hwnd, UINT msg, WPARAM w, LPARAM l) {
    if (msg == WM_ACTIVATE && LOWORD(w) != WA_INACTIVE && fixture_controller)
        AllowSetForegroundWindow(fixture_controller);
    if (msg == WM_SETFOCUS) {
        if (fixture_focus_delay) {
            // Model an app whose foreground window temporarily has no child
            // focus. INFINITE models an app awaiting an explicit focus request.
            SetFocus(nullptr);
            if (fixture_focus_delay != INFINITE) SetTimer(hwnd, 1, fixture_focus_delay, nullptr);
        } else restore_fixture_child(hwnd);
        return 0;
    }
    if (msg == WM_TIMER && w == 1) {
        KillTimer(hwnd, 1);
        if (GetForegroundWindow() == hwnd) restore_fixture_child(hwnd);
        return 0;
    }
    if (msg == WM_CREATE) {
        for (int i = 0; i < 2; ++i) {
            HWND edit = CreateWindowExW(0, L"EDIT", L"", WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_MULTILINE,
                10, 10 + i * 80, 400, 65, hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(101 + i)),
                GetModuleHandleW(nullptr), nullptr);
            const auto previous = reinterpret_cast<WNDPROC>(SetWindowLongPtrW(edit, GWLP_WNDPROC,
                reinterpret_cast<LONG_PTR>(fixture_edit)));
            if (!edit_proc) edit_proc = previous;
        }
        // An owned UIA element with no Value/Text patterns reproduces the
        // interactive crash that native-only receipt tests could not exercise.
        CreateWindowExW(0, L"BUTTON", L"Synthetic pattern-less control", WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10, 175, 400, 25, hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(103)),
            GetModuleHandleW(nullptr), nullptr);
        // Native multiline Edit exposes TextPattern; use a single-line Edit
        // for the positive ValuePattern regression rather than assuming both.
        HWND single = CreateWindowExW(0, L"EDIT", L"", WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10, 205, 400, 25, hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(104)),
            GetModuleHandleW(nullptr), nullptr);
        SetWindowLongPtrW(single, GWLP_WNDPROC, reinterpret_cast<LONG_PTR>(fixture_edit));
        return 0;
    }
    if (msg == kFocus) { SetFocus(GetDlgItem(hwnd, static_cast<int>(w))); return 1; }
    if (msg == kRestoreMode) {
        fixture_focus_delay = static_cast<DWORD>(w);
        fixture_restore_id = static_cast<int>(l);
        KillTimer(hwnd, 1);
        return 1;
    }
    if (msg == kState) {
        HWND field = GetDlgItem(hwnd, 101);
        SetWindowLongPtrW(field, GWL_STYLE, GetWindowLongPtrW(field, GWL_STYLE) | ES_PASSWORD);
        return 1;
    }
    if (msg == kEnterCount) return enter_count;
    if (msg == WM_DESTROY) { PostQuitMessage(0); return 0; }
    return DefWindowProcW(hwnd, msg, w, l);
}
int fixture(DWORD controller) {
    fixture_controller = controller;
    WNDCLASSW cls{};
    cls.lpfnWndProc = fixture_window;
    cls.hInstance = GetModuleHandleW(nullptr);
    cls.lpszClassName = kFixture;
    if (!RegisterClassW(&cls)) return 2;
    HWND root = CreateWindowExW(0, kFixture, L"Synthetic scratch input", WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        100, 100, 460, 290, nullptr, nullptr, cls.hInstance, nullptr);
    if (!root) return 3;
    MSG msg{};
    while (GetMessageW(&msg, nullptr, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
    return 0;
}
struct Child {
    PROCESS_INFORMATION process{};
    HWND root = nullptr;
    Child() {
        wchar_t executable[32768]{};
        GetModuleFileNameW(nullptr, executable, 32768);
        std::wstring cmd = L"\"" + std::wstring(executable) + L"\" --fixture " + std::to_wstring(GetCurrentProcessId());
        STARTUPINFOW info{};
        info.cb = sizeof(info);
        if (!CreateProcessW(nullptr, cmd.data(), nullptr, nullptr, FALSE, 0, nullptr, nullptr, &info, &process))
            throw std::runtime_error("fixture_start_failed");
        // Console-subsystem children can return from WaitForInputIdle before
        // their GUI queue exists. Wait for our actual child window instead.
        for (int attempt = 0; attempt < 100 && !root; ++attempt) {
            EnumWindows([](HWND hwnd, LPARAM data) -> BOOL {
                auto self = reinterpret_cast<Child*>(data);
                DWORD pid = 0;
                GetWindowThreadProcessId(hwnd, &pid);
                if (pid == self->process.dwProcessId && window_class(hwnd) == kFixture) {
                    self->root = hwnd; return FALSE;
                }
                return TRUE;
            }, reinterpret_cast<LPARAM>(this));
            DWORD code = STILL_ACTIVE;
            GetExitCodeProcess(process.hProcess, &code);
            if (code != STILL_ACTIVE) {
                cleanup();
                throw std::runtime_error("fixture_exited_" + std::to_string(code));
            }
            if (!root) Sleep(50);
        }
        if (!root) { cleanup(); throw std::runtime_error("fixture_root_unavailable"); }
    }
    void cleanup() {
        if (process.hProcess) {
            if (root && IsWindow(root)) PostMessageW(root, WM_CLOSE, 0, 0);
            if (WaitForSingleObject(process.hProcess, 2000) == WAIT_TIMEOUT) TerminateProcess(process.hProcess, 2);
            CloseHandle(process.hProcess); CloseHandle(process.hThread);
            process.hProcess = nullptr;
        }
    }
    ~Child() { cleanup(); }
    Child(const Child&) = delete;
    Child& operator=(const Child&) = delete;
    void focus(int id = 101) {
        if (GetForegroundWindow() != root && !SetForegroundWindow(root))
            throw std::runtime_error("fixture_activation_failed");
        for (int i = 0; i < 50 && GetForegroundWindow() != root; ++i) Sleep(10);
        if (GetForegroundWindow() != root) throw std::runtime_error("fixture_activation_unconfirmed");
        SendMessageW(root, kFocus, static_cast<WPARAM>(id), 0);
        for (int i = 0; i < 50; ++i) {
            GUITHREADINFO info{};
            info.cbSize = sizeof(info);
            if (GetGUIThreadInfo(GetWindowThreadProcessId(root, nullptr), &info) &&
                info.hwndFocus == GetDlgItem(root, id)) return;
            Sleep(10);
        }
        throw std::runtime_error("fixture_child_focus_unconfirmed");
    }
    std::wstring text(int id = 101) {
        // Only this owned, synthetic fixture. No external field can reach here.
        wchar_t buffer[256]{};
        SendMessageW(GetDlgItem(root, id), WM_GETTEXT, 256, reinterpret_cast<LPARAM>(buffer));
        return buffer;
    }
    std::wstring await_text(const std::wstring& expected, int id = 101) {
        std::wstring received;
        for (int attempt = 0; attempt < 100; ++attempt) {
            received = text(id);
            if (received == expected) return received;
            Sleep(20);
        }
        return received;
    }
};
struct ComApartment {
    ComApartment() {
        if (FAILED(CoInitializeEx(nullptr, COINIT_MULTITHREADED)))
            throw std::runtime_error("COM_initialization_failed");
    }
    ~ComApartment() { CoUninitialize(); }
    ComApartment(const ComApartment&) = delete;
    ComApartment& operator=(const ComApartment&) = delete;
};
ComPtr<IUIAutomation> automation_client() {
    ComPtr<IUIAutomation2> client;
    ComPtr<IUIAutomation> automation;
    if (SUCCEEDED(CoCreateInstance(CLSID_CUIAutomation8, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&client))) &&
        client && SUCCEEDED(client->put_ConnectionTimeout(750)) &&
        SUCCEEDED(client->put_TransactionTimeout(1000)))
        client.As(&automation);
    return automation;
}
int self_test(const std::filesystem::path& report) {
    std::vector<std::pair<std::string, bool>> checks;
    std::string error, synthetic_received;
    const DWORD clipboard_before = GetClipboardSequenceNumber();
    auto check = [&](const char* name, bool ok) {
        checks.emplace_back(name, ok);
        if (!ok) throw std::runtime_error(name);
    };
    try {
        Child a, b;
        a.focus();
        auto target = capture(nullptr);
        b.focus();
        const UINT sent = submit(target, nullptr);
        check("unicode_events_queued_once", sent == 2 * (std::size(kText) - 1));
        check("original_process_restored", GetForegroundWindow() == a.root);
        const auto received = a.await_text(kText);
        synthetic_received = utf8(received);
        check("chinese_emoji_received_exactly", received == kText);
        check("other_process_untouched", b.text().empty());
        DWORD start = 0, end = 0;
        SendMessageW(GetDlgItem(a.root, 101), EM_GETSEL, reinterpret_cast<WPARAM>(&start), reinterpret_cast<LPARAM>(&end));
        check("caret_after_insert", start == std::size(kText) - 1 && end == start);
        a.focus();
        SendMessageW(GetDlgItem(a.root, 101), WM_SETTEXT, 0, reinterpret_cast<LPARAM>(L"prefix OLD suffix"));
        SendMessageW(GetDlgItem(a.root, 101), EM_SETSEL, 7, 10);
        target = capture(nullptr);
        b.focus();
        check("selection_events_queued_once", submit(target, nullptr) == 2 * (std::size(kText) - 1));
        check("selection_replaced_without_full_field_set", a.await_text(L"prefix " + std::wstring(kText) + L" suffix") == L"prefix " + std::wstring(kText) + L" suffix");
        a.focus(); target = capture(nullptr); a.focus(102);
        bool rejected = false;
        try { submit(target, nullptr); } catch (...) { rejected = true; }
        check("changed_native_field_rejected", rejected && a.text(102).empty());
        a.focus(); target = capture(nullptr);
        target.time -= std::chrono::seconds(61);
        rejected = false;
        try { submit(target, nullptr); } catch (...) { rejected = true; }
        check("expired_target_rejected", rejected);
        a.focus(); SendMessageW(a.root, kState, 0, 0);
        rejected = false;
        try { capture(nullptr); } catch (...) { rejected = true; }
        check("native_password_rejected", rejected);
        b.focus(); target = capture(nullptr); b.cleanup();
        rejected = false;
        try { submit(target, nullptr); } catch (...) { rejected = true; }
        check("closed_target_rejected", rejected);
        auto automation = automation_client();
        check("real_uia_client_available", automation.Get() != nullptr);
        Child c;
        c.focus(103);
        auto control = capture(automation.Get());
        std::cout << "Owned unsupported-pattern fixture: {" << metadata(control) << "}\n";
        check("unsupported_value_pattern_is_absent", control.element &&
            control.control_type == UIA_ButtonControlTypeId && !control.uia_value);
        check("unsupported_text_pattern_is_absent", !control.uia_text);
        c.focus(104);
        auto field = capture(automation.Get());
        std::cout << "Owned single-line UIA fixture: {" << metadata(field) << "}\n";
        check("uia_edit_patterns_present", field.element && field.uia_focused &&
            field.control_type == UIA_EditControlTypeId && field.uia_value && field.uia_text);
        a.focus(102);
        const UINT uia_sent = submit(field, automation.Get());
        check("uia_capture_restoration_and_exact_receipt", uia_sent == 2 * (std::size(kText) - 1) &&
            GetForegroundWindow() == c.root && c.await_text(kText, 104) == kText &&
            c.text().empty() && a.text(102).empty());
        auto prepare_restore = [&](IUIAutomation* client) {
            SendMessageW(c.root, kRestoreMode, 0, 0);
            c.focus(104);
            SendMessageW(GetDlgItem(c.root, 104), WM_SETTEXT, 0, reinterpret_cast<LPARAM>(L""));
            return capture(client);
        };
        field = prepare_restore(nullptr);
        SendMessageW(c.root, kRestoreMode, 140, 0);
        a.focus(102);
        RestoreTrace delayed;
        const UINT delayed_sent = submit(field, nullptr, &delayed);
        std::cout << "Owned delayed native focus: {" << restore_metadata(delayed) << "}\n";
        check("delayed_native_focus_restored_before_dispatch", delayed_sent == 2 * (std::size(kText) - 1) &&
            delayed.null_focus_samples > 0 && delayed.original_focus_confirmed &&
            !delayed.uia_request_attempted && c.await_text(kText, 104) == kText && a.text(102).empty());
        field = prepare_restore(automation.Get());
        SendMessageW(c.root, kRestoreMode, INFINITE, 0);
        a.focus(102);
        RestoreTrace requested;
        const UINT requested_sent = submit(field, automation.Get(), &requested);
        std::cout << "Owned explicit UIA focus: {" << restore_metadata(requested) << "}\n";
        check("uia_requested_only_saved_element_when_native_focus_absent", requested_sent == 2 * (std::size(kText) - 1) &&
            requested.null_focus_samples > 0 && requested.uia_request_attempted &&
            SUCCEEDED(requested.uia_request_result) && requested.original_focus_confirmed &&
            c.await_text(kText, 104) == kText && a.text(102).empty());
        field = prepare_restore(nullptr);
        SendMessageW(c.root, kRestoreMode, INFINITE, 0);
        a.focus(102);
        RestoreTrace timed_out;
        std::string restore_error;
        try { submit(field, nullptr, &timed_out); }
        catch (const std::exception& e) { restore_error = e.what(); }
        std::cout << "Owned missing focus timeout: {" << restore_metadata(timed_out) << "}\n";
        check("focus_restore_timeout_does_not_dispatch", restore_error == "native_focus_restore_timeout" &&
            timed_out.null_focus_samples > 0 && !timed_out.original_focus_confirmed &&
            !timed_out.uia_request_attempted && c.text(104).empty() && a.text(102).empty());
        field = prepare_restore(automation.Get());
        SendMessageW(c.root, kRestoreMode, 140, 102);
        a.focus(102);
        RestoreTrace changed;
        restore_error.clear();
        try { submit(field, automation.Get(), &changed); }
        catch (const std::exception& e) { restore_error = e.what(); }
        std::cout << "Owned delayed different field: {" << restore_metadata(changed) << "}\n";
        check("late_different_field_rejected_without_dispatch", restore_error == "native_focused_field_changed" &&
            changed.null_focus_samples > 0 && !changed.original_focus_confirmed && !changed.uia_request_attempted &&
            c.text(104).empty() && c.text(102).empty() && a.text(102).empty());
        check("no_enter_or_linebreak", SendMessageW(a.root, kEnterCount, 0, 0) == 0 &&
            SendMessageW(c.root, kEnterCount, 0, 0) == 0);
        check("clipboard_untouched", GetClipboardSequenceNumber() == clipboard_before);
    } catch (const std::exception& e) { error = e.what(); }
    std::ostringstream out;
    out << "{\"commit\":\"" VAO_LAB_COMMIT "\",\"passed\":" << (error.empty() ? "true" : "false")
        << ",\"synthetic_fixture_only\":true,\"owned_fixture_received\":" << quoted(synthetic_received)
        << ",\"error\":" << quoted(error) << ",\"checks\":[";
    for (size_t i = 0; i < checks.size(); ++i) {
        if (i) out << ',';
        out << "{\"name\":" << quoted(checks[i].first) << ",\"passed\":" << (checks[i].second ? "true" : "false") << '}';
    }
    out << "]}";
    write_report(report, out.str());
    std::cout << out.str() << '\n';
    return error.empty() ? 0 : 1;
}
std::filesystem::path event_path(const std::filesystem::path& directory, const std::string& prefix) {
    const auto timestamp = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
    return directory / (prefix + "-" + std::to_string(GetCurrentProcessId()) + "-" +
        std::to_string(timestamp) + ".json");
}
int interactive(const std::filesystem::path& directory) {
    const auto startup = event_path(directory, "startup");
    auto stage = [&](const std::string& name, bool uia = false) {
        write_report(startup, "{\"commit\":\"" VAO_LAB_COMMIT "\",\"action\":\"startup\",\"stage\":" +
            quoted(name) + ",\"pid\":" + std::to_string(GetCurrentProcessId()) +
            ",\"uia_available\":" + (uia ? "true" : "false") + ",\"field_text_read\":false}");
    };
    stage("initializing");
    SetConsoleTitleW(L"Vao 文字入口验证器 - 保持此窗口打开");
    DWORD mode = 0;
    const HANDLE console_input = GetStdHandle(STD_INPUT_HANDLE);
    if (GetConsoleMode(console_input, &mode))
        SetConsoleMode(console_input, (mode | ENABLE_EXTENDED_FLAGS) & ~ENABLE_QUICK_EDIT_MODE);
    std::cout << "Vao 文字入口验证器：正在启动……\n"
        "先退出 Vao，只在空白、未保存的测试草稿中操作。\n"
        "不接麦克风/模型，不读取正文，不操作剪贴板，不按 Enter。\n"
        "它没有正文变化保护，不要在真实会话或密码框内使用。\n\n";
    struct Hotkeys {
        std::vector<int> ids;
        ~Hotkeys() { for (const int id : ids) UnregisterHotKey(nullptr, id); }
    } hotkeys;
    for (int id = 1; id <= 3; ++id) {
        stage("registering_hotkey_" + std::to_string(id));
        if (!RegisterHotKey(nullptr, id, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_F8 + id - 1)) {
            const DWORD error = GetLastError();
            throw std::runtime_error("hotkey_registration_failed_id_" + std::to_string(id) +
                "_win32_" + std::to_string(error));
        }
        hotkeys.ids.push_back(id);
    }
    stage("initializing_uia");
    auto automation = automation_client();
    stage("ready", automation.Get() != nullptr);
    std::cout << "已就绪，保持本窗口打开，有效运行 10 分钟。\n"
        "Ctrl+Alt+F8：在测试输入框内记住目标，有效 60 秒。\n"
        "Ctrl+Alt+F9：返回原处，输入一次固定短句，随后解除目标。\n"
        "Ctrl+Alt+F10：取消。关闭本窗口退出。\n"
        "F8 到 F9 之间可切换应用，请勿编辑原草稿或移动原光标。\n"
        "事件入队不代表应用已收到文字，请查看原输入框。\n\n";
    if (!automation) std::cout << "提示：UIA 不可用，此次只有原生窗口/光标定位证据。\n";
    std::unique_ptr<Target> saved;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::minutes(10);
    MSG msg{};
    while (std::chrono::steady_clock::now() < deadline) {
        if (!PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) { Sleep(20); continue; }
        if (msg.message == WM_QUIT) break;
        if (msg.message != WM_HOTKEY) { TranslateMessage(&msg); DispatchMessageW(&msg); continue; }
        if (msg.wParam < 1 || msg.wParam > 3) continue;
        const std::string action = msg.wParam == 1 ? "capture" : msg.wParam == 2 ? "submit" : "cancel";
        const auto report = event_path(directory, "input");
        // Persist before querying an external UIA provider, so a native crash
        // still leaves the last entered action instead of an empty reports folder.
        write_report(report, "{\"commit\":\"" VAO_LAB_COMMIT "\",\"action\":" +
            quoted(action) + ",\"stage\":\"started\",\"field_text_read\":false}");
        std::string details, error;
        RestoreTrace restore;
        UINT sent = 0;
        try {
            if (msg.wParam == 3) {
                saved.reset();
                std::cout << "已取消，不提交文字。\n";
            } else if (msg.wParam == 1) {
                saved.reset();
                auto target = capture(automation.Get());
                details = metadata(target);
                saved = std::make_unique<Target>(std::move(target));
                std::cout << "已记住 " << utf8(saved->app) << "。Ctrl+Alt+F9 提交；F10 取消。\n";
            } else if (msg.wParam == 2) {
                // Consume BEFORE restoration/dispatch. Nothing retries a partial result.
                auto target = std::move(saved);
                if (!target) throw std::runtime_error("no_armed_target_press_Ctrl_Alt_F8_first");
                details = metadata(*target);
                for (int i = 0; i < 100 && !keys_up(); ++i) Sleep(20);
                sent = submit(*target, automation.Get(), &restore);
                if (sent != 2 * (std::size(kText) - 1))
                    throw std::runtime_error("synthetic_queue_incomplete_no_retry");
                std::cout << "已提交一次固定短句并解除目标，请检查原输入框。\n";
            }
        } catch (const std::exception& e) {
            saved.reset(); error = e.what();
            std::cout << "此次操作已停止，未重试。原因：" << error << '\n';
        }
        std::ostringstream out;
        out << "{\"commit\":\"" VAO_LAB_COMMIT "\",\"action\":" << quoted(action)
            << ",\"stage\":\"finished\",\"synthetic_fixed_text_only\":true,\"field_text_read\":false,\"clipboard_accessed\":false,"
            "\"receipt_verified\":false,\"queue_complete\":"
            << (sent == 2 * (std::size(kText) - 1) ? "true" : "false")
            << ",\"queued_events\":" << sent << ",\"expected_events\":"
            << 2 * (std::size(kText) - 1) << ",\"error\":" << quoted(error);
        if (!details.empty()) out << ',' << details;
        if (msg.wParam == 2) out << ',' << restore_metadata(restore);
        out << '}';
        write_report(report, out.str());
    }
    stage("exited", automation.Get() != nullptr);
    std::cout << "验证器已结束；若需继续，请重新双击启动脚本。\n";
    return 0;
}
int wmain(int argc, wchar_t** argv) {
    try {
        SetConsoleOutputCP(CP_UTF8);
        std::cout << std::unitbuf;
        if (argc == 3 && std::wstring(argv[1]) == L"--fixture")
            return fixture(static_cast<DWORD>(std::stoul(argv[2])));
        if (argc == 3 && std::wstring(argv[1]) == L"--build-info") {
            write_report(argv[2], "{\"tool\":\"Vao synthetic Unicode input lab\",\"version\":1,\"commit\":\"" VAO_LAB_COMMIT "\",\"architecture\":\"x64\",\"gui_started\":false}");
            return 0;
        }
        if (argc == 3 && std::wstring(argv[1]) == L"--self-test") {
            ComApartment apartment;
            return self_test(argv[2]);
        }
        if (argc == 3 && std::wstring(argv[1]) == L"--interactive") {
            ComApartment apartment;
            return interactive(argv[2]);
        }
        std::cout << "Use the supplied Start-Input-Lab.cmd. No input was dispatched.\n";
        return 2;
    } catch (const std::exception& e) {
        std::cerr << "验证器启动/运行失败：" << e.what() << '\n';
        if (argc == 3 && std::wstring(argv[1]) == L"--interactive") {
            try {
                write_report(event_path(argv[2], "fatal"), "{\"commit\":\"" VAO_LAB_COMMIT
                    "\",\"action\":\"fatal\",\"error\":" + quoted(e.what()) + ",\"field_text_read\":false}");
            } catch (...) { std::cerr << "错误报告无法写入，请保留本窗口。\n"; }
        }
        return 1;
    }
}
