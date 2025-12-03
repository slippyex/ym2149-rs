//! UI marker components for YM2149 visualization widgets.
//!
//! These components are attached to UI nodes to allow the systems in this crate
//! to locate and update the correct elements.

use bevy::prelude::*;

/// Number of plotted points per channel in the oscilloscope.
pub const OSCILLOSCOPE_RESOLUTION: usize = 256;

/// Vertical size of the oscilloscope canvas in pixels.
pub const OSCILLOSCOPE_HEIGHT: f32 = 130.0;

/// Component for displaying playback status text.
#[derive(Component)]
pub struct PlaybackStatusDisplay;

/// Component for the multi-line detailed channel display.
#[derive(Component)]
pub struct DetailedChannelDisplay;

/// Component for song metadata text.
#[derive(Component)]
pub struct SongInfoDisplay;

/// Root component for the oscilloscope canvas.
#[derive(Component)]
pub struct Oscilloscope;

/// Layer that renders a channel waveform inside the oscilloscope.
#[derive(Component)]
pub struct OscilloscopeChannel {
    /// Channel index (0-2).
    pub index: usize,
    /// RGB color for this channel's waveform.
    pub base_color: Vec3,
}

/// Highlight dot shown at the most recent sample position.
#[derive(Component)]
pub struct OscilloscopeHead {
    /// Channel index (0-2).
    pub channel: usize,
}

/// Individual plotted sample node within the oscilloscope.
#[derive(Component)]
pub struct OscilloscopePoint {
    /// Channel index (0-2).
    pub channel: usize,
    /// Point index within the waveform buffer.
    pub index: usize,
}

/// Grid line element within the oscilloscope background.
#[derive(Component)]
pub struct OscilloscopeGridLine;

/// Single spectrum bar rendered inside the channel ribbon.
#[derive(Component)]
pub struct SpectrumBar {
    /// Channel index (0-2).
    pub channel: usize,
    /// Frequency bin index.
    pub bin: usize,
}

/// Decorative badge element associated with a channel.
#[derive(Clone, Copy, Component)]
pub struct ChannelBadge {
    /// Channel index (0-2).
    pub channel: usize,
    /// Type of badge to display.
    pub kind: BadgeKind,
}

/// Variants of channel badge.
#[derive(Clone, Copy)]
pub enum BadgeKind {
    /// Badge showing amplitude level.
    Amplitude,
    /// Badge indicating high frequency output.
    HighFreq,
}

/// Fill node for the transport progress bar.
#[derive(Component)]
pub struct SongProgressFill;

/// Text label displaying the transport percentage.
#[derive(Component)]
pub struct SongProgressLabel;

/// Text label showing the loop status.
#[derive(Component)]
pub struct LoopStatusLabel;

/// Text label displaying the current note for a channel.
#[derive(Component)]
pub struct ChannelNoteLabel {
    /// Channel index (0-2).
    pub channel: usize,
}

/// Text label displaying the current frequency for a channel.
#[derive(Component)]
pub struct ChannelFreqLabel {
    /// Channel index (0-2).
    pub channel: usize,
}
