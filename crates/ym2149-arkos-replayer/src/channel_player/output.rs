//! Channel output types for PSG parameters and sample commands.

use std::sync::Arc;

/// Output from channel player (PSG parameters for one channel)
#[derive(Debug, Clone, Default)]
pub struct ChannelOutput {
    /// Volume (0-15 for software, 16 for hardware envelope)
    pub volume: u8,
    /// Noise period (0-31, 0 = no noise)
    pub noise: u8,
    /// Whether tone channel is open (audible)
    pub sound_open: bool,
    /// Software period (tone frequency)
    pub software_period: u16,
    /// Hardware envelope period
    pub hardware_period: u16,
    /// Hardware envelope shape (0-15)
    pub hardware_envelope: u8,
    /// Whether to retrigger hardware envelope
    pub hardware_retrig: bool,
}

/// Sample playback parameters emitted by a channel
#[derive(Debug, Clone)]
pub struct SamplePlaybackParams {
    /// PCM data in -1.0..1.0 range
    pub data: Arc<Vec<f32>>,
    /// Loop start index (inclusive)
    pub loop_start: usize,
    /// Loop end index (inclusive)
    pub loop_end: usize,
    /// Whether the sample loops
    pub looping: bool,
    /// Target playback frequency
    pub pitch_hz: f32,
    /// Amplification ratio
    pub amplification: f32,
    /// Volume (0-15)
    pub volume: u8,
    /// PSG sample player frequency in Hz (hardware playback rate)
    pub sample_player_frequency_hz: f32,
    /// Reference frequency in Hz (tuning reference, typically 440 Hz)
    pub reference_frequency_hz: f32,
    /// Start from beginning
    pub play_from_start: bool,
    /// Whether this playback is high priority (digidrum)
    pub high_priority: bool,
}

/// Sample command emitted by a channel for this tick
#[derive(Debug, Clone, Default)]
pub enum SampleCommand {
    /// No change
    #[default]
    None,
    /// Play or update a sample voice
    Play(SamplePlaybackParams),
    /// Stop any currently playing sample
    Stop,
}

/// Complete channel frame result (PSG + optional sample)
#[derive(Debug, Clone)]
pub struct ChannelFrame {
    /// PSG register set generated for this tick
    pub psg: ChannelOutput,
    /// Sample command emitted alongside the PSG changes
    pub sample: SampleCommand,
}

impl Default for ChannelFrame {
    fn default() -> Self {
        Self {
            psg: ChannelOutput::default(),
            sample: SampleCommand::None,
        }
    }
}
