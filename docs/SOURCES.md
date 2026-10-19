# Source and dependency inventory

The application code under crates/, desktop/src/, platform/, ui/app.js, ui/style.css and scripts/ is Vao client implementation from the project's own requirements. Existing Vao concept and interface requirements informed the presentation.

## Brand assets

ui/assets/logo.svg, desktop/icons/32x32.png, desktop/icons/128x128.png and desktop/icons/icon.icns are Vao brand exports dated 2026-10-17. No icon-renderer application code was copied from another product. Brand and trademark rights are separate from dependency licenses.

## Runtime dependencies

Cargo.lock records resolved versions. Major direct dependencies: Tauri and its runtime/build components (MIT/Apache-2.0), serde/serde_json (MIT/Apache-2.0), tokio (MIT), reqwest (MIT/Apache-2.0), futures-util (MIT/Apache-2.0), cpal (Apache-2.0), global-hotkey (MIT/Apache-2.0), rusqlite (MIT), url (MIT/Apache-2.0), keyring (MIT/Apache-2.0), and tauri-plugin-single-instance (MIT/Apache-2.0). Preserve dependency notices and perform the full resolved dependency/assets review before public distribution; this shortlist is not a completed license audit.

The UI has no bundled third-party JavaScript runtime/framework or remote fonts. Node/Playwright are development-only test tools, not copied into the app. The GNU AGPL license text is reproduced as license material.

The macOS adapter uses core-graphics (MIT/Apache-2.0) and core-foundation (MIT/Apache-2.0) as direct macOS dependencies for the platform Fn event tap. Native matcher/Fn state handling belongs to this client. It emits only configured actions or explicitly armed shortcut capture, and stores no typed text.

## Public beta additions

Public first-run UI, per-installation enrollment and packaging are part of this client. The hosted trial service is operated separately and is not included in this source tree. Third-party Rust license texts and their pinned sources are indexed in licenses/index.json. Model runtime dependencies and weights are not redistributed.

## Dependency maintenance status

A RustSec check on 2026-10-19 reported no vulnerability advisories for Cargo.lock. Five `unic-*` 0.9.0 crates used through Tauri → tauri-utils → urlpattern are marked unmaintained: unic-char-property, unic-char-range, unic-common, unic-ucd-ident and unic-ucd-version. These are upstream maintenance warnings, not a claim of a discovered Vao exploit or of zero risk. Track an upstream replacement instead of silently suppressing the warnings.

The full lockfile also includes packages for other platforms, including a glib unsoundness warning outside the Mac dependency graph. Any new platform needs its own dependency and license review. Advisory results change over time; run a current audit before distributing updates.
