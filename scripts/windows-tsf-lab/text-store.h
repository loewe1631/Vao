// Owned plain-text fixture only. No connection to an external application's store.
#pragma once
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <olectl.h>
#include <msctf.h>
#include <textstor.h>
#include <wrl/client.h>
#include <algorithm>
#include <string>
#include <vector>
using Microsoft::WRL::ComPtr;
constexpr UINT kPendingLock = WM_APP + 47;
class TextStore final : public ITextStoreACP, public ITfContextOwnerCompositionSink {
    LONG references_ = 1;
    DWORD lock_ = 0, pending_ = 0;
    ComPtr<ITextStoreACPSink> sink_;
    DWORD sink_mask_ = 0;
public:
    HWND window;
    std::wstring text;
    TS_SELECTION_ACP selection{0, 0, {TS_AE_END, FALSE}};
    bool readonly = false, password = false, composing = false, alive = true, corrupt_next_write = false;
    unsigned long long revision = 1, granted_locks = 0, writes = 0;
    explicit TextStore(HWND own_window) : window(own_window) {}
    bool reading() const { return (lock_ & TS_LF_READ) == TS_LF_READ; }
    bool writing() const { return (lock_ & TS_LF_READWRITE) == TS_LF_READWRITE; }
    bool extent(LONG start, LONG end) const { return start >= 0 && start <= end && static_cast<size_t>(end) <= text.size(); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** output) override {
        if (!output) return E_POINTER; *output = nullptr;
        if (iid == IID_IUnknown || iid == __uuidof(ITextStoreACP)) *output = static_cast<ITextStoreACP*>(this);
        else if (iid == __uuidof(ITfContextOwnerCompositionSink)) *output = static_cast<ITfContextOwnerCompositionSink*>(this);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return static_cast<ULONG>(InterlockedIncrement(&references_)); }
    ULONG STDMETHODCALLTYPE Release() override {
        const LONG remaining = InterlockedDecrement(&references_);
        if (!remaining) delete this; return static_cast<ULONG>(remaining);
    }
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID iid, IUnknown* source, DWORD mask) override {
        if (iid != __uuidof(ITextStoreACPSink) || !source) return E_INVALIDARG;
        ComPtr<ITextStoreACPSink> proposed;
        if (FAILED(source->QueryInterface(IID_PPV_ARGS(&proposed)))) return E_NOINTERFACE;
        if (sink_) {
            ComPtr<IUnknown> a, b; sink_.As(&a); proposed.As(&b);
            if (a.Get() != b.Get()) return CONNECT_E_ADVISELIMIT;
        }
        sink_ = proposed; sink_mask_ = mask; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE UnadviseSink(IUnknown* source) override {
        if (!source || !sink_) return CONNECT_E_NOCONNECTION;
        ComPtr<IUnknown> a, b; sink_.As(&a); source->QueryInterface(IID_PPV_ARGS(&b));
        if (a.Get() != b.Get()) return CONNECT_E_NOCONNECTION;
        sink_.Reset(); sink_mask_ = 0; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE RequestLock(DWORD flags, HRESULT* session) override {
        if (!session) return E_POINTER;
        if (!sink_) return E_UNEXPECTED;
        if (lock_) {
            if (flags & TS_LF_SYNC) { *session = TS_E_SYNCHRONOUS; return S_OK; }
            pending_ = ((pending_ | flags) & TS_LF_READWRITE) == TS_LF_READWRITE ? TS_LF_READWRITE : TS_LF_READ;
            PostMessageW(window, kPendingLock, 0, 0); *session = TS_S_ASYNC; return S_OK;
        }
        if (flags & TS_LF_SYNC) pending_ = 0; // A granted synchronous request supersedes pending asynchronous work.
        lock_ = flags & TS_LF_READWRITE; ++granted_locks;
        *session = sink_->OnLockGranted(flags);
        lock_ = 0; return S_OK;
    }
    void grant_pending() {
        if (lock_ || !pending_ || !sink_) return;
        const DWORD flags = pending_; pending_ = 0;
        HRESULT ignored = E_FAIL; RequestLock(flags, &ignored);
    }
    HRESULT STDMETHODCALLTYPE GetStatus(TS_STATUS* status) override {
        if (!status) return E_POINTER;
        status->dwDynamicFlags = readonly ? TS_SD_READONLY : 0;
        status->dwStaticFlags = TS_SS_NOHIDDENTEXT; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE QueryInsert(LONG start, LONG end, ULONG count, LONG* result_start, LONG* result_end) override {
        if (!result_start || !result_end || !extent(start, end) || count > 16384) return E_INVALIDARG;
        *result_start = start; *result_end = end; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetSelection(ULONG index, ULONG count, TS_SELECTION_ACP* output, ULONG* fetched) override {
        if (!reading()) return TS_E_NOLOCK;
        if (!fetched || !output || !count || (index != TS_DEFAULT_SELECTION && index != 0)) return E_INVALIDARG;
        *output = selection; *fetched = 1; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetSelection(ULONG count, const TS_SELECTION_ACP* value) override {
        if (!writing()) return TS_E_NOLOCK;
        if (count != 1 || !value || !extent(value->acpStart, value->acpEnd)) return E_INVALIDARG;
        selection = *value; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetText(LONG start, LONG end, WCHAR* plain, ULONG plain_capacity, ULONG* plain_read,
        TS_RUNINFO* runs, ULONG run_capacity, ULONG* runs_read, LONG* next) override {
        if (!reading()) return TS_E_NOLOCK;
        if (end == -1) end = static_cast<LONG>(text.size());
        if (!extent(start, end) || (plain_capacity && !plain) || (run_capacity && !runs)) return E_INVALIDARG;
        const ULONG available = static_cast<ULONG>(end - start);
        const ULONG count = std::min(available, plain_capacity);
        if (count) std::copy_n(text.data() + start, count, plain);
        if (plain_read) *plain_read = count;
        const ULONG covered = plain_capacity ? count : (run_capacity ? available : 0);
        if (runs_read) *runs_read = run_capacity && covered ? 1 : 0;
        if (run_capacity && covered) { runs[0].uCount = covered; runs[0].type = TS_RT_PLAIN; }
        if (next) *next = start + static_cast<LONG>(covered);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetText(DWORD, LONG start, LONG end, const WCHAR* value, ULONG count, TS_TEXTCHANGE* change) override {
        if (!writing()) return TS_E_NOLOCK;
        if (readonly) return TS_E_READONLY;
        if (!change || !extent(start, end) || (count && !value) || count > 16384 || text.size() - (end - start) + count > 16384) return E_INVALIDARG;
        try { text.replace(static_cast<size_t>(start), static_cast<size_t>(end - start), value ? value : L"", count); }
        catch (...) { return E_OUTOFMEMORY; }
        if (corrupt_next_write && !text.empty()) { text.back() = L'!'; corrupt_next_write = false; }
        *change = {start, end, start + static_cast<LONG>(count)};
        selection.acpStart = selection.acpEnd = change->acpNewEnd;
        selection.style = {TS_AE_END, FALSE}; ++writes; ++revision;
        // This change originated in a TSF edit session: no recursive OnTextChange.
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFormattedText(LONG, LONG, IDataObject** output) override { if (output) *output = nullptr; return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetEmbedded(LONG, REFGUID, REFIID, IUnknown** output) override { if (output) *output = nullptr; return TS_E_NOOBJECT; }
    HRESULT STDMETHODCALLTYPE QueryInsertEmbedded(const GUID*, const FORMATETC*, BOOL* insertable) override { if (!insertable) return E_POINTER; *insertable = FALSE; return S_OK; }
    HRESULT STDMETHODCALLTYPE InsertEmbedded(DWORD, LONG, LONG, IDataObject*, TS_TEXTCHANGE*) override { return TS_E_FORMAT; }
    HRESULT STDMETHODCALLTYPE InsertTextAtSelection(DWORD flags, const WCHAR* value, ULONG count, LONG* start, LONG* end, TS_TEXTCHANGE* change) override {
        if (!writing()) return TS_E_NOLOCK;
        const LONG a = selection.acpStart, b = selection.acpEnd;
        if (start) *start = a;
        if (flags & TS_IAS_QUERYONLY) { if (end) *end = b; return S_OK; }
        TS_TEXTCHANGE local{};
        const HRESULT hr = SetText(0, a, b, value, count, &local);
        if (SUCCEEDED(hr)) { if (end) *end = local.acpNewEnd; if (change) *change = local; }
        return hr;
    }
    HRESULT STDMETHODCALLTYPE InsertEmbeddedAtSelection(DWORD, IDataObject*, LONG*, LONG*, TS_TEXTCHANGE*) override { return TS_E_FORMAT; }
    HRESULT STDMETHODCALLTYPE RequestSupportedAttrs(DWORD, ULONG, const TS_ATTRID*) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsAtPosition(LONG, ULONG, const TS_ATTRID*, DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsTransitioningAtPosition(LONG, ULONG, const TS_ATTRID*, DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE FindNextAttrTransition(LONG, LONG halt, ULONG, const TS_ATTRID*, DWORD, LONG* next, BOOL* found, LONG* offset) override {
        if (!next || !found || !offset) return E_POINTER; *next = halt; *found = FALSE; *offset = 0; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE RetrieveRequestedAttrs(ULONG, TS_ATTRVAL*, ULONG* fetched) override { if (!fetched) return E_POINTER; *fetched = 0; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetEndACP(LONG* end) override { if (!reading()) return TS_E_NOLOCK; if (!end) return E_POINTER; *end = static_cast<LONG>(text.size()); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetActiveView(TsViewCookie* view) override { if (!view) return E_POINTER; *view = 1; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetACPFromPoint(TsViewCookie, const POINT*, DWORD, LONG*) override { return TS_E_NOLAYOUT; }
    HRESULT STDMETHODCALLTYPE GetTextExt(TsViewCookie, LONG, LONG, RECT* rectangle, BOOL* clipped) override {
        if (rectangle) *rectangle = {}; if (clipped) *clipped = TRUE; return TS_E_NOLAYOUT;
    }
    HRESULT STDMETHODCALLTYPE GetScreenExt(TsViewCookie, RECT* rectangle) override { if (!rectangle) return E_POINTER; GetWindowRect(window, rectangle); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetWnd(TsViewCookie, HWND* output) override { if (!output) return E_POINTER; *output = window; return S_OK; }
    HRESULT STDMETHODCALLTYPE OnStartComposition(ITfCompositionView*, BOOL* accepted) override { if (!accepted) return E_POINTER; *accepted = TRUE; composing = true; return S_OK; }
    HRESULT STDMETHODCALLTYPE OnUpdateComposition(ITfCompositionView*, ITfRange*) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE OnEndComposition(ITfCompositionView*) override { composing = false; return S_OK; }
    bool external_change(const std::wstring& next, LONG start, LONG end) {
        if (lock_ || start < 0 || start > end || static_cast<size_t>(end) > next.size()) return false;
        const LONG old_end = static_cast<LONG>(text.size()); text = next;
        selection = {start, end, {TS_AE_END, FALSE}}; ++revision;
        TS_TEXTCHANGE change{0, old_end, static_cast<LONG>(text.size())};
        if (sink_ && (sink_mask_ & TS_AS_TEXT_CHANGE)) sink_->OnTextChange(0, &change);
        if (sink_ && (sink_mask_ & TS_AS_SEL_CHANGE)) sink_->OnSelectionChange();
        return true;
    }
};
inline LRESULT CALLBACK fixture_window(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* store = reinterpret_cast<TextStore*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == kPendingLock && store) { store->grant_pending(); return 0; }
    return DefWindowProcW(window, message, wparam, lparam);
}
