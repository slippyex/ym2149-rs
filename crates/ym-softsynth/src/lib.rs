//! Experimental Software Synthesizer Backend for YM2149
//!
//! This crate provides a non-bit-accurate, musical software synthesizer that
//! implements the `Ym2149Backend` trait. It can be used as a drop-in replacement
//! for the cycle-accurate hardware emulation when you want a more "synthy" sound.
//!
//! # Features
//!
//! - PWM/saw oscillators with resonant low-pass filters
//! - Envelope-to-filter/PWM modulation
//! - Noise shaping for drum sounds
//! - Mild saturation for warmth
//! - Compatible with YM6 effects (SID, Sync Buzzer)
//!
//! # Example
//!
//! ```no_run
//! use ym2149::Ym2149Backend;
//! use ym_softsynth::SoftSynth;
//!
//! let mut synth = SoftSynth::new();
//! synth.write_register(0x00, 0xF0); // Channel A period
//! synth.write_register(0x08, 0x0F); // Channel A volume
//! synth.clock();
//! let sample = synth.get_sample();
//! ```

#![warn(missing_docs)]

pub use ym2149::Ym2149Backend;

// Re-export the implementation
mod softsynth_impl;
pub use softsynth_impl::SoftSynth;

// Note: SoftPlayer is not exported to avoid circular dependency with ym-replayer.
// SoftSynth (the backend) is the primary export. If a player is needed,
// use Ym6Player from ym-replayer with the SoftSynth backend (when implemented).

// Implement the backend trait
impl Ym2149Backend for SoftSynth {
    fn new() -> Self {
        SoftSynth::new()
    }

    fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        // SoftSynth doesn't use master_clock/sample_rate parameters
        // as it's hardcoded to 2MHz/44.1kHz, but we accept them for compatibility
        let _ = (master_clock, sample_rate);
        SoftSynth::new()
    }

    fn reset(&mut self) {
        *self = SoftSynth::new();
    }

    fn write_register(&mut self, addr: u8, value: u8) {
        self.write_register(addr, value);
    }

    fn read_register(&self, addr: u8) -> u8 {
        let idx = (addr as usize) & 0x0F;
        self.dump_registers()[idx]
    }

    fn load_registers(&mut self, regs: &[u8; 16]) {
        self.load_registers(regs);
    }

    fn dump_registers(&self) -> [u8; 16] {
        self.dump_registers()
    }

    fn clock(&mut self) {
        self.clock();
    }

    fn get_sample(&self) -> f32 {
        self.get_sample()
    }

    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            self.clock();
            samples.push(self.get_sample());
        }
        samples
    }

    fn get_channel_outputs(&self) -> (f32, f32, f32) {
        // SoftSynth doesn't separate channels in the same way
        // Return the mixed sample on all channels
        let sample = self.get_sample();
        (sample / 3.0, sample / 3.0, sample / 3.0)
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.is_channel_muted(channel)
    }

    fn set_color_filter(&mut self, enabled: bool) {
        self.set_color_filter(enabled);
    }
}
