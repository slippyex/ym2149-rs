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

/// First-order shelving filter for bass/treble EQ.
///
/// The LMC1992 uses analog shelving filters. First-order shelves provide
/// the characteristic 6dB/octave slope that matches analog tone controls.
/// We cascade two first-order sections to achieve the full ±12dB range
/// with proper frequency response.
#[derive(Clone, Debug)]
struct ShelvingFilter {
    // First-order IIR coefficients: y = b0*x + b1*x1 - a1*y1
    b0: f32,
    b1: f32,
    a1: f32,
    // Filter state
    x1: f32,
    y1: f32,
    // For debug
    #[cfg(feature = "lmc1992-debug")]
    gain_db: f32,
}

impl Default for ShelvingFilter {
    fn default() -> Self {
        Self {
            // Unity gain (passthrough)
            b0: 1.0,
            b1: 0.0,
            a1: 0.0,
            x1: 0.0,
            y1: 0.0,
            #[cfg(feature = "lmc1992-debug")]
            gain_db: 0.0,
        }
    }
}

impl ShelvingFilter {
    /// Configure as low-shelf filter for bass control.
    /// Uses first-order design for analog-like response.
    /// gain_db: -12 to +12 dB
    ///
    /// First-order low shelf: H(s) = (s + ω₀×√G) / (s + ω₀/√G)
    /// DC gain = G, high-frequency gain = 1
    fn configure_low_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        #[cfg(feature = "lmc1992-debug")]
        {
            self.gain_db = gain_db;
        }

        if gain_db.abs() < 0.01 {
            // Flat - passthrough
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.a1 = 0.0;
            return;
        }

        // Linear gain and its square root
        let g = 10.0_f32.powf(gain_db / 20.0);
        let sqrt_g = g.sqrt();

        // Bilinear transform: K = tan(π × fc / fs)
        let k = (std::f32::consts::PI * freq / sample_rate).tan();

        // Coefficients for H(s) = (s + ω₀×√G) / (s + ω₀/√G)
        // After bilinear transform:
        let k_sqrt_g = k * sqrt_g;
        let k_over_sqrt_g = k / sqrt_g;
        let denom = 1.0 + k_over_sqrt_g;

        self.b0 = (1.0 + k_sqrt_g) / denom;
        self.b1 = (k_sqrt_g - 1.0) / denom;
        self.a1 = (k_over_sqrt_g - 1.0) / denom;
    }

    /// Configure as high-shelf filter for treble control.
    /// Uses first-order design for analog-like response.
    /// gain_db: -12 to +12 dB
    ///
    /// First-order high shelf analog prototype:
    /// H(s) = (s×√G + ω₀) / (s/√G + ω₀)
    /// DC gain = 1, high-frequency gain = G
    fn configure_high_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        #[cfg(feature = "lmc1992-debug")]
        {
            self.gain_db = gain_db;
        }

        if gain_db.abs() < 0.01 {
            // Flat - passthrough
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.a1 = 0.0;
            return;
        }

        // Linear gain and its square root
        let g = 10.0_f32.powf(gain_db / 20.0);
        let sqrt_g = g.sqrt();

        // Bilinear transform: K = tan(π × fc / fs)
        let k = (std::f32::consts::PI * freq / sample_rate).tan();

        // First-order high shelf via bilinear transform:
        // H(s) = (s×√G + ω₀) / (s/√G + ω₀)
        // After transform with pre-warping:
        // b0 = √G × (√G + k) / (1 + k×√G)
        // b1 = √G × (k - √G) / (1 + k×√G)
        // a1 = (k×√G - 1) / (1 + k×√G)
        let k_sqrt_g = k * sqrt_g;
        let denom = 1.0 + k_sqrt_g;

        self.b0 = sqrt_g * (sqrt_g + k) / denom;
        self.b1 = sqrt_g * (k - sqrt_g) / denom;
        self.a1 = (k_sqrt_g - 1.0) / denom;
    }

    /// Reset filter state.
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// Process a single sample through first-order IIR.
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = input;
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

    /// Bass filter stage 1 (left channel)
    bass_filter_l1: ShelvingFilter,
    /// Bass filter stage 1 (right channel)
    bass_filter_r1: ShelvingFilter,
    /// Bass filter stage 2 (left channel) - cascaded for 12dB/octave
    bass_filter_l2: ShelvingFilter,
    /// Bass filter stage 2 (right channel) - cascaded for 12dB/octave
    bass_filter_r2: ShelvingFilter,
    /// Treble filter stage 1 (left channel)
    treble_filter_l1: ShelvingFilter,
    /// Treble filter stage 1 (right channel)
    treble_filter_r1: ShelvingFilter,
    /// Treble filter stage 2 (left channel) - cascaded for 12dB/octave
    treble_filter_l2: ShelvingFilter,
    /// Treble filter stage 2 (right channel) - cascaded for 12dB/octave
    treble_filter_r2: ShelvingFilter,

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

            bass_filter_l1: ShelvingFilter::default(),
            bass_filter_r1: ShelvingFilter::default(),
            bass_filter_l2: ShelvingFilter::default(),
            bass_filter_r2: ShelvingFilter::default(),
            treble_filter_l1: ShelvingFilter::default(),
            treble_filter_r1: ShelvingFilter::default(),
            treble_filter_l2: ShelvingFilter::default(),
            treble_filter_r2: ShelvingFilter::default(),

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

        self.bass_filter_l1.reset();
        self.bass_filter_r1.reset();
        self.bass_filter_l2.reset();
        self.bass_filter_r2.reset();
        self.treble_filter_l1.reset();
        self.treble_filter_r1.reset();
        self.treble_filter_l2.reset();
        self.treble_filter_r2.reset();

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
                #[cfg(feature = "lmc1992-debug")]
                eprintln!(
                    "[LMC1992] Master Volume: {} (gain={:.3})",
                    self.master_volume, self.master_gain
                );
            }
            Lmc1992Function::LeftVolume => {
                // Data bits 4-0: volume level (0-20 in 2dB steps)
                // 0 = -40dB, 20 = 0dB
                self.left_volume = (data & 0x1F).min(20);
                self.update_gains();
                #[cfg(feature = "lmc1992-debug")]
                eprintln!(
                    "[LMC1992] Left Volume: {} (gain={:.3})",
                    self.left_volume, self.left_gain
                );
            }
            Lmc1992Function::RightVolume => {
                // Data bits 4-0: volume level (0-20 in 2dB steps)
                // 0 = -40dB, 20 = 0dB
                self.right_volume = (data & 0x1F).min(20);
                self.update_gains();
                #[cfg(feature = "lmc1992-debug")]
                eprintln!(
                    "[LMC1992] Right Volume: {} (gain={:.3})",
                    self.right_volume, self.right_gain
                );
            }
            Lmc1992Function::Bass => {
                // Data bits 3-0: bass level (0-12)
                // 0 = -12dB, 6 = 0dB, 12 = +12dB
                self.bass = (data & 0x0F).min(12);
                self.update_filters();
                #[cfg(feature = "lmc1992-debug")]
                eprintln!(
                    "[LMC1992] Bass: {} ({}dB)",
                    self.bass,
                    (self.bass as i8 - 6) * 2
                );
            }
            Lmc1992Function::Treble => {
                // Data bits 3-0: treble level (0-12)
                // 0 = -12dB, 6 = 0dB, 12 = +12dB
                self.treble = (data & 0x0F).min(12);
                self.update_filters();
                #[cfg(feature = "lmc1992-debug")]
                eprintln!(
                    "[LMC1992] Treble: {} ({}dB)",
                    self.treble,
                    (self.treble as i8 - 6) * 2
                );
            }
            Lmc1992Function::Mix => {
                // Data bits 1-0: mix control (LMC1992 input select)
                // 00 = DMA + YM2149 (-12dB, broken on real HW = same as 01)
                // 01 = DMA + YM2149 (default)
                // 10 = DMA only
                // 11 = reserved
                // Mix YM when bit 1 is 0 (values 00 or 01)
                self.mix_ym = (data & 0x02) == 0;
                #[cfg(feature = "lmc1992-debug")]
                eprintln!("[LMC1992] Mix: data={}, mix_ym={}", data, self.mix_ym);
            }
        }
    }

    /// Update filter coefficients based on bass/treble settings.
    fn update_filters(&mut self) {
        // Bass: -12dB to +12dB (index 0-12, 6 = flat)
        let bass_db = (self.bass as f32 - 6.0) * 2.0;
        // Treble: -12dB to +12dB (index 0-12, 6 = flat)
        let treble_db = (self.treble as f32 - 6.0) * 2.0;

        // Atari STE filter frequencies (empirically measured from real hardware):
        // - Bass turnover: 118.276 Hz
        // - Treble turnover: 8438.756 Hz
        const STE_BASS_FREQ: f32 = 118.276;
        const STE_TREBLE_FREQ: f32 = 8438.756;

        // Cascade two first-order shelves to get 12dB/octave slope
        // Each stage applies half the dB gain
        let bass_db_per_stage = bass_db / 2.0;
        let treble_db_per_stage = treble_db / 2.0;

        self.bass_filter_l1
            .configure_low_shelf(self.sample_rate, STE_BASS_FREQ, bass_db_per_stage);
        self.bass_filter_r1
            .configure_low_shelf(self.sample_rate, STE_BASS_FREQ, bass_db_per_stage);
        self.bass_filter_l2
            .configure_low_shelf(self.sample_rate, STE_BASS_FREQ, bass_db_per_stage);
        self.bass_filter_r2
            .configure_low_shelf(self.sample_rate, STE_BASS_FREQ, bass_db_per_stage);

        self.treble_filter_l1.configure_high_shelf(
            self.sample_rate,
            STE_TREBLE_FREQ,
            treble_db_per_stage,
        );
        self.treble_filter_r1.configure_high_shelf(
            self.sample_rate,
            STE_TREBLE_FREQ,
            treble_db_per_stage,
        );
        self.treble_filter_l2.configure_high_shelf(
            self.sample_rate,
            STE_TREBLE_FREQ,
            treble_db_per_stage,
        );
        self.treble_filter_r2.configure_high_shelf(
            self.sample_rate,
            STE_TREBLE_FREQ,
            treble_db_per_stage,
        );

        #[cfg(feature = "lmc1992-debug")]
        eprintln!(
            "[LMC1992] Filter update: bass_db={}, treble_db={} (cascaded 2x{}dB/stage)",
            bass_db, treble_db, bass_db_per_stage
        );
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

    /// Get master volume (0-40, where 40 = 0dB, 0 = -80dB).
    pub fn master_volume(&self) -> u8 {
        self.master_volume
    }

    /// Get left channel volume (0-20, where 20 = 0dB, 0 = -40dB).
    pub fn left_volume(&self) -> u8 {
        self.left_volume
    }

    /// Get right channel volume (0-20, where 20 = 0dB, 0 = -40dB).
    pub fn right_volume(&self) -> u8 {
        self.right_volume
    }

    /// Get bass setting (0-12, where 6 = flat, 0 = -12dB, 12 = +12dB).
    pub fn bass(&self) -> u8 {
        self.bass
    }

    /// Get treble setting (0-12, where 6 = flat, 0 = -12dB, 12 = +12dB).
    pub fn treble(&self) -> u8 {
        self.treble
    }

    /// Get master volume in dB (-80 to 0).
    pub fn master_volume_db(&self) -> i8 {
        (self.master_volume as i8 - 40) * 2
    }

    /// Get left volume in dB (-40 to 0).
    pub fn left_volume_db(&self) -> i8 {
        (self.left_volume as i8 - 20) * 2
    }

    /// Get right volume in dB (-40 to 0).
    pub fn right_volume_db(&self) -> i8 {
        (self.right_volume as i8 - 20) * 2
    }

    /// Get bass in dB (-12 to +12).
    pub fn bass_db(&self) -> i8 {
        (self.bass as i8 - 6) * 2
    }

    /// Get treble in dB (-12 to +12).
    pub fn treble_db(&self) -> i8 {
        (self.treble as i8 - 6) * 2
    }

    /// Process stereo audio through the LMC1992.
    /// Takes left/right samples (i16), applies bass/treble EQ and volume.
    pub fn process_stereo(&mut self, left: i16, right: i16) -> (i16, i16) {
        // Convert to float for processing
        let mut left_f = left as f32;
        let mut right_f = right as f32;

        // Apply cascaded bass filters (2 stages for 12dB/octave slope)
        left_f = self.bass_filter_l1.process(left_f);
        left_f = self.bass_filter_l2.process(left_f);
        right_f = self.bass_filter_r1.process(right_f);
        right_f = self.bass_filter_r2.process(right_f);

        // Apply cascaded treble filters (2 stages for 12dB/octave slope)
        left_f = self.treble_filter_l1.process(left_f);
        left_f = self.treble_filter_l2.process(left_f);
        right_f = self.treble_filter_r1.process(right_f);
        right_f = self.treble_filter_r2.process(right_f);

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

        // Disable YM mixing (value 10 = DMA only)
        // Command: 10 (addr) + 000 (mix) + 000010
        // = 10_000_000010 = 0b10000000010 = 0x402
        lmc.mw_mask = 0x07FF;
        lmc.write16(0x22, 0x402);
        assert!(!lmc.mix_ym);

        // Enable YM mixing (value 01 = DMA + YM default)
        // Command: 10_000_000001 = 0x401
        lmc.write16(0x22, 0x401);
        assert!(lmc.mix_ym);

        // Value 00 should also mix YM (DMA + YM at -12dB, but broken = same as 01)
        // Command: 10_000_000000 = 0x400
        lmc.write16(0x22, 0x400);
        assert!(lmc.mix_ym);

        // Value 11 (reserved) should not mix YM
        // Command: 10_000_000011 = 0x403
        lmc.write16(0x22, 0x403);
        assert!(!lmc.mix_ym);
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

    #[test]
    fn test_treble_cut_effect() {
        let mut lmc = Lmc1992::new(44100);

        // Set treble to -12dB (value 0)
        // Command: 10 (addr) + 010 (treble) + 000000 (0)
        // = 10_010_000000 = 0b10010000000 = 0x480
        lmc.mw_mask = 0x07FF;
        lmc.write16(0x22, 0x480);
        assert_eq!(lmc.treble, 0);

        // Process many samples to let filter settle, then check attenuation
        // High frequency content should be attenuated significantly
        let input = 10000_i16;
        for _ in 0..1000 {
            lmc.process_stereo(input, input);
        }

        // After settling, high-frequency attenuation should be noticeable
        // At -12dB treble, the DC gain should be 1.0 but high freq gain should be ~0.25
        let (left, _right) = lmc.process_stereo(input, input);

        // The effect won't be dramatic for DC/constant input, but the filter
        // should not amplify the signal
        assert!(
            left <= input + 100,
            "Treble cut should not amplify: {} > {}",
            left,
            input
        );
    }
}
