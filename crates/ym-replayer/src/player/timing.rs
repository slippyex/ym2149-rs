//! Timing and Duration Management
//!
//! This module handles timing calculations, duration queries,
//! and Sync Buzzer effect control.

use super::ym_player::Ym6PlayerGeneric;
use crate::Result;
use ym2149::Ym2149Backend;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Set samples per frame (default 882 for 44.1kHz at 50Hz)
    ///
    /// # Arguments
    /// * `samples` - Samples per frame; must be > 0 and <= 10000
    ///
    /// # Valid Range
    /// Typical values:
    /// - 441: 100Hz frame rate at 44.1kHz
    /// - 735: 60Hz (NTSC) at 44.1kHz
    /// - 882: 50Hz (PAL) at 44.1kHz
    /// - 1764: 25Hz at 44.1kHz
    ///
    /// # Errors
    /// Returns error if `samples` is 0 or exceeds 10000 (which would imply < 4.41Hz frame rate).
    pub fn set_samples_per_frame(&mut self, samples: u32) -> Result<()> {
        if samples == 0 {
            return Err("samples_per_frame cannot be zero".into());
        }
        if samples > 10000 {
            return Err(format!(
                "samples_per_frame {} exceeds reasonable limit of 10000 (implies < 4.41Hz frame rate)",
                samples
            ).into());
        }
        self.samples_per_frame = samples;
        // Reconfigure VBL with new timing
        self.vbl.reset();
        Ok(())
    }

    /// Get song duration in seconds
    ///
    /// Uses the actual frame rate from loaded YM6 file metadata if available,
    /// otherwise defaults to 50Hz (PAL standard). For frames loaded manually
    /// via `load_frames()`, the default 50Hz is used unless overridden with
    /// `set_samples_per_frame()`.
    pub fn get_duration_seconds(&self) -> f32 {
        if let Some(tracker) = &self.tracker {
            if tracker.player_rate == 0 {
                return 0.0;
            }
            return tracker.total_frames as f32 / f32::from(tracker.player_rate);
        }

        if self.frames.is_empty() {
            return 0.0;
        }

        let frame_rate = self
            .info
            .as_ref()
            .map(|info| info.frame_rate as u32)
            .unwrap_or(50);

        let total_frames = self.frames.len() as u32;
        total_frames as f32 / frame_rate as f32
    }

    /// Enable Sync Buzzer effect with specific timer frequency
    ///
    /// Sync Buzzer is a timer-based effect that repeatedly retriggers the envelope
    /// to create a continuous buzzing sound. This is typically used with envelope
    /// shapes like 0x0D (Hold) or 0x0F (Hold-Sawtooth).
    ///
    /// # Arguments
    /// * `timer_freq` - Timer frequency in Hz (typical range: 4000-8000 Hz)
    ///
    /// # Example
    /// ```ignore
    /// // Enable Sync Buzzer at 6 kHz
    /// player.enable_sync_buzzer(6000)?;
    /// player.play()?;
    /// ```
    pub fn enable_sync_buzzer(&mut self, timer_freq: u32) -> Result<()> {
        if timer_freq == 0 {
            return Err("Sync Buzzer timer frequency must be > 0".into());
        }
        self.effects.sync_buzzer_start(timer_freq);
        Ok(())
    }

    /// Disable Sync Buzzer effect
    pub fn disable_sync_buzzer(&mut self) {
        self.effects.sync_buzzer_stop();
    }
}
