# Windows software updates

About → Software update uses `https://updates.vaovao.app/windows/stable.json`.
Checks and downloads are manual. A missing or unreachable feed is an explicit
failure; it does not mean the installed app is up to date. The client never
falls back to GitHub or the Mac feed.

The envelope matches the Mac protocol: base64 `signed` JSON plus a base64
Minisign Ed25519 prehashed `signature`. Windows schema 1 signs version, build,
notes, URL, SHA256, byte size, minimum Windows (`10.0.17763`), architecture
(`x86_64`), bundle (`app.vaovao.windows`) and installer (`nsis`). The public
verification key is shared with the existing release signing authority; no
private signing key is included. Mac identities and prereleases are rejected.
Download URLs must use same-origin HTTPS under `/downloads/windows/`.

The client bounds the download, verifies size/hash and Windows executable
format, stages it in a unique temporary directory, then asks for confirmation
inside the app. Cancelling keeps the verified download. Before launching, it
checks the bytes again and holds the file without write/delete sharing.
Recording, processing and pending insertion block installation. The session
start and installer gate use the same runtime lock.

Only the current user's registered NSIS installation can update itself; a
portable or different copy gets manual-install guidance. Confirmation starts
the verified NSIS installer with `/UPDATE /P /R` and the existing installation
directory, then exits Vao. NSIS updates the installation and reopens Vao after
success. It preserves settings, dictionary, history, Windows credentials and
the existing startup choice. The updater does not request elevation, automate
Windows security prompts or change the input method. A signed update manifest
does not make the installer Authenticode-signed; Windows may still show a
security prompt. NSIS replacement is not the Mac bundle-swap rollback system.
After installer handoff, a cancelled/failed installation may require reopening
Vao manually; do not claim automatic rollback after a failed install or launch.
The staged installer is retained for that handoff/manual recovery. Abandoned
downloads are removed when checking again or normally releasing updater state.

Existing Windows versions without this feature need a one-time manual install.
Publishing a GitHub asset alone does not create an app update. After reviewing
a stable candidate, run `python3 scripts/sign-windows-update.py --help` with the
release owner's signing-key **path**, verified candidate directory, notes file
and exact version/build download URL. The script prepares a Windows-only
overlay locally and never deploys. Do not pass an internal preview to stable.

The shared Cloudflare Pages project is `vao-updates`, production branch `main`.
Its owner must merge the Windows overlay into the complete existing update
site, preserving Mac manifests and all versioned downloads. Deploying a Windows
overlay by itself would remove the existing site and is prohibited. Add
`/windows/stable.json` with `Cache-Control: no-store` to the site's headers;
versioned `/downloads/windows/*` files are immutable. Verify signature, size,
hash, direct HTTPS, missing-path 404 and Range 206 after deployment. The website
project `vao-site` and model servers are outside this operation.

Unit/HTTP fixtures, browser IPC tests and CI NSIS lifecycle checks are engineering
evidence. The first actual Windows in-app check/download/confirm/install/reopen
flow and data retention still require a separate laptop acceptance record.
