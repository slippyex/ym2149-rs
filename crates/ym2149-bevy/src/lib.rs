//! # ⚠️ Deprecated: Use `bevy_ym2149` instead
//!
//! This crate has been renamed to [`bevy_ym2149`](https://docs.rs/bevy_ym2149/) to follow
//! Bevy plugin naming conventions. This crate is now a compatibility shim that re-exports
//! everything from `bevy_ym2149`.
//!
//! **Please migrate to `bevy_ym2149` for future updates.**
//!
//! ## Migration Guide
//!
//! Update your `Cargo.toml`:
//! ```toml
//! # Old (deprecated):
//! ym2149-bevy = "0.5"
//!
//! # New (recommended):
//! bevy_ym2149 = "0.5"
//! ```
//!
//! Update your imports:
//! ```rust,ignore
//! // Old:
//! use ym2149_bevy::Ym2149Plugin;
//!
//! // New:
//! use bevy_ym2149::Ym2149Plugin;
//! ```
//!
//! See the [bevy_ym2149 documentation](https://docs.rs/bevy_ym2149/) for the latest features and API details.

#[deprecated(
    since = "0.5.0",
    note = "ym2149-bevy has been renamed to bevy_ym2149 to follow Bevy naming conventions. Please migrate to bevy_ym2149."
)]
pub use bevy_ym2149::*;
