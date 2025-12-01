use bevy::prelude::Resource;

/// Configuration object used to enable/disable individual subsystems of the plugin.
///
/// All features are enabled by default. Modify fields directly to customize:
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
    /// Enable playlist support for sequential track playback.
    pub playlists: bool,
    /// Emit [`ChannelSnapshot`](crate::events::ChannelSnapshot) events each frame.
    pub channel_events: bool,
    /// Enable the music state machine for adaptive audio.
    pub music_state: bool,
    /// Register Bevy diagnostics for buffer fill levels and frame position.
    pub diagnostics: bool,
    /// Route YM2149 audio through Bevy's audio graph for effects/mixing.
    pub bevy_audio_bridge: bool,
    /// Emit [`PatternTriggered`](crate::events::PatternTriggered) events when patterns match.
    pub pattern_events: bool,
    /// Optional frames-per-beat override for [`BeatHit`](crate::events::BeatHit) events.
    ///
    /// Default is `None`, which uses 50 frames (60 BPM at 50Hz).
    pub frames_per_beat: Option<u64>,
}

impl Default for Ym2149PluginConfig {
    fn default() -> Self {
        Self {
            playlists: true,
            channel_events: true,
            // spatial_audio: true, // Removed - use Bevy's native spatial audio
            music_state: true,
            diagnostics: true,
            bevy_audio_bridge: true,
            pattern_events: true,
            frames_per_beat: None,
        }
    }
}
