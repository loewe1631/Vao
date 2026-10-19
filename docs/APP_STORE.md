# App Store preparation

The current application is the direct-download Mac client. The Xcode project builds that client; it does not establish App Store eligibility. There is no completed store target or store binary in this release.

## Shared source

Keep scene selection, writing presets, preference learning, model protocols, history and UI in the existing workspace. The macOS platform boundary owns selection capture, shortcuts, audio and delivery. A future store target should provide its supported platform capabilities and packaging configuration without copying the whole application into another source tree.

GitHub is the common development repository. Source tags identify releases; pre-release binaries need their own acceptance before distribution. Website and App Store packages may have different supported capabilities and schedules, but shared changes should remain in this repository.

## Work still required

- Validate recording, shortcuts, Keychain, model networking and data handling in the actual sandbox configuration.
- Design supported alternatives for cross-application Accessibility selection/focus and synthesized input. Current AX integration is not established as sandbox-compatible; a helper outside the sandbox is not a workaround for store restrictions.
- Review current private macOS API use, entitlements, privacy declarations and platform API choices.
- Review AGPL and all redistributed dependencies against the proposed store distribution terms before submission. This release does not change or promise an alternative license.
- Once membership is active, configure the developer's own identifiers, provisioning and distribution signing outside version control; then validate archive/export, installation, upgrades and App Review requirements.

Compiler success, an Apple certificate and direct-download notarization each establish different facts. None alone proves sandbox compatibility or store approval. iOS requires a separate product/platform design and is not implemented here.
