//! Unified chiptune player trait.
//!
//! Defines the common interface for all YM2149-based music players.

use crate::PlaybackMetadata;

/// Playback state for chiptune players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// Player is stopped (at beginning or end).
    #[default]
    Stopped,
    /// Player is actively playing.
    Playing,
    /// Player is paused (can resume).
    Paused,
}

/// Unified player interface for chiptune formats.
///
/// This trait provides a common API for playing YM, AKS, AY and other
/// chiptune formats. All players support basic playback control and
/// sample generation.
///
/// # Metadata
///
/// Each player provides metadata through [`PlaybackMetadata`]. The trait
/// uses an associated type to allow format-specific metadata structs while
/// still providing a common interface.
///
/// # Sample Generation
///
/// Players generate mono f32 samples in the range -1.0 to 1.0.
/// The output sample rate is typically 44100 Hz but may vary.
///
/// # Example
///
/// ```ignore
/// use ym2149_common::{ChiptunePlayer, PlaybackState};
///
/// fn play_song(player: &mut impl ChiptunePlayer) {
///     player.play();
///
///     let mut buffer = vec![0.0; 1024];
///     while player.state() == PlaybackState::Playing {
///         player.generate_samples_into(&mut buffer);
///         // ... send buffer to audio device
///     }
/// }
/// ```
pub trait ChiptunePlayer {
    /// The metadata type for this player.
    type Metadata: PlaybackMetadata;

    /// Start or resume playback.
    fn play(&mut self);

    /// Pause playback (keeps position).
    fn pause(&mut self);

    /// Stop playback and reset to beginning.
    fn stop(&mut self);

    /// Get current playback state.
    fn state(&self) -> PlaybackState;

    /// Check if currently playing.
    fn is_playing(&self) -> bool {
        self.state() == PlaybackState::Playing
    }

    /// Get song metadata.
    fn metadata(&self) -> &Self::Metadata;

    /// Generate samples into a new buffer.
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0; count];
        self.generate_samples_into(&mut buffer);
        buffer
    }

    /// Generate samples into an existing buffer.
    ///
    /// Fills the entire buffer with audio samples. If playback is stopped
    /// or paused, the buffer is filled with silence (zeros).
    fn generate_samples_into(&mut self, buffer: &mut [f32]);

    /// Get the output sample rate in Hz.
    ///
    /// Typical value is 44100 Hz.
    fn sample_rate(&self) -> u32 {
        44100
    }
}
