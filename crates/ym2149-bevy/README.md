# ym2149-bevy (Deprecated)

**⚠️ This crate has been renamed to [`bevy_ym2149`](https://crates.io/crates/bevy_ym2149/)**

This crate is now a compatibility shim that re-exports everything from `bevy_ym2149`. It will continue to work, but **we recommend migrating to `bevy_ym2149`** for future updates.

## Migration

Update your `Cargo.toml`:

```toml
# Old (deprecated)
ym2149-bevy = "0.5"

# New (recommended)
bevy_ym2149 = "0.5"
```

Update your imports:

```rust
// Old
use ym2149_bevy::Ym2149Plugin;

// New
use bevy_ym2149::Ym2149Plugin;
```

## Why the rename?

The Bevy plugin ecosystem follows a naming convention where plugins are named `bevy_*` instead of `*_bevy`. This makes it easier to discover Bevy plugins and maintain consistency across the ecosystem.

## Backward Compatibility

All public APIs remain unchanged. Your code will continue to work with this crate, but you'll receive deprecation warnings. The shim will be maintained alongside the new `bevy_ym2149` crate for backward compatibility.

For the full documentation and latest features, see [`bevy_ym2149`](https://docs.rs/bevy_ym2149/).
