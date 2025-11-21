use bevy::prelude::*;

/// Snapshot of a channel's current amplitude/frequency.
#[derive(Event, Message, Clone, Debug)]
pub struct ChannelSnapshot {
    pub entity: Entity,
    pub channel: usize,
    pub amplitude: f32,
    pub frequency: Option<f32>,
}

/// Fired whenever a playback entity begins emitting audio (after a source is loaded).
#[derive(Event, Message, Clone, Debug)]
pub struct TrackStarted {
    pub entity: Entity,
}

/// Fired when a playback entity reaches the end of its track and stops (non-looping).
#[derive(Event, Message, Clone, Debug)]
pub struct TrackFinished {
    pub entity: Entity,
}

/// Fired every VBL-frame (50Hz) with timestamp and loop info.
#[derive(Event, Message, Clone, Debug)]
pub struct PlaybackFrameMarker {
    pub entity: Entity,
    pub frame: u64,
    pub elapsed_seconds: f32,
    pub looped: bool,
}

/// Request to switch to a named music state.
#[derive(Event, Message, Clone, Debug)]
pub struct MusicStateRequest {
    pub state: String,
    pub target: Option<Entity>,
}

/// Request to advance a playlist to a specific entry (or the next one if `index` is `None`).
#[derive(Event, Message, Clone, Debug)]
pub struct PlaylistAdvanceRequest {
    pub entity: Entity,
    pub index: Option<usize>,
}

/// Request to route a playback entity through Bevy's audio graph.
#[derive(Event, Message, Clone, Debug)]
pub struct AudioBridgeRequest {
    pub entity: Entity,
}

/// Trigger a lightweight YM2149 SFX tone on a playback entity (or all entities if `target` is `None`).
#[derive(Event, Message, Clone, Debug)]
pub struct YmSfxRequest {
    pub target: Option<Entity>,
    pub channel: usize,
    pub freq_hz: f32,
    pub volume: f32,
    pub duration_frames: u32,
}

/// Beat marker derived from frame markers (e.g. every N frames/BPM-grid).
#[derive(Event, Message, Clone, Debug)]
pub struct BeatHit {
    pub entity: Entity,
    pub beat_index: u64,
    pub elapsed_seconds: f32,
}

/// Fired when a [`PatternTrigger`](crate::patterns::PatternTrigger) matches.
#[derive(Event, Message, Clone, Debug)]
pub struct PatternTriggered {
    pub entity: Entity,
    pub pattern_id: String,
    pub channel: usize,
    pub amplitude: f32,
    pub frequency: Option<f32>,
    pub frame: u64,
    pub elapsed_seconds: f32,
}
