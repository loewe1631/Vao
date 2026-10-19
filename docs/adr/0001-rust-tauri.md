# ADR 0001: Rust shared core, Tauri UI, platform adapters

Date: 2026-10-18. Accepted for the first Mac alpha after the cross-platform discussion.

## Context

The first release targets Mac and Windows, with a Mac test build first. A native Swift-only client would optimize Mac integration but increase the cost of a shared future implementation. This application is a separate Vao implementation.

## Decision

Use a Rust workspace with UI-independent session/adaptation contracts; shared HTML/CSS/JavaScript rendered by Tauri; per-platform native capabilities behind adapters. Use SQLite for bounded state/history and the native credential store for keys. Begin with a modular desktop app. The macOS Accessibility/pasteboard helper is a small bundled child process retaining opaque target references, not a second speech/session engine.

## Alternatives and consequences

Two fully native UIs with a Rust core would improve platform-specific control but require two presentation implementations plus language bridges. Flutter remains possible but does not remove platform integration work. Tauri reuses its generic runtime. Framework reuse does not establish source provenance.

Core tests run without Tauri, credentials or microphone. The current helper and keychain are macOS only; Windows adapters, CI and installer tests remain necessary. No performance superiority or clean-room claim is made. Package boundaries may be refined as the second platform is implemented.
