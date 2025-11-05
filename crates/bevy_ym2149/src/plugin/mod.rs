//! Plugin orchestration for YM2149 playback within Bevy.
//!
//! This module contains the main Bevy plugin definition, configuration, and
//! system wiring that integrates YM2149 playback into any Bevy application.

mod systems;

use self::systems::{initialize_playback, update_playback};
use crate::audio_bridge::{
    drive_bridge_audio_buffers, handle_bridge_requests, AudioBridgeBuffers, AudioBridgeMixes,
    AudioBridgeTargets, BridgeAudioDevice, BridgeAudioSinks,
};
use crate::audio_source::{Ym2149AudioSource, Ym2149Loader};
use crate::diagnostics::{register as register_diagnostics, update_diagnostics};
use crate::events::{
    AudioBridgeRequest, ChannelSnapshot, MusicStateRequest, PlaylistAdvanceRequest, TrackFinished,
    TrackStarted,
};
use crate::music_state::{process_music_state_requests, MusicStateGraph};
use crate::playback::Ym2149Settings;
use crate::playlist::{
    advance_playlist_players, handle_playlist_requests, register_playlist_assets, Ym2149Playlist,
};
use crate::spatial::update_spatial_audio;
#[cfg(feature = "visualization")]
use crate::uniforms::{OscilloscopeUniform, SpectrumUniform};
#[cfg(feature = "visualization")]
use crate::viz_components::OscilloscopeBuffer;
#[cfg(feature = "visualization")]
use crate::viz_systems::{
    update_detailed_channel_display, update_oscilloscope, update_song_info, update_song_progress,
    update_status_display,
};
use bevy::prelude::*;

/// Configuration object used to enable/disable individual subsystems of the plugin.
#[derive(Debug, Clone, Resource)]
pub struct Ym2149PluginConfig {
    pub visualization: bool,
    pub playlists: bool,
    pub channel_events: bool,
    pub spatial_audio: bool,
    pub music_state: bool,
    pub shader_uniforms: bool,
    pub diagnostics: bool,
    pub bevy_audio_bridge: bool,
}

impl Default for Ym2149PluginConfig {
    fn default() -> Self {
        Self {
            visualization: cfg!(feature = "visualization"),
            playlists: true,
            channel_events: true,
            spatial_audio: true,
            music_state: true,
            shader_uniforms: cfg!(feature = "visualization"),
            diagnostics: true,
            bevy_audio_bridge: true,
        }
    }
}

impl Ym2149PluginConfig {
    pub fn visualization(mut self, enabled: bool) -> Self {
        self.visualization = enabled;
        self
    }

    pub fn playlists(mut self, enabled: bool) -> Self {
        self.playlists = enabled;
        self
    }

    pub fn channel_events(mut self, enabled: bool) -> Self {
        self.channel_events = enabled;
        self
    }

    pub fn spatial_audio(mut self, enabled: bool) -> Self {
        self.spatial_audio = enabled;
        self
    }

    pub fn music_state(mut self, enabled: bool) -> Self {
        self.music_state = enabled;
        self
    }

    pub fn shader_uniforms(mut self, enabled: bool) -> Self {
        self.shader_uniforms = enabled;
        self
    }

    pub fn diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    pub fn bevy_audio_bridge(mut self, enabled: bool) -> Self {
        self.bevy_audio_bridge = enabled;
        self
    }
}

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

        // Event channels always exist; individual systems check configuration flags
        // before emitting to avoid unnecessary work if the user disables them.
        app.add_message::<ChannelSnapshot>();
        app.add_message::<TrackStarted>();
        app.add_message::<TrackFinished>();
        app.add_message::<MusicStateRequest>();
        app.add_message::<PlaylistAdvanceRequest>();
        app.add_message::<AudioBridgeRequest>();

        // Core playback lifecycle.
        app.add_systems(PreUpdate, initialize_playback);
        if self.config.spatial_audio {
            app.add_systems(Update, update_spatial_audio);
        }
        app.add_systems(Update, update_playback);

        // Optional playlist support.
        if self.config.playlists {
            app.init_asset::<Ym2149Playlist>();
            register_playlist_assets(app);
            app.add_systems(Update, advance_playlist_players);
            app.add_systems(Update, handle_playlist_requests);
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
            app.add_systems(Update, (handle_bridge_requests, drive_bridge_audio_buffers));
        }

        if self.config.diagnostics {
            register_diagnostics(app);
            app.add_systems(Update, update_diagnostics);
        }

        // Visualization extras (oscilloscope, labels, progress).
        #[cfg(feature = "visualization")]
        if self.config.visualization {
            app.init_resource::<OscilloscopeBuffer>();
            app.init_resource::<OscilloscopeUniform>();
            app.init_resource::<SpectrumUniform>();

            app.add_systems(Update, update_song_info);
            app.add_systems(Update, update_status_display);
            app.add_systems(Update, update_detailed_channel_display);
            app.add_systems(Update, update_song_progress);
            app.add_systems(Update, update_oscilloscope);
        }
    }
}
