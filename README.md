# Vao

Your words. More like you.

[中文](README.zh-CN.md) · [Website and downloads](https://vaovao.app/) · [Privacy](PRIVACY.md) · [Architecture](docs/ARCHITECTURE.md) · [Contributing](CONTRIBUTING.md)

Vao is an open-source Mac voice-input app. It turns speech into usable text, chooses writing styles from your content and destination app, and remembers the layout choices you explicitly ask it to keep.

**0.2.0 · Apple Silicon · macOS 12+.** This release ships Mac installers only. Windows adapters are being developed on a separate branch; Intel Mac builds are not available. Current direct downloads are not Apple-notarized. See [release notes](RELEASE_NOTES.md) for source and binary versions.

## Features

- **Speak and insert.** Refine speech and fill the original input field, without sending the message.
- **Adapt to context.** Built-in presets cover messages, tasks, plans, technical records and documents. Content takes priority; app hints resolve remaining cases. No manual task/chat switch.
- **Keep your choices.** Remember paragraph or numbered-list preferences for a context. One explicit choice takes effect next time. Pause, disable, undo or delete it whenever you want.
- **Select and ask.** Highlight text, press the main shortcut and speak a question or an edit. Answers appear above the voice bar; edits can replace the selection.
- **Translate a segment.** Start translated recording directly, or turn translation on and off while recording.
- **Switch apps while speaking.** Consult another app, then return to the starting input when it still passes target validation.

- **Optional input enhancements.** Independent recording-preview switch, bounded input context and optional nearby text; Realtime requires an explicitly compatible service.
- **Portable vocabulary.** Merge and deduplicate TXT/JSON imports; export terms only.

Selection operations cover editing, translation, composing replies and understanding the selected text. Custom text services run an additional purpose check, adding a request, waiting time and possible cost. Unrelated requests or failed checks stop. Dictated questions remain input text rather than questions answered by Vao.

Adaptation currently learns explicit layout choices for messages, tasks and technical writing. It does not monitor unrelated edits or learn silently from model output. Long-term adaptation quality remains to be evaluated. See [writing presets](docs/WRITING_PRESETS.md).

## Install and start

1. Download from [vaovao.app](https://vaovao.app/), extract the ZIP or open the DMG, then drag **Vao.app** to **Applications**.
2. Open Vao. If macOS blocks an unnotarized download, follow [the installation guide](https://vaovao.app/install/). Do not disable Gatekeeper globally.
3. Follow the first-run checklist: activate the optional trial or connect your models, grant needed permissions and check shortcuts.
4. Click your destination input, press the main shortcut, speak, then press it again to finish.

| Default shortcut | Action |
| --- | --- |
| Control + Option + Space | Start or finish recording |
| Control + Option + T | Start translation when idle; toggle translation during recording |
| Escape | Cancel the active session |

Shortcuts support key capture or typed entry, including **Fn** and **Fn+Space**. Fn needs Accessibility and may conflict with the system Globe-key action. If Vao is absent from Input Monitoring, add the installed app with the system **+** button, enable it and restart. Requesting a permission does not grant it.

Automatic insertion is enabled by default; turn it off to preview and copy first. Selecting text and pressing the main shortcut immediately records your instruction. Questions display and copy an answer without replacing the source. Edits try to replace the selection when automatic insertion is enabled.

Cross-app insertion depends on the destination's Accessibility support. Closed windows, changed contents or conflicting caret positions preserve the result instead of pasting elsewhere. Opaque inputs keep stricter foreground checks. WeChat currently needs a click in its message field. A posted paste event does not prove receipt by another app.

## Models and optional trial

Configure ASR and text independently. Enter a URL and optional key, fetch models, select one and save; Advanced allows manual IDs. A model list proves discovery only. Speech and text connection tests validate the selected protocol and show the fixed input, actual output and round-trip latency. Speech tests use a built-in English phrase without enabling the microphone; provider usage applies. Realtime diagnostics require final WebSocket transcription and never fall back to file recognition. Follow with a short real recording to check your microphone and output quality.

| Stage | Supported contract |
| --- | --- |
| ASR | OpenAI-compatible multipart `/v1/audio/transcriptions`, returning JSON `text` |
| Realtime ASR (explicit selection) | OpenAI Realtime transcription WebSocket sessions; requires compatible service and model support, independently of file transcription |
| Text, translation, selected-text requests | `/v1/chat/completions`, returning a final non-streaming message |
| Restricted refinement | Vao `/v1/refine`; ordinary dictation only |

For local ASR consider **Qwen3-ASR-1.7B**, or **0.6B** with fewer resources, through a server exposing this contract. NVIDIA and Apple Silicon MLX services need protocol checks; a model name does not prove compatibility. Model weights and servers are not bundled. Remote endpoints require HTTPS; loopback HTTP is supported.

The optional hosted trial is for testing before connecting your own service. It requires explicit privacy consent and issues an individual credential; no shared API key is embedded. Shared capacity and queues can slow responses; availability and quality are not guaranteed. Use your own cloud or local models for regular use. [Service and data processing](PRIVACY.md).

Custom ASR and LLM connections take priority independently. Failures never silently fall back to the trial. An explicitly configured backup LLM handles only transport errors, HTTP 429 and server errors.

## Privacy

Audio uses a temporary session buffer and is sent to the chosen ASR; no audio history is retained. Local text history is limited to seven days and 200 results. Keys stay in macOS Keychain. Dictionary terms and relevant preferences accompany model requests. Selected text is sent for the selected-text operation. Read [PRIVACY.md](PRIVACY.md) for permissions, clipboard use and retention.

## Build and check

Use Apple Silicon, Xcode Command Line Tools, Python 3.11+, Node.js 22+ and stable Rust with Clippy. Fetch locked dependencies, then build:

With full Xcode installed, you can instead open **[xcode/Vao.xcodeproj](xcode/Vao.xcodeproj)**, select **Vao Direct** and choose **Product → Build**. This builds the same complete client with local ad-hoc signing. See [Xcode setup](docs/XCODE.md). The current target is for direct installation; [App Store preparation](docs/APP_STORE.md) is separate work in this repository.

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
python3 scripts/package-source.py
```

This explicitly opts into **local ad-hoc signing**. It is not Apple notarization; changing signatures may require new permissions. Use `VAO_SIGNING_IDENTITY` for persistent signing; see [distribution](docs/DISTRIBUTION.md). Packages go to `dist/`; intermediate build files use `.build/` and the Cargo target directory. Scripts support `CARGO_TARGET_DIR` and never install or replace apps.

Browser checks use synthetic IPC doubles; they do not certify microphone, model quality or delivery. See [testing](docs/TESTING.md) for native checks and side effects. Source archives include a SHA256 inventory and exclude local builds/user data. Refresh [third-party notices](THIRD_PARTY_NOTICES.md) with `python3 scripts/collect-notices.py` when Rust dependencies change.

## Contribute and license

Start with [contributing](CONTRIBUTING.md), the [roadmap](docs/ROADMAP.md) and [security reporting](SECURITY.md). Windows adapters, model compatibility, synthetic cases and accessibility are useful contribution areas.

AGPL-3.0-only. Product direction comes from experience using and adapting several open-source projects. This client is Vao's own implementation. [Provenance](PROVENANCE.md), [asset sources](docs/SOURCES.md) and third-party notices must remain with redistributed material.
