# Windows preview

This branch develops the Windows x64 client. Candidates are unsigned GitHub Actions artifacts, not a public Windows release. Windows 10 1809+ and WebView2 are the build target; the first desktop test target is Windows 11 x64.

Build on a Windows host with Rust MSVC, Visual C++ build tools, Node 22+ and WebView2:

```powershell
npm ci
cd desktop
npx tauri build --target x86_64-pc-windows-msvc --config tauri.windows.conf.json --bundles nsis -- --locked
cd ..
python scripts/package-windows.py --commit (git rev-parse HEAD) --run-id 0
```

The `Windows candidate` workflow runs workspace tests and Clippy before producing a per-user NSIS installer, portable ZIP, source/run identity and SHA256SUMS. It does not publish Releases, sign binaries, update the website or merge branches. The existing Mac/Linux checks also remain enabled.

Keep candidate data separate: `app.vaovao.windows.preview`. Do not import another installation's credentials, settings, history or learning. Default shortcuts are Ctrl+Alt+Space (record), Ctrl+Alt+T (translation) and Escape (cancel while active). Hardware Fn is not supported on Windows; choose a normal combination in Settings.

CI compilation is not microphone, hotkey, selection, focus or input-delivery acceptance. Test these in a logged-in desktop session. SSH belongs to a different session. Current platform gaps and desktop test instructions will be updated with each candidate.
