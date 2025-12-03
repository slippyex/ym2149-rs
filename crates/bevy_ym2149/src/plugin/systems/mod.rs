//! Bevy systems for YM2149 playback.
//!
//! This module contains all ECS systems for:
//! - Playback initialization and state management
//! - Audio frame processing and generation
//! - Diagnostics and event emission
//! - Crossfade transitions
//! - SFX layer handling
//!
//! # Module Organization
//!
//! - [`main_systems`] - Core playback state, frame processing, diagnostics, and SFX
//! - [`crossfade`] - Dual-deck crossfade transitions
//! - [`loader`] - Asset loading helpers
//!
//! # System Overview
//!
//! ```text
//! PreUpdate:
//!   initialize_playback    - Load assets, create audio sources
//!   drive_playback_state   - Sync playback state (play/pause/stop)
//!
//! Update:
//!   process_playback_frames - Generate audio samples, emit FrameAudioData
//!   emit_playback_diagnostics - Channel snapshots, oscilloscope
//!   publish_bridge_audio   - Mirror samples to Bevy audio graph
//!   emit_frame_markers     - Timing events for game sync
//!   update_audio_reactive_state - Smoothed metrics for gameplay
//!   detect_pattern_triggers - Pattern-based events
//!   emit_beat_hits         - Beat timing from frame markers
//!   process_sfx_requests   - One-shot SFX overlay
//! ```

pub(super) mod crossfade;
pub(super) mod loader;

// Main systems module - re-export all public functions
mod main_systems;
pub(super) use main_systems::*;
