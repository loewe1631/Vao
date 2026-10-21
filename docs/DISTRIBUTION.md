# Distribution

Source releases do not require Apple Developer Program membership. Free/open-source binaries still undergo Gatekeeper checks. Current direct downloads are not Apple-notarized; do not claim otherwise based on a local build or persistent self-signed certificate.

## Current public downloads

[Vao 0.2.1](https://github.com/loewe1631/Vao/releases/tag/v0.2.1) is a unified download page for Mac and Windows. Mac DMG/ZIP and `Vao-0.2.1-source.*` match tag `v0.2.1` (build 20261021.1202); Windows EXE and `Vao-0.2.1-windows-source.*` match tag `windows-v0.2.1` (build 20261021.3013). `SHA256SUMS` covers Mac assets; `SHA256SUMS-windows` covers Windows assets. Tags and shipped binaries retain their original identities.

The Windows installer is not code-signed. It targets Windows 10 1809+ x64 and installs WebView2 from Microsoft if missing. Windows installation and build information are in [Windows](WINDOWS.md) and [Windows build](WINDOWS_BUILD.md). The default branch remains the Mac source; a unified Release does not merge platform source trees. The following signing instructions apply to Mac.

## Build identity

`release.json` owns the product, version, build and bundle identity. The host version/identifier must agree; compilation checks this. Source publication maintenance changes are listed separately in RELEASE_NOTES.md and do not replace an existing website binary automatically.

For local development:

```sh
cargo fetch --locked --target aarch64-apple-darwin
VAO_ALLOW_ADHOC=1 bash scripts/build-macos.sh
```

For a persistent identity already in Keychain:

```sh
VAO_SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)' bash scripts/build-macos.sh
```

There is no silent ad-hoc fallback. `CARGO_TARGET_DIR` may select a separate build cache. Scripts never install, publish, reset permissions or export a signing key. Never place certificate private keys, provisioning secrets or service credentials in this repository.

The [shared Xcode project](../xcode/Vao.xcodeproj) uses the same packaging scripts. Its **Vao Direct** local target explicitly permits ad-hoc signing when no signer is supplied. An explicit signer still takes precedence and fails if unavailable. See [Xcode builds](XCODE.md).

## Direct-download release

A Developer ID build signs the nested helper and application using hardened runtime and secure timestamps. After obtaining the appropriate Apple credentials, submit to Apple's notarization service, inspect the result and staple the ticket. Validate the stapled app, rearchive it with `scripts/archive-macos.py`, and test a genuinely downloaded copy on another Mac.

Record source revision, build, archive checksums and known limitations. Retain LICENSE, PROVENANCE.md, THIRD_PARTY_NOTICES.md and licenses/ in source and app resources. A valid signature does not prove macOS permissions or successful target receipt.

## Source upload

Run `python3 scripts/check-source.py` and `python3 scripts/package-source.py`. The allowlisted archive and SHA256 inventory appear in `dist/`. Upload its checked contents or the corresponding clean Git tree, not the entire parent development directory. Caches, installed apps, diagnostic outputs and private deployments are excluded. Re-run the checks after any last-minute edit.

Before opening the repository to contributions, enable GitHub private vulnerability reporting under Security if available; SECURITY.md describes the fallback without inventing a contact address. Configure branch protection separately. CI checks pull requests but has no release/deployment credentials or automatic publication step.

## App Store

This client is not sandbox-enabled or ready for App Store submission. Cross-app AX integration, private API usage, entitlements, signing and AGPL/store-term compatibility need a separate review. A paid membership or direct-download notarization does not establish App Store acceptance. The shared-source plan and remaining work are in [App Store preparation](APP_STORE.md).
