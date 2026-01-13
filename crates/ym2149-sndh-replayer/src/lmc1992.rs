//! LMC1992 STE Audio Mixer/Filter emulation.
//!
//! The LMC1992 is a digitally controlled audio processor used in the Atari STE
//! for volume, bass, and treble control. It's controlled via the Microwire interface.
//!
//! Microwire registers:
//! - $FF8922: Data register (11-bit command)
//! - $FF8924: Mask register (indicates transmission complete when all 1s)
//!
//! Command format (11 bits):
//! - Bits 10-9: Device address (10 for LMC1992)
//! - Bits 8-6: Function select
//! - Bits 5-0: Data value

/// Biquad filter for bass/treble EQ.
#[derive(Clone, Debug)]
struct BiquadFilter {
    // Coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for BiquadFilter {
    fn default() -> Self {
        Self {
            // Unity gain (passthrough)
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl BiquadFilter {
    /// Configure as low-shelf filter for bass control.
    /// gain_db: -12 to +12 dB
    fn configure_low_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / 0.9 - 1.0) + 2.0).sqrt();

        let a_plus_1 = a + 1.0;
        let a_minus_1 = a - 1.0;
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let a0 = a_plus_1 + a_minus_1 * cos_w0 + two_sqrt_a_alpha;

        self.b0 = (a * (a_plus_1 - a_minus_1 * cos_w0 + two_sqrt_a_alpha)) / a0;
        self.b1 = (2.0 * a * (a_minus_1 - a_plus_1 * cos_w0)) / a0;
        self.b2 = (a * (a_plus_1 - a_minus_1 * cos_w0 - two_sqrt_a_alpha)) / a0;
        self.a1 = (-2.0 * (a_minus_1 + a_plus_1 * cos_w0)) / a0;
        self.a2 = (a_plus_1 + a_minus_1 * cos_w0 - two_sqrt_a_alpha) / a0;
    }

    /// Configure as high-shelf filter for treble control.
    /// gain_db: -12 to +12 dB
    fn configure_high_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / 0.9 - 1.0) + 2.0).sqrt();

        let a_plus_1 = a + 1.0;
        let a_minus_1 = a - 1.0;
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let a0 = a_plus_1 - a_minus_1 * cos_w0 + two_sqrt_a_alpha;

        self.b0 = (a * (a_plus_1 + a_minus_1 * cos_w0 + two_sqrt_a_alpha)) / a0;
        self.b1 = (-2.0 * a * (a_minus_1 + a_plus_1 * cos_w0)) / a0;
        self.b2 = (a * (a_plus_1 + a_minus_1 * cos_w0 - two_sqrt_a_alpha)) / a0;
        self.a1 = (2.0 * (a_minus_1 - a_plus_1 * cos_w0)) / a0;
        self.a2 = (a_plus_1 - a_minus_1 * cos_w0 - two_sqrt_a_alpha) / a0;
    }

    /// Reset filter state.
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Process a single sample.
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

/// LMC1992 function codes (bits 8-6 of command).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Lmc1992Function {
    Mix = 0,
    Bass = 1,
    Treble = 2,
    MasterVolume = 3,
    RightVolume = 4,
    LeftVolume = 5,
}

impl Lmc1992Function {
    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Mix),
            1 => Some(Self::Bass),
            2 => Some(Self::Treble),
            3 => Some(Self::MasterVolume),
            4 => Some(Self::RightVolume),
            5 => Some(Self::LeftVolume),
            _ => None,
        }
    }
}

/// LMC1992 STE Audio Mixer emulation.
#[derive(Clone)]
pub struct Lmc1992 {
    /// Microwire data register ($FF8922)
    mw_data: u16,
    /// Microwire mask register ($FF8924) - value written by software
    mw_mask: u16,
    /// Transmission state: 0 = idle, 1 = started (return 0), 2 = done (return mask)
    mw_transmission_state: u8,

    /// Master volume (0-40, maps to -80dB to 0dB in 2dB steps)
    master_volume: u8,
    /// Left channel volume (0-20, maps to -40dB to 0dB in 2dB steps)
    left_volume: u8,
    /// Right channel volume (0-20, maps to -40dB to 0dB in 2dB steps)
    right_volume: u8,
    /// Bass setting (0-12, maps to -12dB to +12dB in 2dB steps)
    bass: u8,
    /// Treble setting (0-12, maps to -12dB to +12dB in 2dB steps)
    treble: u8,
    /// Mix control: true = mix YM2149, false = don't mix
    mix_ym: bool,

    /// Bass filter (left channel)
    bass_filter_l: BiquadFilter,
    /// Bass filter (right channel)
    bass_filter_r: BiquadFilter,
    /// Treble filter (left channel)
    treble_filter_l: BiquadFilter,
    /// Treble filter (right channel)
    treble_filter_r: BiquadFilter,

    /// Sample rate for filter calculations
    sample_rate: f32,

    /// Precomputed volume multipliers
    master_gain: f32,
    left_gain: f32,
    right_gain: f32,
}

impl Lmc1992 {
    /// Create a new LMC1992 instance.
    pub fn new(sample_rate: u32) -> Self {
        let mut lmc = Self {
            mw_data: 0,
            mw_mask: 0x07FF,
            mw_transmission_state: 0,

            // Default: 0dB (maximum) for all volumes
            master_volume: 40,
            left_volume: 20,
            right_volume: 20,
            // Default: flat (0dB) for bass/treble (index 6 = 0dB)
            bass: 6,
            treble: 6,
            // Default: mix YM2149
            mix_ym: true,

            bass_filter_l: BiquadFilter::default(),
            bass_filter_r: BiquadFilter::default(),
            treble_filter_l: BiquadFilter::default(),
            treble_filter_r: BiquadFilter::default(),

            sample_rate: sample_rate as f32,

            master_gain: 1.0,
            left_gain: 1.0,
            right_gain: 1.0,
        };
        lmc.update_filters();
        lmc.update_gains();
        lmc
    }

    /// Reset to default state.
    pub fn reset(&mut self, sample_rate: u32) {
        self.mw_data = 0;
        self.mw_mask = 0x07FF;
        self.mw_transmission_state = 0;

        self.master_volume = 40;
        self.left_volume = 20;
        self.right_volume = 20;
        self.bass = 6;
        self.treble = 6;
        self.mix_ym = true;

        self.sample_rate = sample_rate as f32;

        self.bass_filter_l.reset();
        self.bass_filter_r.reset();
        self.treble_filter_l.reset();
        self.treble_filter_r.reset();

        self.update_filters();
        self.update_gains();
    }

    /// Read from microwire data register (byte).
    /// Note: Byte reads don't advance transmission state (only word reads do).
    pub fn read8(&self, offset: u8) -> u8 {
        match offset {
            0x22 => (self.mw_data >> 8) as u8,
            0x23 => self.mw_data as u8,
            0x24 => (self.peek_mask() >> 8) as u8,
            0x25 => self.peek_mask() as u8,
            _ => 0xFF,
        }
    }

    /// Read from microwire registers (word).
    /// Word read of mask register advances transmission state.
    pub fn read16(&mut self, offset: u8) -> u16 {
        match offset {
            0x22 => self.mw_data,
            0x24 => self.read_mask_and_advance(),
            _ => 0xFFFF,
        }
    }

    /// Peek at mask register without advancing state (for byte reads).
    fn peek_mask(&self) -> u16 {
        match self.mw_transmission_state {
            1 => 0, // Transmission in progress
            _ => self.mw_mask,
        }
    }

    /// Read mask register and advance transmission state.
    /// Software waits for mask != 0x7FF (transmission started),
    /// then waits for mask == 0x7FF (transmission complete).
    fn read_mask_and_advance(&mut self) -> u16 {
        match self.mw_transmission_state {
            1 => {
                // Transmission in progress - return 0 (not ready)
                self.mw_transmission_state = 2;
                0
            }
            2 => {
                // Transmission complete - return mask value
                self.mw_transmission_state = 0;
                self.mw_mask
            }
            _ => {
                // Idle - return mask value
                self.mw_mask
            }
        }
    }

    /// Write to microwire data register (byte).
    pub fn write8(&mut self, offset: u8, value: u8) {
        match offset {
            0x22 => {
                self.mw_data = (self.mw_data & 0x00FF) | ((value as u16) << 8);
            }
            0x23 => {
                self.mw_data = (self.mw_data & 0xFF00) | (value as u16);
                self.start_transmission();
            }
            0x24 => {
                self.mw_mask = (self.mw_mask & 0x00FF) | ((value as u16) << 8);
            }
            0x25 => {
                self.mw_mask = (self.mw_mask & 0xFF00) | (value as u16);
            }
            _ => {}
        }
    }

    /// Write to microwire registers (word).
    pub fn write16(&mut self, offset: u8, value: u16) {
        match offset {
            0x22 => {
                self.mw_data = value;
                self.start_transmission();
            }
            0x24 => {
                self.mw_mask = value;
            }
            _ => {}
        }
    }

    /// Start microwire transmission.
    fn start_transmission(&mut self) {
        // Set transmission state so software can detect it started
        self.mw_transmission_state = 1;

        // Extract 11-bit command using mask
        let command = self.mw_data & self.mw_mask & 0x07FF;

        // Check device address (bits 10-9 must be 10 binary = 2)
        let address = (command >> 9) & 0x03;
        if address != 2 {
            return;
        }

        // Extract function (bits 8-6)
        let function_bits = ((command >> 6) & 0x07) as u8;
        let Some(function) = Lmc1992Function::from_bits(function_bits) else {
            return;
        };

        // Extract data (bits 5-0)
        let data = (command & 0x3F) as u8;

        self.process_command(function, data);
    }

    /// Process an LMC1992 command.
    fn process_command(&mut self, function: Lmc1992Function, data: u8) {
        match function {
            Lmc1992Function::MasterVolume => {
                // Data bits 5-0: volume level (0-40 in 2dB steps)
                // 0 = -80dB, 40 = 0dB
                self.master_volume = data.min(40);
                self.update_gains();
            }
            Lmc1992Function::LeftVolume => {
                // Data bits 4-0: volume level (0-20 in 2dB steps)
                // 0 = -40dB, 20 = 0dB
                self.left_volume = (data & 0x1F).min(20);
                self.update_gains();
            }
            Lmc1992Function::RightVolume => {
                // Data bits 4-0: volume level (0-20 in 2dB steps)
                // 0 = -40dB, 20 = 0dB
                self.right_volume = (data & 0x1F).min(20);
                self.update_gains();
            }
            Lmc1992Function::Bass => {
                // Data bits 3-0: bass level (0-12)
                // 0 = -12dB, 6 = 0dB, 12 = +12dB
                self.bass = (data & 0x0F).min(12);
                self.update_filters();
            }
            Lmc1992Function::Treble => {
                // Data bits 3-0: treble level (0-12)
                // 0 = -12dB, 6 = 0dB, 12 = +12dB
                self.treble = (data & 0x0F).min(12);
                self.update_filters();
            }
            Lmc1992Function::Mix => {
                // Data bits 1-0: mix control
                // 01 = mix YM2149, 10 = don't mix
                self.mix_ym = (data & 0x03) == 0x01;
            }
        }
    }

    /// Update filter coefficients based on bass/treble settings.
    fn update_filters(&mut self) {
        // Bass: -12dB to +12dB (index 0-12, 6 = flat)
        let bass_db = (self.bass as f32 - 6.0) * 2.0;
        // Treble: -12dB to +12dB (index 0-12, 6 = flat)
        let treble_db = (self.treble as f32 - 6.0) * 2.0;

        // Bass shelf at ~100Hz
        self.bass_filter_l
            .configure_low_shelf(self.sample_rate, 100.0, bass_db);
        self.bass_filter_r
            .configure_low_shelf(self.sample_rate, 100.0, bass_db);

        // Treble shelf at ~10kHz
        self.treble_filter_l
            .configure_high_shelf(self.sample_rate, 10000.0, treble_db);
        self.treble_filter_r
            .configure_high_shelf(self.sample_rate, 10000.0, treble_db);
    }

    /// Update volume gain multipliers.
    fn update_gains(&mut self) {
        // Master: 0 = -80dB, 40 = 0dB (2dB steps)
        let master_db = (self.master_volume as f32 - 40.0) * 2.0;
        self.master_gain = 10.0_f32.powf(master_db / 20.0);

        // Left/Right: 0 = -40dB, 20 = 0dB (2dB steps)
        let left_db = (self.left_volume as f32 - 20.0) * 2.0;
        let right_db = (self.right_volume as f32 - 20.0) * 2.0;
        self.left_gain = 10.0_f32.powf(left_db / 20.0);
        self.right_gain = 10.0_f32.powf(right_db / 20.0);
    }

    /// Check if YM2149 should be mixed.
    pub fn should_mix_ym(&self) -> bool {
        self.mix_ym
    }

    /// Process stereo audio through the LMC1992.
    /// Takes left/right samples (i16), applies bass/treble EQ and volume.
    pub fn process_stereo(&mut self, left: i16, right: i16) -> (i16, i16) {
        // Convert to float for processing
        let mut left_f = left as f32;
        let mut right_f = right as f32;

        // Apply bass filter
        left_f = self.bass_filter_l.process(left_f);
        right_f = self.bass_filter_r.process(right_f);

        // Apply treble filter
        left_f = self.treble_filter_l.process(left_f);
        right_f = self.treble_filter_r.process(right_f);

        // Apply volume controls
        left_f *= self.master_gain * self.left_gain;
        right_f *= self.master_gain * self.right_gain;

        // Clamp and convert back to i16
        let left_out = left_f.clamp(-32768.0, 32767.0) as i16;
        let right_out = right_f.clamp(-32768.0, 32767.0) as i16;

        (left_out, right_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let lmc = Lmc1992::new(44100);
        assert!(lmc.mix_ym);
        assert_eq!(lmc.master_volume, 40); // 0dB
        assert_eq!(lmc.left_volume, 20); // 0dB
        assert_eq!(lmc.right_volume, 20); // 0dB
        assert_eq!(lmc.bass, 6); // flat
        assert_eq!(lmc.treble, 6); // flat
    }

    #[test]
    fn test_microwire_command() {
        let mut lmc = Lmc1992::new(44100);

        // Set master volume to -20dB (value 30)
        // Command: 10 (addr) + 011 (master vol) + 011110 (30)
        // = 10_011_011110 = 0b10011011110 = 0x4DE
        lmc.mw_mask = 0x07FF;
        lmc.write16(0x22, 0x04DE);

        assert_eq!(lmc.master_volume, 30);
    }

    #[test]
    fn test_mix_control() {
        let mut lmc = Lmc1992::new(44100);

        // Disable YM mixing
        // Command: 10 (addr) + 000 (mix) + 000010 (don't mix)
        // = 10_000_000010 = 0b10000000010 = 0x402
        lmc.mw_mask = 0x07FF;
        lmc.write16(0x22, 0x402);

        assert!(!lmc.mix_ym);

        // Enable YM mixing
        // Command: 10_000_000001 = 0x401
        lmc.write16(0x22, 0x401);

        assert!(lmc.mix_ym);
    }

    #[test]
    fn test_passthrough_at_default() {
        let mut lmc = Lmc1992::new(44100);

        // With default settings (0dB everything, flat EQ),
        // output should approximately equal input
        let input = 1000_i16;
        let (left, right) = lmc.process_stereo(input, input);

        // Allow small deviation due to filter settling
        assert!((left - input).abs() < 50);
        assert!((right - input).abs() < 50);
    }
}
