#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
VAO_SIGNING_IDENTITY="${VAO_SIGNING_IDENTITY:-}"
if [[ -z "$VAO_SIGNING_IDENTITY" || "$VAO_SIGNING_IDENTITY" == "-" ]]; then
  if [[ "${VAO_ALLOW_ADHOC:-0}" != "1" ]]; then
    echo "Set VAO_SIGNING_IDENTITY to a persistent signer. Ad-hoc signing is only available with VAO_ALLOW_ADHOC=1 for local development." >&2
    exit 2
  fi
  VAO_SIGNING_IDENTITY=-
fi
mkdir -p .build dist
xcrun swiftc -target arm64-apple-macosx12.0 -O platform/macos/Bridge.swift -o .build/vao-macos-bridge
cargo build --offline --release --locked -p vao-desktop
python3 scripts/package-macos.py
VAO_BUNDLE_ID="$(python3 -c 'import json; print(json.load(open("release.json"))["bundle"])')"
if [[ "$VAO_SIGNING_IDENTITY" == "-" ]]; then
  codesign --force --sign - --identifier "$VAO_BUNDLE_ID.file-dialog" "dist/Vao.app/Contents/Resources/Vao File Dialog.app"
  codesign --force --sign - --identifier "$VAO_BUNDLE_ID.bridge" "dist/Vao.app/Contents/Resources/vao-macos-bridge"
  codesign --force --sign - --identifier "$VAO_BUNDLE_ID" "dist/Vao.app"
else
  VAO_TIMESTAMP=--timestamp=none
  if [[ "$VAO_SIGNING_IDENTITY" == Developer\ ID\ Application:* ]]; then VAO_TIMESTAMP=--timestamp; fi
  codesign --force --sign "$VAO_SIGNING_IDENTITY" --options runtime "$VAO_TIMESTAMP" --identifier "$VAO_BUNDLE_ID.file-dialog" "dist/Vao.app/Contents/Resources/Vao File Dialog.app"
  codesign --force --sign "$VAO_SIGNING_IDENTITY" --options runtime "$VAO_TIMESTAMP" --identifier "$VAO_BUNDLE_ID.bridge" "dist/Vao.app/Contents/Resources/vao-macos-bridge"
  codesign --force --sign "$VAO_SIGNING_IDENTITY" --options runtime "$VAO_TIMESTAMP" --entitlements desktop/entitlements.plist --identifier "$VAO_BUNDLE_ID" "dist/Vao.app"
fi
codesign --verify --deep --strict "dist/Vao.app"
python3 scripts/check-macos-package.py "dist/Vao.app"
python3 scripts/archive-macos.py
