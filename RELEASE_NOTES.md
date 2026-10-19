# Vao 0.1.0-beta.5

## Source release · build 20261019.1011

- Added a shared Xcode project and scheme that build the same complete client as the command-line scripts, without maintainer-specific paths or signing identities.
- Documented the shared-source distribution plan and remaining App Store sandbox work. The current Xcode target is a non-sandboxed direct-install build.
- Added Xcode user-state exclusion checks and a Mac CI build of the shared scheme.

- Updated bilingual setup/build/contribution documentation and removed obsolete development-stage descriptions.
- Consolidated UI translations and corrected selection/automatic-insertion guidance.
- Replaced personal diagnostic examples with synthetic questions.
- Added checked source packaging, boundary tests, locked browser-test dependencies and CI checks.
- Bounded trial JSON responses and blocked unfinished/expired results from learning.
- Supported separate Cargo build caches during packaging.

This is a source release. No new installable binary accompanies this tag; the website download remains Beta4/build20261019.1009 until a separately accepted binary update. Installed applications are not updated by a source publication.

## Previous binary release · 0.1.0-beta.4 / build 20261019.1009

- Built-in message, action, plan, technical-record and document presets work before any layout feedback. Content rules and app hints choose the preset automatically.
- Saved per-scene layout preferences remain in effect. Prompt examples match the chosen layout; paragraph preference still separates shared restrictions and diagnostic facts.
- Distinguishes task instructions from observations and generic everyday verbs. Technical records preserve unverified scope, hypotheses and version/error terminology.
- Keeps cancellation information, quoted examples, outside explanations, currency differences and uncertain plans. Single actions keep their attributes together.
- Translation uses an independent prompt; selected-text instructions remain separate from ordinary dictation.
- Narrow output checks repair lost dates or protected quote content once, preserving the source and marking incomplete refinement if repair still fails. Single numbered items are normalized without a manual review step.
- Includes 30 fixed-text replay cases, request-contract tests and an opt-in read-only model replay command. These checks do not replace real speech and cross-app acceptance.

## Current behavior and limitations

- Automatic insertion defaults on for new users; explicit existing settings are retained. Selection questions display answers without replacing source text. Edits can replace the selection.
- Recording locks the starting field and can return after app switches when its content/caret snapshot remains valid. Opaque targets retain stricter foreground checks; WeChat needs a click in the input field.
- Nonactivating voice bar and answer overlay support dragging and multiple displays. Empty speech and missing microphones use small notices.
- Custom ASR and primary/backup LLM connections support discovery and text connection tests with latency, input and output. No silent custom-to-trial fallback.
- Optional trial activation requires consent and individual credentials. Shared queues can delay output; use your own service for regular work. Current limits are in PRIVACY.md.
- Fn and Command combinations support native key capture or typed entry. Permission state reflects macOS; manual-add guidance is provided for Input Monitoring.
- No Windows/Intel release, Apple notarization or App Store compatibility is claimed. Synthetic tests do not replace fresh-Mac or arbitrary-app acceptance.
