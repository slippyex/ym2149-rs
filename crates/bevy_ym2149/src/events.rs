//! Events emitted by the YM2149 plugin during playback.
//!
//! These events can be used to react to playback state changes, channel activity,
//! and custom triggers in your game logic.

use bevy::prelude::*;

/// Snapshot of a channel's current amplitude/frequency.
#[derive(Event, Message, Clone, Debug)]
pub struct ChannelSnapshot {
    /// The playback entity this snapshot belongs to.
    pub entity: Entity,
    /// Channel index (0-2 for channels A, B, C).
    pub channel: usize,
    /// Current amplitude (0.0 to 1.0).
    pub amplitude: f32,
    /// Current frequency in Hz, if tone is enabled.
    pub frequency: Option<f32>,
}

/// Fired whenever a playback entity begins emitting audio (after a source is loaded).
#[derive(Event, Message, Clone, Debug)]
pub struct TrackStarted {
    /// The playback entity that started.
    pub entity: Entity,
}

/// Fired when a playback entity reaches the end of its track and stops (non-looping).
#[derive(Event, Message, Clone, Debug)]
pub struct TrackFinished {
    /// The playback entity that finished.
    pub entity: Entity,
}

/// Fired every VBL-frame (50Hz) with timestamp and loop info.
#[derive(Event, Message, Clone, Debug)]
pub struct PlaybackFrameMarker {
    /// The playback entity.
    pub entity: Entity,
    /// Current frame number since playback started.
    pub frame: u64,
    /// Elapsed time in seconds.
    pub elapsed_seconds: f32,
    /// Whether the song has looped at least once.
    pub looped: bool,
}

/// Request to switch to a named music state.
#[derive(Event, Message, Clone, Debug)]
pub struct MusicStateRequest {
    /// Name of the target state (e.g., "menu", "gameplay", "boss").
    pub state: String,
    /// Optional target entity. If `None`, applies to all playback entities.
    pub target: Option<Entity>,
}

/// Request to advance a playlist to a specific entry (or the next one if `index` is `None`).
#[derive(Event, Message, Clone, Debug)]
pub struct PlaylistAdvanceRequest {
    /// The playlist entity to advance.
    pub entity: Entity,
    /// Target index, or `None` for next track.
    pub index: Option<usize>,
}

/// Request to route a playback entity through Bevy's audio graph.
#[derive(Event, Message, Clone, Debug)]
pub struct AudioBridgeRequest {
    /// The playback entity to bridge.
    pub entity: Entity,
}

/// Trigger a lightweight YM2149 SFX tone on a playback entity (or all entities if `target` is `None`).
#[derive(Event, Message, Clone, Debug)]
pub struct YmSfxRequest {
    /// Target entity, or `None` for all.
    pub target: Option<Entity>,
    /// Channel to use (0-2).
    pub channel: usize,
    /// Frequency in Hz.
    pub freq_hz: f32,
    /// Volume (0.0 to 1.0).
    pub volume: f32,
    /// Duration in VBL frames (50Hz).
    pub duration_frames: u32,
}

/// Beat marker derived from frame markers (e.g. every N frames/BPM-grid).
#[derive(Event, Message, Clone, Debug)]
pub struct BeatHit {
    /// The playback entity.
    pub entity: Entity,
    /// Beat index since playback started.
    pub beat_index: u64,
    /// Elapsed time in seconds.
    pub elapsed_seconds: f32,
}

/// Fired when a [`PatternTrigger`](crate::patterns::PatternTrigger) matches.
#[derive(Event, Message, Clone, Debug)]
pub struct PatternTriggered {
    /// The playback entity.
    pub entity: Entity,
    /// ID of the triggered pattern.
    pub pattern_id: String,
    /// Channel that matched (0-2).
    pub channel: usize,
    /// Amplitude at trigger time.
    pub amplitude: f32,
    /// Frequency at trigger time, if available.
    pub frequency: Option<f32>,
    /// Frame number at trigger time.
    pub frame: u64,
    /// Elapsed time in seconds.
    pub elapsed_seconds: f32,
}
