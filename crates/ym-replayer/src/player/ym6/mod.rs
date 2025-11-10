//! YM6 Player module - Split into logical submodules for maintainability

pub(super) mod helpers;
pub(super) mod types;

// Re-export public types
pub use types::{LoadSummary, Ym6Info, YmFileFormat};

// Re-export internal types and helpers for ym_player
pub(super) use helpers::{read_be_u16, read_be_u32, read_c_string};
pub(super) use types::PlaybackStateInit;
