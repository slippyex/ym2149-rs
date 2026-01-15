#!/bin/bash
#
# Publish all ym2149-rs crates to crates.io in dependency order
#
# Prerequisites:
#   - cargo login (authenticate with crates.io token)
#   - All tests pass: cargo test --workspace
#   - Version bumped in root Cargo.toml
#
# Usage:
#   ./scripts/publish-crates.sh          # Publish all crates
#   ./scripts/publish-crates.sh --dry-run # Test without publishing
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

DRY_RUN=""
if [[ "$1" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    warn "DRY RUN MODE - no actual publishing"
    echo ""
fi

# Crates in dependency order
CRATES=(
    "ym2149-common"
    "ym2149"
    "ym2149-ym-replayer"
    "ym2149-arkos-replayer"
    "ym2149-ay-replayer"
    "ym2149-sndh-replayer"
    "ym2149-gist-replayer"
    "bevy_ym2149"
    "bevy_ym2149_viz"
    "ym2149-replayer-cli"
    "ym2149-wasm"
)

cd "$PROJECT_ROOT"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
info "Publishing ym2149-rs v$VERSION to crates.io"
echo ""

# Check login status
if [[ -z "$DRY_RUN" ]]; then
    if ! cargo search ym2149 &>/dev/null; then
        error "Not logged in to crates.io. Run 'cargo login' first."
    fi
fi

for crate in "${CRATES[@]}"; do
    info "Publishing $crate..."

    if cargo publish -p "$crate" $DRY_RUN 2>&1; then
        success "$crate published successfully"
    else
        error "Failed to publish $crate"
    fi

    # Wait for crates.io index to update (skip for dry-run)
    if [[ -z "$DRY_RUN" ]]; then
        info "Waiting 45s for crates.io index update..."
        sleep 45
    fi

    echo ""
done

echo ""
success "All crates published!"
info "Verify at: https://crates.io/crates/ym2149"
