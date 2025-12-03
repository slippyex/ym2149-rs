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

use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::song_player::{SharedSongPlayer, YmSongPlayer};
use crate::synth::YmSynthController;
use bevy::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

/// Fixed output sample rate used by the YM2149 mixer.
pub const YM2149_SAMPLE_RATE: u32 = 44_100;
/// Convenience f32 representation of [`YM2149_SAMPLE_RATE`].
pub const YM2149_SAMPLE_RATE_F32: f32 = YM2149_SAMPLE_RATE as f32;

/// Summary of a loaded track used for progress/duration calculations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackMetrics {
    /// Total number of frames in the track.
    pub frame_count: usize,
    /// Number of audio samples generated per frame (depends on frame rate).
    pub samples_per_frame: u32,
}

impl PlaybackMetrics {
    /// Returns the total number of audio samples in the track.
    pub fn total_samples(&self) -> usize {
        self.frame_count
            .saturating_mul(self.samples_per_frame as usize)
    }

    /// Returns the track duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        self.total_samples() as f32 / YM2149_SAMPLE_RATE_F32
    }
}

impl From<&ym2149_ym_replayer::LoadSummary> for PlaybackMetrics {
    fn from(summary: &ym2149_ym_replayer::LoadSummary) -> Self {
        Self {
            frame_count: summary.frame_count,
            samples_per_frame: summary.samples_per_frame,
        }
    }
}

/// Per-playback tone shaping options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneSettings {
    /// Soft saturation amount (0.0 = off).
    pub saturation: f32,
    /// Dynamic accent amount (0.0 = off).
    pub accent: f32,
    /// Stereo widening amount (0.0 = mono).
    pub widen: f32,
    /// ST-style color filter applied after tone shaping.
    pub color_filter: bool,
}

impl Default for ToneSettings {
    fn default() -> Self {
        Self {
            saturation: 0.0,
            accent: 0.0,
            widen: 0.0,
            color_filter: true,
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
///
/// Note: Uses Arc<RwLock<...>> to enable shared ownership between the crossfade
/// state and the audio source decoder. This is necessary because:
/// 1. Crossfade needs simultaneous access to both primary and secondary players
/// 2. The audio decoder holds a reference that must outlive individual system calls
/// 3. Bevy's audio system requires thread-safe shared access
///
/// RwLock allows multiple concurrent readers while ensuring exclusive write access
/// during sample generation. This reduces lock contention compared to Mutex.
pub(crate) struct ActiveCrossfade {
    pub player: SharedSongPlayer,
    pub metrics: PlaybackMetrics,
    pub song_title: String,
    pub song_author: String,
    pub elapsed: f32,
    pub duration: f32,
    pub target_index: usize,
    pub audio_handle: Handle<crate::audio_source::Ym2149AudioSource>,
    /// Raw YM data for recreating the AudioPlayer after crossfade completes
    pub data: Arc<Vec<u8>>,
    /// Entity of the separate AudioPlayer playing the incoming track during crossfade
    pub crossfade_entity: Option<Entity>,
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
    /// Left channel gain used during stereo mixing.
    /// Use the [`set_stereo_gain()`](Self::set_stereo_gain) method to modify
    pub(crate) left_gain: f32,
    /// Right channel gain used during stereo mixing.
    /// Use the [`set_stereo_gain()`](Self::set_stereo_gain) method to modify
    pub(crate) right_gain: f32,
    /// Shared stereo gain handle used by both decoder and diagnostics.
    pub(crate) stereo_gain: Arc<RwLock<(f32, f32)>>,
    /// Internal YM player instance (created by plugin systems)
    ///
    /// Uses `Arc<RwLock<_>>` for shared ownership with the audio decoder.
    /// See [`ActiveCrossfade`] documentation for rationale.
    pub(crate) player: Option<SharedSongPlayer>,
    /// Flag to trigger reloading the player on next play
    pub(crate) needs_reload: bool,
    /// Song title extracted from YM file metadata
    pub song_title: String,
    /// Song author extracted from YM file metadata
    pub song_author: String,
    /// Tone-shaping configuration shared with the decoder
    pub tone_settings: Arc<RwLock<ToneSettings>>,
    /// Summary of the currently loaded song (if available).
    pub(crate) metrics: Option<PlaybackMetrics>,
    /// Pending playlist index update once a crossfade completed.
    pub(crate) pending_playlist_index: Option<usize>,
    /// Requested crossfade that is waiting for the secondary deck to load.
    pub(crate) pending_crossfade: Option<CrossfadeRequest>,
    /// Active crossfade state that mixes the next deck.
    pub(crate) crossfade: Option<ActiveCrossfade>,
    /// Indicates that the playback uses an inline (synth) player instead of streamed assets.
    pub(crate) inline_player: bool,
    pub(crate) inline_audio_ready: bool,
    pub(crate) inline_metadata: Option<Ym2149Metadata>,
    /// Pending subsong index to set after reload (1-based, None means default)
    pub(crate) pending_subsong: Option<usize>,
    /// Cached subsong count (preserved during reload)
    pub(crate) cached_subsong_count: usize,
    /// Cached current subsong index (preserved during reload, 1-based)
    pub(crate) cached_current_subsong: usize,
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
    /// * `source_path` - Path to a YM file (YM2-YM6 formats supported).
    ///   Should not be empty; an empty path will cause a load error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bevy_ym2149::Ym2149Playback;
    /// let playback = Ym2149Playback::new("assets/music/song.ym");
    /// ```
    pub fn new(source_path: impl Into<String>) -> Self {
        let path = source_path.into();
        debug_assert!(!path.is_empty(), "source_path should not be empty");
        Self {
            source_path: Some(path),
            source_bytes: None,
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            stereo_gain: Arc::new(RwLock::new((1.0, 1.0))),
            player: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
            inline_player: false,
            inline_audio_ready: false,
            inline_metadata: None,
            pending_subsong: None,
            cached_subsong_count: 1,
            cached_current_subsong: 1,
            tone_settings: Arc::new(RwLock::new(ToneSettings::default())),
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
            stereo_gain: Arc::new(RwLock::new((1.0, 1.0))),
            player: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
            inline_player: false,
            inline_audio_ready: false,
            inline_metadata: None,
            pending_subsong: None,
            cached_subsong_count: 1,
            cached_current_subsong: 1,
            tone_settings: Arc::new(RwLock::new(ToneSettings::default())),
        }
    }

    /// Create a new playback component backed by an in-memory YM buffer.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw YM file data. Should not be empty; empty data will cause
    ///   a load error.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        let data = bytes.into();
        debug_assert!(!data.is_empty(), "bytes should not be empty");
        Self {
            source_path: None,
            source_bytes: Some(Arc::new(data)),
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            stereo_gain: Arc::new(RwLock::new((1.0, 1.0))),
            player: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
            inline_player: false,
            inline_audio_ready: false,
            inline_metadata: None,
            pending_subsong: None,
            cached_subsong_count: 1,
            cached_current_subsong: 1,
            tone_settings: Arc::new(RwLock::new(ToneSettings::default())),
        }
    }

    /// Create a playback component that drives a live YM2149 synthesizer.
    pub fn synth(controller: YmSynthController) -> Self {
        let synth_player = YmSongPlayer::new_synth(controller);
        let metadata = synth_player.metadata().clone();
        let metrics = synth_player.metrics().unwrap_or(PlaybackMetrics {
            frame_count: metadata.frame_count,
            samples_per_frame: YM2149_SAMPLE_RATE / 50,
        });
        let player = Arc::new(RwLock::new(synth_player));
        Self {
            source_path: None,
            source_bytes: None,
            source_asset: None,
            state: PlaybackState::Idle,
            frame_position: 0,
            volume: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
            stereo_gain: Arc::new(RwLock::new((1.0, 1.0))),
            player: Some(player),
            needs_reload: false,
            song_title: metadata.title.clone(),
            song_author: metadata.author.clone(),
            metrics: Some(metrics),
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
            inline_player: true,
            inline_audio_ready: false,
            inline_metadata: Some(metadata),
            pending_subsong: None,
            cached_subsong_count: 1,
            cached_current_subsong: 1,
            tone_settings: Arc::new(RwLock::new(ToneSettings::default())),
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
        self.metrics = None;
        self.player = None;
        self.inline_audio_ready = false;
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

    /// Set the playback volume (global gain, unclamped upper bound).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0);
    }

    /// Current tone settings (copied out of the shared state).
    pub fn tone_settings(&self) -> ToneSettings {
        *self.tone_settings.read()
    }

    /// Update tone-shaping settings (saturation/accent/widen).
    pub fn set_tone_settings(&mut self, settings: ToneSettings) {
        *self.tone_settings.write() = settings;
    }

    /// Set stereo gains (pan/attenuation) applied during mixing.
    pub fn set_stereo_gain(&mut self, left: f32, right: f32) {
        let clamped_left = left.clamp(0.0, 1.0);
        let clamped_right = right.clamp(0.0, 1.0);
        {
            let mut gains = self.stereo_gain.write();
            *gains = (clamped_left, clamped_right);
        }
        self.left_gain = clamped_left;
        self.right_gain = clamped_right;
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

    /// Clone the internal YM player handle for read-only inspection.
    ///
    /// Returns a shared reference to the player wrapped in Arc<RwLock<>>.
    /// This is primarily useful for debugging or advanced use cases that need
    /// direct access to the player state.
    ///
    /// Note: Use read() for concurrent read access or write() for exclusive access:
    /// ```ignore
    /// if let Some(player_arc) = playback.player_handle() {
    ///     let player = player_arc.read();  // Concurrent reads allowed
    ///     let frame = player.current_frame();
    /// }
    /// ```
    pub fn player_handle(&self) -> Option<SharedSongPlayer> {
        self.player.as_ref().map(Arc::clone)
    }

    /// Query the current audio sink buffer fill percentage (0.0 - 1.0).
    ///
    /// Note: This method is deprecated with Bevy audio and always returns None.
    /// Audio buffering is now handled internally by Bevy's audio system.
    pub fn audio_buffer_fill(&self) -> Option<f32> {
        None
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

    /// Get the number of subsongs/tracks available in the current file.
    /// Returns 1 for formats that don't support multiple subsongs.
    pub fn subsong_count(&self) -> usize {
        // Use cached value if player is not available (during reload)
        self.player
            .as_ref()
            .map(|p| p.read().subsong_count())
            .unwrap_or(self.cached_subsong_count)
    }

    /// Get the current subsong index (1-based).
    /// Returns 1 for formats that don't support multiple subsongs.
    pub fn current_subsong(&self) -> usize {
        // If we have a pending subsong, report that as current
        if let Some(pending) = self.pending_subsong {
            return pending;
        }
        // Use cached value if player is not available (during reload)
        self.player
            .as_ref()
            .map(|p| p.read().current_subsong())
            .unwrap_or(self.cached_current_subsong)
    }

    /// Update the cached subsong info from the player.
    /// Called by the playback system after loading.
    pub(crate) fn update_subsong_cache(&mut self) {
        if let Some(player) = &self.player {
            let player_guard = player.read();
            self.cached_subsong_count = player_guard.subsong_count();
            self.cached_current_subsong = player_guard.current_subsong();
        }
    }

    /// Switch to a different subsong. Returns true if successful.
    /// The index is 1-based (first subsong is 1).
    /// This triggers a full reload to ensure audio output is in sync.
    pub fn set_subsong(&mut self, index: usize) -> bool {
        let count = self.cached_subsong_count;
        if index < 1 || index > count {
            return false;
        }
        // Update cached current subsong immediately
        self.cached_current_subsong = index;
        // Store the desired subsong and trigger a full reload
        // Clear the player to force a complete reload path
        self.pending_subsong = Some(index);
        self.player = None;
        self.metrics = None;
        self.needs_reload = true;
        true
    }

    /// Switch to the next subsong (wraps around).
    /// Returns the new subsong index, or None if not supported.
    pub fn next_subsong(&mut self) -> Option<usize> {
        let count = self.cached_subsong_count;
        if count <= 1 {
            return None;
        }
        let current = self.current_subsong();
        let next = if current >= count { 1 } else { current + 1 };
        if self.set_subsong(next) {
            Some(next)
        } else {
            None
        }
    }

    /// Switch to the previous subsong (wraps around).
    /// Returns the new subsong index, or None if not supported.
    pub fn prev_subsong(&mut self) -> Option<usize> {
        let count = self.cached_subsong_count;
        if count <= 1 {
            return None;
        }
        let current = self.current_subsong();
        let prev = if current <= 1 { count } else { current - 1 };
        if self.set_subsong(prev) {
            Some(prev)
        } else {
            None
        }
    }

    /// Check if this playback supports multiple subsongs.
    pub fn has_subsongs(&self) -> bool {
        self.cached_subsong_count > 1
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
            stereo_gain: Arc::new(RwLock::new((1.0, 1.0))),
            player: None,
            needs_reload: false,
            song_title: String::new(),
            song_author: String::new(),
            metrics: None,
            pending_playlist_index: None,
            pending_crossfade: None,
            crossfade: None,
            inline_player: false,
            inline_audio_ready: false,
            inline_metadata: None,
            pending_subsong: None,
            cached_subsong_count: 1,
            cached_current_subsong: 1,
            tone_settings: Arc::new(RwLock::new(ToneSettings::default())),
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
