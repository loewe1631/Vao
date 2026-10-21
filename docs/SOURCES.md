# Source and dependency inventory

The application code under crates/, desktop/src/, platform/, ui/app.js, ui/style.css and scripts/ is Vao client implementation from the project's own requirements. Existing Vao concept and interface requirements informed the presentation.

## Brand assets

ui/assets/logo.svg, desktop/icons/32x32.png, desktop/icons/128x128.png and desktop/icons/icon.icns are Vao brand exports dated 2026-10-17. The Windows ICO and installer BMP files under desktop/icons/windows* are exports of the same Vao SVG, rendered by scripts/generate-windows-branding.cjs on 2026-10-21. No icon-renderer application code was copied from another product. Brand and trademark rights are separate from dependency licenses.

## Runtime dependencies

Cargo.lock records resolved versions. Major direct dependencies: Tauri and its runtime/build components (MIT/Apache-2.0), serde/serde_json (MIT/Apache-2.0), tokio (MIT), reqwest (MIT/Apache-2.0), futures-util (MIT/Apache-2.0), cpal (Apache-2.0), global-hotkey (MIT/Apache-2.0), rusqlite (MIT), url (MIT/Apache-2.0), keyring (MIT/Apache-2.0), and tauri-plugin-single-instance (MIT/Apache-2.0). Preserve dependency notices and perform the full resolved dependency/assets review before public distribution; this shortlist is not a completed license audit.

The UI has no bundled third-party JavaScript runtime/framework or remote fonts. Node/Playwright are development-only test tools, not copied into the app. The GNU AGPL license text is reproduced as license material.

The macOS adapter uses core-graphics (MIT/Apache-2.0) and core-foundation (MIT/Apache-2.0) as direct macOS dependencies for the platform Fn event tap. Native matcher/Fn state handling belongs to this client. It emits only configured actions or explicitly armed shortcut capture, and stores no typed text.

## Public beta additions

Public first-run UI, per-installation enrollment and packaging are part of this client. The hosted service is operated separately. This tree includes generic selection-policy adapters in scripts/gateway/, not production configuration, credentials, databases or a complete hosted-service implementation. Third-party Rust license texts and their pinned sources are indexed in licenses/index.json. Model runtime dependencies and weights are not redistributed.

## Dependency maintenance status

The 0.2.0 review on 2026-10-21 updated rustls-webpki to 0.103.16 and webpki-roots to 1.0.10 for RUSTSEC-2026-0382 / RUSTSEC-2026-0381 (distrusted certificate authorities). A refreshed RustSec scan reported no vulnerability advisories for the Mac release graph. Five `unic-*` 0.9.0 crates used through Tauri → tauri-utils → urlpattern are marked unmaintained: unic-char-property, unic-char-range, unic-common, unic-ucd-ident and unic-ucd-version. These are upstream maintenance warnings, not a claim of a discovered Vao exploit or of zero risk. Track an upstream replacement instead of silently suppressing the warnings.

The full lockfile also includes packages for other platforms, including a glib unsoundness warning outside the Mac dependency graph. Any new platform needs its own dependency and license review. Advisory results change over time; run a current audit before distributing updates.

## Connection-test audio

`desktop/src/asr_test_audio.json` contains a 2.351-second, 16 kHz mono PCM16 synthetic English phrase ("This is a speech recognition test."), generated offline with eSpeak NG 1.52.0, en-us voice, rate 155, then resampled. It contains no user recording. No speech engine or voice model is redistributed. The generator is [eSpeak NG](https://github.com/espeak-ng/espeak-ng); the PCM SHA256 and generation details are included in the asset. This fixed test phrase is only sent when the user requests an ASR connection test, using the selected provider and protocol.
