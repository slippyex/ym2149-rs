//! High-level GIST sound player
//!
//! Provides a simple API for playing GIST sound effects, similar to other
//! replayer crates in the workspace (ym2149-ym-replayer, ym2149-arkos-replayer).
//!
//! # GIST Sound System
//!
//! GIST (Graphics, Images, Sound, Text) was a sound tool created by Dave Becker
//! for Antic Software on the Atari ST. It allows playing sound effects on the
//! YM2149 PSG chip with automatic envelope and LFO processing.
//!
//! The player operates at 200 Hz (matching the original Atari ST Timer C interrupt)
//! and supports 3 simultaneous voices corresponding to the 3 channels of the YM2149.
//!
//! # Sound Effects vs Musical Notes
//!
//! GIST sounds can be played in two modes:
//!
//! - **Sound effect mode** (pitch = -1): The sound plays using its stored frequency
//!   and duration. It automatically stops when the duration elapses.
//!
//! - **Musical note mode** (pitch = 24-108): The pitch is interpreted as a MIDI-style
//!   note number. The sound plays indefinitely until explicitly stopped with
//!   [`GistPlayer::stop_voice`] or [`GistPlayer::stop_all`].
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
    /// This plays the sound in "sound effect mode" using the frequency
    /// stored in the sound data. The sound will automatically stop when
    /// its duration elapses (plus any release envelope time).
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to play
    /// * `volume` - Optional volume override (0-15), or `None` to use sound's default.
    ///   Volume 0 is silent, 15 is maximum.
    /// * `priority` - Optional priority (0-32767, higher = harder to preempt), or
    ///   `None` for maximum priority. When a voice is released with [`stop_voice`](Self::stop_voice),
    ///   its priority drops to zero, making it easier to reuse. Priority should
    ///   normally be at least 1.
    ///
    /// # Returns
    ///
    /// The voice index (0-2) the sound was assigned to, or `None` if no voice available
    /// (all voices busy with higher priority sounds).
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Play with default settings
    /// player.play_sound(&explosion, None, None);
    ///
    /// // Play at half volume with low priority
    /// player.play_sound(&ambient, Some(8), Some(10));
    /// ```
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
    /// Use this when you need precise control over which YM2149 channel is used,
    /// for example when coordinating multiple sound effects or when certain
    /// effects should always use the same channel.
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to play
    /// * `voice` - Voice index (0, 1, or 2) corresponding to YM2149 channels A, B, C
    /// * `volume` - Optional volume override (0-15), or `None` to use sound's default
    /// * `priority` - Optional priority (0-32767), or `None` for maximum priority
    ///
    /// # Returns
    ///
    /// The voice index if successful, or `None` if the voice is busy with a higher
    /// priority sound.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Always play laser on voice 0, explosion on voice 1
    /// player.play_sound_on_voice(&laser, 0, None, Some(100));
    /// player.play_sound_on_voice(&explosion, 1, None, Some(200));
    /// ```
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

    /// Play a sound effect with a specific pitch (musical note mode).
    ///
    /// When a pitch is specified, the sound is treated as a **musical note** rather
    /// than a sound effect. The key difference is:
    ///
    /// - **Sound effect** (pitch = -1): Plays for the duration stored in the sound
    ///   data, then automatically stops (with release envelope).
    /// - **Musical note** (pitch specified): Plays **indefinitely** until explicitly
    ///   stopped with [`stop_voice`](Self::stop_voice) or [`stop_all`](Self::stop_all).
    ///
    /// # Pitch Values (MIDI Note Numbers)
    ///
    /// The pitch parameter uses MIDI-style note numbers:
    ///
    /// | Pitch | Note | Frequency |
    /// |-------|------|----------|
    /// | 24    | C1   | ~33 Hz   |
    /// | 36    | C2   | ~65 Hz   |
    /// | 48    | C3   | ~131 Hz  |
    /// | 60    | C4   | 262 Hz (Middle C) |
    /// | 72    | C5   | ~523 Hz  |
    /// | 84    | C6   | ~1047 Hz |
    /// | 96    | C7   | ~2093 Hz |
    /// | 108   | C8   | ~4186 Hz |
    ///
    /// The valid range is 24-108 (about 7 octaves). Values outside this range
    /// are octave-wrapped to fit.
    ///
    /// # Arguments
    ///
    /// * `sound` - The GIST sound effect to use as the instrument/timbre
    /// * `pitch` - MIDI note number (24-108, where 60 = middle C at 262 Hz)
    /// * `voice` - Optional voice index (0, 1, or 2), or `None` for auto-selection
    /// * `volume` - Optional volume override (0-15)
    /// * `priority` - Optional priority (0-32767)
    ///
    /// # Returns
    ///
    /// The voice index if successful, or `None` if no voice available.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Play middle C
    /// player.play_sound_pitched(&piano, 60, None, None, None);
    /// // Play one octave higher
    /// player.play_sound_pitched(&piano, 72, None, None, None);
    /// // Let it play for a while...
    /// player.generate_samples(44100); // 1 second
    /// // Stop the note
    /// player.stop_all();
    /// ```
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

    /// Release a specific voice (graceful stop).
    ///
    /// This moves the sound into its **release phase**. If the sound has a
    /// volume release envelope, it will fade out naturally. The voice's
    /// priority is also reduced to zero, making it available for other sounds.
    ///
    /// Unlike [`stop_all`](Self::stop_all), this allows the release envelope
    /// to complete, giving a more natural sound ending.
    ///
    /// # Arguments
    ///
    /// * `voice` - Voice index (0, 1, or 2). Other values are ignored.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Start a note
    /// let voice = player.play_sound_pitched(&piano, 60, None, None, None);
    /// // Let it play...
    /// player.generate_samples(22050); // 0.5 seconds
    /// // Release with fade-out
    /// if let Some(v) = voice {
    ///     player.stop_voice(v);
    /// }
    /// ```
    pub fn stop_voice(&mut self, voice: usize) {
        self.driver.snd_off(voice);
    }

    /// Stop all voices immediately (hard stop).
    ///
    /// This completely stops all sounds on all channels. Unlike
    /// [`stop_voice`](Self::stop_voice), there is **no release phase** -
    /// sounds are cut off instantly regardless of what phase they are in
    /// (attack, decay, sustain, or release).
    ///
    /// Use this for:
    /// - Emergency silence (e.g., game pause)
    /// - Resetting audio state
    /// - When immediate silence is required
    ///
    /// For a more musical fade-out, call [`stop_voice`](Self::stop_voice)
    /// on each active voice instead.
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
