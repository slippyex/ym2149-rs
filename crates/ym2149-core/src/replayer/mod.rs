//! YM Music Playback Engine Domain
//!
//! Handles playback of YM music files including frame sequencing,
//! VBL synchronization, cycle counting, and timing control.

pub mod cycle_counter;
pub mod effects_manager;
mod madmax_digidrums;
pub mod vbl_sync;
pub mod ym_player;

pub use cycle_counter::CycleCounter;
pub use effects_manager::EffectsManager;
pub use vbl_sync::VblSync;
pub use ym_player::{load_song, LoadSummary, Player, Ym6Info, Ym6Player, YmFileFormat};

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
