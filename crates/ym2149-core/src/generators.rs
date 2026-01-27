//! Sound generators for the YM2149 PSG
//!
//! This module contains the individual generator components:
//! - Tone generators (3 channels)
//! - Noise generator (shared LFSR)
//! - Envelope generator

use crate::tables::{ENV_DATA, SHAPE_TO_ENV};

/// Number of tone channels
pub const NUM_CHANNELS: usize = 3;

/// Tone generator for a single channel
///
/// Each channel has a 12-bit period counter that toggles output when it reaches zero.
#[derive(Clone, Debug, Default)]
pub struct ToneGenerator {
    /// Current counter value
    counter: u32,
    /// Period from registers (12-bit, R0/R1, R2/R3, R4/R5)
    period: u32,
    /// Edge state packed into bits (5 bits per channel)
    edge_bits: u32,
    /// Pending edge reset (for sync-buzzer effects)
    pending_reset: bool,
}

impl ToneGenerator {
    /// Create a new tone generator
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the period from register values
    #[inline]
    pub fn set_period(&mut self, period: u32) {
        self.period = period;
    }

    /// Get current period
    #[inline]
    #[allow(dead_code)]
    pub fn period(&self) -> u32 {
        self.period
    }

    /// Check if this channel should output at half amplitude (period <= 1)
    #[inline]
    pub fn is_half_amplitude(&self) -> bool {
        self.period <= 1
    }

    /// Mark edge for pending reset (used by timer IRQ effects)
    #[inline]
    pub fn mark_pending_reset(&mut self) {
        self.pending_reset = true;
    }

    /// Apply pending reset if any
    #[inline]
    pub fn apply_pending_reset(&mut self, channel_shift: u32) {
        if self.pending_reset {
            self.edge_bits ^= 0x1f << channel_shift;
            self.counter = 0;
            self.pending_reset = false;
        }
    }

    /// Tick the generator, returns the edge mask for this channel
    #[inline]
    pub fn tick(&mut self, channel_shift: u32) -> u32 {
        self.counter += 1;
        if self.counter >= self.period {
            self.edge_bits ^= 0x1f << channel_shift;
            self.counter = 0;
        }
        self.edge_bits
    }

    /// Get the current edge bits
    #[inline]
    #[allow(dead_code)]
    pub fn edge_bits(&self) -> u32 {
        self.edge_bits
    }

    /// Set edge bits (used for power-on randomization)
    #[inline]
    pub fn set_edge_bits(&mut self, bits: u32) {
        self.edge_bits = bits;
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.counter = 0;
        self.period = 0;
        self.pending_reset = false;
        // Note: edge_bits preserved for randomization
    }
}

/// Noise generator using 17-bit LFSR
///
/// The noise generator runs at half the tone generator rate and produces
/// a pseudo-random sequence using XOR feedback.
#[derive(Clone, Debug)]
pub struct NoiseGenerator {
    /// Current counter value
    counter: u32,
    /// Period from register R6 (5-bit)
    period: u32,
    /// 17-bit LFSR state
    lfsr: u32,
    /// Current output mask (all 1s or all 0s)
    output_mask: u32,
    /// Half-rate toggle
    half_tick: bool,
}

impl NoiseGenerator {
    /// Create a new noise generator
    pub fn new() -> Self {
        Self {
            counter: 0,
            period: 0,
            lfsr: 1, // Must be non-zero
            output_mask: 0,
            half_tick: false,
        }
    }

    /// Set the period from register R6
    #[inline]
    pub fn set_period(&mut self, period: u32) {
        self.period = period;
    }

    /// Tick the generator (runs at half rate)
    ///
    /// Uses a 17-bit Galois LFSR with taps at bits 13 and 16,
    /// matching real YM2149/AY-3-8910 hardware.
    #[inline]
    pub fn tick(&mut self) -> u32 {
        self.half_tick = !self.half_tick;

        if self.half_tick {
            self.counter += 1;
            // Period 0 is treated as period 1 on real hardware
            let effective_period = self.period.max(1);
            if self.counter >= effective_period {
                // Galois LFSR: shift right, XOR taps at bits 13 and 16 when LSB is 1
                let lsb = self.lfsr & 1;
                self.lfsr >>= 1;
                if lsb != 0 {
                    self.lfsr ^= 0x12000; // Taps at bits 13 (0x2000) and 16 (0x10000)
                }
                self.output_mask = if lsb != 0 { !0 } else { 0 };
                self.counter = 0;
            }
        }

        self.output_mask
    }

    /// Get current output mask
    #[inline]
    #[allow(dead_code)]
    pub fn output_mask(&self) -> u32 {
        self.output_mask
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.counter = 0;
        self.lfsr = 1;
        self.output_mask = 0;
        self.half_tick = false;
    }
}

impl Default for NoiseGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Envelope generator with 16 hardware shapes
///
/// The envelope provides amplitude modulation using one of 10 unique waveforms
/// (16 register values map to 10 patterns via SHAPE_TO_ENV).
#[derive(Clone, Debug, Default)]
pub struct EnvelopeGenerator {
    /// Current counter value
    counter: u32,
    /// Period from registers R11/R12 (16-bit)
    period: u32,
    /// Current position in envelope (-64 to 63, starts at -64 on trigger)
    position: i32,
    /// Offset into ENV_DATA for current shape
    data_offset: usize,
}

impl EnvelopeGenerator {
    /// Create a new envelope generator
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the period from registers R11/R12
    #[inline]
    pub fn set_period(&mut self, period: u32) {
        self.period = period;
    }

    /// Set the envelope shape from register R13
    ///
    /// This triggers an envelope restart.
    ///
    /// # Envelope Data Layout
    ///
    /// The `ENV_DATA` table contains 10 unique envelope shapes, each with 128 entries
    /// (32 steps × 4 phases). Shape register values 0-15 map to these 10 shapes via
    /// `SHAPE_TO_ENV`. Access pattern: `ENV_DATA[data_offset + (position + 64)]`
    ///
    /// - `data_offset`: 0..=1152 (shape 0-9 × 128)
    /// - `position + 64`: 0..=127 (position range -64..=63)
    /// - Max index: 1152 + 127 = 1279 < 1280 (array size)
    #[inline]
    pub fn set_shape(&mut self, shape: u8) {
        let shape_index = (shape & 0x0f) as usize;
        self.data_offset = SHAPE_TO_ENV[shape_index] as usize * 32 * 4;
        debug_assert!(self.data_offset <= 9 * 32 * 4, "data_offset out of range");
        self.position = -64;
        self.counter = 0;
    }

    /// Trigger envelope restart without changing shape
    #[inline]
    pub fn trigger(&mut self) {
        self.position = -64;
        self.counter = 0;
    }

    /// Tick the generator
    #[inline]
    pub fn tick(&mut self) {
        self.counter += 1;
        if self.counter >= self.period {
            self.position += 1;
            if self.position > 0 {
                self.position &= 63;
            }
            self.counter = 0;
        }
    }

    /// Get the current envelope level (0-31)
    #[inline]
    pub fn level(&self) -> u32 {
        let index = self.data_offset + (self.position + 64) as usize;
        debug_assert!(index < ENV_DATA.len(), "envelope index {index} out of bounds");
        // SAFETY: index is bounded by data_offset (0..=1152) + position+64 (0..=127) = 0..=1279 < 1280
        ENV_DATA[index] as u32
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.counter = 0;
        self.position = 0;
        self.data_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone_generator_period() {
        let mut tone = ToneGenerator::new();
        tone.set_period(100);
        assert_eq!(tone.period(), 100);
        assert!(!tone.is_half_amplitude());

        tone.set_period(1);
        assert!(tone.is_half_amplitude());
    }

    #[test]
    fn test_noise_generator_lfsr() {
        let mut noise = NoiseGenerator::new();

        // LFSR should produce varying output
        let mut outputs = Vec::new();
        for _ in 0..100 {
            noise.tick();
            outputs.push(noise.output_mask());
        }

        // Should have some variation (not all same)
        let has_variation = outputs.windows(2).any(|w| w[0] != w[1]);
        assert!(
            has_variation,
            "Noise generator should produce varying output"
        );
    }

    #[test]
    fn test_envelope_trigger() {
        let mut envelope = EnvelopeGenerator::new();
        envelope.set_period(10);
        envelope.set_shape(0);

        // Position should start at -64
        assert_eq!(envelope.position, -64);

        // Advance and trigger again
        for _ in 0..50 {
            envelope.tick();
        }
        envelope.trigger();
        assert_eq!(envelope.position, -64);
    }
}
