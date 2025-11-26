//! YM Music Playback Engine Domain
//!
//! Handles playback of YM music files including frame sequencing,
//! VBL synchronization, cycle counting, and timing control.

mod chiptune_player;
pub mod cycle_counter;
pub mod effects_manager;
/// High-level wrapper around [`EffectsManager`] that tracks active effect state.
pub mod effects_pipeline;
pub mod format_profile;
mod frame_sequencer;
mod loader;
mod madmax_digidrums;
mod metadata;
mod sample_generation;
mod state;
mod timing;
mod tracker_player;
pub mod vbl_sync;
mod ym6;
pub mod ym_player;

pub use chiptune_player::Ym6Metadata;
pub use cycle_counter::CycleCounter;
pub use effects_manager::EffectsManager;
pub use effects_pipeline::EffectsPipeline;
pub use format_profile::{FormatMode, FormatProfile, create_profile};
pub use frame_sequencer::{AdvanceResult, FrameSequencer};
pub use vbl_sync::VblSync;
pub use ym_player::{Player, Ym6Player, load_song};
pub use ym6::{LoadSummary, Ym6Info, YmFileFormat};

use crate::Result;

/// Timing configuration for ATARI ST playback
#[derive(Debug, Clone, Copy)]
pub struct TimingConfig {
    /// Audio sample rate in Hz
    pub sample_rate: u32,
    /// VBL frequency (50Hz for PAL ATARI ST)
    pub vbl_frequency: f32,
    /// PSG clock frequency (2 MHz for ATARI ST)
    pub psg_clock_frequency: u32,
}

impl TimingConfig {
    /// Default PAL ATARI ST timing (50Hz VBL, 44.1kHz audio)
    pub fn pal_atari_st() -> Self {
        TimingConfig {
            sample_rate: 44100,
            vbl_frequency: 50.0,
            psg_clock_frequency: 2_000_000, // 2 MHz
        }
    }

    /// Samples per VBL interrupt
    pub fn samples_per_vbl(&self) -> u32 {
        (self.sample_rate as f32 / self.vbl_frequency) as u32
    }

    /// PSG clock cycles per sample
    pub fn psg_cycles_per_sample(&self) -> u32 {
        self.psg_clock_frequency / self.sample_rate
    }
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self::pal_atari_st()
    }
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Stopped
    Stopped,
    /// Currently playing
    Playing,
    /// Paused
    Paused,
}

/// Simple playback controller trait for future expansion
pub trait PlaybackController {
    /// Start playback
    fn play(&mut self) -> Result<()>;

    /// Pause playback
    fn pause(&mut self) -> Result<()>;

    /// Stop playback
    fn stop(&mut self) -> Result<()>;

    /// Get current playback state
    fn state(&self) -> PlaybackState;
}
