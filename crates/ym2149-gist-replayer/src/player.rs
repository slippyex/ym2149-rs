//! High-level GIST sound player
//!
//! Provides a simple API for playing GIST sound effects, similar to other
//! replayer crates in the workspace (ym2149-ym-replayer, ym2149-arkos-replayer).
//!
//! # Example
//!
//! ```rust,no_run
//! use ym2149_gist_replayer::{GistPlayer, GistSound};
//!
//! let sound = GistSound::load("effect.snd").unwrap();
//! let mut player = GistPlayer::new();
//!
//! player.play_sound(&sound, None, None);
//!
//! // Generate audio samples
//! let samples = player.generate_samples(882); // ~20ms at 44100 Hz
//! ```

use crate::gist::TICK_RATE;
use crate::gist::driver::GistDriver;
use crate::gist::gist_sound::GistSound;
use ym2149::Ym2149;

/// Default output sample rate in Hz.
/// 44100 Hz is the standard CD-quality sample rate.
///
/// Design rationale:
/// - Industry standard for audio playback
/// - Compatible with most audio hardware and software
/// - Provides good balance between quality and CPU usage
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

/// High-level GIST sound effect player.
///
/// Wraps `GistDriver` and `Ym2149` chip to provide a simple API for
/// playing sound effects and generating audio samples.
///
/// # Architecture
///
/// The player manages:
/// - A YM2149 PSG chip emulator for audio synthesis
/// - A GIST driver for processing sound effect envelopes/LFOs at 200 Hz
/// - Sample generation with automatic tick timing
///
/// # Usage Patterns
///
/// ## Simple one-shot playback
/// ```rust,no_run
/// use ym2149_gist_replayer::{GistPlayer, GistSound};
///
/// let sound = GistSound::load("explosion.snd").unwrap();
/// let mut player = GistPlayer::new();
/// player.play_sound(&sound, None, None);
///
/// while player.is_playing() {
///     let samples = player.generate_samples(512);
///     // Send samples to audio output...
/// }
/// ```
///
/// ## Multi-voice playback
/// ```rust,no_run
/// use ym2149_gist_replayer::{GistPlayer, GistSound};
///
/// let laser = GistSound::load("laser.snd").unwrap();
/// let explosion = GistSound::load("explosion.snd").unwrap();
/// let mut player = GistPlayer::new();
///
/// // Play laser on voice 0
/// player.play_sound_on_voice(&laser, 0, None, None);
/// // Play explosion on voice 1 (both play simultaneously)
/// player.play_sound_on_voice(&explosion, 1, None, None);
/// ```
pub struct GistPlayer {
    /// YM2149 PSG chip emulator
    chip: Ym2149,
    /// GIST sound driver
    driver: GistDriver,
    /// Output sample rate in Hz
    sample_rate: u32,
    /// Tick accumulator for timing (fixed-point)
    tick_accumulator: u32,
}

impl Default for GistPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl GistPlayer {
    /// Create a new GIST player with default sample rate (44100 Hz).
    pub fn new() -> Self {
        Self::with_sample_rate(DEFAULT_SAMPLE_RATE)
    }

    /// Create a new GIST player with a custom sample rate.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Output sample rate in Hz (e.g., 44100, 48000)
    pub fn with_sample_rate(sample_rate: u32) -> Self {
        Self {
            chip: Ym2149::new(),
            driver: GistDriver::new(),
            sample_rate,
            tick_accumulator: 0,
        }
    }

    /// Get the output sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Check if any sound is currently playing.
    pub fn is_playing(&self) -> bool {
        self.driver.is_playing()
    }

    /// Play a sound effect on an automatically chosen voice.
    ///
    /// The driver will pick the first available voice, or preempt
    /// the lowest-priority voice if all are in use.
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to play
    /// * `volume` - Optional volume override (0-15), or None to use sound's default
    /// * `priority` - Optional priority (higher = harder to preempt), or None for maximum
    ///
    /// # Returns
    ///
    /// The voice index (0-2) the sound was assigned to, or None if no voice available.
    pub fn play_sound(
        &mut self,
        sound: &GistSound,
        volume: Option<i16>,
        priority: Option<i16>,
    ) -> Option<usize> {
        let priority = priority.unwrap_or(i16::MAX - 1);
        self.driver
            .snd_on(&mut self.chip, sound, None, volume, -1, priority)
    }

    /// Play a sound effect on a specific voice.
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to play
    /// * `voice` - Voice index (0, 1, or 2)
    /// * `volume` - Optional volume override (0-15), or None to use sound's default
    /// * `priority` - Optional priority (higher = harder to preempt), or None for maximum
    ///
    /// # Returns
    ///
    /// The voice index if successful, or None if the voice is busy with higher priority.
    pub fn play_sound_on_voice(
        &mut self,
        sound: &GistSound,
        voice: usize,
        volume: Option<i16>,
        priority: Option<i16>,
    ) -> Option<usize> {
        let priority = priority.unwrap_or(i16::MAX - 1);
        self.driver
            .snd_on(&mut self.chip, sound, Some(voice), volume, -1, priority)
    }

    /// Play a sound effect with a specific pitch.
    ///
    /// When pitch is specified, the sound plays indefinitely until stopped.
    /// Use `stop_voice()` or `stop_all()` to stop it.
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to play
    /// * `pitch` - MIDI-style note number (24-108, where 60 = middle C)
    /// * `voice` - Optional voice index, or None for auto-selection
    /// * `volume` - Optional volume override (0-15)
    /// * `priority` - Optional priority
    pub fn play_sound_pitched(
        &mut self,
        sound: &GistSound,
        pitch: i16,
        voice: Option<usize>,
        volume: Option<i16>,
        priority: Option<i16>,
    ) -> Option<usize> {
        let priority = priority.unwrap_or(i16::MAX - 1);
        self.driver
            .snd_on(&mut self.chip, sound, voice, volume, pitch, priority)
    }

    /// Stop a specific voice.
    ///
    /// # Arguments
    ///
    /// * `voice` - Voice index (0, 1, or 2)
    pub fn stop_voice(&mut self, voice: usize) {
        self.driver.snd_off(voice);
    }

    /// Stop all voices immediately.
    pub fn stop_all(&mut self) {
        self.driver.stop_all(&mut self.chip);
    }

    /// Generate audio samples.
    ///
    /// This method handles the timing between the 200 Hz driver tick rate
    /// and the output sample rate automatically.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of samples to generate
    ///
    /// # Returns
    ///
    /// Vector of f32 samples in range approximately -1.0..1.0
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0f32; count];
        self.generate_samples_into(&mut buffer);
        buffer
    }

    /// Generate audio samples directly into a provided buffer.
    ///
    /// This avoids allocations on the hot path for real-time audio.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice to fill with samples
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            // Advance tick timing
            self.tick_accumulator += TICK_RATE;
            if self.tick_accumulator >= self.sample_rate {
                self.tick_accumulator -= self.sample_rate;
                self.driver.tick(&mut self.chip);
            }

            // Generate PSG sample
            self.chip.clock();
            *sample = self.chip.get_sample();
        }
    }

    /// Get a reference to the underlying YM2149 chip.
    ///
    /// Useful for advanced usage like reading register state.
    pub fn chip(&self) -> &Ym2149 {
        &self.chip
    }

    /// Get a mutable reference to the underlying YM2149 chip.
    ///
    /// Useful for advanced usage like direct register manipulation.
    pub fn chip_mut(&mut self) -> &mut Ym2149 {
        &mut self.chip
    }

    /// Get a reference to the underlying GIST driver.
    ///
    /// Useful for advanced usage like checking voice states.
    pub fn driver(&self) -> &GistDriver {
        &self.driver
    }

    /// Get a mutable reference to the underlying GIST driver.
    ///
    /// Useful for advanced usage like enabling debug mode.
    pub fn driver_mut(&mut self) -> &mut GistDriver {
        &mut self.driver
    }

    /// Enable or disable debug output on the driver.
    pub fn set_debug(&mut self, enabled: bool) {
        self.driver.set_debug(enabled);
    }

    /// Reset the player state.
    ///
    /// Stops all sounds and resets the chip to initial state.
    pub fn reset(&mut self) {
        self.stop_all();
        self.chip = Ym2149::new();
        self.tick_accumulator = 0;
    }

    /// Calculate duration of sound in seconds.
    ///
    /// Note: This is the base duration. Sounds with envelopes may play
    /// longer during the release phase.
    pub fn sound_duration_seconds(sound: &GistSound) -> f32 {
        sound.duration as f32 / TICK_RATE as f32
    }

    /// Calculate duration of sound in samples at current sample rate.
    pub fn sound_duration_samples(&self, sound: &GistSound) -> usize {
        let seconds = Self::sound_duration_seconds(sound);
        (seconds * self.sample_rate as f32) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let player = GistPlayer::new();
        assert_eq!(player.sample_rate(), DEFAULT_SAMPLE_RATE);
        assert!(!player.is_playing());
    }

    #[test]
    fn test_custom_sample_rate() {
        let player = GistPlayer::with_sample_rate(48000);
        assert_eq!(player.sample_rate(), 48000);
    }

    #[test]
    fn test_generate_samples() {
        let mut player = GistPlayer::new();
        let samples = player.generate_samples(100);
        assert_eq!(samples.len(), 100);
        // Samples should be finite values
        assert!(samples.iter().all(|&s| s.is_finite()));
    }

    #[test]
    fn test_reset() {
        let mut player = GistPlayer::new();
        // Generate some samples to advance state
        player.generate_samples(1000);
        // Reset should not panic
        player.reset();
        assert!(!player.is_playing());
    }
}
