use bevy::prelude::Resource;

/// Configuration object used to enable/disable individual subsystems of the plugin.
///
/// All features except `bevy_audio_bridge` (experimental) are enabled by default.
/// Modify fields directly to customize:
///
/// ```
/// # use bevy_ym2149::Ym2149PluginConfig;
/// let config = Ym2149PluginConfig {
///     playlists: false,
///     diagnostics: false,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Resource)]
pub struct Ym2149PluginConfig {
    pub playlists: bool,
    pub channel_events: bool,
    // Spatial audio removed - use Bevy's native spatial audio instead
    // pub spatial_audio: bool,
    pub music_state: bool,
    pub diagnostics: bool,
    pub bevy_audio_bridge: bool,
}

impl Default for Ym2149PluginConfig {
    fn default() -> Self {
        Self {
            playlists: true,
            channel_events: true,
            // spatial_audio: true, // Removed - use Bevy's native spatial audio
            music_state: true,
            diagnostics: true,
            bevy_audio_bridge: false, // Experimental - enable explicitly if needed
        }
    }
}
