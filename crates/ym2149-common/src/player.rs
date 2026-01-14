//! Unified chiptune player trait.
//!
//! Defines the common interface for all YM2149-based music players.
//!
//! # Trait Hierarchy
//!
//! - [`ChiptunePlayerBase`] - Object-safe base trait for dynamic dispatch
//! - [`ChiptunePlayer`] - Full trait with associated `Metadata` type
//!
//! Use `ChiptunePlayerBase` when you need trait objects (`Box<dyn ChiptunePlayerBase>`).
//! Use `ChiptunePlayer` when you need access to the specific metadata type.

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

/// Object-safe base trait for chiptune players.
///
/// This trait provides all playback functionality without the associated
/// `Metadata` type, making it usable as a trait object (`Box<dyn ChiptunePlayerBase>`).
///
/// All types implementing [`ChiptunePlayer`] automatically implement this trait.
///
/// # Example
///
/// ```ignore
/// use ym2149_common::{ChiptunePlayerBase, PlaybackState};
///
/// fn play_any(player: &mut dyn ChiptunePlayerBase) {
///     player.play();
///     while player.state() == PlaybackState::Playing {
///         let mut buffer = vec![0.0; 1024];
///         player.generate_samples_into(&mut buffer);
///         // ... send buffer to audio device
///     }
/// }
/// ```
pub trait ChiptunePlayerBase: Send {
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

    /// Generate samples into an existing buffer.
    ///
    /// Fills the entire buffer with audio samples. If playback is stopped
    /// or paused, the buffer is filled with silence (zeros).
    fn generate_samples_into(&mut self, buffer: &mut [f32]);

    /// Generate samples into a new buffer.
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = vec![0.0; count];
        self.generate_samples_into(&mut buffer);
        buffer
    }

    /// Get the output sample rate in Hz.
    ///
    /// Typical value is 44100 Hz.
    fn sample_rate(&self) -> u32 {
        44100
    }

    /// Mute or unmute a specific channel (0-2).
    ///
    /// Default implementation does nothing. Override if the player
    /// supports channel muting.
    fn set_channel_mute(&mut self, _channel: usize, _mute: bool) {}

    /// Check if a channel is muted.
    ///
    /// Default returns false. Override if the player supports channel muting.
    fn is_channel_muted(&self, _channel: usize) -> bool {
        false
    }

    /// Get playback position as a percentage (0.0 to 1.0).
    ///
    /// Default returns 0.0. Override if position tracking is available.
    fn playback_position(&self) -> f32 {
        0.0
    }

    /// Seek to a position (0.0 to 1.0).
    ///
    /// Returns `true` if seeking is supported and successful.
    /// Default returns `false` (seeking not supported).
    fn seek(&mut self, _position: f32) -> bool {
        false
    }

    /// Get the total duration in seconds.
    ///
    /// Returns 0.0 if duration is unknown.
    fn duration_seconds(&self) -> f32 {
        0.0
    }

    /// Get elapsed time in seconds based on playback position.
    ///
    /// Uses `playback_position()` and `duration_seconds()` for calculation.
    fn elapsed_seconds(&self) -> f32 {
        self.playback_position() * self.duration_seconds()
    }

    /// Get the number of subsongs in this file.
    ///
    /// Default returns 1. Override for formats with multiple subsongs.
    fn subsong_count(&self) -> usize {
        1
    }

    /// Get the current subsong index (1-based).
    ///
    /// Default returns 1. Override for formats with multiple subsongs.
    fn current_subsong(&self) -> usize {
        1
    }

    /// Switch to a different subsong by 1-based index.
    ///
    /// Returns `true` if successful. Default returns `false`.
    fn set_subsong(&mut self, _index: usize) -> bool {
        false
    }

    /// Check if this player supports multiple subsongs.
    fn has_subsongs(&self) -> bool {
        self.subsong_count() > 1
    }

    /// Get the number of PSG chips used by this player.
    ///
    /// Most players use a single chip (returns 1). Arkos Tracker songs
    /// can use multiple PSGs for 6+ channel music.
    fn psg_count(&self) -> usize {
        1
    }

    /// Get the total number of audio channels.
    ///
    /// Each PSG chip has 3 channels (A, B, C), so this returns `psg_count() * 3`.
    fn channel_count(&self) -> usize {
        self.psg_count() * 3
    }
}

/// Unified player interface for chiptune formats.
///
/// This trait extends [`ChiptunePlayerBase`] with metadata access.
/// It provides a common API for playing YM, AKS, AY and other
/// chiptune formats.
///
/// # Metadata
///
/// Each player provides metadata through [`PlaybackMetadata`]. The trait
/// uses an associated type to allow format-specific metadata structs while
/// still providing a common interface.
///
/// # Object Safety
///
/// This trait is **not** object-safe due to the associated `Metadata` type.
/// Use [`ChiptunePlayerBase`] when you need trait objects.
///
/// # Example
///
/// ```ignore
/// use ym2149_common::{ChiptunePlayer, PlaybackState};
///
/// fn play_song(player: &mut impl ChiptunePlayer) {
///     println!("Playing: {}", player.metadata().title());
///     player.play();
///
///     let mut buffer = vec![0.0; 1024];
///     while player.state() == PlaybackState::Playing {
///         player.generate_samples_into(&mut buffer);
///         // ... send buffer to audio device
///     }
/// }
/// ```
pub trait ChiptunePlayer: ChiptunePlayerBase {
    /// The metadata type for this player.
    type Metadata: PlaybackMetadata;

    /// Get song metadata.
    fn metadata(&self) -> &Self::Metadata;
}
