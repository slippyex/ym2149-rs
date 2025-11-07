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
//! use bevy_ym2149::Ym2149Playback;
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
//! use bevy_ym2149::Ym2149Playback;
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
use crate::audio_source::Ym2149AudioSource;
use bevy::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use ym2149::replayer::Ym6Player;

/// Fixed output sample rate used by the YM2149 mixer.
pub const YM2149_SAMPLE_RATE: u32 = 44_100;
/// Convenience f32 representation of [`YM2149_SAMPLE_RATE`].
pub const YM2149_SAMPLE_RATE_F32: f32 = YM2149_SAMPLE_RATE as f32;

/// Summary of a loaded track used for progress/duration calculations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlaybackMetrics {
    pub frame_count: usize,
    pub samples_per_frame: u32,
}

impl PlaybackMetrics {
    pub fn total_samples(&self) -> usize {
        self.frame_count
            .saturating_mul(self.samples_per_frame as usize)
    }

    pub fn duration_seconds(&self) -> f32 {
        self.total_samples() as f32 / YM2149_SAMPLE_RATE_F32
    }
}

impl From<&ym2149::LoadSummary> for PlaybackMetrics {
    fn from(summary: &ym2149::LoadSummary) -> Self {
        Self {
            frame_count: summary.frame_count,
            samples_per_frame: summary.samples_per_frame,
        }
    }
}

/// Source descriptor used when queueing a crossfade request.
#[derive(Clone)]
pub(crate) enum TrackSource {
    File(String),
    Asset(Handle<Ym2149AudioSource>),
    Bytes(Arc<Vec<u8>>),
}

/// Pending crossfade to be loaded by the playback systems.
#[derive(Clone)]
pub(crate) struct CrossfadeRequest {
    pub source: TrackSource,
    pub duration: f32,
    pub target_index: usize,
}

/// Active crossfade layer being mixed alongside the primary player.
pub(crate) struct ActiveCrossfade {
    pub player: Arc<Mutex<Ym6Player>>,
    pub metrics: PlaybackMetrics,
    pub song_title: String,
    pub song_author: String,
    pub elapsed: f32,
    pub duration: f32,
    pub target_index: usize,
}

/// Component for managing YM2149 playback on an entity
///
/// This is the primary interface for controlling YM file playback. Spawn this component
/// on an entity to load and play a YM file. The plugin's systems automatically handle
/// audio generation and state management.
///
/// # Fields
///
/// - `source_path`: Optional filesystem path to a YM file
/// - `source_asset`: Optional Bevy asset handle referencing a YM file
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
/// use bevy_ym2149::Ym2149Playback;
///
/// fn my_system(mut playbacks: Query<&mut Ym2149Playback>) {
///     for mut pb in playbacks.iter_mut() {
///         if pb.is_playing() {
///             println!("Frame: {}", pb.frame_position());
///             println!("Title: {}", pb.song_title);
///             pb.set_volume(0.5);
///         }
///     }
/// }
/// ```
#[derive(Component)]
pub struct Ym2149Playback {
    /// Path to the YM file to load and play
    pub source_path: Option<String>,
    /// In-memory YM data buffer
    pub source_bytes: Option<Arc<Vec<u8>>>,
    /// Handle to a YM2149 asset
    pub source_asset: Option<Handle<crate::audio_source::Ym2149AudioSource>>,
    /// Current playback state
    pub state: PlaybackState,
    /// Current frame position in the song
    /// Use the [`frame_position()`](Self::frame_position) getter or [`seek()`](Self::seek) method to access/modify
    pub(crate) frame_position: u32,
    /// Volume level (0.0 = mute, 1.0 = full volume)
    pub volume: f32,
    /// Left channel gain used during spatial mixing.
    /// Use the [`set_stereo_gain()`](Self::set_stereo_gain) method to modify
    pub(crate) left_gain: f32,
    /// Right channel gain used during spatial mixing.
    /// Use the [`set_stereo_gain()`](Self::set_stereo_gain) method to modify
    pub(crate) right_gain: f32,
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
    /// Summary of the currently loaded song (if available).
    pub(crate) metrics: Option<PlaybackMetrics>,
    /// Pending playlist index update once a crossfade completed.
    pub(crate) pending_playlist_index: Option<usize>,
    /// Requested crossfade that is waiting for the secondary deck to load.
    pub(crate) pending_crossfade: Option<CrossfadeRequest>,
    /// Active crossfade state that mixes the next deck.
    pub(crate) crossfade: Option<ActiveCrossfade>,
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
            source_path: Some(source_path.into()),
            source_bytes: None,
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
        }
    }

    /// Create a new playback component that will stream from a loaded Bevy asset.
    pub fn from_asset(handle: Handle<crate::audio_source::Ym2149AudioSource>) -> Self {
        Self {
            source_path: None,
            source_bytes: None,
            source_asset: Some(handle),
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
        }
    }

    /// Create a new playback component backed by an in-memory YM buffer.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            source_path: None,
            source_bytes: Some(Arc::new(bytes.into())),
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
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
        self.crossfade = None;
        self.pending_crossfade = None;
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
        self.crossfade = None;
        self.pending_crossfade = None;
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

    /// Set stereo gains (pan/attenuation) applied during mixing.
    pub fn set_stereo_gain(&mut self, left: f32, right: f32) {
        self.left_gain = left.clamp(0.0, 1.0);
        self.right_gain = right.clamp(0.0, 1.0);
    }

    /// Replace the playback source with a new filesystem path.
    pub fn set_source_path(&mut self, path: impl Into<String>) {
        self.source_path = Some(path.into());
        self.source_bytes = None;
        self.source_asset = None;
        self.needs_reload = true;
        self.metrics = None;
        self.pending_playlist_index = None;
        self.pending_crossfade = None;
        self.crossfade = None;
    }

    /// Replace the playback source with a Bevy asset handle.
    pub fn set_source_asset(&mut self, handle: Handle<crate::audio_source::Ym2149AudioSource>) {
        self.source_asset = Some(handle);
        self.source_path = None;
        self.source_bytes = None;
        self.needs_reload = true;
        self.metrics = None;
        self.pending_playlist_index = None;
        self.pending_crossfade = None;
        self.crossfade = None;
    }

    /// Replace the playback source with in-memory bytes.
    pub fn set_source_bytes(&mut self, bytes: impl Into<Vec<u8>>) {
        self.source_bytes = Some(Arc::new(bytes.into()));
        self.source_path = None;
        self.source_asset = None;
        self.needs_reload = true;
        self.metrics = None;
        self.pending_playlist_index = None;
        self.pending_crossfade = None;
        self.crossfade = None;
    }

    /// Access the configured filesystem path, if any.
    pub fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }

    /// Access the configured asset handle, if any.
    pub fn source_asset(&self) -> Option<&Handle<crate::audio_source::Ym2149AudioSource>> {
        self.source_asset.as_ref()
    }

    /// Access the configured in-memory bytes, if any.
    pub fn source_bytes(&self) -> Option<Arc<Vec<u8>>> {
        self.source_bytes.as_ref().map(Arc::clone)
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

    /// Access the metrics of the currently loaded track, if known.
    pub(crate) fn metrics(&self) -> Option<PlaybackMetrics> {
        self.metrics
    }

    /// Returns whether a crossfade (pending or active) is already configured.
    pub(crate) fn is_crossfade_pending(&self) -> bool {
        self.pending_crossfade.is_some() || self.crossfade.is_some()
    }

    pub(crate) fn is_crossfade_active(&self) -> bool {
        self.crossfade.is_some()
    }

    /// Replace the existing crossfade request (if any).
    pub(crate) fn set_crossfade_request(&mut self, request: CrossfadeRequest) {
        self.pending_crossfade = Some(request);
    }

    /// Clear the crossfade request if one exists.
    pub(crate) fn clear_crossfade_request(&mut self) {
        self.pending_crossfade = None;
    }

    /// Access and clear the pending playlist index update produced by a crossfade.
    pub(crate) fn take_pending_playlist_index(&mut self) -> Option<usize> {
        self.pending_playlist_index.take()
    }

    pub(crate) fn has_pending_playlist_index(&self) -> bool {
        self.pending_playlist_index.is_some()
    }
}

impl Default for Ym2149Playback {
    fn default() -> Self {
        Self {
            source_path: None,
            source_bytes: None,
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            player: None,
            audio_device: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
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
/// use bevy_ym2149::Ym2149Settings;
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
