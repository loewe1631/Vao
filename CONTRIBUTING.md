# Contributing

Read the architecture, roadmap and privacy notes before changing behavior. Open a focused issue with the problem, scope, expected result and how it should be checked.

1. Agree on ownership before editing a shared module. Session state, feedback, privacy, persistence and public contracts need an RFC first.
2. Keep changes small. Include a synthetic counterexample or a reproducible check.
3. Identify third-party sources and licenses. Do not add private recordings, chat logs, credentials, personal endpoints or another project's private configuration.
4. A pull request should say what changed, why, what was tested, and what remains unverified. Document-only changes should not claim application tests.

Human-written and AI-assisted contributions are welcome. Review the actual changes and provide reproducible checks and their results, including anything still unverified. The same contribution and licensing requirements apply regardless of the tools used. See [AI collaboration](AI_DEVELOPMENT.en.md) ([中文](AI_DEVELOPMENT.md)) for how this project is made.

Choose the correct platform source before starting. The default `main` branch currently builds Mac; Windows release source is `windows-v0.2.1`, and Windows development uses `codex/windows`. See [Windows build and validation](docs/WINDOWS_BUILD.md). Coordinate changes to shared modules across both source lines.

Build and test on Apple Silicon:

```sh
cargo fetch --locked --target aarch64-apple-darwin
cargo test --offline --workspace --locked
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
VAO_ALLOW_ADHOC=1 bash scripts/build-macos.sh
npm ci
npx playwright install chromium
npm run test:ui
npm run test:source
npm run check:source
```

The shared [Xcode project](xcode/Vao.xcodeproj) builds this same workspace; see [Xcode setup](docs/XCODE.md). Keep shared behavior in the existing core/UI/platform modules. [App Store preparation](docs/APP_STORE.md) and Windows adapters should reuse those boundaries instead of creating another copy of the client.

Use synthetic data. Do not post API keys, recordings or private text in issues. Contributions do not publish releases. The project license is AGPL-3.0-only; state that your contribution can be included under that license and identify any exception. See SECURITY.md for sensitive reports and GOVERNANCE.md for review and release responsibilities. No response-time promise is made.
