#!/usr/bin/env bash
#
# Build and prepare ym2149-wasm for npm publishing
#
# Usage:
#   ./scripts/build-npm.sh           # Build only
#   ./scripts/build-npm.sh --publish # Build and publish to npm
#   ./scripts/build-npm.sh --dry-run # Build and run npm publish --dry-run
#
# Requirements:
#   - wasm-pack: cargo install wasm-pack
#   - npm account with publish access to ym2149-wasm
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WASM_CRATE="$PROJECT_ROOT/crates/ym2149-wasm"
PKG_DIR="$WASM_CRATE/pkg"
NPM_DIR="$PROJECT_ROOT/npm-package"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info() { echo -e "${CYAN}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check for wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    error "wasm-pack not found. Install with: cargo install wasm-pack"
fi

# Parse arguments
PUBLISH=false
DRY_RUN=false
for arg in "$@"; do
    case $arg in
        --publish)
            PUBLISH=true
            ;;
        --dry-run)
            DRY_RUN=true
            ;;
        *)
            warn "Unknown argument: $arg"
            ;;
    esac
done

# Get version from workspace Cargo.toml
VERSION=$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
info "Building ym2149-wasm v$VERSION for npm..."

# Step 1: Build WASM with wasm-pack
info "Running wasm-pack build..."
cd "$WASM_CRATE"
wasm-pack build --release --target web --out-dir pkg

# Step 2: Create npm package directory
info "Preparing npm package..."
rm -rf "$NPM_DIR"
mkdir -p "$NPM_DIR"

# Step 3: Copy WASM artifacts
cp "$PKG_DIR/ym2149_wasm_bg.wasm" "$NPM_DIR/"
cp "$PKG_DIR/ym2149_wasm_bg.wasm.d.ts" "$NPM_DIR/"
cp "$PKG_DIR/ym2149_wasm.js" "$NPM_DIR/"
cp "$PKG_DIR/ym2149_wasm.d.ts" "$NPM_DIR/"

# Step 4: Create package.json with correct version
cat > "$NPM_DIR/package.json" << EOF
{
  "name": "ym2149-wasm",
  "version": "$VERSION",
  "description": "Hardware-accurate YM2149 PSG emulator in WebAssembly - play Atari ST, Amstrad CPC & ZX Spectrum chiptunes in the browser",
  "type": "module",
  "main": "ym2149_wasm.js",
  "module": "ym2149_wasm.js",
  "types": "ym2149_wasm.d.ts",
  "exports": {
    ".": {
      "types": "./ym2149_wasm.d.ts",
      "import": "./ym2149_wasm.js",
      "default": "./ym2149_wasm.js"
    },
    "./ym2149_wasm_bg.wasm": "./ym2149_wasm_bg.wasm"
  },
  "files": [
    "ym2149_wasm_bg.wasm",
    "ym2149_wasm_bg.wasm.d.ts",
    "ym2149_wasm.js",
    "ym2149_wasm.d.ts",
    "README.md"
  ],
  "sideEffects": false,
  "author": {
    "name": "slippyex",
    "url": "https://github.com/slippyex"
  },
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/slippyex/ym2149-rs.git",
    "directory": "crates/ym2149-wasm"
  },
  "bugs": {
    "url": "https://github.com/slippyex/ym2149-rs/issues"
  },
  "homepage": "https://ym2149-rs.org",
  "keywords": [
    "ym2149",
    "psg",
    "chiptune",
    "8bit",
    "retro",
    "atari",
    "atari-st",
    "amstrad",
    "cpc",
    "zx-spectrum",
    "ay-3-8910",
    "wasm",
    "webassembly",
    "audio",
    "emulator",
    "demoscene",
    "tracker",
    "sndh",
    "ym",
    "aks",
    "arkos"
  ],
  "engines": {
    "node": ">=16.0.0"
  },
  "browser": {
    "fs": false,
    "path": false
  }
}
EOF

# Step 5: Copy README
cp "$PKG_DIR/README.md" "$NPM_DIR/"

# Step 6: Show package contents
info "Package contents:"
ls -lh "$NPM_DIR"
echo ""

# Calculate sizes
WASM_SIZE=$(du -h "$NPM_DIR/ym2149_wasm_bg.wasm" | cut -f1)
JS_SIZE=$(du -h "$NPM_DIR/ym2149_wasm.js" | cut -f1)
TOTAL_SIZE=$(du -sh "$NPM_DIR" | cut -f1)

info "Bundle sizes:"
echo "  ym2149_wasm_bg.wasm: $WASM_SIZE"
echo "  ym2149_wasm.js:      $JS_SIZE"
echo "  Total:               $TOTAL_SIZE"
echo ""

success "npm package prepared at: $NPM_DIR"

# Step 7: Publish if requested
if [ "$DRY_RUN" = true ]; then
    info "Running npm publish --dry-run..."
    cd "$NPM_DIR"
    npm publish --dry-run
    success "Dry run completed successfully!"
elif [ "$PUBLISH" = true ]; then
    info "Publishing to npm..."
    cd "$NPM_DIR"
    npm publish --access public
    success "Published ym2149-wasm@$VERSION to npm!"
    echo ""
    echo "View at: https://www.npmjs.com/package/ym2149-wasm"
else
    echo ""
    info "To publish, run:"
    echo "  cd $NPM_DIR && npm publish --access public"
    echo ""
    info "Or re-run this script with --publish or --dry-run"
fi
