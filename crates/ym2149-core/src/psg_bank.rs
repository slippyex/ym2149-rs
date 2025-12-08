//! Multi-PSG Bank for Arkos Tracker and PlayCity support
//!
//! Manages multiple YM2149/AY-3-8912 chips for expanded polyphony.
//! Used by Arkos Tracker 3 which supports n-PSGs with n×3 channels.
//!
//! # Examples
//!
//! ```
//! use ym2149::{PsgBank, Ym2149};
//!
//! // Create a 2-PSG bank (6 channels total)
//! let mut bank = PsgBank::new(2, 2_000_000);
//!
//! // Write to PSG 0, Channel A
//! bank.write_register(0, 0x00, 0x1C); // Period low
//! bank.write_register(0, 0x08, 0x0F); // Volume
//!
//! // Write to PSG 1, Channel A
//! bank.write_register(1, 0x00, 0x2A); // Different period
//! bank.write_register(1, 0x08, 0x0F); // Volume
//!
//! // Generate interleaved samples
//! let mut buffer = vec![0.0f32; 882];
//! bank.generate_samples_interleaved(&mut buffer);
//! ```

use crate::chip::Ym2149;
use ym2149_common::Ym2149Backend;

const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// A bank of multiple PSG chips for expanded polyphony.
///
/// Each PSG provides 3 channels (A, B, C), so a bank with N PSGs
/// provides N×3 channels total. This is used by Arkos Tracker 3
/// and systems like PlayCity (Amstrad CPC expansion).
///
/// # Channel Mapping
///
/// Channels are numbered sequentially across PSGs:
/// - PSG 0: Channels 0 (A), 1 (B), 2 (C)
/// - PSG 1: Channels 3 (A), 4 (B), 5 (C)
/// - PSG 2: Channels 6 (A), 7 (B), 8 (C)
/// - etc.
#[derive(Debug)]
pub struct PsgBank {
    /// The individual PSG chips
    chips: Vec<Ym2149>,
    /// Clock frequency for each PSG (in Hz)
    frequencies: Vec<u32>,
    /// Scratch buffer reused between calls to avoid per-call allocations
    scratch: Vec<f32>,
}

impl PsgBank {
    /// Creates a new PSG bank with all chips at the same frequency.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of PSG chips (must be > 0)
    /// * `frequency` - Clock frequency in Hz (e.g., 2_000_000 for Atari ST)
    ///
    /// # Panics
    ///
    /// Panics if `count` is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use ym2149::PsgBank;
    ///
    /// // PlayCity setup: 2 PSGs at 2MHz
    /// let bank = PsgBank::new(2, 2_000_000);
    /// assert_eq!(bank.psg_count(), 2);
    /// assert_eq!(bank.channel_count(), 6);
    /// ```
    pub fn new(count: usize, frequency: u32) -> Self {
        assert!(count > 0, "PSG bank must have at least one chip");

        let chips = (0..count)
            .map(|_| Ym2149::with_clocks(frequency, DEFAULT_SAMPLE_RATE))
            .collect();
        let frequencies = vec![frequency; count];

        Self {
            chips,
            frequencies,
            scratch: Vec::new(),
        }
    }

    /// Creates a new PSG bank where each chip can have a different frequency.
    ///
    /// This is useful for mixed configurations like CPC + PlayCity where
    /// PSG 0 runs at 1MHz and PSGs 1-2 run at 2MHz.
    ///
    /// # Arguments
    ///
    /// * `frequencies` - Clock frequencies in Hz for each PSG
    ///
    /// # Panics
    ///
    /// Panics if `frequencies` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use ym2149::PsgBank;
    ///
    /// // CPC with PlayCity: mixed frequencies
    /// let bank = PsgBank::new_with_frequencies(vec![
    ///     1_000_000, // CPC internal PSG
    ///     2_000_000, // PlayCity PSG 1
    ///     2_000_000, // PlayCity PSG 2
    /// ]);
    /// assert_eq!(bank.psg_count(), 3);
    /// assert_eq!(bank.channel_count(), 9);
    /// ```
    pub fn new_with_frequencies(frequencies: Vec<u32>) -> Self {
        assert!(
            !frequencies.is_empty(),
            "PSG bank must have at least one chip"
        );

        let chips = frequencies
            .iter()
            .map(|&freq| Ym2149::with_clocks(freq, DEFAULT_SAMPLE_RATE))
            .collect();

        Self {
            chips,
            frequencies,
            scratch: Vec::new(),
        }
    }

    /// Returns the number of PSG chips in this bank.
    #[inline]
    pub fn psg_count(&self) -> usize {
        self.chips.len()
    }

    /// Returns the total number of channels (PSG count × 3).
    #[inline]
    pub fn channel_count(&self) -> usize {
        self.psg_count() * 3
    }

    /// Gets the clock frequency for a specific PSG.
    ///
    /// # Arguments
    ///
    /// * `psg_index` - Index of the PSG (0..psg_count)
    ///
    /// # Panics
    ///
    /// Panics if `psg_index` is out of bounds.
    #[inline]
    pub fn get_frequency(&self, psg_index: usize) -> u32 {
        self.frequencies[psg_index]
    }

    /// Gets a reference to a specific PSG chip.
    ///
    /// # Arguments
    ///
    /// * `psg_index` - Index of the PSG (0..psg_count)
    ///
    /// # Panics
    ///
    /// Panics if `psg_index` is out of bounds.
    #[inline]
    pub fn get_chip(&self, psg_index: usize) -> &Ym2149 {
        &self.chips[psg_index]
    }

    /// Gets a mutable reference to a specific PSG chip.
    ///
    /// # Arguments
    ///
    /// * `psg_index` - Index of the PSG (0..psg_count)
    ///
    /// # Panics
    ///
    /// Panics if `psg_index` is out of bounds.
    #[inline]
    pub fn get_chip_mut(&mut self, psg_index: usize) -> &mut Ym2149 {
        &mut self.chips[psg_index]
    }

    /// Writes a value to a register on a specific PSG.
    ///
    /// # Arguments
    ///
    /// * `psg_index` - Index of the PSG (0..psg_count)
    /// * `register` - Register number (0-15)
    /// * `value` - Value to write (0-255)
    ///
    /// # Panics
    ///
    /// Panics if `psg_index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use ym2149::PsgBank;
    ///
    /// let mut bank = PsgBank::new(2, 2_000_000);
    ///
    /// // Set volume on PSG 0, Channel A
    /// bank.write_register(0, 0x08, 0x0F);
    ///
    /// // Set volume on PSG 1, Channel A
    /// bank.write_register(1, 0x08, 0x0F);
    /// ```
    #[inline]
    pub fn write_register(&mut self, psg_index: usize, register: u8, value: u8) {
        self.chips[psg_index].write_register(register, value);
    }

    /// Reads a register value from a specific PSG.
    ///
    /// # Arguments
    ///
    /// * `psg_index` - Index of the PSG (0..psg_count)
    /// * `register` - Register number (0-15)
    ///
    /// # Panics
    ///
    /// Panics if `psg_index` is out of bounds.
    #[inline]
    pub fn read_register(&self, psg_index: usize, register: u8) -> u8 {
        self.chips[psg_index].read_register(register)
    }

    /// Generates audio samples with all PSG outputs mixed together (interleaved).
    ///
    /// This is the most common use case - all PSGs mixed to a single mono output.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Output buffer to fill with samples
    ///
    /// # Examples
    ///
    /// ```
    /// use ym2149::PsgBank;
    ///
    /// let mut bank = PsgBank::new(2, 2_000_000);
    /// let mut buffer = vec![0.0f32; 882]; // 50Hz frame at 44.1kHz
    ///
    /// bank.generate_samples_interleaved(&mut buffer);
    /// ```
    pub fn generate_samples_interleaved(&mut self, buffer: &mut [f32]) {
        buffer.fill(0.0);
        if self.scratch.len() < buffer.len() {
            self.scratch.resize(buffer.len(), 0.0);
        }
        let scratch = &mut self.scratch[..buffer.len()];

        // Mix all PSGs into the buffer
        for chip in &mut self.chips {
            chip.generate_samples_into(scratch);
            for (out, sample) in buffer.iter_mut().zip(scratch.iter()) {
                *out += *sample;
            }
        }

        // Normalize by PSG count to prevent clipping
        let scale = 1.0 / self.psg_count() as f32;
        for sample in buffer.iter_mut() {
            *sample *= scale;
        }
    }

    /// Generates audio samples with each PSG output in a separate buffer.
    ///
    /// This is useful when you want to apply different effects or mixing to each PSG,
    /// or when rendering to separate audio tracks.
    ///
    /// # Arguments
    ///
    /// * `buffers` - Slice of output buffers, one per PSG
    ///
    /// # Panics
    ///
    /// Panics if the number of buffers doesn't match the PSG count.
    ///
    /// # Examples
    ///
    /// ```
    /// use ym2149::PsgBank;
    ///
    /// let mut bank = PsgBank::new(2, 2_000_000);
    /// let mut buffer0 = vec![0.0f32; 882];
    /// let mut buffer1 = vec![0.0f32; 882];
    /// let mut buffers = vec![&mut buffer0[..], &mut buffer1[..]];
    ///
    /// bank.generate_samples_separate(&mut buffers);
    /// // Now buffer0 contains PSG 0 output, buffer1 contains PSG 1 output
    /// ```
    pub fn generate_samples_separate(&mut self, buffers: &mut [&mut [f32]]) {
        assert_eq!(
            buffers.len(),
            self.psg_count(),
            "Buffer count must match PSG count"
        );

        for (chip, buffer) in self.chips.iter_mut().zip(buffers.iter_mut()) {
            chip.generate_samples_into(buffer);
        }
    }

    /// Resets all PSG chips to their initial state.
    pub fn reset(&mut self) {
        for chip in &mut self.chips {
            chip.reset();
        }
    }

    /// Dumps the register state of all PSGs for debugging.
    ///
    /// Returns a vector of register dumps, one per PSG.
    pub fn dump_all_registers(&self) -> Vec<[u8; 16]> {
        self.chips
            .iter()
            .map(|chip| chip.dump_registers())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psg_bank_creation() {
        let bank = PsgBank::new(2, 2_000_000);
        assert_eq!(bank.psg_count(), 2);
        assert_eq!(bank.channel_count(), 6);
        assert_eq!(bank.get_frequency(0), 2_000_000);
        assert_eq!(bank.get_frequency(1), 2_000_000);
    }

    #[test]
    fn test_psg_bank_mixed_frequencies() {
        let bank = PsgBank::new_with_frequencies(vec![1_000_000, 2_000_000, 2_000_000]);
        assert_eq!(bank.psg_count(), 3);
        assert_eq!(bank.channel_count(), 9);
        assert_eq!(bank.get_frequency(0), 1_000_000);
        assert_eq!(bank.get_frequency(1), 2_000_000);
        assert_eq!(bank.get_frequency(2), 2_000_000);
    }

    #[test]
    fn test_write_read_register() {
        let mut bank = PsgBank::new(2, 2_000_000);

        // Write to PSG 0
        bank.write_register(0, 0x08, 0x0F);
        assert_eq!(bank.read_register(0, 0x08), 0x0F);

        // Write to PSG 1
        bank.write_register(1, 0x08, 0x0A);
        assert_eq!(bank.read_register(1, 0x08), 0x0A);

        // Verify they're independent
        assert_eq!(bank.read_register(0, 0x08), 0x0F);
    }

    #[test]
    fn test_generate_samples_interleaved() {
        let mut bank = PsgBank::new(2, 2_000_000);

        // Setup simple tone on both PSGs
        for i in 0..2 {
            bank.write_register(i, 0x07, 0x3E); // Enable tone A
            bank.write_register(i, 0x00, 0x1C); // Period low
            bank.write_register(i, 0x01, 0x01); // Period high
            bank.write_register(i, 0x08, 0x0F); // Max volume
        }

        let mut buffer = vec![0.0f32; 882];
        bank.generate_samples_interleaved(&mut buffer);

        // Should have generated some non-zero samples
        let has_signal = buffer.iter().any(|&s| s.abs() > 0.01);
        assert!(has_signal, "Expected non-zero samples");
    }

    #[test]
    fn test_generate_samples_separate() {
        let mut bank = PsgBank::new(2, 2_000_000);

        // Setup different tones on each PSG
        bank.write_register(0, 0x07, 0x3E);
        bank.write_register(0, 0x00, 0x1C);
        bank.write_register(0, 0x08, 0x0F);

        bank.write_register(1, 0x07, 0x3E);
        bank.write_register(1, 0x00, 0x2A); // Different period
        bank.write_register(1, 0x08, 0x0F);

        let mut buffer0 = vec![0.0f32; 882];
        let mut buffer1 = vec![0.0f32; 882];
        let mut buffers = vec![&mut buffer0[..], &mut buffer1[..]];

        bank.generate_samples_separate(&mut buffers);

        // Both should have signal
        assert!(buffer0.iter().any(|&s| s.abs() > 0.01));
        assert!(buffer1.iter().any(|&s| s.abs() > 0.01));
    }

    #[test]
    fn test_reset() {
        let mut bank = PsgBank::new(2, 2_000_000);

        bank.write_register(0, 0x08, 0x0F);
        bank.write_register(1, 0x08, 0x0F);

        bank.reset();

        // After reset, volumes should be 0
        assert_eq!(bank.read_register(0, 0x08), 0x00);
        assert_eq!(bank.read_register(1, 0x08), 0x00);
    }

    #[test]
    #[should_panic(expected = "PSG bank must have at least one chip")]
    fn test_empty_bank_panics() {
        PsgBank::new(0, 2_000_000);
    }

    #[test]
    #[should_panic(expected = "PSG bank must have at least one chip")]
    fn test_empty_frequencies_panics() {
        PsgBank::new_with_frequencies(vec![]);
    }
}
