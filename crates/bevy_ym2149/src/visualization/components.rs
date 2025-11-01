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
    pub index: usize,
    pub base_color: Vec3,
}

/// Highlight dot shown at the most recent sample position.
#[derive(Component)]
pub struct OscilloscopeHead {
    pub channel: usize,
}

/// Individual plotted sample node within the oscilloscope.
#[derive(Component)]
pub struct OscilloscopePoint {
    pub channel: usize,
    pub index: usize,
}

/// Grid line element within the oscilloscope background.
#[derive(Component)]
pub struct OscilloscopeGridLine;

/// Single spectrum bar rendered inside the channel ribbon.
#[derive(Component)]
pub struct SpectrumBar {
    pub channel: usize,
    pub bin: usize,
}

/// Decorative badge element associated with a channel.
#[derive(Clone, Copy, Component)]
pub struct ChannelBadge {
    pub channel: usize,
    pub kind: BadgeKind,
}

/// Variants of channel badge.
#[derive(Clone, Copy)]
pub enum BadgeKind {
    Amplitude,
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
    pub channel: usize,
}

/// Text label displaying the current frequency for a channel.
#[derive(Component)]
pub struct ChannelFreqLabel {
    pub channel: usize,
}

/// Rolling buffer of oscilloscope samples.
#[derive(Resource, Clone)]
pub struct OscilloscopeBuffer {
    samples: Vec<[f32; 3]>,
    capacity: usize,
    index: usize,
}

impl OscilloscopeBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![[0.0; 3]; capacity],
            capacity,
            index: 0,
        }
    }

    pub fn push_sample(&mut self, sample: [f32; 3]) {
        self.samples[self.index] = [
            sample[0].clamp(-1.0, 1.0),
            sample[1].clamp(-1.0, 1.0),
            sample[2].clamp(-1.0, 1.0),
        ];
        self.index = (self.index + 1) % self.capacity;
    }

    pub fn get_samples(&self) -> Vec<[f32; 3]> {
        (0..self.capacity)
            .map(|offset| {
                let idx = (self.index + offset) % self.capacity;
                self.samples[idx]
            })
            .collect()
    }
}

impl Default for OscilloscopeBuffer {
    fn default() -> Self {
        Self::new(512)
    }
}
