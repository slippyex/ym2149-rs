/// GIST Sound Definition
///
/// Represents a complete sound effect for the GIST (Graphics, Images, Sound, Text)
/// driver on the Atari ST. GIST was developed by Dave Becker and distributed by
/// Antic Software in the late 1980s.
///
/// Each sound is 112 bytes (56 big-endian i16 words) and defines a complex
/// synthesizer patch with:
/// - **ADSR-style envelopes** for volume, frequency, and noise
/// - **LFO modulation** for vibrato, tremolo, and noise effects
/// - **Tone and noise mixing** via the YM2149 PSG chip
///
/// The driver runs at 200 Hz (Timer C on the Atari ST) and processes all
/// three voices on each tick. Values are stored in 16.16 fixed-point format
/// for smooth envelope transitions.
///
/// # Sound Structure
///
/// The sound is organized into three main sections:
/// 1. **Volume control** (words 0-17): Duration, initial values, ADSR envelope, LFO
/// 2. **Frequency control** (words 18-39): Pitch envelope with attack/decay/release + LFO
/// 3. **Noise control** (words 40-55): Noise frequency envelope + LFO
///
/// # Envelope Phases
///
/// Each envelope (volume, frequency, noise) has phases:
/// - Phase 0: No envelope (static value)
/// - Phase 1: Attack (ramp up)
/// - Phase 2: Decay (ramp down to sustain)
/// - Phase 3: Sustain (hold)
/// - Phase 4: Release (fade out)
#[derive(Clone, Copy, Debug, Default)]
pub struct GistSound {
    /// Sound duration in ticks (200 Hz). Also serves as "in use" flag during playback.
    /// When the driver counts this down to 0, the sound ends (unless pitch >= 0).
    pub duration: i16,

    /// Initial tone frequency as YM2149 period value.
    /// Lower values = higher pitch. Set to -1 to disable tone generation.
    /// Valid range: 0-4095 (12-bit). Common values: 478 (A4), 956 (A3).
    pub initial_freq: i16,

    /// Initial noise frequency (0-31 for YM2149 noise generator).
    /// Set to -1 to disable noise. Lower values = higher frequency noise.
    pub initial_noise_freq: i16,

    /// Initial volume level (0-15). Can be overridden by snd_on() volume parameter.
    pub initial_volume: i16,

    /// Volume envelope starting phase (0=none, 1=attack, 2=decay, 3=sustain, 4=release).
    /// Most sounds start at phase 1 (attack) for a natural fade-in.
    pub vol_phase: i16,

    /// Volume attack step (16.16 fixed-point). Added each tick during attack phase.
    /// Positive value; attack ends when accumulator reaches 0x000F0000.
    pub vol_attack: i32,

    /// Volume decay step (16.16 fixed-point). Typically negative.
    /// Added each tick during decay phase until reaching vol_sustain level.
    pub vol_decay: i32,

    /// Volume sustain level (16.16 fixed-point). Target for decay phase.
    /// Sound holds at this level during sustain phase.
    pub vol_sustain: i32,

    /// Volume release step (16.16 fixed-point). Typically negative.
    /// Added each tick during release phase until reaching 0.
    pub vol_release: i32,

    /// Volume LFO amplitude limit (16.16 fixed-point).
    /// LFO oscillates between +limit and -limit. Set to 0 to disable LFO.
    /// Creates tremolo effect when non-zero.
    pub vol_lfo_limit: i32,

    /// Volume LFO step (16.16 fixed-point). Added each tick.
    /// Automatically negated when LFO reaches its limits.
    pub vol_lfo_step: i32,

    /// Volume LFO delay in ticks before LFO starts. Allows clean attack before modulation.
    pub vol_lfo_delay: i16,

    /// Frequency envelope starting phase (0=none, 1=attack, 2=decay, 3=sustain, 4=release).
    pub freq_env_phase: i16,

    /// Frequency attack step (16.16 fixed-point). Can be positive or negative.
    /// Sign determines direction of pitch sweep during attack.
    pub freq_attack: i32,

    /// Frequency attack target (16.16 fixed-point). Attack ends when this value is reached.
    pub freq_attack_target: i32,

    /// Frequency decay step (16.16 fixed-point).
    pub freq_decay: i32,

    /// Frequency decay target (16.16 fixed-point).
    pub freq_decay_target: i32,

    /// Frequency release step (16.16 fixed-point).
    pub freq_release: i32,

    /// Frequency LFO positive limit (16.16 fixed-point).
    /// Creates vibrato effect when non-zero.
    pub freq_lfo_limit: i32,

    /// Frequency LFO step (16.16 fixed-point).
    pub freq_lfo_step: i32,

    /// Frequency LFO reset value when hitting positive limit (handles overflow).
    pub freq_lfo_reset_positive: i32,

    /// Frequency LFO negative limit (16.16 fixed-point).
    pub freq_lfo_negative_limit: i32,

    /// Frequency LFO reset value when hitting negative limit (handles underflow).
    pub freq_lfo_reset_negative: i32,

    /// Frequency LFO delay in ticks before LFO starts.
    pub freq_lfo_delay: i16,

    /// Noise envelope starting phase (0=none, 1=attack, 2=decay, 3=sustain, 4=release).
    pub noise_env_phase: i16,

    /// Noise attack step (16.16 fixed-point).
    pub noise_attack: i32,

    /// Noise attack target (16.16 fixed-point).
    pub noise_attack_target: i32,

    /// Noise decay step (16.16 fixed-point).
    pub noise_decay: i32,

    /// Noise decay target (16.16 fixed-point).
    pub noise_decay_target: i32,

    /// Noise release step (16.16 fixed-point).
    pub noise_release: i32,

    /// Noise LFO limit (16.16 fixed-point). Creates noise frequency modulation.
    pub noise_lfo_limit: i32,

    /// Noise LFO step (16.16 fixed-point).
    pub noise_lfo_step: i32,

    /// Noise LFO delay in ticks before LFO starts.
    pub noise_lfo_delay: i16,
}

/// Create a GistSound from raw 56-word sound data.
///
/// The format is big-endian, matching the original Atari ST GIST driver.
/// Each sound consists of 56 i16 words (112 bytes).
impl From<&[i16; 56]> for GistSound {
    fn from(words: &[i16; 56]) -> Self {
        fn long(words: &[i16; 56], index: usize) -> i32 {
            let hi = words[index] as i32;
            let lo = words[index + 1] as u16 as i32;
            (hi << 16) | lo
        }
        Self {
            duration: words[0],
            initial_freq: words[1],
            initial_noise_freq: words[2],
            initial_volume: words[3],
            vol_phase: words[4],
            vol_attack: long(words, 5),
            vol_decay: long(words, 7),
            vol_sustain: long(words, 9),
            vol_release: long(words, 11),
            vol_lfo_limit: long(words, 13),
            vol_lfo_step: long(words, 15),
            vol_lfo_delay: words[17],
            freq_env_phase: words[18],
            freq_attack: long(words, 19),
            freq_attack_target: long(words, 21),
            freq_decay: long(words, 23),
            freq_decay_target: long(words, 25),
            freq_release: long(words, 27),
            freq_lfo_limit: long(words, 29),
            freq_lfo_step: long(words, 31),
            freq_lfo_reset_positive: long(words, 33),
            freq_lfo_negative_limit: long(words, 35),
            freq_lfo_reset_negative: long(words, 37),
            freq_lfo_delay: words[39],
            noise_env_phase: words[40],
            noise_attack: long(words, 41),
            noise_attack_target: long(words, 43),
            noise_decay: long(words, 45),
            noise_decay_target: long(words, 47),
            noise_release: long(words, 49),
            noise_lfo_limit: long(words, 51),
            noise_lfo_step: long(words, 53),
            noise_lfo_delay: words[55],
        }
    }
}

impl GistSound {
    /// Load a GistSound from a .snd file.
    ///
    /// Reads 112 bytes (56 big-endian i16 words) in the original Atari ST format.
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        if data.len() < 112 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("File too small: {} bytes (expected 112)", data.len()),
            ));
        }
        let mut words = [0i16; 56];
        for i in 0..56 {
            words[i] = i16::from_be_bytes([data[i * 2], data[i * 2 + 1]]);
        }
        Ok(Self::from(&words))
    }

    /// Read a GistSound from a reader.
    ///
    /// Reads 112 bytes (56 big-endian i16 words) in the original Atari ST format.
    pub fn read<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buf = [0u8; 112];
        reader.read_exact(&mut buf)?;

        let mut words = [0i16; 56];
        for i in 0..56 {
            words[i] = i16::from_be_bytes([buf[i * 2], buf[i * 2 + 1]]);
        }
        Ok(Self::from(&words))
    }

    /// Write a GistSound to a writer.
    ///
    /// Writes 112 bytes (56 big-endian i16 words) in the original Atari ST format.
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        #[inline(always)]
        fn write_word<W: std::io::Write>(w: &mut W, val: i16) -> std::io::Result<()> {
            w.write_all(&val.to_be_bytes())
        }

        #[inline(always)]
        fn write_long<W: std::io::Write>(w: &mut W, val: i32) -> std::io::Result<()> {
            w.write_all(&val.to_be_bytes())
        }

        write_word(writer, self.duration)?;
        write_word(writer, self.initial_freq)?;
        write_word(writer, self.initial_noise_freq)?;
        write_word(writer, self.initial_volume)?;
        write_word(writer, self.vol_phase)?;
        write_long(writer, self.vol_attack)?;
        write_long(writer, self.vol_decay)?;
        write_long(writer, self.vol_sustain)?;
        write_long(writer, self.vol_release)?;
        write_long(writer, self.vol_lfo_limit)?;
        write_long(writer, self.vol_lfo_step)?;
        write_word(writer, self.vol_lfo_delay)?;
        write_word(writer, self.freq_env_phase)?;
        write_long(writer, self.freq_attack)?;
        write_long(writer, self.freq_attack_target)?;
        write_long(writer, self.freq_decay)?;
        write_long(writer, self.freq_decay_target)?;
        write_long(writer, self.freq_release)?;
        write_long(writer, self.freq_lfo_limit)?;
        write_long(writer, self.freq_lfo_step)?;
        write_long(writer, self.freq_lfo_reset_positive)?;
        write_long(writer, self.freq_lfo_negative_limit)?;
        write_long(writer, self.freq_lfo_reset_negative)?;
        write_word(writer, self.freq_lfo_delay)?;
        write_word(writer, self.noise_env_phase)?;
        write_long(writer, self.noise_attack)?;
        write_long(writer, self.noise_attack_target)?;
        write_long(writer, self.noise_decay)?;
        write_long(writer, self.noise_decay_target)?;
        write_long(writer, self.noise_release)?;
        write_long(writer, self.noise_lfo_limit)?;
        write_long(writer, self.noise_lfo_step)?;
        write_word(writer, self.noise_lfo_delay)?;

        Ok(())
    }
}
