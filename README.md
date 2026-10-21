# Vao

Your words. More like you.

[中文](README.zh-CN.md) · [Website and downloads](https://vaovao.app/) · [Privacy](PRIVACY.md) · [Architecture](docs/ARCHITECTURE.md) · [Contributing](CONTRIBUTING.md)

Vao is an open-source voice-input app for macOS and Windows. It turns speech into usable text, chooses writing styles from your content and destination app, and remembers the layout choices you explicitly ask it to keep.

**0.2.1 · Available for macOS and Windows.** The [unified release](https://github.com/loewe1631/Vao/releases/tag/v0.2.1) includes both platforms, matching sources and checksums.

| Platform | Download | Requirements |
| --- | --- | --- |
| macOS | [DMG](https://github.com/loewe1631/Vao/releases/download/v0.2.1/Vao-0.2.1-build20261021.1202-mac-arm64.dmg) · [ZIP](https://github.com/loewe1631/Vao/releases/download/v0.2.1/Vao-0.2.1-build20261021.1202-mac-arm64.zip) | Apple Silicon · macOS 12+ |
| Windows | [EXE installer](https://github.com/loewe1631/Vao/releases/download/v0.2.1/Vao-0.2.1-setup.exe) | Windows 10 1809+ · x64; downloads WebView2 if missing |

Mac downloads are not yet Apple-notarized; the Windows installer is not code-signed. See the [installation guide](https://vaovao.app/install/) for first-launch prompts. Intel Mac is not available. See [release notes](RELEASE_NOTES.md) for source and build identities.

## Features

- **Speak and insert.** Refine speech and fill a destination input field, without sending the message.
- **Adapt to context.** Built-in presets cover messages, tasks, plans, technical records and documents. Content takes priority; app hints resolve remaining cases. No manual task/chat switch.
- **Keep your choices.** Remember paragraph or numbered-list preferences for a context. One explicit choice takes effect next time. Pause, disable, undo or delete it whenever you want.
- **Select and ask.** Highlight text, press the main shortcut and speak a question or an edit. Answers appear above the voice bar; edits can replace the selection.
- **Translate a segment.** Start translated recording directly, or turn translation on and off while recording.
- **Switch apps while speaking.** On Mac, return to the validated starting input. Windows defaults to the current caret when ready, with an optional return-to-start mode.

- **Optional input enhancements.** Independent recording-preview switch, bounded input context and optional window reference (nearby text on Mac; title only on Windows); Realtime requires an explicitly compatible service.
- **Portable vocabulary.** Merge and deduplicate TXT/JSON imports; export terms only.

Selection operations cover editing, translation, composing replies and understanding the selected text. Custom text services run an additional purpose check, adding a request, waiting time and possible cost. Unrelated requests or failed checks stop. Dictated questions remain input text rather than questions answered by Vao.

Adaptation currently learns explicit layout choices for messages, tasks and technical writing. It does not monitor unrelated edits or learn silently from model output. Long-term adaptation quality remains to be evaluated. See [writing presets](docs/WRITING_PRESETS.md).

## Install and start

1. Choose your platform at [vaovao.app](https://vaovao.app/). On Mac, drag **Vao.app** to **Applications**; on Windows, run **Vao-0.2.1-setup.exe** and follow the installer.
2. Open Vao. See [installation and permissions](https://vaovao.app/install/) for system prompts and the [Windows guide](docs/WINDOWS.md) for platform-specific steps.
3. Follow the first-run checklist: activate the optional trial or connect your models, grant needed permissions and check shortcuts.
4. Click your destination input, press the main shortcut, speak, then press it again to finish.

| Default shortcut | Action |
| --- | --- |
| Mac: Control + Option + Space; Windows: Ctrl + Alt + Space | Start or finish recording |
| Mac: Control + Option + T; Windows: Ctrl + Alt + T | Start translation when idle; toggle translation during recording |
| Escape | Cancel the active session |

On Mac, shortcuts support key capture or typed entry, including **Fn** and **Fn+Space**. Fn needs Accessibility and may conflict with the system Globe-key action. If Vao is absent from Input Monitoring, add the installed app with the system **+** button, enable it and restart. Requesting a permission does not grant it.

Automatic insertion is enabled by default; turn it off to preview and copy first. Selecting text and pressing the main shortcut immediately records your instruction. Questions display and copy an answer without replacing the source. Edits try to replace the selection when automatic insertion is enabled.

On Mac, cross-app insertion depends on the destination's Accessibility support. Closed windows, changed contents or conflicting caret positions preserve the result instead of pasting elsewhere. Opaque inputs keep stricter foreground checks. WeChat currently needs a click in its message field. A posted paste event does not prove receipt by another app.

Windows uses regular key combinations, without hardware Fn support. Ordinary dictation pastes at the current caret when ready; keep the destination focused during processing. Returning to the starting input is optional and cannot restore every app’s caret. Sign-in startup is off by default.

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

Audio uses a temporary session buffer and is sent to the chosen ASR; no audio history is retained. Local text history is limited to seven days and 200 results. Keys stay in macOS Keychain or Windows Credential Manager. Dictionary terms and relevant preferences accompany model requests. Selected text is sent for the selected-text operation. Read [PRIVACY.md](PRIVACY.md) for permissions, clipboard use and retention.

## Source and builds

The default `main` branch currently retains the Mac client and Xcode entry point. Windows release source is at [`windows-v0.2.1`](https://github.com/loewe1631/Vao/tree/windows-v0.2.1); ongoing development uses [`codex/windows`](https://github.com/loewe1631/Vao/tree/codex/windows). The tags remain separate. GitHub’s automatic “Source code” links on the unified release refer to the Mac tag; for Windows use the `windows-source` archive or its tag. See [Windows build steps](docs/WINDOWS_BUILD.md).

### Mac build and check

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

Start with [contributing](CONTRIBUTING.md), the [roadmap](docs/ROADMAP.md) and [security reporting](SECURITY.md). Cross-platform app compatibility, model protocols, synthetic cases and accessibility are useful contribution areas.

AGPL-3.0-only. Product direction comes from experience using and adapting several open-source projects. This client is Vao's own implementation. [Provenance](PROVENANCE.md), [asset sources](docs/SOURCES.md) and third-party notices must remain with redistributed material.
