# Windows build and validation

Windows release metadata is in `release.windows.json` and `desktop/tauri.windows.conf.json`. The product version is 0.2.1; build identity is separate. The Windows Git tag is `windows-v0.2.1`; the existing Mac `v0.2.1` tag remains unchanged.

The default `main` branch currently contains the Mac client. Use the Windows release tag for a reproducible Windows build, or `codex/windows` for ongoing Windows development:

```sh
git clone --branch windows-v0.2.1 https://github.com/loewe1631/Vao.git
cd Vao
```

On a Windows x64 build host with stable Rust, Node.js 22+, Python and the Tauri build prerequisites:

```powershell
npm ci
python scripts/check-source.py
cargo +stable test --workspace --locked
cargo +stable clippy --workspace --all-targets --locked -- -D warnings
cd desktop
npx tauri build --target x86_64-pc-windows-msvc --config tauri.windows.conf.json --bundles nsis -- --locked
```

The Windows workflow also builds debug native fixtures, checks WebView2 input, Unicode receipt, paste/clipboard ownership, hidden startup/manual activation, and an isolated installer/update/uninstall lifecycle. Its branding check verifies default PE icon resources and the complete installed executable against the built application, permitting only Tauri's fixed NSIS bundle-type marker. These tests use synthetic content without recording audio or contacting models. Installer and startup probes refuse non-CI hosts to protect real settings.

Browser UI tests use synthetic IPC. They do not establish microphone, login, destination-app or model quality. Desktop acceptance is separate. The source inventory excludes installed apps, caches, diagnostic outputs, credentials and private project material. Run source checks before archiving and scan the final artifacts before publication. The workflow itself publishes no Release and holds no deployment credentials.

Release data uses `app.vaovao.windows`; early test data uses a separate identity. No implicit settings, history or credential migration occurs. Startup defaults to off and respects Windows system disablement. Uninstall hooks remove only matching startup ownership. TSF registration and changing the default input method are not part of installation.

`node scripts/generate-windows-branding.cjs` regenerates the dedicated Windows ICO and installer BMP files from Vao's existing SVG logo. Mac assets are independent. Retain LICENSE, PROVENANCE.md, THIRD_PARTY_NOTICES.md and licenses/ in distributions.
