#!/bin/bash
#
# Build release binaries for all supported platforms
# Outputs separate zipped artifacts per binary to ./releases/
#
# Requirements:
#   - Rust toolchain with cross-compilation targets
#   - For cross-compilation: cargo-cross (https://github.com/cross-rs/cross)
#
# Install targets:
#   rustup target add x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
#
# Install cross (for Linux/Windows from macOS):
#   cargo install cross --git https://github.com/cross-rs/cross
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RELEASE_DIR="$PROJECT_ROOT/releases"
VERSION="${VERSION:-$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info() { echo -e "${CYAN}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Platform configurations
# Format: TARGET:OS_NAME:EXE_SUFFIX:BUILD_METHOD:CLI_ONLY
# CLI_ONLY=1 means skip Bevy examples (require complex system deps)
PLATFORMS=(
    "x86_64-apple-darwin:macos-x86_64::native:0"
    "aarch64-apple-darwin:macos-arm64::native:0"
    "x86_64-unknown-linux-gnu:linux-x86_64::cross:1"
    "x86_64-pc-windows-gnu:windows-x86_64:.exe:cross:0"
)

# Package/binary configurations
CLI_PACKAGE="ym2149-replayer-cli"
CLI_BIN_NAME="ym-replayer"
EXAMPLES_PACKAGE="bevy_ym2149_examples"

cd "$PROJECT_ROOT"

# Clean and create release directory
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

info "Building YM2149-rs v$VERSION release binaries"
echo ""

# Check if cross is available
CROSS_AVAILABLE=false
if command -v cross &> /dev/null; then
    CROSS_AVAILABLE=true
    info "cross detected - cross-compilation enabled"
else
    warn "cross not found - only native builds will be performed"
    warn "Install with: cargo install cross --git https://github.com/cross-rs/cross"
fi
echo ""

create_cli_zip() {
    local TARGET="$1"
    local OS_NAME="$2"
    local EXE_SUFFIX="$3"
    local BUILD_CMD="$4"

    info "  Building CLI replayer..."
    $BUILD_CMD build --release --package "$CLI_PACKAGE" --target "$TARGET" 2>&1 | tail -5 || {
        warn "  Failed to build CLI for $OS_NAME"
        return 1
    }

    local CLI_BIN="$PROJECT_ROOT/target/$TARGET/release/${CLI_BIN_NAME}${EXE_SUFFIX}"
    if [[ ! -f "$CLI_BIN" ]]; then
        warn "  CLI binary not found at: $CLI_BIN"
        return 1
    fi

    local STAGE_DIR="$RELEASE_DIR/stage-cli-$OS_NAME"
    mkdir -p "$STAGE_DIR"
    cp "$CLI_BIN" "$STAGE_DIR/"

    cat > "$STAGE_DIR/README.txt" << EOF
YM2149-rs v$VERSION - CLI Replayer ($OS_NAME)

Terminal-based chiptune player with TUI visualization, oscilloscope,
and spectrum display. Supports YM, AKS, AY, and SNDH formats.

Usage:
  ./ym-replayer path/to/music.ym
  ./ym-replayer ~/music/chiptunes/   # Browse directory

Keyboard Controls:
  Space     Play/Pause
  P         Open playlist
  1-3       Mute channel 1/2/3
  0         Unmute all
  +/-       Volume up/down
  [ ]       Previous/Next track
  Q         Quit

For more information: https://ym2149-rs.org
GitHub: https://github.com/slippyex/ym2149-rs
EOF

    local ZIP_NAME="ym2149-rs-v${VERSION}-cli-${OS_NAME}.zip"
    cd "$STAGE_DIR"
    zip -r "$RELEASE_DIR/$ZIP_NAME" . -x "*.DS_Store" > /dev/null
    cd "$PROJECT_ROOT"
    rm -rf "$STAGE_DIR"

    local ZIP_SIZE=$(du -h "$RELEASE_DIR/$ZIP_NAME" | cut -f1)
    success "  Created $ZIP_NAME ($ZIP_SIZE)"
}

create_example_zip() {
    local TARGET="$1"
    local OS_NAME="$2"
    local EXE_SUFFIX="$3"
    local BUILD_CMD="$4"
    local EXAMPLE_NAME="$5"
    local EXAMPLE_TITLE="$6"
    local EXAMPLE_DESC="$7"
    local INCLUDE_ASSETS="${8:-1}"  # Default: include assets

    info "  Building $EXAMPLE_NAME..."
    $BUILD_CMD build --release --package "$EXAMPLES_PACKAGE" --example "$EXAMPLE_NAME" --target "$TARGET" 2>&1 | tail -5 || {
        warn "  Failed to build $EXAMPLE_NAME for $OS_NAME"
        return 1
    }

    local EXAMPLE_BIN="$PROJECT_ROOT/target/$TARGET/release/examples/${EXAMPLE_NAME}${EXE_SUFFIX}"
    if [[ ! -f "$EXAMPLE_BIN" ]]; then
        warn "  Example binary not found at: $EXAMPLE_BIN"
        return 1
    fi

    local STAGE_DIR="$RELEASE_DIR/stage-${EXAMPLE_NAME}-$OS_NAME"
    mkdir -p "$STAGE_DIR"
    cp "$EXAMPLE_BIN" "$STAGE_DIR/"

    # Copy assets only if requested
    if [[ "$INCLUDE_ASSETS" == "1" ]] && [[ -d "$PROJECT_ROOT/crates/bevy_ym2149_examples/assets" ]]; then
        cp -r "$PROJECT_ROOT/crates/bevy_ym2149_examples/assets" "$STAGE_DIR/"
    fi

    if [[ "$INCLUDE_ASSETS" == "1" ]]; then
        cat > "$STAGE_DIR/README.txt" << EOF
YM2149-rs v$VERSION - $EXAMPLE_TITLE ($OS_NAME)

$EXAMPLE_DESC

Usage:
  ./$EXAMPLE_NAME

The assets/ folder contains sample music files.

For more information: https://ym2149-rs.org
GitHub: https://github.com/slippyex/ym2149-rs
EOF
    else
        cat > "$STAGE_DIR/README.txt" << EOF
YM2149-rs v$VERSION - $EXAMPLE_TITLE ($OS_NAME)

$EXAMPLE_DESC

Usage:
  ./$EXAMPLE_NAME

For more information: https://ym2149-rs.org
GitHub: https://github.com/slippyex/ym2149-rs
EOF
    fi

    local ZIP_NAME="ym2149-rs-v${VERSION}-${EXAMPLE_NAME}-${OS_NAME}.zip"
    cd "$STAGE_DIR"
    zip -r "$RELEASE_DIR/$ZIP_NAME" . -x "*.DS_Store" > /dev/null
    cd "$PROJECT_ROOT"
    rm -rf "$STAGE_DIR"

    local ZIP_SIZE=$(du -h "$RELEASE_DIR/$ZIP_NAME" | cut -f1)
    success "  Created $ZIP_NAME ($ZIP_SIZE)"
}

build_for_platform() {
    local TARGET="$1"
    local OS_NAME="$2"
    local EXE_SUFFIX="$3"
    local BUILD_METHOD="$4"
    local CLI_ONLY="${5:-0}"

    if [[ "$CLI_ONLY" == "1" ]]; then
        info "Building for $OS_NAME ($TARGET) [CLI only]..."
    else
        info "Building for $OS_NAME ($TARGET)..."
    fi

    # Determine build command
    local BUILD_CMD="cargo"
    if [[ "$BUILD_METHOD" == "cross" ]]; then
        if [[ "$CROSS_AVAILABLE" == "false" ]]; then
            warn "Skipping $OS_NAME - cross not available"
            return
        fi
        BUILD_CMD="cross"
    fi

    # Build CLI replayer
    create_cli_zip "$TARGET" "$OS_NAME" "$EXE_SUFFIX" "$BUILD_CMD"

    # Build Bevy examples (skip if CLI_ONLY)
    if [[ "$CLI_ONLY" != "1" ]]; then
        create_example_zip "$TARGET" "$OS_NAME" "$EXE_SUFFIX" "$BUILD_CMD" \
            "advanced_example" \
            "Advanced Example" \
            "Full-featured Bevy demo with tracker-style UI, oscilloscope visualization,
channel controls, and drag-and-drop file support." \
            "1"  # Include assets

        create_example_zip "$TARGET" "$OS_NAME" "$EXE_SUFFIX" "$BUILD_CMD" \
            "demoscene" \
            "Demoscene Example" \
            "Shader-heavy demo scene with audio-reactive visuals synchronized
to YM2149 playback." \
            "0"  # Assets bundled in binary
    else
        warn "  Skipping Bevy examples (requires native build with system libraries)"
    fi

    echo ""
}

# Build for each platform
for PLATFORM in "${PLATFORMS[@]}"; do
    IFS=':' read -r TARGET OS_NAME EXE_SUFFIX BUILD_METHOD CLI_ONLY <<< "$PLATFORM"
    build_for_platform "$TARGET" "$OS_NAME" "$EXE_SUFFIX" "$BUILD_METHOD" "$CLI_ONLY"
done

# Summary
echo ""
info "========================================="
info "Release build complete!"
info "========================================="
echo ""
info "Generated archives in $RELEASE_DIR:"
ls -lh "$RELEASE_DIR"/*.zip 2>/dev/null || warn "No archives generated"
echo ""
info "Upload these to https://ym2149-rs.org/releases/"
