//! Playback components and systems for YM2149 audio
//!
//! This module provides the core playback component (`Ym2149Playback`) that manages
//! the lifecycle of a YM2149 audio file playback, including state management, volume control,
//! and metadata handling. It also defines the playback state enum and global settings resource.
//!
//! # Spawning a Playback Entity
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ym2149_bevy::Ym2149Playback;
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Ym2149Playback::new("path/to/song.ym"));
//! }
//! ```
//!
//! # Controlling Playback
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ym2149_bevy::Ym2149Playback;
//!
//! fn control(mut playbacks: Query<&mut Ym2149Playback>) {
//!     for mut pb in playbacks.iter_mut() {
//!         pb.play();
//!         pb.set_volume(0.8);
//!         pb.pause();
//!         pb.restart();
//!     }
//! }
//! ```

use crate::audio_sink::AudioSink;
use bevy::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use ym2149::replayer::Ym6Player;

/// Component for managing YM2149 playback on an entity
///
/// This is the primary interface for controlling YM file playback. Spawn this component
/// on an entity to load and play a YM file. The plugin's systems automatically handle
/// audio generation and state management.
///
/// # Fields
///
/// - `source_path`: Path to the YM file to play
/// - `state`: Current playback state (Idle, Playing, Paused, Finished)
/// - `frame_position`: Current frame in the song
/// - `volume`: Volume multiplier (0.0 = silent, 1.0 = full)
/// - `song_title`: Extracted from YM file metadata
/// - `song_author`: Extracted from YM file metadata
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use ym2149_bevy::Ym2149Playback;
///
/// fn my_system(mut playbacks: Query<&mut Ym2149Playback>) {
///     for mut pb in playbacks.iter_mut() {
///         if pb.is_playing() {
///             println!("Frame: {}", pb.frame_position);
///             println!("Title: {}", pb.song_title);
///             pb.set_volume(0.5);
///         }
///     }
/// }
/// ```
#[derive(Component)]
pub struct Ym2149Playback {
    /// Path to the YM file to load and play
    pub source_path: String,
    /// Current playback state
    pub state: PlaybackState,
    /// Current frame position in the song
    pub frame_position: u32,
    /// Volume level (0.0 = mute, 1.0 = full volume)
    pub volume: f32,
    /// Internal YM player instance (created by plugin systems)
    pub(crate) player: Option<Arc<Mutex<Ym6Player>>>,
    /// Audio output device (created by plugin systems)
    pub(crate) audio_device: Option<Arc<dyn AudioSink>>,
    /// Flag to trigger reloading the player on next play
    pub(crate) needs_reload: bool,
    /// Song title extracted from YM file metadata
    pub song_title: String,
    /// Song author extracted from YM file metadata
    pub song_author: String,
}

/// The current state of YM2149 playback
///
/// This enum represents the possible states a playback entity can be in.
/// State transitions are managed by the plugin's playback systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Not playing, no song loaded
    Idle,
    /// Currently playing audio
    Playing,
    /// Paused (can resume)
    Paused,
    /// Song finished playing
    Finished,
}

impl Ym2149Playback {
    /// Create a new playback component with a source path
    ///
    /// The component starts in the `Idle` state. Call `play()` to start playback.
    /// The file will be loaded lazily when playback begins.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to a YM file (YM2-YM6 formats supported)
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
        }
    }

    /// Start playback from the beginning
    ///
    /// Transitions the state from `Idle` or `Paused` to `Playing` and resets the frame position to 0.
    /// If already playing, this method has no effect. The plugin's systems will automatically
    /// load the YM file if not already loaded.
    pub fn play(&mut self) {
        if matches!(self.state, PlaybackState::Playing) {
            return;
        }
        self.state = PlaybackState::Playing;
        self.frame_position = 0;
    }

    /// Resume playback from pause
    ///
    /// Transitions from `Paused` state to `Playing` without resetting the frame position.
    /// Has no effect if not currently paused.
    pub fn resume(&mut self) {
        if matches!(self.state, PlaybackState::Paused) {
            self.state = PlaybackState::Playing;
        }
    }

    /// Pause playback
    ///
    /// Transitions from `Playing` to `Paused` state. The current frame position is preserved,
    /// allowing `resume()` to continue from where it stopped.
    pub fn pause(&mut self) {
        if matches!(self.state, PlaybackState::Playing) {
            self.state = PlaybackState::Paused;
        }
    }

    /// Stop playback and reset position
    ///
    /// Transitions to `Idle` state and resets the frame position to 0.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Idle;
        self.frame_position = 0;
    }

    /// Restart playback from the beginning
    ///
    /// Resets the frame position to 0, sets the state to `Idle`, and marks the file for reload.
    /// This is useful when you want to reload a file that may have changed on disk.
    /// Call `play()` after this to start playback.
    pub fn restart(&mut self) {
        self.state = PlaybackState::Idle;
        self.frame_position = 0;
        self.needs_reload = true;
    }

    /// Seek to a specific frame
    ///
    /// Updates the frame position without changing the playback state.
    /// The requested frame will be played next if in `Playing` state.
    ///
    /// # Arguments
    ///
    /// * `frame` - The target frame number to seek to
    pub fn seek(&mut self, frame: u32) {
        self.frame_position = frame;
    }

    /// Set the playback volume
    ///
    /// The volume is clamped to the range [0.0, 1.0] where:
    /// - 0.0 = muted (silent)
    /// - 0.5 = half volume
    /// - 1.0 = full volume
    ///
    /// # Arguments
    ///
    /// * `volume` - The desired volume level (will be clamped to 0.0-1.0)
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Check if currently playing
    ///
    /// Returns true if the playback state is `Playing`.
    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    /// Get the current frame position
    ///
    /// Returns the index of the current frame being played (0-based).
    pub fn frame_position(&self) -> u32 {
        self.frame_position
    }
}

impl Default for Ym2149Playback {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
        }
    }
}

/// Resource for managing global YM2149 playback settings
///
/// This resource controls plugin-wide settings that affect all playback instances.
/// Access it in systems using `ResMut<Ym2149Settings>`.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use ym2149_bevy::Ym2149Settings;
///
/// fn toggle_loop(mut settings: ResMut<Ym2149Settings>) {
///     settings.loop_enabled = !settings.loop_enabled;
///     println!("Looping: {}", settings.loop_enabled);
/// }
/// ```
#[derive(Resource)]
pub struct Ym2149Settings {
    /// Global master volume multiplier (0.0 - 1.0)
    ///
    /// This is a multiplier applied to individual playback volumes.
    /// 0.0 = muted, 1.0 = full volume. Defaults to 1.0.
    pub master_volume: f32,
    /// Whether songs should loop when they finish
    ///
    /// When enabled, a finished song will automatically restart from the beginning.
    /// Defaults to false (no looping).
    pub loop_enabled: bool,
}

impl Default for Ym2149Settings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            loop_enabled: false,
        }
    }
}
