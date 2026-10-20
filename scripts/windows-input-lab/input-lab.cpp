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
GUITHREADINFO gui(HWND root) {
    GUITHREADINFO info{};
    info.cbSize = sizeof(info);
    const DWORD thread = GetWindowThreadProcessId(root, nullptr);
    if (!thread || !GetGUIThreadInfo(thread, &info) || !info.hwndFocus)
        throw std::runtime_error("native_focus_unavailable");
    if (GetAncestor(info.hwndFocus, GA_ROOT) != root)
        throw std::runtime_error("focus_outside_root");
    if (info.flags & (GUI_INMENUMODE | GUI_INMOVESIZE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE))
        throw std::runtime_error("menu_or_move_active");
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
    if (automation && SUCCEEDED(automation->GetFocusedElement(&target.element))) {
        BOOL password = FALSE, enabled = FALSE, focused = FALSE;
        if (FAILED(target.element->get_CurrentIsPassword(&password)) || password ||
            FAILED(target.element->get_CurrentIsEnabled(&enabled)) || !enabled)
            throw std::runtime_error("protected_or_unavailable_uia_element");
        if (SUCCEEDED(target.element->get_CurrentHasKeyboardFocus(&focused)))
            target.uia_focused = focused != FALSE;
        target.element->get_CurrentControlType(&target.control_type);
        ComPtr<IUIAutomationTextPattern> text;
        target.uia_text = SUCCEEDED(target.element->GetCurrentPatternAs(UIA_TextPatternId, IID_PPV_ARGS(&text)));
        ComPtr<IUIAutomationValuePattern> value;
        target.uia_value = SUCCEEDED(target.element->GetCurrentPatternAs(UIA_ValuePatternId, IID_PPV_ARGS(&value)));
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
        if (FAILED(automation->GetFocusedElement(&current)) ||
            FAILED(automation->CompareElements(target.element.Get(), current.Get(), &same)) || !same ||
            FAILED(current->get_CurrentIsPassword(&password)) || password ||
            FAILED(current->get_CurrentIsEnabled(&enabled)) || !enabled)
            throw std::runtime_error("uia_focused_element_changed");
    }
}
UINT submit(const Target& target, IUIAutomation* automation) {
    identity(target);
    if (!keys_up()) throw std::runtime_error("physical_key_held");
    // Never force a different child field or click coordinates to recover focus.
    if (gui(target.root).hwndFocus != target.focus.hwndFocus)
        throw std::runtime_error("saved_native_focus_changed");
    if (IsIconic(target.root)) ShowWindow(target.root, SW_RESTORE);
    if (GetForegroundWindow() != target.root && !SetForegroundWindow(target.root))
        throw std::runtime_error("windows_denied_activation");
    for (int i = 0; i < 50 && GetForegroundWindow() != target.root; ++i) Sleep(10);
    if (GetForegroundWindow() != target.root) throw std::runtime_error("activation_unconfirmed");
    verify_focus(target, automation);
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
    return SendInput(static_cast<UINT>(input.size()), input.data(), sizeof(INPUT));
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
LRESULT CALLBACK fixture_edit(HWND hwnd, UINT msg, WPARAM w, LPARAM l) {
    if ((msg == WM_CHAR && (w == '\r' || w == '\n')) || (msg == WM_KEYDOWN && w == VK_RETURN))
        ++enter_count;
    return CallWindowProcW(edit_proc, hwnd, msg, w, l);
}
LRESULT CALLBACK fixture_window(HWND hwnd, UINT msg, WPARAM w, LPARAM l) {
    if (msg == WM_ACTIVATE && LOWORD(w) != WA_INACTIVE && fixture_controller)
        AllowSetForegroundWindow(fixture_controller);
    if (msg == WM_CREATE) {
        for (int i = 0; i < 2; ++i) {
            HWND edit = CreateWindowExW(0, L"EDIT", L"", WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_MULTILINE,
                10, 10 + i * 80, 400, 65, hwnd, reinterpret_cast<HMENU>(static_cast<INT_PTR>(101 + i)),
                GetModuleHandleW(nullptr), nullptr);
            const auto previous = reinterpret_cast<WNDPROC>(SetWindowLongPtrW(edit, GWLP_WNDPROC,
                reinterpret_cast<LONG_PTR>(fixture_edit)));
            if (!edit_proc) edit_proc = previous;
        }
        return 0;
    }
    if (msg == kFocus) { SetFocus(GetDlgItem(hwnd, static_cast<int>(w))); return 1; }
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
        100, 100, 460, 260, nullptr, nullptr, cls.hInstance, nullptr);
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
        SendMessageW(root, kFocus, static_cast<WPARAM>(id), 0);
        Sleep(100);
    }
    std::wstring text(int id = 101) {
        // Only this owned, synthetic fixture. No external field can reach here.
        wchar_t buffer[256]{};
        SendMessageW(GetDlgItem(root, id), WM_GETTEXT, 256, reinterpret_cast<LPARAM>(buffer));
        return buffer;
    }
};
int self_test(const std::filesystem::path& report) {
    std::vector<std::pair<std::string, bool>> checks;
    std::string error;
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
        Sleep(250);
        check("unicode_events_queued_once", sent == 2 * (std::size(kText) - 1));
        check("original_process_restored", GetForegroundWindow() == a.root);
        check("chinese_emoji_received_exactly", a.text() == kText);
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
        Sleep(250);
        check("selection_replaced_without_full_field_set", a.text() == L"prefix " + std::wstring(kText) + L" suffix");
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
        check("no_enter_or_linebreak", SendMessageW(a.root, kEnterCount, 0, 0) == 0);
        check("clipboard_untouched", GetClipboardSequenceNumber() == clipboard_before);
    } catch (const std::exception& e) { error = e.what(); }
    std::ostringstream out;
    out << "{\"commit\":\"" VAO_LAB_COMMIT "\",\"passed\":" << (error.empty() ? "true" : "false")
        << ",\"synthetic_fixture_only\":true,\"error\":" << quoted(error) << ",\"checks\":[";
    for (size_t i = 0; i < checks.size(); ++i) {
        if (i) out << ',';
        out << "{\"name\":" << quoted(checks[i].first) << ",\"passed\":" << (checks[i].second ? "true" : "false") << '}';
    }
    out << "]}";
    write_report(report, out.str());
    std::cout << out.str() << '\n';
    return error.empty() ? 0 : 1;
}
int interactive(const std::filesystem::path& directory) {
    std::cout << "SYNTHETIC INPUT LAB - use an EMPTY, UNSAVED SCRATCH field only.\n"
        "No microphone/model/clipboard. Does not read field text. Never sends Enter.\n"
        "This is NOT the normal Vao input mode. It does NOT protect against text edits.\n"
        "Close Vao before testing. Keep all target apps at normal privilege.\n\n"
        "Ctrl+Alt+F8: remember the focused window/field (valid for 60 seconds).\n"
        "Ctrl+Alt+F9: return there and insert ONE FIXED TEST PHRASE, then disarm.\n"
        "Ctrl+Alt+F10: cancel. Close this console to quit.\n"
        "You may switch apps between F8 and F9. Do not edit the original scratch field.\n"
        "A queued event is NOT confirmation that the target received text. Inspect it.\n\n";
    if (!RegisterHotKey(nullptr, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_F8) ||
        !RegisterHotKey(nullptr, 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_F9) ||
        !RegisterHotKey(nullptr, 3, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_F10))
        throw std::runtime_error("lab_hotkey_conflict_close_lab_and_retry");
    struct Hotkeys { ~Hotkeys() { for (int id = 1; id <= 3; ++id) UnregisterHotKey(nullptr, id); } } hotkeys;
    ComPtr<IUIAutomation2> client;
    ComPtr<IUIAutomation> automation;
    if (SUCCEEDED(CoCreateInstance(CLSID_CUIAutomation8, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&client))) &&
        SUCCEEDED(client->put_ConnectionTimeout(750)) && SUCCEEDED(client->put_TransactionTimeout(1000)))
        client.As(&automation);
    std::unique_ptr<Target> saved;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::minutes(10);
    MSG msg{};
    while (std::chrono::steady_clock::now() < deadline) {
        if (!PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) { Sleep(20); continue; }
        if (msg.message == WM_QUIT) break;
        if (msg.message != WM_HOTKEY) { TranslateMessage(&msg); DispatchMessageW(&msg); continue; }
        std::string details, error;
        UINT sent = 0;
        if (msg.wParam == 3) { saved.reset(); std::cout << "Cancelled.\n"; continue; }
        try {
            if (msg.wParam == 1) {
                saved.reset();
                auto target = capture(automation.Get());
                details = metadata(target);
                saved = std::make_unique<Target>(std::move(target));
                std::cout << "Armed for " << utf8(saved->app) << ". F9 inserts; F10 cancels.\n";
            } else if (msg.wParam == 2) {
                // Consume BEFORE restoration/dispatch. Nothing retries a partial result.
                auto target = std::move(saved);
                if (!target) throw std::runtime_error("no_armed_target_press_F8_first");
                details = metadata(*target);
                for (int i = 0; i < 100 && !keys_up(); ++i) Sleep(20);
                sent = submit(*target, automation.Get());
                std::cout << "Disarmed. Queued " << sent << " events. CHECK THE ORIGINAL FIELD.\n";
            }
        } catch (const std::exception& e) { saved.reset(); error = e.what(); std::cout << "Stopped: " << error << '\n'; }
        std::ostringstream out;
        out << "{\"commit\":\"" VAO_LAB_COMMIT "\",\"action\":\"" << (msg.wParam == 1 ? "capture" : "submit")
            << "\",\"synthetic_fixed_text_only\":true,\"field_text_read\":false,\"clipboard_accessed\":false,"
            "\"receipt_verified\":false,\"queue_complete\":"
            << (sent == 2 * (std::size(kText) - 1) ? "true" : "false")
            << ",\"queued_events\":" << sent << ",\"expected_events\":"
            << 2 * (std::size(kText) - 1) << ",\"error\":" << quoted(error);
        if (!details.empty()) out << ',' << details;
        out << '}';
        const auto timestamp = std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::system_clock::now().time_since_epoch()).count();
        write_report(directory / ("input-" + std::to_string(timestamp) + ".json"), out.str());
    }
    return 0;
}
int wmain(int argc, wchar_t** argv) {
    try {
        SetConsoleOutputCP(CP_UTF8);
        if (argc == 3 && std::wstring(argv[1]) == L"--fixture")
            return fixture(static_cast<DWORD>(std::stoul(argv[2])));
        if (argc == 3 && std::wstring(argv[1]) == L"--build-info") {
            write_report(argv[2], "{\"tool\":\"Vao synthetic Unicode input lab\",\"version\":1,\"commit\":\"" VAO_LAB_COMMIT "\",\"architecture\":\"x64\",\"gui_started\":false}");
            return 0;
        }
        if (argc == 3 && std::wstring(argv[1]) == L"--self-test") return self_test(argv[2]);
        if (argc == 3 && std::wstring(argv[1]) == L"--interactive") {
            const HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
            if (FAILED(hr)) throw std::runtime_error("COM_initialization_failed");
            const int result = interactive(argv[2]);
            CoUninitialize();
            return result;
        }
        std::cout << "Use the supplied Start-Input-Lab.cmd. No input was dispatched.\n";
        return 2;
    } catch (const std::exception& e) { std::cerr << e.what() << '\n'; return 1; }
}
