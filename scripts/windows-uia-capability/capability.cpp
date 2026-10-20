// Read-only, explicitly requested UIA capability inspection. No input path.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <objbase.h>
#include <UIAutomation.h>
#include <wrl/client.h>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>
#ifndef VAO_CAP_COMMIT
#define VAO_CAP_COMMIT "uncommitted"
#endif
using Microsoft::WRL::ComPtr;
constexpr size_t kMaximumNodes = 8;
constexpr DWORD kWorkerDeadlineMs = 5000;
struct Node {
    int pid = 0, type = 0, selection = -1, readonly = -1;
    HRESULT text_hr = E_FAIL, text2_hr = E_FAIL, text_edit_hr = E_FAIL, value_hr = E_FAIL;
    bool focused = false, enabled = false, text = false, text2 = false;
    bool text_edit = false, value = false;
};
struct Result {
    std::string status;
    std::vector<Node> nodes;
};
std::string serialize(const Result& result) {
    std::ostringstream out;
    out << std::boolalpha << "{\"commit\":\"" VAO_CAP_COMMIT "\",\"status\":\"" << result.status
        << "\",\"maximum_nodes\":" << kMaximumNodes << ",\"deadline_ms\":" << kWorkerDeadlineMs
        << ",\"field_text_read\":false,\"input_events\":0,\"focus_changed_by_tool\":false,\"nodes\":[";
    bool first = true;
    for (const auto& node : result.nodes) {
        if (!first) out << ',';
        first = false;
        out << "{\"depth\":" << (&node - result.nodes.data()) << ",\"same_root\":true,\"pid\":" << node.pid
            << ",\"control_type\":" << node.type << ",\"focused\":" << node.focused << ",\"enabled\":" << node.enabled
            << ",\"text_pattern\":" << node.text << ",\"text2_pattern\":" << node.text2
            << ",\"text_edit_pattern\":" << node.text_edit << ",\"value_pattern\":" << node.value
            << ",\"text_hresult\":" << node.text_hr << ",\"text2_hresult\":" << node.text2_hr
            << ",\"text_edit_hresult\":" << node.text_edit_hr << ",\"value_hresult\":" << node.value_hr
            << ",\"supported_selection\":" << node.selection << ",\"value_readonly\":" << node.readonly << '}';
    }
    out << "]}";
    return out.str();
}
void save(const std::filesystem::path& path, const std::string& content) {
    std::filesystem::create_directories(path.parent_path());
    const auto temporary = path.wstring() + L".tmp";
    {
        std::ofstream out(temporary, std::ios::binary | std::ios::trunc);
        out << content << '\n';
        out.close();
        if (!out) throw std::runtime_error("report_write_failed");
    }
    if (!MoveFileExW(temporary.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH))
        throw std::runtime_error("report_commit_failed");
}
ULONGLONG process_created(DWORD pid) {
    const HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) return 0;
    FILETIME begin{}, end{}, kernel{}, user{};
    const BOOL ok = GetProcessTimes(process, &begin, &end, &kernel, &user);
    CloseHandle(process);
    return ok ? (static_cast<ULONGLONG>(begin.dwHighDateTime) << 32) | begin.dwLowDateTime : 0;
}
bool root_valid(HWND root, DWORD pid, ULONGLONG created) {
    DWORD current = 0;
    GetWindowThreadProcessId(root, &current);
    return root && created && IsWindow(root) && GetForegroundWindow() == root &&
        root != GetDesktopWindow() && root != GetShellWindow() && current == pid && process_created(pid) == created;
}
struct Apartment {
    Apartment() { if (FAILED(CoInitializeEx(nullptr, COINIT_MULTITHREADED))) throw std::runtime_error("com_unavailable"); }
    ~Apartment() { CoUninitialize(); }
};
Result inspect(HWND root, DWORD pid, ULONGLONG created, size_t maximum_nodes) {
    auto fail = [](const char* status) { return Result{status, {}}; };
    if (!root_valid(root, pid, created)) return fail("root_identity_or_foreground_changed");
    if (maximum_nodes == 0 || maximum_nodes > kMaximumNodes) return fail("invalid_depth_limit");
    ComPtr<IUIAutomation2> client;
    if (FAILED(CoCreateInstance(CLSID_CUIAutomation8, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&client))) || !client ||
        FAILED(client->put_ConnectionTimeout(500)) || FAILED(client->put_TransactionTimeout(500))) return fail("uia_unavailable");
    ComPtr<IUIAutomationElement> boundary, focused;
    ComPtr<IUIAutomationTreeWalker> walker;
    if (FAILED(client->ElementFromHandle(root, &boundary)) || !boundary ||
        FAILED(client->GetFocusedElement(&focused)) || !focused ||
        FAILED(client->get_RawViewWalker(&walker)) || !walker) return fail("uia_identity_unavailable");
    // Prove the complete bounded focus-to-root ancestry before querying patterns.
    // Nodes with another native top-level window are rejected, never traversed.
    std::vector<ComPtr<IUIAutomationElement>> chain;
    ComPtr<IUIAutomationElement> current = focused;
    bool reached_root = false;
    for (size_t depth = 0; depth < maximum_nodes; ++depth) {
        UIA_HWND native{};
        if (FAILED(current->get_CurrentNativeWindowHandle(&native))) return fail("native_identity_unavailable");
        const HWND window = reinterpret_cast<HWND>(native);
        if (window && (!IsWindow(window) || GetAncestor(window, GA_ROOT) != root)) return fail("outside_original_root");
        for (const auto& previous : chain) {
            BOOL equal = FALSE;
            if (FAILED(client->CompareElements(previous.Get(), current.Get(), &equal))) return fail("comparison_unavailable");
            if (equal) return fail("ancestry_cycle");
        }
        chain.push_back(current);
        BOOL at_root = FALSE;
        if (FAILED(client->CompareElements(current.Get(), boundary.Get(), &at_root))) return fail("comparison_unavailable");
        if (at_root) { reached_root = true; break; }
        if (depth + 1 == maximum_nodes) break;
        ComPtr<IUIAutomationElement> parent;
        if (FAILED(walker->GetParentElement(current.Get(), &parent)) || !parent) return fail("parent_unavailable");
        current = parent;
    }
    if (!reached_root) return fail("root_not_proven_within_depth_limit");
    BOOL focused_now = FALSE, password = TRUE;
    if (FAILED(focused->get_CurrentHasKeyboardFocus(&focused_now)) || !focused_now) return fail("focus_unconfirmed");
    if (FAILED(focused->get_CurrentIsPassword(&password)) || password) return fail("protected_field");
    Result result{"complete", {}};
    for (const auto& element : chain) {
        Node node;
        BOOL enabled = FALSE, has_focus = FALSE;
        if (FAILED(element->get_CurrentProcessId(&node.pid)) || FAILED(element->get_CurrentControlType(&node.type)) ||
            FAILED(element->get_CurrentIsEnabled(&enabled)) || FAILED(element->get_CurrentHasKeyboardFocus(&has_focus)))
            return fail("metadata_unavailable");
        node.enabled = enabled != FALSE; node.focused = has_focus != FALSE;
        ComPtr<IUIAutomationTextPattern> text;
        ComPtr<IUIAutomationTextPattern2> text2;
        ComPtr<IUIAutomationTextEditPattern> text_edit;
        ComPtr<IUIAutomationValuePattern> value;
        node.text_hr = element->GetCurrentPatternAs(UIA_TextPatternId, IID_PPV_ARGS(&text));
        node.text = SUCCEEDED(node.text_hr) && text;
        node.text2_hr = element->GetCurrentPatternAs(UIA_TextPattern2Id, IID_PPV_ARGS(&text2));
        node.text2 = SUCCEEDED(node.text2_hr) && text2;
        node.text_edit_hr = element->GetCurrentPatternAs(UIA_TextEditPatternId, IID_PPV_ARGS(&text_edit));
        node.text_edit = SUCCEEDED(node.text_edit_hr) && text_edit;
        node.value_hr = element->GetCurrentPatternAs(UIA_ValuePatternId, IID_PPV_ARGS(&value));
        node.value = SUCCEEDED(node.value_hr) && value;
        if (node.text) {
            SupportedTextSelection selection{};
            if (SUCCEEDED(text->get_SupportedTextSelection(&selection))) node.selection = static_cast<int>(selection);
        }
        if (node.value) {
            BOOL readonly = TRUE;
            if (SUCCEEDED(value->get_CurrentIsReadOnly(&readonly))) node.readonly = readonly ? 1 : 0;
        }
        result.nodes.push_back(node);
    }
    ComPtr<IUIAutomationElement> latest;
    BOOL same = FALSE;
    if (!root_valid(root, pid, created) || FAILED(client->GetFocusedElement(&latest)) || !latest ||
        FAILED(client->CompareElements(focused.Get(), latest.Get(), &same)) || !same)
        return fail("focus_changed_during_inspection");
    return result;
}
std::wstring own_binary() {
    wchar_t path[32768]{};
    const DWORD count = GetModuleFileNameW(nullptr, path, 32768);
    if (!count || count >= 32768) throw std::runtime_error("own_binary_unavailable");
    return std::wstring(path, count);
}
std::string run_worker(const std::wstring& binary, HWND root, DWORD pid, ULONGLONG created,
    const std::filesystem::path& report, size_t depth = kMaximumNodes,
    DWORD deadline = kWorkerDeadlineMs, bool delayed_fixture = false) {
    save(report, serialize({"started", {}}));
    std::wstring command = L"\"" + binary + L"\" " + (delayed_fixture ? L"--delay-fixture " : L"--worker ") +
        std::to_wstring(reinterpret_cast<ULONG_PTR>(root)) + L" " + std::to_wstring(pid) + L" " +
        std::to_wstring(created) + L" " + std::to_wstring(depth) + L" \"" + report.wstring() + L"\"";
    STARTUPINFOW startup{}; startup.cb = sizeof(startup);
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(binary.c_str(), command.data(), nullptr, nullptr, FALSE, CREATE_NO_WINDOW,
        nullptr, nullptr, &startup, &process)) {
        save(report, serialize({"worker_start_failed", {}})); return "worker_start_failed";
    }
    CloseHandle(process.hThread);
    const auto until = GetTickCount64() + deadline;
    DWORD exit_code = 1;
    std::string status;
    while (true) {
        if (WaitForSingleObject(process.hProcess, 0) == WAIT_OBJECT_0) {
            GetExitCodeProcess(process.hProcess, &exit_code);
            status = exit_code == 0 ? "finished" : "worker_failed"; break;
        }
        if (GetTickCount64() >= until) { status = "worker_timeout"; break; }
        // Native fixtures may need their own message pump to answer UIA queries.
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&message); DispatchMessageW(&message);
        }
        MsgWaitForMultipleObjects(1, &process.hProcess, FALSE, 10, QS_ALLINPUT);
    }
    if (status != "finished") {
        TerminateProcess(process.hProcess, 1);
        WaitForSingleObject(process.hProcess, 1000);
        save(report, serialize({status, {}}));
    }
    CloseHandle(process.hProcess);
    return status;
}
int interactive(const std::filesystem::path& directory) {
    std::filesystem::create_directories(directory);
    if (!RegisterHotKey(nullptr, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_F8)) throw std::runtime_error("inspection_hotkey_unavailable");
    struct Hotkey { ~Hotkey() { UnregisterHotKey(nullptr, 1); } } hotkey;
    save(directory / "ready.json", serialize({"ready", {}}));
    std::cout << "已就绪：点击 WPS 测试文档的文字位置，按 Ctrl+Alt+F8。\n"
        "只检查接口，不输入文字、不切换窗口。保持本窗口打开，10分钟后自动结束。\n";
    const auto until = GetTickCount64() + 600000;
    MSG message{};
    while (GetTickCount64() < until) {
        if (!PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { Sleep(20); continue; }
        if (message.message == WM_QUIT) break;
        if (message.message != WM_HOTKEY || message.wParam != 1) continue;
        const HWND root = GetForegroundWindow();
        DWORD pid = 0; GetWindowThreadProcessId(root, &pid);
        const auto report = directory / (L"capability-" + std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64()) + L".json");
        const auto status = run_worker(own_binary(), root, pid, process_created(pid), report);
        std::cout << (status == "finished" ? "结构诊断已保存；无需按F9，也无需上传文件。\n" : "此次检查已停止，诊断已保存。\n");
    }
    return 0;
}
#ifndef VAO_CAPABILITY_TEST_INCLUDE
int wmain(int argc, wchar_t** argv) {
    SetConsoleOutputCP(CP_UTF8); std::cout << std::unitbuf;
    try {
        if (argc == 3 && std::wstring(argv[1]) == L"--build-info") {
            save(argv[2], "{\"tool\":\"Vao read-only UIA capabilities\",\"commit\":\"" VAO_CAP_COMMIT "\",\"architecture\":\"x64\",\"gui_started\":false}"); return 0;
        }
        if (argc == 7 && (std::wstring(argv[1]) == L"--worker" || std::wstring(argv[1]) == L"--delay-fixture")) {
            if (std::wstring(argv[1]) == L"--delay-fixture") { Sleep(10000); return 1; }
            Apartment apartment;
            const auto result = inspect(reinterpret_cast<HWND>(std::stoull(argv[2])), static_cast<DWORD>(std::stoul(argv[3])),
                std::stoull(argv[4]), static_cast<size_t>(std::stoull(argv[5])));
            save(argv[6], serialize(result)); return 0;
        }
        if (argc == 3 && std::wstring(argv[1]) == L"--interactive") return interactive(argv[2]);
        return 2;
    } catch (...) { std::cerr << "检查工具已停止。未输入文字。\n"; return 1; }
}
#endif
