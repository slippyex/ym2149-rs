//! Visualization module composed of reusable components, builders, and systems.

mod builders;
mod components;
mod helpers;
mod systems;

pub use builders::{
    create_channel_visualization, create_detailed_channel_display, create_oscilloscope,
    create_song_info_display, create_status_display,
};
pub use components::{
    BadgeKind, ChannelBadge, ChannelFreqLabel, ChannelNoteLabel, DetailedChannelDisplay,
    LoopStatusLabel, Oscilloscope, OscilloscopeBuffer, OscilloscopeChannel, OscilloscopeGridLine,
    OscilloscopeHead, OscilloscopePoint, PlaybackStatusDisplay, SongInfoDisplay, SongProgressFill,
    SongProgressLabel, SpectrumBar,
};
pub use systems::{
    update_detailed_channel_display, update_oscilloscope, update_song_info, update_song_progress,
    update_status_display,
};
