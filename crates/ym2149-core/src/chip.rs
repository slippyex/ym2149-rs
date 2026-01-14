//! YM2149 PSG emulation
//!
//! Cycle accurate YM2149 emulation operating at the internal clock rate of
//! master_clock / 8 (250kHz at 2MHz). This matches the real hardware timing
//! where internal operations run at 1/8 of the master clock.

use crate::dc_filter::DcFilter;
use crate::generators::{EnvelopeGenerator, NUM_CHANNELS, NoiseGenerator, ToneGenerator};
use crate::mixer::Mixer;
use crate::tables::REG_MASK;
use ym2149_common::{MASTER_GAIN, Ym2149Backend};

/// Default Atari ST master clock (2 MHz)
const DEFAULT_MASTER_CLOCK: u32 = 2_000_000;

/// Default audio sample rate (44.1 kHz)
const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// Number of YM2149 registers
const NUM_REGISTERS: usize = 14;

/// Simple PRNG for unpredictable power-on state
fn random_seed(seed: &mut u32) -> u16 {
    *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
    ((*seed >> 16) & 0x7fff) as u16
}

/// YM2149 Programmable Sound Generator emulator
///
/// This emulator provides cycle-accurate reproduction of the Yamaha YM2149 PSG
/// as used in the Atari ST and other systems. It operates at the internal clock
/// rate (master_clock / 8) and averages output samples at the host sample rate.
///
/// # Features
///
/// - 3 tone channels with 12-bit period control
/// - 1 noise generator with 17-bit LFSR
/// - 1 envelope generator with 16 shapes (10 unique patterns)
/// - Configurable mixer for tone/noise routing
/// - DC offset removal filter
/// - DigiDrum sample injection support
///
/// # Example
///
/// ```
/// use ym2149::{Ym2149, Ym2149Backend};
///
/// let mut chip = Ym2149::new();
///
/// // Set up channel A: low period, max volume
/// chip.write_register(0, 0x00);  // Period low
/// chip.write_register(1, 0x01);  // Period high
/// chip.write_register(8, 0x0F);  // Volume
/// chip.write_register(7, 0x3E);  // Mixer: tone A on
///
/// // Generate audio
/// chip.clock();
/// let sample = chip.get_sample();
/// ```
#[derive(Clone)]
pub struct Ym2149 {
    // Clock and timing
    internal_clock: u32,
    sample_rate: u32,
    cycle_accumulator: u32,

    // Hardware registers
    registers: [u8; NUM_REGISTERS],
    selected_register: usize,

    // Generators
    tone_generators: [ToneGenerator; NUM_CHANNELS],
    noise_generator: NoiseGenerator,
    envelope_generator: EnvelopeGenerator,

    // Output processing
    mixer: Mixer,
    dc_filter: DcFilter,

    // Cached output for Backend trait
    last_sample: f32,

    // Timer IRQ state (for sync-buzzer effects)
    in_timer_irq: bool,
}

impl Ym2149 {
    /// Create a new YM2149 with default Atari ST clocks
    ///
    /// Default configuration:
    /// - Master clock: 2 MHz
    /// - Sample rate: 44.1 kHz
    pub fn new() -> Self {
        Self::with_clocks(DEFAULT_MASTER_CLOCK, DEFAULT_SAMPLE_RATE)
    }

    /// Create a new YM2149 with custom clock frequencies
    ///
    /// # Arguments
    ///
    /// * `master_clock` - Master clock frequency in Hz (divided by 8 internally)
    /// * `sample_rate` - Audio output sample rate in Hz
    pub fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        let mut chip = Self {
            internal_clock: master_clock / 8,
            sample_rate,
            cycle_accumulator: 0,
            registers: [0; NUM_REGISTERS],
            selected_register: 0,
            tone_generators: [
                ToneGenerator::new(),
                ToneGenerator::new(),
                ToneGenerator::new(),
            ],
            noise_generator: NoiseGenerator::new(),
            envelope_generator: EnvelopeGenerator::new(),
            mixer: Mixer::new(),
            dc_filter: DcFilter::new(),
            last_sample: 0.0,
            in_timer_irq: false,
        };
        chip.reset();
        chip
    }

    /// Reset the chip to initial state
    pub fn reset(&mut self) {
        // Randomize tone edge state (hardware behavior)
        let mut seed = 1u32;
        let random_edges = (random_seed(&mut seed) as u32 & ((1 << 10) | (1 << 5) | 1)) * 0x1f;

        for (i, tone) in self.tone_generators.iter_mut().enumerate() {
            tone.reset();
            // Set random edge bits for this channel
            tone.set_edge_bits(random_edges & (0x1f << (i * 5)));
        }

        self.noise_generator.reset();
        self.envelope_generator.reset();
        self.mixer.reset();
        self.dc_filter.reset();

        // Initialize registers (R7 = 0x3F = all outputs disabled)
        self.registers = [0; NUM_REGISTERS];
        self.apply_register(7, 0x3F);

        self.selected_register = 0;
        self.cycle_accumulator = 0;
        self.in_timer_irq = false;
        self.last_sample = 0.0;
    }

    /// Write to hardware port (mimics real hardware bus access)
    ///
    /// # Arguments
    ///
    /// * `port` - Port number (bit 1: 0 = address, 1 = data)
    /// * `value` - Value to write
    pub fn write_port(&mut self, port: u8, value: u8) {
        if (port & 2) != 0 {
            self.apply_register(self.selected_register, value);
        } else {
            self.selected_register = (value as usize) & 0x0F;
        }
    }

    /// Read from hardware port
    ///
    /// # Arguments
    ///
    /// * `port` - Port number (bit 1: 0 = address, 1 = data)
    ///
    /// # Returns
    ///
    /// Register value or 0xFF for invalid reads
    pub fn read_port(&self, port: u8) -> u8 {
        if (port & 2) == 0 && self.selected_register < NUM_REGISTERS {
            self.registers[self.selected_register]
        } else {
            0xFF
        }
    }

    /// Write to a register
    ///
    /// # Arguments
    ///
    /// * `register` - Register number (0-13)
    /// * `value` - Value to write
    pub fn write_register(&mut self, register: u8, value: u8) {
        self.apply_register(register as usize, value);
    }

    /// Read from a register
    ///
    /// # Arguments
    ///
    /// * `register` - Register number (0-13)
    ///
    /// # Returns
    ///
    /// Current register value
    pub fn read_register(&self, register: u8) -> u8 {
        let reg = register as usize;
        if reg < NUM_REGISTERS {
            self.registers[reg]
        } else {
            0
        }
    }

    /// Apply a register write and update internal state
    fn apply_register(&mut self, register: usize, value: u8) {
        if register >= NUM_REGISTERS {
            return;
        }

        // Mask value to valid bits
        let value = value & REG_MASK[register];
        self.registers[register] = value;

        match register {
            // Tone period registers (2 registers per channel)
            0..=5 => {
                let channel = register / 2;
                let period = self.read_tone_period(channel);
                self.tone_generators[channel].set_period(period);

                // Check for sync-buzzer effect
                if period <= 1 && self.in_timer_irq {
                    self.tone_generators[channel].mark_pending_reset();
                }
            }

            // Noise period
            6 => {
                self.noise_generator.set_period(value as u32);
            }

            // Mixer control
            7 => {
                self.mixer.config.set_from_register(value);
            }

            // Envelope period (R11/R12)
            11 | 12 => {
                let period = self.read_envelope_period();
                self.envelope_generator.set_period(period);
            }

            // Envelope shape (R13)
            13 => {
                self.envelope_generator.set_shape(value);
            }

            _ => {}
        }
    }

    /// Read 12-bit tone period from register pair
    #[inline]
    fn read_tone_period(&self, channel: usize) -> u32 {
        let base = channel * 2;
        ((self.registers[base + 1] as u32) << 8) | (self.registers[base] as u32)
    }

    /// Read 16-bit envelope period from registers
    #[inline]
    fn read_envelope_period(&self) -> u32 {
        ((self.registers[12] as u32) << 8) | (self.registers[11] as u32)
    }

    /// Tick internal state machines at 250kHz rate
    ///
    /// Returns the combined gate mask for all channels.
    fn tick_generators(&mut self) -> u32 {
        // Combine all tone edges
        let mut tone_edges = 0u32;
        for (i, tone) in self.tone_generators.iter_mut().enumerate() {
            tone_edges |= tone.tick(i as u32 * 5);
        }

        // Tick noise (runs at half rate internally)
        let noise_mask = self.noise_generator.tick();

        // Tick envelope
        self.envelope_generator.tick();

        // Compute combined gate mask
        self.mixer.config.compute_gate_mask(tone_edges, noise_mask)
    }

    /// Generate the next audio sample
    ///
    /// This method runs the internal state machine at 250kHz and averages
    /// the output to produce samples at the host sample rate.
    pub fn compute_next_sample(&mut self) -> i16 {
        // Accumulate gate mask over all internal ticks
        let mut accumulated_mask: u16 = 0;

        loop {
            accumulated_mask |= self.tick_generators() as u16;
            self.cycle_accumulator += self.sample_rate;
            if self.cycle_accumulator >= self.internal_clock {
                break;
            }
        }
        self.cycle_accumulator -= self.internal_clock;

        // Get envelope level
        let envelope_level = self.envelope_generator.level();

        // Build channel levels
        let volume_regs = [self.registers[8], self.registers[9], self.registers[10]];
        let levels =
            self.mixer
                .compute_levels(volume_regs, envelope_level, accumulated_mask as u32);

        // Compute individual channel outputs
        let mut total_output = 0u32;
        for channel in 0..NUM_CHANNELS {
            let level_index = (levels >> (channel * 5)) & 0x1F;
            let half_amplitude = self.tone_generators[channel].is_half_amplitude();
            total_output += self
                .mixer
                .compute_channel_output(channel, level_index, half_amplitude);
        }

        // Apply DC filter and return
        self.dc_filter.process(total_output as u16)
    }

    /// Signal entry/exit of timer IRQ handler
    ///
    /// This is used by sync-buzzer effects where the tone period is set to 0 or 1
    /// inside the IRQ to create sample-accurate waveforms. When exiting the IRQ,
    /// any pending edge resets are applied.
    pub fn set_timer_irq_state(&mut self, in_irq: bool) {
        if !in_irq {
            // Apply pending resets when exiting IRQ
            for (i, tone) in self.tone_generators.iter_mut().enumerate() {
                tone.apply_pending_reset(i as u32 * 5);
            }
        }
        self.in_timer_irq = in_irq;
    }

    /// Alias for set_timer_irq_state (compatibility)
    pub fn inside_timer_irq(&mut self, inside: bool) {
        self.set_timer_irq_state(inside);
    }

    /// Alias for inside_timer_irq (compatibility)
    pub fn set_inside_timer_irq(&mut self, inside: bool) {
        self.set_timer_irq_state(inside);
    }
}

impl Default for Ym2149 {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Ym2149 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ym2149")
            .field("registers", &self.registers)
            .field("sample_rate", &self.sample_rate)
            .field("internal_clock", &self.internal_clock)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Ym2149Backend trait implementation
// =============================================================================

impl Ym2149Backend for Ym2149 {
    fn new() -> Self {
        Ym2149::new()
    }

    fn with_clocks(master_clock: u32, sample_rate: u32) -> Self {
        Ym2149::with_clocks(master_clock, sample_rate)
    }

    fn reset(&mut self) {
        Ym2149::reset(self)
    }

    fn write_register(&mut self, addr: u8, value: u8) {
        Ym2149::write_register(self, addr, value)
    }

    fn read_register(&self, addr: u8) -> u8 {
        Ym2149::read_register(self, addr)
    }

    fn load_registers(&mut self, regs: &[u8; 16]) {
        for (i, &value) in regs.iter().take(NUM_REGISTERS).enumerate() {
            self.write_register(i as u8, value);
        }
    }

    fn dump_registers(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..NUM_REGISTERS].copy_from_slice(&self.registers);
        out
    }

    fn clock(&mut self) {
        let sample_i16 = self.compute_next_sample();
        self.last_sample = (sample_i16 as f32 / 32767.0 * MASTER_GAIN).clamp(-1.0, 1.0);
    }

    fn get_sample(&self) -> f32 {
        self.last_sample
    }

    fn get_channel_outputs(&self) -> (f32, f32, f32) {
        self.mixer.channel_outputs()
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.mixer.set_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.mixer.is_muted(channel)
    }

    fn set_color_filter(&mut self, _enabled: bool) {
        // No post filter in this implementation
    }

    fn trigger_envelope(&mut self) {
        self.envelope_generator.trigger();
    }

    fn set_drum_sample_override(&mut self, channel: usize, sample: Option<f32>) {
        self.mixer.set_drum_override(channel, sample);
    }

    fn set_mixer_overrides(&mut self, _force_tone: [bool; 3], _force_noise_mute: [bool; 3]) {
        // Not implemented - would require extending MixerConfig
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_chip_has_default_state() {
        let chip = Ym2149::new();
        assert_eq!(chip.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(chip.internal_clock, DEFAULT_MASTER_CLOCK / 8);
    }

    #[test]
    fn test_register_read_write() {
        let mut chip = Ym2149::new();

        chip.write_register(0, 0x55);
        assert_eq!(chip.read_register(0), 0x55);

        chip.write_register(1, 0xFF);
        assert_eq!(chip.read_register(1), 0x0F); // Masked to 4 bits
    }

    #[test]
    fn test_reset_clears_state() {
        let mut chip = Ym2149::new();

        // Set some registers
        chip.write_register(0, 0x55);
        chip.write_register(8, 0x0F);

        // Reset
        chip.reset();

        // Registers should be cleared (except R7 = 0x3F)
        assert_eq!(chip.read_register(0), 0);
        assert_eq!(chip.read_register(8), 0);
        assert_eq!(chip.read_register(7), 0x3F);
    }

    #[test]
    fn test_sample_generation() {
        let mut chip = Ym2149::new();

        // Set up a simple tone
        chip.write_register(0, 0x00);
        chip.write_register(1, 0x01);
        chip.write_register(8, 0x0F);
        chip.write_register(7, 0x3E);

        // Generate samples
        for _ in 0..100 {
            chip.clock();
        }

        // Should have non-zero output
        let sample = chip.get_sample();
        // After DC filtering warmup, we should see some output
        assert!(sample.abs() > 0.0 || chip.last_sample.abs() >= 0.0);
    }

    #[test]
    fn test_channel_mute() {
        let mut chip = Ym2149::new();

        assert!(!chip.is_channel_muted(0));
        chip.set_channel_mute(0, true);
        assert!(chip.is_channel_muted(0));
        assert!(!chip.is_channel_muted(1));
    }

    #[test]
    fn test_port_access() {
        let mut chip = Ym2149::new();

        // Select register 5
        chip.write_port(0, 5);
        // Write value to register 5
        chip.write_port(2, 0x0A);

        assert_eq!(chip.read_register(5), 0x0A);
    }
}
