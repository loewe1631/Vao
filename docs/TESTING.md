# Testing

## Repeatable local checks

```sh
cargo fetch --locked --target aarch64-apple-darwin
cargo test --offline --workspace --locked
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
npm ci
npx playwright install chromium
npm run test:ui
npm run test:source
npm run check:source
```

Rust tests use generated audio, loopback mock HTTP and temporary/in-memory databases. Browser tests use explicit IPC doubles and write screenshots/results under `artifacts/checks/`. They do not call production models, capture a microphone, grant permissions or send messages. `VAO_PLAYWRIGHT_MODULE` can select an existing Playwright installation; normal contributors should use the locked npm dependency.

`check-source.py` inspects the same inventory as `package-source.py`. Boundary tests exercise private files, symlink escapes, redacted reports and deterministic archives. Dependency vulnerability reports are time-dependent and should be checked again before releases.

## Build validation

```sh
VAO_ALLOW_ADHOC=1 bash scripts/build-macos.sh
python3 scripts/package-source.py
```

The build packages and signs `dist/Vao.app`, checks its metadata/helper identity and creates a ZIP. It does not install or launch it. `CARGO_TARGET_DIR` may select a separate cache. Ad-hoc signing is for local development; a self-signed persistent identity is still not Apple notarization.

With full Xcode installed, also verify the shared build entry:

```sh
xcodebuild -project xcode/Vao.xcodeproj -scheme 'Vao Direct' -configuration Release build
```

It builds the same source and output; it does not provide an App Store archive. See [Xcode builds](XCODE.md) for toolchain/signing setup and Run behavior.

## Native desktop diagnostics

These are opt-in interactive integration tests, not unit tests. They can activate synthetic windows, temporarily register shortcuts, exercise the clipboard, move/click the overlay and save/restore its position. Run in a test account, quit other Vao copies and close sensitive work first. Diagnostics do not grant macOS permissions.

Prepare an empty output directory and fixtures:

```sh
mkdir -p artifacts/native
python3 scripts/prepare-native-fixtures.py artifacts/native
open -n dist/Vao.app --args --smoke-test "$PWD/artifacts/native/result.json" --verify-ui
```

`--verify-ui` requires `--smoke-test` and uses an isolated database beside the output. Credential namespace and app identity still belong to that signed app; do not supply real model configuration to the harness. Missing Accessibility access causes explicit skips, not a pass for physical Fn or cross-app input. Output and progress go to `result.json` and `result.json.trace`. Inspect failed/skipped checks and restore/quit the test app before normal use.

Real microphone, physical shortcut, another Mac's initial installation and actual target receipt each need separate acceptance. Use synthetic text, never a live chat as a paste target.

## Explicit model replay

[Writing presets](WRITING_PRESETS.md) documents the opt-in fixed-text replay. It opens settings read-only but sends fixture text to the selected model and consumes quota. A local/synthetic test pass cannot establish provider quality or universal semantic preservation.


## 0.2.2 silence auto-stop

With “Stop after silence” enabled, use the default five-second delay first. Speak valid audio, stop speaking, and confirm that the countdown appears near the deadline and the session finishes automatically. Continue speaking during the countdown and confirm that it resets. A session with no valid speech must remain open. Verify that the main shortcut still stops immediately, Escape cancels, and disabling the option restores manual-stop behavior. The local VAD does not upload audio; a detection failure must keep manual-stop behavior.

## 0.2.1 dictionary dialog regression

Test importing and exporting a small synthetic TXT/JSON dictionary, then cancelling each file dialog. Confirm that the dialog helper exits after every selection or cancellation, Vao and Finder remain interactive, and no unresponsive dialog process remains. Imported terms should be saved and deduplicated; cancelled or invalid imports must not change the saved dictionary. Follow with dictation, translation and original-target insertion to check that the long-lived input bridge remains usable.

## 0.2.0 focused acceptance

Use an isolated Mac test build and retain the previous app for rollback. Test dictionary TXT/JSON round trips, duplicates, invalid/over-capacity input and cancelling either file panel. Unsaved edits must not disappear. Confirm exports have terms only.

With both preview and context off, regress short dictation, Fn, translation, selection and original-target insertion. Then enable recording preview alone; test file windows and cancellation. For a service supporting public Realtime sessions and file transcription with the configured model, explicitly select Realtime and test confirmed stop-time output, protocol rejection and file fallback. Protocol mocks establish request/response handling only, not provider compatibility or speech quality.

Test inserting in the middle of existing text: only the new spoken phrase should be inserted. Enable bounded input context, then its separate expanded option. Verify that reference text helps continuity without being copied; switching applications must keep the original target. Compare final completeness, edits required and measured stop-to-output time. Default hosted access is file-based; backend free-use policy does not imply realtime ASR.

## Protocol-specific connection diagnostics

In Models, select a protocol under Advanced and use Test speech connection / Test text connection. Speech tests send the built-in English phrase without activating the microphone; provider usage applies. The displayed draft endpoint/model/key are used without saving or changing the connection. Success requires a nonempty response from that protocol. Realtime tests must complete session negotiation and final transcription and never use file fallback. The report includes protocol, elapsed time, source phrase and returned text. It does not grade recognition accuracy. Changing connection fields clears its old report. Test ordinary file support and Realtime independently; model discovery or a successful final recording after fallback does not prove Realtime support.
