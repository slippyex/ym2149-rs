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
