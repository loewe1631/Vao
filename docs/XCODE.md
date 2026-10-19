# Xcode builds

The shared project is an entry point into the same Cargo workspace, UI and macOS helper used by command-line builds. It contains no copied client tree, developer-specific path, signing certificate or team identifier.

## Requirements

Use an Apple Silicon Mac with macOS 12+, a full Xcode installation, Python 3.11+ and stable Rust. Node.js 22+ is needed for browser checks, not for building the app. The first build needs network access to fetch the locked Rust dependencies. Select the intended Xcode installation with the system developer tools setting if needed.

## Build

Open `xcode/Vao.xcodeproj`, select the shared **Vao Direct** scheme and choose **Product → Build**. The project uses an External Build System target to call `scripts/build-xcode.sh`, which runs the existing Mac build and packaging scripts.

The equivalent terminal command, from the repository root, is:

```sh
xcodebuild -project xcode/Vao.xcodeproj -scheme 'Vao Direct' -configuration Release build
```

Both paths build an optimized Release app at `dist/Vao.app` and a ZIP in `dist/`. They do not install, launch or replace an application. To test it, open the built app yourself after quitting any other copy of Vao. The shared scheme also provides a Run action for this output; running it uses the public app identity and can access that identity's existing local data. Use a separate macOS account for a clean first-run test.

The Xcode target explicitly permits local ad-hoc signing when no signing identity is supplied. It does not search for or embed the maintainer's certificate. For a persistent signer, run:

```sh
VAO_SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
  xcodebuild -project xcode/Vao.xcodeproj -scheme 'Vao Direct' -configuration Release build
```

An invalid explicit signer fails; it does not fall back to ad-hoc signing. Local ad-hoc signing is not Apple notarization and may require new macOS permissions after a rebuild. See [distribution](DISTRIBUTION.md).

`CARGO_HOME`, `RUSTUP_HOME` and `CARGO_TARGET_DIR` can be supplied through the launching environment for an existing toolchain/cache. No sibling checkout or pre-existing Vao build is required. `clean` removes the local helper/app products and retains the Cargo dependency cache and source archives.

## Checks and contribution workflow

Run the Rust, browser and source checks in [testing](TESTING.md). Xcode builds and terminal builds use the same release identity from `release.json`. Keep the host and Cargo/npm versions aligned when changing a release.

Commit shared schemes under `xcshareddata`; keep `xcuserdata`, local build settings, signing material and generated outputs out of Git. Changes to the core, model adapters, UI and platform contracts belong in their existing directories so all distribution targets can reuse them.

## App Store work

**Vao Direct is a non-sandboxed build, not an App Store submission target.** The project does not implement an App Store Archive/Export workflow. The planned store target will share the core/UI while adapting platform capabilities and distribution configuration. Current constraints and acceptance gates are in [App Store preparation](APP_STORE.md).
