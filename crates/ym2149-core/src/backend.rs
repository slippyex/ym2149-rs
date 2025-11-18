//! Backend trait abstraction for YM2149 chip implementations
//!
//! This module defines the core interface that all YM2149 backends must implement,
//! whether they are cycle-accurate hardware emulations or experimental synthesizers.

/// Common interface for YM2149 chip backends
///
/// This trait allows different implementations to be used interchangeably:
/// - Hardware-accurate emulation (cycle-exact, bit-perfect)
/// - Experimental software synthesizers (musical, non-accurate)
/// - Future implementations (FPGA cores, etc.)
///
/// # Example
///
/// ```
/// use ym2149::{Ym2149Backend, Ym2149};
///
/// fn play_note<B: Ym2149Backend>(chip: &mut B) {
///     chip.write_register(0x00, 0xF0); // Channel A period low
///     chip.write_register(0x01, 0x01); // Channel A period high
///     chip.write_register(0x08, 0x0F); // Channel A volume
///     chip.write_register(0x07, 0x3E); // Mixer: enable tone A
///
///     chip.clock();
///     let sample = chip.get_sample();
/// }
/// ```
pub trait Ym2149Backend: Send {
    /// Create a new backend instance with default clocks
    ///
    /// Default clocks:
    /// - Master clock: 2,000,000 Hz (Atari ST frequency)
    /// - Sample rate: 44,100 Hz
    fn new() -> Self
    where
        Self: Sized;

    /// Create a backend with custom master clock and sample rate
    ///
    /// # Arguments
    ///
    /// * `master_clock` - YM2149 master clock frequency in Hz
    /// * `sample_rate` - Audio output sample rate in Hz
    fn with_clocks(master_clock: u32, sample_rate: u32) -> Self
    where
        Self: Sized;

    /// Reset the backend to initial state
    ///
    /// Clears all registers, resets generators, and stops all audio output.
    fn reset(&mut self);

    /// Write to a YM2149 register
    ///
    /// # Arguments
    ///
    /// * `addr` - Register address (0x00-0x0F)
    /// * `value` - Register value (0x00-0xFF)
    ///
    /// Registers outside the valid range are ignored.
    fn write_register(&mut self, addr: u8, value: u8);

    /// Read from a YM2149 register
    ///
    /// # Arguments
    ///
    /// * `addr` - Register address (0x00-0x0F)
    ///
    /// # Returns
    ///
    /// Current register value, or 0x00 for invalid addresses
    fn read_register(&self, addr: u8) -> u8;

    /// Load all 16 YM2149 registers at once
    ///
    /// More efficient than 16 individual `write_register` calls.
    ///
    /// # Arguments
    ///
    /// * `regs` - Array of 16 register values (R0-R15)
    fn load_registers(&mut self, regs: &[u8; 16]);

    /// Dump all 16 YM2149 registers
    ///
    /// # Returns
    ///
    /// Current state of all registers (R0-R15)
    fn dump_registers(&self) -> [u8; 16];

    /// Advance the chip by one clock cycle
    ///
    /// Updates all internal generators (tone, noise, envelope) and produces
    /// a new audio sample. Call this at the backend's sample rate.
    fn clock(&mut self);

    /// Get the last generated audio sample
    ///
    /// # Returns
    ///
    /// Normalized audio sample in range [-1.0, 1.0]
    fn get_sample(&self) -> f32;

    /// Generate multiple audio samples
    ///
    /// # Arguments
    ///
    /// * `count` - Number of samples to generate
    ///
    /// # Returns
    ///
    /// Vector of normalized audio samples in range [-1.0, 1.0]
    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = vec![0.0; count];
        self.generate_samples_into(&mut samples);
        samples
    }

    /// Generate multiple audio samples into a caller-provided buffer
    ///
    /// This avoids per-call allocations; prefer this in hot paths.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Output slice to fill with normalized audio samples in range [-1.0, 1.0]
    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            self.clock();
            *sample = self.get_sample();
        }
    }

    /// Get individual channel outputs
    ///
    /// # Returns
    ///
    /// Tuple of (channel_a, channel_b, channel_c) samples in range [-1.0, 1.0]
    fn get_channel_outputs(&self) -> (f32, f32, f32);

    /// Mute or unmute a channel
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index (0=A, 1=B, 2=C)
    /// * `mute` - true to mute, false to unmute
    fn set_channel_mute(&mut self, channel: usize, mute: bool);

    /// Check if a channel is muted
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index (0=A, 1=B, 2=C)
    ///
    /// # Returns
    ///
    /// true if channel is muted, false otherwise
    fn is_channel_muted(&self, channel: usize) -> bool;

    /// Enable or disable post-processing color filter
    ///
    /// # Arguments
    ///
    /// * `enabled` - true to enable filter, false to disable
    fn set_color_filter(&mut self, enabled: bool);

    /// Trigger envelope restart (used by YM6 Sync Buzzer effect)
    ///
    /// This is a hardware-specific feature. Default implementation is a no-op.
    /// Only Ym2149 provides full implementation.
    fn trigger_envelope(&mut self) {
        // Default: no-op for backends that don't support this
    }

    /// Override drum sample for a channel (used by YM6 DigiDrum effect)
    ///
    /// This is a hardware-specific feature. Default implementation is a no-op.
    /// Only Ym2149 provides full implementation.
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index (0=A, 1=B, 2=C)
    /// * `sample` - Optional sample value to inject, None to disable override
    fn set_drum_sample_override(&mut self, _channel: usize, _sample: Option<f32>) {
        // Default: no-op for backends that don't support this
    }

    /// Set mixer tone/noise overrides (used by YM6 DigiDrum effect)
    ///
    /// This is a hardware-specific feature. Default implementation is a no-op.
    /// Only Ym2149 provides full implementation.
    ///
    /// # Arguments
    ///
    /// * `force_tone` - Per-channel flags to force tone enable
    /// * `force_noise_mute` - Per-channel flags to force noise mute
    fn set_mixer_overrides(&mut self, _force_tone: [bool; 3], _force_noise_mute: [bool; 3]) {
        // Default: no-op for backends that don't support this
    }
}
