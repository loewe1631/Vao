# Security

Security fixes target the latest public beta. Older builds may not receive backports.

## Reporting

Use this repository's **Security → Advisories → Report a vulnerability** when the maintainer has enabled private vulnerability reporting. If the button is unavailable, open an issue asking for a private channel, without describing the vulnerability or attaching sensitive files. Do not post exploit details, keys, recordings, clipboard contents or private text publicly.

Through the private channel, provide version/build, operating system, expected behavior and a minimal synthetic reproduction. No response time is guaranteed. Rotate exposed keys at the provider; deleting a comment does not revoke them.

## Boundaries

- The WebView loads bundled assets under a Content Security Policy. Provider text is displayed as text, never executed as HTML or a command.
- Model keys are bound to service URLs and stored in Keychain. HTTP is limited to loopback providers; requests do not follow redirects.
- The trial URL is a public gateway, not a credential. Enrollment needs consent and issues an individual credential. The separately operated server is outside this repository.
- Accessibility and clipboard operations support user-initiated workflows. Posting a paste event cannot prove receipt.
- This beta is not an App Sandbox or App Store build. A local build is not notarization or an audit of every provider or target app.

Before uploading, run `python3 scripts/check-source.py`. It checks inventory, private-file rules and common secret patterns, but cannot prove that every possible secret is absent. Review dependencies and binaries separately.
