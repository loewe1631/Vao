// In-process TSF foundation on owned hidden fixtures. No registration or external IPC.
#include "text-store.h"
#include <chrono>
#include <filesystem>
#include <fstream>
#include <functional>
#include <memory>
#include <sstream>
#include <stdexcept>
#ifndef VAO_TSF_COMMIT
#define VAO_TSF_COMMIT "uncommitted"
#endif
struct Hr { HRESULT value; };
void need(HRESULT hr) { if (FAILED(hr)) throw Hr{hr}; }
bool same_object(IUnknown* a, IUnknown* b) {
    if (!a || !b) return false;
    ComPtr<IUnknown> left, right;
    return SUCCEEDED(a->QueryInterface(IID_PPV_ARGS(&left))) && SUCCEEDED(b->QueryInterface(IID_PPV_ARGS(&right))) && left.Get() == right.Get();
}
struct Host {
    ComPtr<ITfThreadMgrEx> manager;
    ComPtr<ITfDocumentMgr> document;
    ComPtr<ITfContext> context;
    ComPtr<TextStore> store;
    TfClientId client;
    HWND window = nullptr;
    unsigned long long identity;
    explicit Host(ITfThreadMgrEx* thread, TfClientId id, unsigned long long owner) : manager(thread), client(id), identity(owner) {
        window = CreateWindowExW(0, L"VaoTsfOwnedFixture", L"Owned TSF fixture", WS_OVERLAPPED, 0, 0, 400, 200,
            nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        if (!window) throw Hr{E_FAIL};
        store.Attach(new TextStore(window));
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(store.Get()));
        need(manager->CreateDocumentMgr(&document));
        TfEditCookie store_cookie{};
        need(document->CreateContext(client, 0, static_cast<ITextStoreACP*>(store.Get()), &context, &store_cookie));
        need(document->Push(context.Get()));
    }
    bool focused() const {
        ComPtr<ITfDocumentMgr> current;
        return store->alive && SUCCEEDED(manager->GetFocus(&current)) && same_object(document.Get(), current.Get());
    }
    void focus() { need(manager->SetFocus(document.Get())); }
    void close() {
        if (!store || !store->alive) return;
        if (focused()) manager->SetFocus(nullptr);
        store->alive = false; ++store->revision;
        if (document) document->Pop(TF_POPF_ALL);
        context.Reset(); document.Reset();
        SetWindowLongPtrW(window, GWLP_USERDATA, 0); DestroyWindow(window); window = nullptr;
    }
    ~Host() { close(); }
    Host(const Host&) = delete;
    Host& operator=(const Host&) = delete;
};
class Edit final : public ITfEditSession {
    LONG references_ = 1;
    std::function<HRESULT(TfEditCookie)> function_;
public:
    unsigned calls = 0;
    explicit Edit(std::function<HRESULT(TfEditCookie)> function) : function_(std::move(function)) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** output) override {
        if (!output) return E_POINTER; *output = nullptr;
        if (iid != IID_IUnknown && iid != __uuidof(ITfEditSession)) return E_NOINTERFACE;
        *output = static_cast<ITfEditSession*>(this); AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&references_)); }
    ULONG STDMETHODCALLTYPE Release() override { const LONG n = InterlockedDecrement(&references_); if (!n) delete this; return static_cast<ULONG>(n); }
    HRESULT STDMETHODCALLTYPE DoEditSession(TfEditCookie cookie) override {
        ++calls;
        try { return function_(cookie); } catch (const Hr& error) { return error.value; } catch (...) { return E_FAIL; }
    }
};
HRESULT session(Host& host, DWORD access, std::function<HRESULT(TfEditCookie)> function) {
    if (!host.context) return E_FAIL;
    ComPtr<Edit> edit; edit.Attach(new Edit(std::move(function)));
    HRESULT result = E_FAIL;
    const HRESULT requested = host.context->RequestEditSession(host.client, edit.Get(), TF_ES_SYNC | access, &result);
    if (FAILED(requested)) return requested;
    return edit->calls == 1 ? result : E_FAIL;
}
bool boundary(const std::wstring& text, LONG index) {
    if (index < 0 || static_cast<size_t>(index) > text.size()) return false;
    return index == 0 || static_cast<size_t>(index) == text.size() ||
        !(text[static_cast<size_t>(index) - 1] >= 0xd800 && text[static_cast<size_t>(index) - 1] <= 0xdbff && text[index] >= 0xdc00 && text[index] <= 0xdfff);
}
bool plain_unicode(const std::wstring& text) {
    if (text.size() > 4096) return false;
    for (size_t i = 0; i < text.size(); ++i) {
        const auto ch = static_cast<unsigned>(text[i]);
        if (!ch || ch == 0xfffc) return false;
        if (ch >= 0xd800 && ch <= 0xdbff) {
            if (++i >= text.size() || text[i] < 0xdc00 || text[i] > 0xdfff) return false;
        } else if (ch >= 0xdc00 && ch <= 0xdfff) return false;
    }
    return true;
}
struct Snapshot {
    std::wstring text;
    LONG start = 0, end = 0;
    TF_SELECTIONSTYLE style{TF_AE_NONE, FALSE};
};
Snapshot snapshot(Host& host, TfEditCookie cookie) {
    ComPtr<ITfRange> full, finish;
    need(host.context->GetStart(cookie, &full)); need(host.context->GetEnd(cookie, &finish));
    need(full->ShiftEndToRange(cookie, finish.Get(), TF_ANCHOR_END));
    ComPtr<ITfRangeACP> acp; need(full.As(&acp));
    LONG start = -1, length = -1; need(acp->GetExtent(&start, &length));
    if (start != 0 || length < 0 || length > 4096) throw Hr{E_INVALIDARG};
    std::wstring text(static_cast<size_t>(length) + 1, L'\0');
    ULONG received = 0; need(full->GetText(cookie, 0, text.data(), static_cast<ULONG>(text.size()), &received));
    if (received != static_cast<ULONG>(length)) throw Hr{E_FAIL};
    text.resize(received);
    if (!plain_unicode(text)) throw Hr{E_INVALIDARG};
    TF_SELECTION selected{}; ULONG fetched = 0;
    need(host.context->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &selected, &fetched));
    ComPtr<ITfRange> selected_range; selected_range.Attach(selected.range);
    if (fetched != 1 || !selected_range || selected.style.fInterimChar) throw Hr{E_FAIL};
    need(selected_range.As(&acp)); LONG offset = -1, count = -1; need(acp->GetExtent(&offset, &count));
    if (offset < 0 || count < 0 || offset > length || count > length - offset ||
        !boundary(text, offset) || !boundary(text, offset + count)) throw Hr{E_INVALIDARG};
    return {std::move(text), offset, offset + count, selected.style};
}
struct Armed {
    unsigned long long owner = 0, revision = 0;
    Snapshot before;
    std::chrono::steady_clock::time_point expiry;
    bool used = false, canceled = false;
};
using Token = std::shared_ptr<Armed>;
bool eligible(Host& host) { return host.store->alive && !host.store->readonly && !host.store->password && !host.store->composing && host.focused(); }
Token capture(Host& host) {
    if (!eligible(host)) return {};
    auto token = std::make_shared<Armed>();
    const HRESULT result = session(host, TF_ES_READ, [&](TfEditCookie cookie) {
        if (!eligible(host)) return E_FAIL;
        token->before = snapshot(host, cookie); token->owner = host.identity; token->revision = host.store->revision;
        token->expiry = std::chrono::steady_clock::now() + std::chrono::seconds(30); return S_OK;
    });
    return SUCCEEDED(result) ? token : Token{};
}
bool submit(Host& host, const Token& token, const std::wstring& insertion) {
    if (!token || token->used) return false;
    token->used = true; // Shared by every alias, before any edit-session request.
    if (token->canceled || token->owner != host.identity || token->expiry < std::chrono::steady_clock::now() ||
        !eligible(host) || !plain_unicode(insertion) || insertion.empty()) return false;
    bool verified = false;
    const HRESULT result = session(host, TF_ES_READWRITE, [&](TfEditCookie cookie) {
        if (!eligible(host) || token->revision != host.store->revision) return E_FAIL;
        const auto now = snapshot(host, cookie);
        const auto& before = token->before;
        if (now.text != before.text || now.start != before.start || now.end != before.end ||
            now.style.ase != before.style.ase || now.style.fInterimChar != before.style.fInterimChar) return E_FAIL;
        const auto expected = before.text.substr(0, static_cast<size_t>(before.start)) + insertion + before.text.substr(static_cast<size_t>(before.end));
        if (expected.size() > 4096) return E_INVALIDARG;
        TF_SELECTION selected{}; ULONG fetched = 0;
        need(host.context->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &selected, &fetched));
        ComPtr<ITfRange> range; range.Attach(selected.range);
        if (fetched != 1 || !range) return E_FAIL;
        need(range->SetText(cookie, 0, insertion.data(), static_cast<LONG>(insertion.size())));
        need(range->Collapse(cookie, TF_ANCHOR_END));
        TF_SELECTION caret{range.Get(), {TF_AE_NONE, FALSE}};
        need(host.context->SetSelection(cookie, 1, &caret));
        const auto after = snapshot(host, cookie);
        const LONG expected_caret = before.start + static_cast<LONG>(insertion.size());
        verified = after.text == expected && after.start == expected_caret && after.end == expected_caret;
        return verified ? S_OK : E_FAIL;
    });
    return SUCCEEDED(result) && verified;
}
void save(const std::filesystem::path& path, const std::string& json) {
    std::filesystem::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary | std::ios::trunc); out << json << '\n';
    out.close(); if (!out) throw Hr{E_FAIL};
}
int self_test(const std::filesystem::path& report) {
    std::vector<std::pair<std::string, bool>> checks;
    std::string failure;
    HRESULT failure_hr = S_OK;
    auto check = [&](const char* name, bool ok) { checks.emplace_back(name, ok); if (!ok) throw std::runtime_error(name); };
    const DWORD clipboard = GetClipboardSequenceNumber(); const HKL layout = GetKeyboardLayout(0); const HWND foreground = GetForegroundWindow();
    ComPtr<ITfThreadMgrEx> manager; bool activated = false;
    unsigned long long locks = 0, writes = 0;
    try {
        WNDCLASSW cls{}; cls.lpfnWndProc = fixture_window; cls.hInstance = GetModuleHandleW(nullptr); cls.lpszClassName = L"VaoTsfOwnedFixture";
        if (!RegisterClassW(&cls)) throw Hr{E_FAIL};
        need(CoCreateInstance(CLSID_TF_ThreadMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&manager)));
        TfClientId client{}; need(manager->ActivateEx(&client, TF_TMAE_NOACTIVATETIP | TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)); activated = true;
        Host a(manager.Get(), client, 1), b(manager.Get(), client, 2);
        a.focus();
        auto seed = [&](const std::wstring& value, LONG start, LONG end) { if (!a.store->external_change(value, start, end)) throw Hr{E_FAIL}; a.focus(); };
        seed(L"prefix suffix", 7, 7);
        auto token = capture(a);
        check("real_context_and_read_edit_cookie", token && token->before.text == L"prefix suffix" && token->before.start == 7 && a.store->granted_locks > 0);
        TS_TEXTCHANGE change{};
        check("store_write_without_lock_rejected", a.store->SetText(0, 0, 0, L"X", 1, &change) == TS_E_NOLOCK && a.store->writes == 0);
        const HRESULT read_write = session(a, TF_ES_READ, [&](TfEditCookie cookie) {
            ComPtr<ITfRange> range; need(a.context->GetStart(cookie, &range)); return range->SetText(cookie, 0, L"X", 1);
        });
        check("readonly_edit_cookie_cannot_write", FAILED(read_write) && a.store->text == L"prefix suffix" && a.store->writes == 0);
        const std::wstring insertion = L"\u4e2d\u6587\U0001f642 e\u0301\n";
        const auto alias = token;
        check("unicode_multiline_exact_text_and_caret_receipt", submit(a, token, insertion) && a.store->text == L"prefix " + insertion + L"suffix");
        check("single_store_write_no_replay_through_alias", a.store->writes == 1 && !submit(a, alias, L"X") && a.store->writes == 1);
        seed(L"prefix OLD suffix", 7, 10); token = capture(a);
        check("selection_replacement_preserves_surrounding_text", submit(a, token, L"NEW") && a.store->text == L"prefix NEW suffix");
        seed(L"original", 3, 3); token = capture(a); seed(L"changed", 3, 3); auto count = a.store->writes;
        check("content_conflict_rejected", !submit(a, token, L"X") && a.store->text == L"changed" && a.store->writes == count);
        seed(L"original", 3, 3); token = capture(a); seed(L"original", 4, 4); count = a.store->writes;
        check("caret_conflict_rejected", !submit(a, token, L"X") && a.store->writes == count);
        seed(L"original", 2, 4); token = capture(a); seed(L"original", 2, 5);
        check("selection_conflict_rejected", !submit(a, token, L"X") && a.store->writes == count);
        seed(L"original", 3, 3); token = capture(a); seed(L"original", 3, 3);
        check("generation_change_rejected_even_when_text_matches", !submit(a, token, L"X") && a.store->writes == count);
        token = capture(a); b.focus();
        check("other_document_focus_rejected", !submit(a, token, L"X") && a.store->writes == count && b.store->text.empty());
        a.focus(); token = capture(a);
        check("wrong_context_owner_rejected", !submit(b, token, L"X") && b.store->writes == 0 && a.store->writes == count);
        token = capture(a); if (token) token->expiry = std::chrono::steady_clock::now() - std::chrono::seconds(1);
        check("expired_token_rejected", token && !submit(a, token, L"X") && a.store->writes == count);
        token = capture(a); if (token) token->canceled = true;
        check("canceled_token_rejected", token && !submit(a, token, L"X") && a.store->writes == count);
        token = capture(a); a.store->readonly = true;
        check("readonly_transition_and_capture_rejected", !submit(a, token, L"X") && !capture(a) && a.store->writes == count); a.store->readonly = false;
        a.store->password = true; check("owned_password_policy_rejects_capture", !capture(a)); a.store->password = false;
        token = capture(a); a.store->composing = true;
        check("composition_policy_rejects_capture_and_submission", !capture(a) && !submit(a, token, L"X") && a.store->writes == count); a.store->composing = false;
        seed(std::wstring(4097, L'a'), 0, 0);
        check("large_document_rejected_before_text_export", !capture(a));
        seed(L"A\U0001f642Z", 2, 2);
        check("surrogate_split_selection_rejected", !capture(a));
        seed(L"original", 3, 3); token = capture(a);
        check("malformed_utf16_insertion_rejected", !submit(a, token, std::wstring(1, static_cast<wchar_t>(0xd800))) && a.store->writes == count);
        token = capture(a);
        check("embedded_object_insertion_rejected", !submit(a, token, std::wstring(1, static_cast<wchar_t>(0xfffc))) && a.store->writes == count);
        seed(std::wstring(4090, L'a'), 0, 0); token = capture(a);
        check("result_size_bound_rejected", !submit(a, token, L"0123456789") && a.store->writes == count);
        seed(L"original", 3, 3); token = capture(a); a.store->corrupt_next_write = true; count = a.store->writes;
        check("receipt_mismatch_rejected_after_single_write", !submit(a, token, L"X") && a.store->writes == count + 1 && a.store->text == L"oriXgina!");
        check("failed_receipt_token_cannot_replay", !submit(a, token, L"X") && a.store->writes == count + 1);
        seed(L"original", 3, 3); token = capture(a); a.close(); locks = a.store->granted_locks; writes = a.store->writes;
        check("detached_context_token_rejected_without_edit_session", token && !submit(a, token, L"X") && a.store->writes == writes && a.store->granted_locks == locks);
        check("clipboard_unchanged", GetClipboardSequenceNumber() == clipboard);
        check("keyboard_layout_unchanged", GetKeyboardLayout(0) == layout);
        check("foreground_window_unchanged", GetForegroundWindow() == foreground);
    } catch (const Hr& error) { failure = "tsf_hresult"; failure_hr = error.value; }
      catch (const std::exception& error) { failure = error.what(); }
    if (activated) manager->Deactivate();
    std::ostringstream out; out << std::boolalpha << "{\"commit\":\"" VAO_TSF_COMMIT "\",\"passed\":" << failure.empty()
        << ",\"synthetic_fixture_only\":true,\"external_app_context\":false,\"profile_registered\":false,\"dll_registered\":false,\"ipc_implemented\":false,"
        "\"field_text_exported\":false,\"input_events\":0,\"granted_locks\":" << locks << ",\"store_writes\":" << writes
        << ",\"failure\":\"" << failure << "\",\"hresult\":" << failure_hr << ",\"checks\":[";
    bool first = true;
    for (const auto& item : checks) { if (!first) out << ','; first = false; out << "{\"name\":\"" << item.first << "\",\"passed\":" << item.second << '}'; }
    out << "]}"; save(report, out.str()); return failure.empty() ? 0 : 1;
}
int wmain(int argc, wchar_t** argv) {
    try {
        if (argc == 3 && std::wstring(argv[1]) == L"--build-info") {
            save(argv[2], "{\"tool\":\"Vao owned TSF foundation lab\",\"commit\":\"" VAO_TSF_COMMIT "\",\"gui_started\":false,\"profile_registered\":false}"); return 0;
        }
        if (argc == 3 && std::wstring(argv[1]) == L"--self-test") {
            need(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED));
            const int result = self_test(argv[2]); CoUninitialize(); return result;
        }
        return 2;
    } catch (...) { return 1; }
}
