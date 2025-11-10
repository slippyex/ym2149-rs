//! YM6 Player module - Split into logical submodules for maintainability

pub(super) mod types;
pub(super) mod helpers;

// Re-export public types
pub use types::{YmFileFormat, LoadSummary, Ym6Info};

// Re-export internal types and helpers for ym_player
pub(super) use types::PlaybackStateInit;
pub(super) use helpers::{read_be_u16, read_be_u32, read_c_string};
