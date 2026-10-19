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

## First candidate scope

| Area | Implementation | Remaining evidence/work |
| --- | --- | --- |
| Install/start | Per-user NSIS, portable ZIP, isolated identity, WebView IPC probe | Actual laptop installation, upgrade and uninstall |
| Credentials | Windows Credential Manager; URL-bound connection keys | Native round-trip test runs in Windows CI; user setup test |
| Audio/models | CPAL WASAPI, microphone access probe, existing stage routing/cancellation | Real microphone → ASR → refinement on laptop |
| Shortcuts | Ordinary customizable RegisterHotKey combinations, conflict rollback | Physical triggers and conflicts on laptop; no hardware Fn |
| Overlay | WS_EX_NOACTIVATE, no-activation show/position, work-area/DPI placement, dragging | Focus, multiple monitors and scaling on laptop |
| Result | Overlay, history, explicit manual copy; failures preserve text | Clipboard test on laptop |
| Selection/input | Disabled; no speculative paste into external apps | UIA target identity, selection, conflict checks and verified insertion in W2 |

Install the candidate in the normal logged-in user desktop, then configure custom ASR/LLM connections or explicitly enroll in the trial. Test Ctrl+Alt+Space twice, a clear Chinese/English recording, translation, Escape cancellation, no microphone, denied microphone access and a failed model connection. Confirm the result remains available and focus is preserved. Copy into Notepad manually. Selection questions and automatic original-target insertion are not included in this candidate.

`--build-info <output.json>` writes build identity and exits without starting the UI, touching credentials or making network/audio requests. `--windows-smoke --smoke-test <output.json>` uses isolated local state and reports native WebView/IPC startup. The CI probe terminates only the process it starts. These probes do not certify real desktop receipt.
