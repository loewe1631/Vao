// Native tests read only these owned synthetic controls, never user fields.
#define VAO_CAPABILITY_TEST_INCLUDE
#include "capability.cpp"
std::string load(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary);
    return std::string(std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>());
}
std::wstring owned_text(HWND field) {
    wchar_t text[128]{}; GetWindowTextW(field, text, 128); return text;
}
int wmain(int argc, wchar_t** argv) {
    if (argc != 3) return 2;
    const std::filesystem::path report = argv[2], directory = report.parent_path() / "fixture-reports";
    std::filesystem::create_directories(directory);
    std::vector<std::pair<std::string, bool>> checks;
    auto check = [&](const char* name, bool passed) {
        checks.emplace_back(name, passed); if (!passed) throw std::runtime_error(name);
    };
    const DWORD clipboard = GetClipboardSequenceNumber();
    HWND a = nullptr, b = nullptr;
    std::string error;
    try {
        a = CreateWindowExW(0, L"STATIC", L"Owned capability fixture", WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            40, 40, 480, 300, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        b = CreateWindowExW(0, L"STATIC", L"Owned second fixture", WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            60, 60, 480, 300, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        const HWND edit = CreateWindowExW(0, L"EDIT", L"fixture_only", WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL,
            10, 10, 300, 30, a, nullptr, GetModuleHandleW(nullptr), nullptr);
        const HWND button = CreateWindowExW(0, L"BUTTON", L"Owned button", WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10, 60, 200, 30, a, nullptr, GetModuleHandleW(nullptr), nullptr);
        const HWND password = CreateWindowExW(0, L"EDIT", L"fixture_password", WS_CHILD | WS_VISIBLE | ES_PASSWORD,
            10, 110, 300, 30, a, nullptr, GetModuleHandleW(nullptr), nullptr);
        check("owned_windows_created", a && b && edit && button && password);
        auto focus = [&](HWND field) {
            SetForegroundWindow(a); SetActiveWindow(a); SetFocus(field);
            if (GetForegroundWindow() != a || GetFocus() != field) throw std::runtime_error("owned_focus_failed");
        };
        const DWORD pid = GetCurrentProcessId();
        const auto created = process_created(pid);
        auto probe = [&](const wchar_t* name, HWND root, DWORD expected_pid, ULONGLONG expected_created,
            size_t depth = kMaximumNodes, DWORD deadline = kWorkerDeadlineMs, bool delayed = false) {
            const auto path = directory / name;
            const auto status = run_worker(argv[1], root, expected_pid, expected_created, path, depth, deadline, delayed);
            if (!delayed && status != "finished") throw std::runtime_error("worker_did_not_finish");
            return load(path);
        };
        focus(edit);
        auto value = probe(L"edit.json", a, pid, created);
        check("edit_bounded_root_chain_and_patterns", value.find("\"status\":\"complete\"") != std::string::npos &&
            value.find("\"depth\":0,\"same_root\":true") != std::string::npos &&
            value.find("\"text_pattern\":true") != std::string::npos && value.find("\"value_pattern\":true") != std::string::npos);
        check("inspection_does_not_change_focus", GetForegroundWindow() == a && GetFocus() == edit);
        check("report_has_no_owned_text_or_names", value.find("fixture_only") == std::string::npos && value.find("Owned") == std::string::npos);
        focus(button);
        value = probe(L"button.json", a, pid, created);
        const auto first_end = value.find('}', value.find("\"nodes\":[{"));
        const auto first = value.substr(0, first_end);
        check("unsupported_patterns_do_not_crash", value.find("\"status\":\"complete\"") != std::string::npos &&
            first.find("\"text_pattern\":false") != std::string::npos && first.find("\"value_pattern\":false") != std::string::npos);
        focus(password);
        value = probe(L"password.json", a, pid, created);
        check("password_rejected_without_partial_nodes", value.find("\"status\":\"protected_field\"") != std::string::npos && value.find("\"nodes\":[]") != std::string::npos);
        focus(edit);
        value = probe(L"depth.json", a, pid, created, 1);
        check("depth_limit_rejects_unproven_root", value.find("\"status\":\"root_not_proven_within_depth_limit\"") != std::string::npos && value.find("\"nodes\":[]") != std::string::npos);
        value = probe(L"wrong-window.json", b, pid, created);
        check("other_window_not_inspected", value.find("\"status\":\"root_identity_or_foreground_changed\"") != std::string::npos && value.find("\"nodes\":[]") != std::string::npos);
        value = probe(L"wrong-pid.json", a, pid + 1, created);
        check("wrong_process_identity_rejected", value.find("\"status\":\"root_identity_or_foreground_changed\"") != std::string::npos);
        value = probe(L"wrong-generation.json", a, pid, created + 1);
        check("wrong_process_creation_rejected", value.find("\"status\":\"root_identity_or_foreground_changed\"") != std::string::npos);
        SendMessageW(edit, EM_SETREADONLY, TRUE, 0);
        value = probe(L"readonly.json", a, pid, created);
        check("readonly_capability_reported", value.find("\"status\":\"complete\"") != std::string::npos && value.find("\"value_readonly\":1") != std::string::npos);
        const auto started = GetTickCount64();
        value = probe(L"timeout.json", a, pid, created, kMaximumNodes, 150, true);
        check("worker_timeout_bounded_and_empty", GetTickCount64() - started < 2000 && value.find("\"status\":\"worker_timeout\"") != std::string::npos && value.find("\"nodes\":[]") != std::string::npos);
        check("owned_field_content_unchanged", owned_text(edit) == L"fixture_only" && owned_text(password) == L"fixture_password");
        check("clipboard_sequence_unchanged", GetClipboardSequenceNumber() == clipboard);
        check("no_input_or_focus_action_reported", value.find("\"input_events\":0") != std::string::npos && value.find("\"focus_changed_by_tool\":false") != std::string::npos);
    } catch (const std::exception& exception) { error = exception.what(); }
    if (a) DestroyWindow(a); if (b) DestroyWindow(b);
    std::ostringstream out;
    out << std::boolalpha << "{\"commit\":\"" VAO_CAP_COMMIT "\",\"passed\":" << error.empty()
        << ",\"synthetic_fixture_only\":true,\"error\":\"" << error << "\",\"checks\":[";
    bool first = true;
    for (const auto& item : checks) {
        if (!first) out << ','; first = false;
        out << "{\"name\":\"" << item.first << "\",\"passed\":" << item.second << '}';
    }
    out << "]}"; save(report, out.str());
    std::cout << out.str() << '\n';
    return error.empty() ? 0 : 1;
}
