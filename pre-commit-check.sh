#!/bin/bash

# Check for changes in ym2149-rs
if git diff --name-only HEAD^ HEAD | grep -q 'ym2149-rs'; then
    echo "Changes detected in ym2149-rs"
    exit 1
fi
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --all-targets --all-features
cargo doc --all-features --no-deps
