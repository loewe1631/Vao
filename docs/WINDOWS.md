# Windows preview

This branch develops the Windows x64 client. Candidates are unsigned GitHub Actions artifacts, not a public Windows release. Windows 10 1809+ and WebView2 are the build target; the first desktop test target is Windows 11 x64.

Build on a Windows host with Rust MSVC, Visual C++ build tools, Node 22+ and WebView2:

```powershell
npm ci
cd desktop
npx tauri build --target x86_64-pc-windows-msvc --config tauri.windows.conf.json --bundles nsis -- --locked
cd ..
pwsh -File scripts/check-windows-startup.ps1
python scripts/package-windows.py --commit (git rev-parse HEAD) --run-id 0
```

The `Windows candidate` workflow runs workspace tests and Clippy before producing a per-user NSIS installer, portable ZIP, source/run identity and SHA256SUMS. It does not publish Releases, sign binaries, update the website or merge branches. The existing Mac/Linux checks also remain enabled.

Keep candidate data separate: `app.vaovao.windows.preview`. Do not import another installation's credentials, settings, history or learning. Default shortcuts are Ctrl+Alt+Space (record), Ctrl+Alt+T (translation) and Escape (cancel while active). Hardware Fn is not supported on Windows; choose a normal combination in Settings.

CI compilation is not microphone, hotkey, selection, focus or input-delivery acceptance. Test these in a logged-in desktop session. SSH belongs to a different session. Current platform gaps and desktop test instructions will be updated with each candidate.

## Candidate 3007: empty Edit representations

The opt-in Unicode adapter now recognizes an empty writable Edit whose owned
Text range exactly equals its bounded accessible name, with a collapsed owned
caret at a proven boundary. The name remains only in the transient snapshot;
changing it or the content invalidates delivery. Nonempty field values are
never removed as labels. The existing whole-field Value adapter retains its
previous requirements.

A complete single-LF empty paragraph with a collapsed owned boundary caret
can disappear on the first native insertion. Its receipt accepts either the
original exact expected text/caret or precisely the inserted text with the
corresponding collapsed caret. Ordinary in-document LF, generated text and
other mismatches retain receipt warnings; no input is resent or caret moved.

Native WebView2 regressions now require verified empty input/textarea fields
with names/placeholders, empty editable containers and paragraph/BR containers,
at both paragraph caret boundaries, plus exact existing-text insertion. Generated pseudo-content still produces
an unconfirmed receipt. Changed empty-field contents/name, nonempty names and
real LF normalization are checked separately. Desktop dsh/Edge acceptance and
WPS text-context support remain unresolved until tested on the laptop.

### Structural diagnostics introduced in 3006

Desktop feedback for 3005 includes visible dsh input followed by a receipt
warning, an Edge Value/Text mismatch at capture, and a WPS pane without the
required text/selection interfaces. These are unresolved compatibility issues.
3006 adds structural diagnostics (lengths, offsets, newline/object-marker flags
and equality results) to distinguish them without exporting field text or
hashes. It preserves all capture, delivery and receipt requirements.

CI also observes six owned field shapes: input/textarea placeholders, empty
contenteditable, an empty paragraph with BR, generated placeholder content and
existing editable text. Observations explicitly distinguish capture refusal,
visible insertion and verified receipt; completing an observation does not
mean that shape supports delivery. A normalization regression checks that a
changed field produces a receipt warning and its consumed token cannot replay.

### Explicit preview launch

The portable archive includes `Start-Unicode-Preview.cmd`. Exit any running Vao
before launching it. This explicitly enables a Unicode input preview for the
current process; it does not change settings or the default input method.
Launching Vao normally retains the existing verified native/UIA paths.

The preview still requires a focused editable UIA field, a complete bounded
content and selection snapshot, an unchanged original target and a verified
post-input receipt. It sends one Unicode event batch after physical keys are
released; it does not replace the whole field, move the caret after writing,
paste, send Enter or retry a possible partial insertion. Controls that cannot
prove these conditions retain the result for copying. Window-only identification
is insufficient, including document panes without text/selection interfaces.

Multiline output remains supported by the existing native Edit/RichEdit adapter.
The UIA preview rejects line breaks and control characters until an editor
contract can distinguish text insertion from sending. The CI runs both the
original UIA tests and additional native WebView2 Unicode receipt, trusted input,
undo/redo, conflict and window-restoration tests. CI success does not establish
physical desktop recording acceptance.

The preview launcher writes `input-preview.trace` beside the executable with
stage names, HRESULTs and numeric/boolean metadata only. Field contents, selections,
window titles and provider error descriptions are excluded. Only explicitly
selected text enters the existing selection feature; complete field snapshots
remain transient in the existing UIA worker.

## Existing capability-based input

Input routing now follows control capabilities rather than an application-name list. The existing native Edit/RichEdit path remains first. Other focused plain Edit controls can use Windows UI Automation when they expose both writable ValuePattern and a single TextPattern selection. The same adapter is intended for compatible Win32, XAML and web fields; a browser, chat product or executable name does not grant eligibility. Actual laptop acceptance remains pending.

UIA targets live on a dedicated MTA worker with provider connection/transaction timeouts, a bounded request deadline and recording generations. Every recording invalidates previous adapters and tokens. Capture checks the focused element's ancestry under the original root window, root and element process creation times, non-password/enabled/editable state, contents and selection. Delivery restores only the original root, confirms the exact focused element and unchanged snapshot, sets the plain value through its provider, restores the caret by a verified text range and checks the received contents/caret. It does not inject keys, use the clipboard, send Enter or overwrite a rich document through a generic value fallback. A write and a user edit are not atomic; a receipt failure can follow a partial write. Inspect the source before copying to avoid duplication.

Full field snapshots stay in memory; only explicit selected text enters the shared model path. Native Win32 snapshots allow at most 256K UTF-16 units. UIA fields and resulting values must additionally stay below 8 KiB of UTF-8 text: some providers truncate both Value and Text consistently, so agreement alone cannot establish that a large field was read in full. Chromium's [string serializer](https://chromium.googlesource.com/chromium/src/+/9dffde6c9419b8cf0caa061a4e2f192607916b40/third_party/blink/renderer/modules/accessibility/ax_object.cc) truncates values at 10,000 UTF-8 bytes; the smaller UIA bound applies to every provider without checking app names. UIA calls can allocate a provider value before its length can be checked; oversized values are rejected immediately. Unknown/opaque editors, protected fields, ambiguous selections, provider errors and conflicts preserve the result for copying. UIA ValuePattern has no portable multiline capability flag, so this first UIA adapter preserves multiline insertion results for copying instead of risking a single-line field stripping line breaks. The native multiline Edit/RichEdit path is unchanged.

The user's metadata-only desktop inspection confirmed compatible Value/Text patterns in Edge webpage search and Windows Search. WPS's inspected document pane exposed neither; the inspected dsh focus returned a Chrome pane instead of an editable element. These are unresolved compatibility cases, not an executable allowlist to grow one app at a time. A specialized editor path is appropriate only when its real interface and target/selection protection can be verified.

Windows CI also runs `--windows-input-probe --smoke-test <output.json>` against actual WebView2 fields, with separate state, no audio/model calls and no user content. It checks Unicode/selection receipt, page input events, stale/one-shot tokens, A/B/A field changes, content/caret/field conflicts, protected/disabled/oversized inputs and no Enter/form submission. Native unit fixtures additionally check A/B/A across processes and UIA restoration to a foreign process. The browser IPC doubles continue to check the interface; the native checks and user laptop acceptance are separate evidence.

## Candidate 3003 scope

Candidate 3002 was tested by the user: dictation reached Notepad, switching apps during recording or translation returned to the original field, and a selected-text follow-up question succeeded. This confirms those reported flows on that device, not every application. Clicking the overlay exposed a native "Vao Voice Bar" caption. Candidate 3003 replaces the Windows non-client drag loop with bounded physical mouse polling and non-activating position updates; a persistent style guard prevents framework restyles from restoring caption/edge styles. The isolated native probe checks repeated clicks, movement, release and attempted restyling without injecting global mouse/keyboard input. Actual laptop click/drag appearance remains a user acceptance step.

| Area | Implementation | Remaining evidence/work |
| --- | --- | --- |
| Install/start | Per-user NSIS, portable ZIP, isolated identity, WebView IPC probe | 3001 installation/start and 3002 runtime confirmed by user; 3003 upgrade/uninstall pending |
| Credentials | Windows Credential Manager; URL-bound connection keys | Native round-trip test runs in Windows CI; user setup test |
| Audio/models | CPAL WASAPI, microphone access probe, existing stage routing/cancellation | 3002 dictation, translation, cross-app return and selected-text question confirmed by user; 3003 regression pending |
| Shortcuts | Ordinary customizable RegisterHotKey combinations, conflict rollback | Physical triggers and conflicts on laptop; no hardware Fn |
| Overlay | No native frame/shadow, content-sized height, WS_EX_NOACTIVATE, work-area/DPI placement and dragging | Focus, multiple monitors and scaling on laptop |
| Result | Verified native-field insertion, overlay/history and explicit copy; failures preserve text | Clipboard test on laptop |
| Selection/input | Native Edit/RichEdit plus capability-based UIA plain Edit snapshots; one-shot insertion and receipt checks | 3002 Notepad flows accepted; 3004 UIA, dsh and WPS compatibility need physical desktop evidence |

Install the candidate in the normal logged-in user desktop, then configure custom ASR/LLM connections or explicitly enroll in the trial. Test Ctrl+Alt+Space twice, a clear Chinese/English recording, translation, Escape cancellation, no microphone, denied microphone access and a failed model connection. Confirm the result remains available and focus is preserved. Enable automatic insertion and test Notepad: put the caret in the field before the recording shortcut, switch apps while recording, then finish. Text should return to the original field without sending. Changed content/caret, a different field, closed/disabled/read-only/password/unsupported controls preserve the result for copying. Native and eligible UIA selected text support edits/questions through the shared pipeline. The guide currently advertises dictation and translation only.

`--build-info <output.json>` writes build identity and exits without starting the UI, touching credentials or making network/audio requests. `--windows-smoke --smoke-test <output.json>` uses isolated local state and reports native WebView/IPC startup. The CI probe terminates only the process it starts. These probes do not certify real desktop receipt.

The native Win32 adapter supports focused `Edit`, `RichEdit20W`, `RICHEDIT50W` and `RichEditD2DPT` controls only when the Win32 text and selection APIs respond consistently. Windows 11 Notepad still needs real-device receipt validation. It captures only the explicitly focused control at recording start, keeps at most 256K UTF-16 units in memory, and sends only the selected portion to the model. No input snapshot is written to history/logs. Process creation time, window/control identity, full contents and caret/selection must still match. Foreground restoration must be accepted by Windows. Direct `EM_REPLACESEL` is followed by a content/caret receipt check; no synthesized keyboard/Enter or clipboard writes are used. The final check and write are not an atomic transaction against concurrent user edits. A receipt failure may follow a partial write: inspect the source before copying, to avoid duplicates. Other fields are evaluated by the UIA capability adapter; opaque or unverified fields keep results for copying.

The Windows runner includes a separate native Edit/RichEdit fixture process for real message receipt, Unicode/newlines, selected replacement, content/caret/field conflicts, stale/reused tokens, app restoration, closed/disabled/password/read-only targets and clipboard preservation. Browser guide/overlay checks use IPC doubles. Neither is real laptop acceptance. Upgrades retain the same preview identity, settings, history, learning and credentials.
