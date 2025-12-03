#!/usr/bin/env bash

# Rebuild the wasm bundle and copy it into the examples directory.
# Usage:
#   scripts/build-wasm-examples.sh [additional wasm-pack args...]

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
wasm_crate="$repo_root/crates/ym2149-wasm"

cd "$wasm_crate"

echo "Building ym2149-wasm via wasm-pack..."
wasm-pack build --target web --out-dir pkg "$@"

echo "Copying pkg/ -> crates/ym2149-wasm/examples/pkg..."
rm -rf examples/pkg
cp -R pkg examples/

echo "Copying pkg/ -> docs/pkg (for GitHub Pages)..."
rm -rf "$repo_root/docs/pkg"
cp -R pkg "$repo_root/docs/"

echo "Done. Restart your static server and reload simple-player.html."
