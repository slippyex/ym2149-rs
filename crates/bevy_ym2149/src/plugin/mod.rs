//! Plugin orchestration for YM2149 playback within Bevy.
//!
//! This module contains the main Bevy plugin definition, configuration, and
//! system wiring that integrates YM2149 playback into any Bevy application.

mod config;
mod systems;

pub use config::Ym2149PluginConfig;

use self::systems::{
    FrameAudioData, drive_playback_state, emit_beat_hits, emit_frame_markers,
    emit_playback_diagnostics, initialize_playback, process_playback_frames, process_sfx_requests,
    publish_bridge_audio, update_audio_reactive_state,
};
use crate::audio_bridge::{
    AudioBridgeBuffers, AudioBridgeMixes, AudioBridgeTargets, BridgeAudioDevice, BridgeAudioSinks,
    drive_bridge_audio_buffers, handle_bridge_requests,
};
use crate::audio_reactive::AudioReactiveState;
use crate::audio_source::{Ym2149AudioSource, Ym2149Loader};
use crate::diagnostics::{register as register_diagnostics, update_diagnostics};
use crate::events::{
    AudioBridgeRequest, BeatHit, ChannelSnapshot, MusicStateRequest, PlaybackFrameMarker,
    PlaylistAdvanceRequest, TrackFinished, TrackStarted, YmSfxRequest,
};
use crate::music_state::{MusicStateGraph, process_music_state_requests};
use crate::playback::Ym2149Settings;
use crate::playlist::{
    Ym2149Playlist, advance_playlist_players, drive_crossfade_playlists, handle_playlist_requests,
    register_playlist_assets,
};
// Spatial audio removed - use Bevy's native spatial audio instead
use bevy::audio::AddAudioSource;
use bevy::prelude::*;

/// Bevy plugin responsible for YM2149 playback integration.
#[derive(Default)]
pub struct Ym2149Plugin {
    config: Ym2149PluginConfig,
}

impl Ym2149Plugin {
    /// Create a plugin instance with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a plugin instance using the provided configuration.
    pub fn with_config(config: Ym2149PluginConfig) -> Self {
        Self { config }
    }

    /// Apply mutations to the internal configuration prior to registering.
    pub fn configure(mut self, configure: impl FnOnce(&mut Ym2149PluginConfig)) -> Self {
        configure(&mut self.config);
        self
    }

    /// Access the current configuration.
    pub fn config(&self) -> &Ym2149PluginConfig {
        &self.config
    }
}

impl Plugin for Ym2149Plugin {
    fn build(&self, app: &mut App) {
        // Expose configuration and global playback settings.
        app.insert_resource(self.config.clone());
        app.init_resource::<Ym2149Settings>();

        // Register YM assets with Bevy's asset server.
        app.init_asset::<Ym2149AudioSource>();
        app.init_asset_loader::<Ym2149Loader>();
        // Register Ym2149AudioSource as a Decodable audio source
        app.add_audio_source::<Ym2149AudioSource>();

        // Event channels always exist; individual systems check configuration flags
        // before emitting to avoid unnecessary work if the user disables them.
        app.add_message::<ChannelSnapshot>();
        app.add_message::<TrackStarted>();
        app.add_message::<TrackFinished>();
        app.add_message::<MusicStateRequest>();
        app.add_message::<PlaylistAdvanceRequest>();
        app.add_message::<AudioBridgeRequest>();
        app.add_message::<FrameAudioData>();
        app.add_message::<PlaybackFrameMarker>();
        app.add_message::<BeatHit>();
        app.add_message::<YmSfxRequest>();
        app.init_resource::<AudioReactiveState>();

        // Core playback lifecycle.
        app.add_systems(PreUpdate, (initialize_playback, drive_playback_state));
        app.add_systems(
            Update,
            (
                process_sfx_requests.before(process_playback_frames),
                process_playback_frames,
                emit_frame_markers.after(process_playback_frames),
                update_audio_reactive_state.after(process_playback_frames),
                emit_beat_hits.after(emit_frame_markers),
            ),
        );
        // Optional playlist support.
        if self.config.playlists {
            app.init_asset::<Ym2149Playlist>();
            register_playlist_assets(app);
            app.add_systems(
                Update,
                (
                    drive_crossfade_playlists,
                    advance_playlist_players,
                    handle_playlist_requests,
                ),
            );
        }

        if self.config.channel_events || self.config.diagnostics {
            app.add_systems(Update, emit_playback_diagnostics);
        }

        // Optional music state graph.
        if self.config.music_state {
            app.init_resource::<MusicStateGraph>();
            app.add_systems(Update, process_music_state_requests);
        }

        if self.config.bevy_audio_bridge {
            app.init_resource::<AudioBridgeTargets>();
            app.init_resource::<AudioBridgeBuffers>();
            app.init_resource::<AudioBridgeMixes>();
            app.init_resource::<BridgeAudioDevice>();
            app.init_resource::<BridgeAudioSinks>();
            app.add_systems(
                Update,
                (
                    publish_bridge_audio,
                    handle_bridge_requests,
                    drive_bridge_audio_buffers,
                ),
            );
        }

        if self.config.diagnostics {
            register_diagnostics(app);
            app.add_systems(Update, update_diagnostics);
        }
    }
}
