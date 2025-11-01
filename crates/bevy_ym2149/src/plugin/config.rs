use bevy::prelude::Resource;

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
