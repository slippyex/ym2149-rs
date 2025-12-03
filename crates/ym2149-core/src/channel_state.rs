//! Channel state extraction from YM2149 registers.
//!
//! This module provides a unified way to extract visualization-ready data
//! from YM2149 register dumps. This works for all formats (YM, AKS, AY, SNDH)
//! since they all ultimately write to the same YM2149 registers.
//!
//! # Example
//!
//! ```
//! use ym2149::{Ym2149, Ym2149Backend};
//! use ym2149::channel_state::ChannelStates;
//!
//! let chip = Ym2149::new();
//! let states = ChannelStates::from_registers(&chip.dump_registers());
//!
//! for (i, ch) in states.channels.iter().enumerate() {
//!     println!("Channel {}: {:?}Hz, amp={}", i, ch.frequency_hz, ch.amplitude);
//! }
//! ```

/// Standard Atari ST master clock for frequency calculations.
const ATARI_ST_CLOCK: f32 = 2_000_000.0;

/// State of a single YM2149 channel extracted from registers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelState {
    /// Tone period from registers (12-bit, 0-4095).
    pub tone_period: u16,
    /// Calculated frequency in Hz (None if period is 0).
    pub frequency_hz: Option<f32>,
    /// Musical note name (e.g., "A4", "C#5").
    pub note_name: Option<&'static str>,
    /// MIDI note number (21-108 for piano range, None if out of range).
    pub midi_note: Option<u8>,
    /// Raw amplitude value (0-15).
    pub amplitude: u8,
    /// Normalized amplitude (0.0-1.0) for visualization.
    pub amplitude_normalized: f32,
    /// Whether tone output is enabled for this channel.
    pub tone_enabled: bool,
    /// Whether noise output is enabled for this channel.
    pub noise_enabled: bool,
    /// Whether envelope mode is enabled (bit 4 of amplitude register).
    pub envelope_enabled: bool,
}

/// Envelope generator state.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvelopeState {
    /// Envelope period from registers (16-bit).
    pub period: u16,
    /// Envelope shape (0-15).
    pub shape: u8,
    /// Human-readable shape description.
    pub shape_name: &'static str,
    /// Whether envelope is in "sustain" mode (shapes 8-15).
    pub is_sustaining: bool,
    /// Envelope frequency in Hz (None if period is 0).
    pub frequency_hz: Option<f32>,
}

/// Noise generator state.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoiseState {
    /// Noise period (5-bit, 0-31).
    pub period: u8,
    /// Whether noise is enabled on any channel.
    pub any_channel_enabled: bool,
}

/// Complete state of all YM2149 channels and generators.
#[derive(Debug, Clone, Default)]
pub struct ChannelStates {
    /// State of channels A, B, C.
    pub channels: [ChannelState; 3],
    /// Envelope generator state.
    pub envelope: EnvelopeState,
    /// Noise generator state.
    pub noise: NoiseState,
    /// Raw mixer register value (for debugging).
    pub mixer_raw: u8,
}

impl ChannelStates {
    /// Extract channel states from a YM2149 register dump.
    ///
    /// This is the main entry point for visualization. It works with any
    /// register dump regardless of the source format (YM, AKS, AY, SNDH).
    ///
    /// # Arguments
    ///
    /// * `regs` - 16-byte register dump from `Ym2149Backend::dump_registers()`
    ///
    /// # Returns
    ///
    /// Complete state of all channels and generators.
    pub fn from_registers(regs: &[u8; 16]) -> Self {
        Self::from_registers_with_clock(regs, ATARI_ST_CLOCK)
    }

    /// Extract channel states with a custom master clock frequency.
    ///
    /// Use this when emulating non-Atari ST systems with different clock rates.
    ///
    /// # Arguments
    ///
    /// * `regs` - 16-byte register dump
    /// * `master_clock` - Master clock frequency in Hz
    pub fn from_registers_with_clock(regs: &[u8; 16], master_clock: f32) -> Self {
        let mixer = regs[7];

        // Extract channel states
        let channels = [
            Self::extract_channel(regs, 0, mixer, master_clock),
            Self::extract_channel(regs, 1, mixer, master_clock),
            Self::extract_channel(regs, 2, mixer, master_clock),
        ];

        // Extract envelope state
        let env_period = (regs[11] as u16) | ((regs[12] as u16) << 8);
        let env_shape = regs[13] & 0x0F;
        let envelope = EnvelopeState {
            period: env_period,
            shape: env_shape,
            shape_name: envelope_shape_name(env_shape),
            is_sustaining: env_shape >= 8,
            frequency_hz: if env_period > 0 {
                // Envelope frequency = master_clock / (256 * period)
                Some(master_clock / (256.0 * env_period as f32))
            } else {
                None
            },
        };

        // Extract noise state
        let noise_period = regs[6] & 0x1F;
        let noise = NoiseState {
            period: noise_period,
            any_channel_enabled: (mixer & 0x38) != 0x38, // Bits 3-5 inverted
        };

        ChannelStates {
            channels,
            envelope,
            noise,
            mixer_raw: mixer,
        }
    }

    fn extract_channel(
        regs: &[u8; 16],
        channel: usize,
        mixer: u8,
        master_clock: f32,
    ) -> ChannelState {
        // Register offsets per channel
        let period_lo_reg = channel * 2;
        let period_hi_reg = channel * 2 + 1;
        let amp_reg = 8 + channel;

        // Extract period (12-bit)
        let period_lo = regs[period_lo_reg] as u16;
        let period_hi = (regs[period_hi_reg] & 0x0F) as u16;
        let tone_period = period_lo | (period_hi << 8);

        // Extract amplitude
        let amp_raw = regs[amp_reg];
        let amplitude = amp_raw & 0x0F;
        let envelope_enabled = (amp_raw & 0x10) != 0;

        // Mixer bits (active low)
        let tone_bit = 1 << channel;
        let noise_bit = 8 << channel;
        let tone_enabled = (mixer & tone_bit) == 0;
        let noise_enabled = (mixer & noise_bit) == 0;

        // Calculate frequency
        let frequency_hz = if tone_period > 0 {
            // Frequency = master_clock / (16 * period)
            Some(master_clock / (16.0 * tone_period as f32))
        } else {
            None
        };

        // Convert to musical note
        let (note_name, midi_note) = frequency_hz.map(frequency_to_note).unwrap_or((None, None));

        ChannelState {
            tone_period,
            frequency_hz,
            note_name,
            midi_note,
            amplitude,
            amplitude_normalized: amplitude as f32 / 15.0,
            tone_enabled,
            noise_enabled,
            envelope_enabled,
        }
    }

    /// Get the maximum amplitude across all channels (for VU meter).
    pub fn max_amplitude(&self) -> f32 {
        self.channels
            .iter()
            .map(|ch| ch.amplitude_normalized)
            .fold(0.0, f32::max)
    }

    /// Check if any channel has envelope mode enabled.
    pub fn any_envelope_enabled(&self) -> bool {
        self.channels.iter().any(|ch| ch.envelope_enabled)
    }

    /// Get channels that are actively producing sound.
    ///
    /// A channel is "active" if it has amplitude > 0 and either tone or noise enabled.
    pub fn active_channels(&self) -> impl Iterator<Item = (usize, &ChannelState)> {
        self.channels.iter().enumerate().filter(|(_, ch)| {
            ch.amplitude > 0 && (ch.tone_enabled || ch.noise_enabled || ch.envelope_enabled)
        })
    }
}

/// Get human-readable name for envelope shape.
fn envelope_shape_name(shape: u8) -> &'static str {
    match shape & 0x0F {
        0x00..=0x03 => "\\___", // Decay
        0x04..=0x07 => "/___",  // Attack
        0x08 => "\\\\\\\\",     // Sawtooth down
        0x09 => "\\___",        // Decay (one-shot)
        0x0A => "\\/\\/",       // Triangle
        0x0B => "\\¯¯¯",        // Decay + hold high
        0x0C => "////",         // Sawtooth up
        0x0D => "/¯¯¯",         // Attack + hold high
        0x0E => "/\\/\\",       // Triangle (inverted)
        0x0F => "/___",         // Attack (one-shot)
        _ => "????",
    }
}

/// Convert frequency to musical note.
///
/// Returns (note_name, midi_note) or (None, None) if out of range.
fn frequency_to_note(freq: f32) -> (Option<&'static str>, Option<u8>) {
    if !(20.0..=20000.0).contains(&freq) {
        return (None, None);
    }

    // MIDI note number: 69 = A4 = 440Hz
    // n = 12 * log2(f / 440) + 69
    let midi_float = 12.0 * (freq / 440.0).log2() + 69.0;
    let midi = midi_float.round() as i32;

    if !(0..=127).contains(&midi) {
        return (None, None);
    }

    let midi_u8 = midi as u8;

    // Note names
    static NOTE_NAMES: [&str; 128] = [
        "C-1", "C#-1", "D-1", "D#-1", "E-1", "F-1", "F#-1", "G-1", "G#-1", "A-1", "A#-1", "B-1",
        "C0", "C#0", "D0", "D#0", "E0", "F0", "F#0", "G0", "G#0", "A0", "A#0", "B0", "C1", "C#1",
        "D1", "D#1", "E1", "F1", "F#1", "G1", "G#1", "A1", "A#1", "B1", "C2", "C#2", "D2", "D#2",
        "E2", "F2", "F#2", "G2", "G#2", "A2", "A#2", "B2", "C3", "C#3", "D3", "D#3", "E3", "F3",
        "F#3", "G3", "G#3", "A3", "A#3", "B3", "C4", "C#4", "D4", "D#4", "E4", "F4", "F#4", "G4",
        "G#4", "A4", "A#4", "B4", "C5", "C#5", "D5", "D#5", "E5", "F5", "F#5", "G5", "G#5", "A5",
        "A#5", "B5", "C6", "C#6", "D6", "D#6", "E6", "F6", "F#6", "G6", "G#6", "A6", "A#6", "B6",
        "C7", "C#7", "D7", "D#7", "E7", "F7", "F#7", "G7", "G#7", "A7", "A#7", "B7", "C8", "C#8",
        "D8", "D#8", "E8", "F8", "F#8", "G8", "G#8", "A8", "A#8", "B8", "C9", "C#9", "D9", "D#9",
        "E9", "F9", "F#9", "G9",
    ];

    let note_name = NOTE_NAMES.get(midi_u8 as usize).copied();
    (note_name, Some(midi_u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_channel_a() {
        // Set up registers for channel A: period=284 (A4), amplitude=15
        let mut regs = [0u8; 16];
        regs[0] = 0x1C; // Period low (284 & 0xFF = 28 = 0x1C)
        regs[1] = 0x01; // Period high (284 >> 8 = 1)
        regs[7] = 0x3E; // Mixer: tone A on (bit 0 = 0)
        regs[8] = 0x0F; // Volume A = 15

        let states = ChannelStates::from_registers(&regs);

        assert_eq!(states.channels[0].tone_period, 284);
        assert_eq!(states.channels[0].amplitude, 15);
        assert!(states.channels[0].tone_enabled);
        assert!(!states.channels[0].noise_enabled);

        // Frequency should be ~440Hz (A4)
        let freq = states.channels[0].frequency_hz.unwrap();
        assert!((freq - 440.0).abs() < 5.0, "Expected ~440Hz, got {}", freq);
    }

    #[test]
    fn test_envelope_mode() {
        let mut regs = [0u8; 16];
        regs[8] = 0x1F; // Volume A = envelope mode (bit 4 set)
        regs[11] = 0x00; // Envelope period low
        regs[12] = 0x10; // Envelope period high (4096)
        regs[13] = 0x0E; // Envelope shape = triangle

        let states = ChannelStates::from_registers(&regs);

        assert!(states.channels[0].envelope_enabled);
        assert_eq!(states.envelope.period, 4096);
        assert_eq!(states.envelope.shape, 0x0E);
        assert!(states.envelope.is_sustaining);
    }

    #[test]
    fn test_frequency_to_note_a4() {
        let (name, midi) = frequency_to_note(440.0);
        assert_eq!(name, Some("A4"));
        assert_eq!(midi, Some(69));
    }

    #[test]
    fn test_frequency_to_note_c4() {
        let (name, midi) = frequency_to_note(261.63);
        assert_eq!(name, Some("C4"));
        assert_eq!(midi, Some(60));
    }
}
