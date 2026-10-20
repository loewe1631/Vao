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

## Candidate 3002 scope

| Area | Implementation | Remaining evidence/work |
| --- | --- | --- |
| Install/start | Per-user NSIS, portable ZIP, isolated identity, WebView IPC probe | 3001 installation/start confirmed by user; 3002 upgrade/uninstall pending |
| Credentials | Windows Credential Manager; URL-bound connection keys | Native round-trip test runs in Windows CI; user setup test |
| Audio/models | CPAL WASAPI, microphone access probe, existing stage routing/cancellation | 3001 Chinese refinement/translation confirmed by user; 3002 regression pending |
| Shortcuts | Ordinary customizable RegisterHotKey combinations, conflict rollback | Physical triggers and conflicts on laptop; no hardware Fn |
| Overlay | No native frame/shadow, content-sized height, WS_EX_NOACTIVATE, work-area/DPI placement and dragging | Focus, multiple monitors and scaling on laptop |
| Result | Verified native-field insertion, overlay/history and explicit copy; failures preserve text | Clipboard test on laptop |
| Selection/input | Focused native Edit/RichEdit snapshot; one-shot direct insertion with receipt check | Real Windows 11 Notepad test; browser/Chromium/UIA fields remain unsupported |

Install the candidate in the normal logged-in user desktop, then configure custom ASR/LLM connections or explicitly enroll in the trial. Test Ctrl+Alt+Space twice, a clear Chinese/English recording, translation, Escape cancellation, no microphone, denied microphone access and a failed model connection. Confirm the result remains available and focus is preserved. Enable automatic insertion and test Notepad: put the caret in the field before the recording shortcut, switch apps while recording, then finish. Text should return to the original field without sending. Changed content/caret, a different field, closed/disabled/read-only/password/unsupported controls preserve the result for copying. Native selected text supports edits/questions through the shared pipeline; generic UIA selection support is not included. The guide currently advertises dictation and translation only.

`--build-info <output.json>` writes build identity and exits without starting the UI, touching credentials or making network/audio requests. `--windows-smoke --smoke-test <output.json>` uses isolated local state and reports native WebView/IPC startup. The CI probe terminates only the process it starts. These probes do not certify real desktop receipt.

Native insertion supports focused `Edit`, `RichEdit20W`, `RICHEDIT50W` and `RichEditD2DPT` controls only when the Win32 text and selection APIs respond consistently. Windows 11 Notepad still needs real-device receipt validation. It captures only the explicitly focused control at recording start, keeps at most 256K UTF-16 units in memory, and sends only the selected portion to the model. No input snapshot is written to history/logs. Process creation time, window/control identity, full contents and caret/selection must still match. Foreground restoration must be accepted by Windows. Direct `EM_REPLACESEL` is followed by a content/caret receipt check; no synthesized keyboard/Enter or clipboard writes are used. The final check and write are not an atomic transaction against concurrent user edits. A receipt failure may follow a partial write: inspect the source before copying, to avoid duplicates. Unknown browser/chat controls use manual copying.

The Windows runner includes a separate native Edit/RichEdit fixture process for real message receipt, Unicode/newlines, selected replacement, content/caret/field conflicts, stale/reused tokens, app restoration, closed/disabled/password/read-only targets and clipboard preservation. Browser guide/overlay checks use IPC doubles. Neither is real laptop acceptance. Upgrades retain the same preview identity, settings, history, learning and credentials.
