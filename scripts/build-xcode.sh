#!/bin/bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

case "${1:-${ACTION:-build}}" in
  clean)
    rm -f .build/vao-macos-bridge
    rm -rf dist/Vao.app
    echo "Removed local app products; Cargo cache and source archives were retained."
    exit 0
    ;;
  build) ;;
  *)
    echo "This target builds the direct-download app. App Store archiving is not supported." >&2
    exit 2
    ;;
esac

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust is required. Install stable Rust or set CARGO_HOME before opening Xcode." >&2
  exit 2
fi

# The shared Xcode target is a local build; an explicit signer takes precedence.
if [[ -z "${VAO_SIGNING_IDENTITY:-}" || "${VAO_SIGNING_IDENTITY:-}" == "-" ]]; then
  export VAO_ALLOW_ADHOC="${VAO_ALLOW_ADHOC:-1}"
fi
cargo fetch --locked --target aarch64-apple-darwin
bash scripts/build-macos.sh
