//! YM6 Special Effects Decoder
//!
//! Decodes MFP timer-based effects from YM6 register frames.
//!
//! Effects are encoded in the upper nibbles of specific registers:
//! - Effect Slot 1: code in r1[7-4], prescaler in r6[7-5], counter in r14[7-0]
//! - Effect Slot 2: code in r3[7-4], prescaler in r8[7-5], counter in r15[7-0]

/// MFP timer prescaler values
const MFP_PREDIV: [u32; 8] = [0, 4, 10, 16, 50, 64, 100, 200];

/// MFP clock frequency in Hz
pub const MFP_CLOCK: u32 = 2_457_600;

/// Effect command decoded from YM6 frame registers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCommand {
    /// No effect active
    None,

    /// SID Voice (square-wave amplitude modulation)
    /// Creates C64-like sound on Atari ST
    SidStart {
        /// Voice channel index (0=A, 1=B, 2=C)
        voice: u8,
        /// Timer frequency in Hz controlling modulation rate
        freq: u32,
        /// Amplitude level (0-15) for modulation
        volume: u8,
    },

    /// Sinus SID (smooth sinusoidal amplitude modulation)
    /// Rare effect in practice
    SinusSidStart {
        /// Voice channel index (0=A, 1=B, 2=C)
        voice: u8,
        /// Timer frequency in Hz controlling modulation rate
        freq: u32,
        /// Amplitude level (0-15) for modulation
        volume: u8,
    },

    /// DigiDrum (sample playback with timer-controlled pitch)
    /// Requires sample data storage
    DigiDrumStart {
        /// Voice channel index (0=A, 1=B, 2=C)
        voice: u8,
        /// DigiDrum sample number
        drum_num: u8,
        /// Timer frequency in Hz (for pitch control)
        freq: u32,
    },

    /// Sync Buzzer (synchronized tone generation across voices)
    SyncBuzzerStart {
        /// Buzzer frequency in Hz
        freq: u32,
        /// Envelope shape index (0-15)
        env_shape: u8,
    },
}

/// Decodes YM6 special effects from register frames
pub struct Ym6EffectDecoder;

impl Ym6EffectDecoder {
    /// Create a new effect decoder
    pub fn new() -> Self {
        Ym6EffectDecoder
    }

    /// Decode effects from a YM6 frame's 16 registers
    ///
    /// YM6 format stores effects in two independent "slots":
    /// - Slot 1: controlled by r1 (code), r6 (prescaler), r14 (counter)
    /// - Slot 2: controlled by r3 (code), r8 (prescaler), r15 (counter)
    ///
    /// Both slots can have effects simultaneously (e.g., Sync Buzzer + SID Voice).
    pub fn decode_effects(&self, registers: &[u8; 16]) -> [EffectCommand; 2] {
        let effect1 = self.decode_effect_slot(registers[1], registers[6], registers[14], registers);
        let effect2 = self.decode_effect_slot(registers[3], registers[8], registers[15], registers);

        [effect1, effect2]
    }

    /// Decode a single effect slot
    ///
    /// # Effect Code Encoding (bits 7-4 of code register)
    /// ```text
    /// 0000: No effect
    /// 0001: SID Voice A
    /// 0010: SID Voice B
    /// 0011: SID Voice C
    /// 0100: Extended FX Voice A (reserved)
    /// 0101: DigiDrum Voice A
    /// 0110: DigiDrum Voice B
    /// 0111: DigiDrum Voice C
    /// 1000: Extended FX Voice B (reserved)
    /// 1001: Sinus SID Voice A
    /// 1010: Sinus SID Voice B
    /// 1011: Sinus SID Voice C
    /// 1100: Extended FX Voice C (reserved)
    /// 1101: Sync Buzzer Voice A
    /// 1110: Sync Buzzer Voice B
    /// 1111: Sync Buzzer Voice C
    /// ```
    fn decode_effect_slot(
        &self,
        code_reg: u8,
        prediv_reg: u8,
        count_reg: u8,
        registers: &[u8; 16],
    ) -> EffectCommand {
        let effect_code = (code_reg >> 4) & 0x0F;

        // Check for no effect
        if effect_code == 0 {
            return EffectCommand::None;
        }

        // Extract prescaler and counter
        let prediv_idx = ((prediv_reg >> 5) & 0x07) as usize;
        let prediv = MFP_PREDIV[prediv_idx];
        let counter = count_reg;

        // Check if timer is actually configured
        if prediv == 0 || counter == 0 {
            return EffectCommand::None;
        }

        // Calculate timer frequency
        // Safety: MFP_CLOCK / (prediv * counter) where both > 0
        let timer_freq = MFP_CLOCK / (prediv * counter as u32);

        // Decode effect type from code bits 3-2 and voice from bits 1-0
        match effect_code {
            0x1..=0x3 => {
                // SID Voice (effect bits 11)
                let voice = effect_code - 1;
                let volume = registers[8 + voice as usize] & 0x0F;
                EffectCommand::SidStart {
                    voice,
                    freq: timer_freq,
                    volume,
                }
            }

            0x5..=0x7 => {
                // DigiDrum (effect bits 101)
                let voice = effect_code - 5;
                let drum_num = registers[8 + voice as usize] & 0x1F;
                EffectCommand::DigiDrumStart {
                    voice,
                    drum_num,
                    freq: timer_freq,
                }
            }

            0x9..=0xB => {
                // Sinus SID (effect bits 1001-1011)
                let voice = effect_code - 9;
                let volume = registers[8 + voice as usize] & 0x0F;
                EffectCommand::SinusSidStart {
                    voice,
                    freq: timer_freq,
                    volume,
                }
            }

            0xD..=0xF => {
                // Sync Buzzer (effect bits 1101-1111)
                // Note: Sync Buzzer affects all voices, env_shape from r13
                let env_shape = registers[13] & 0x0F;
                EffectCommand::SyncBuzzerStart {
                    freq: timer_freq,
                    env_shape,
                }
            }

            _ => EffectCommand::None,
        }
    }
}

impl Default for Ym6EffectDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode YM5 effects from a frame (SID and DigiDrum only)
///
/// YM5 encodes two 2-bit codes for effects:
/// - R1\[5:4\]: SID voice selector (1=A,2=B,3=C)
///   - Timer prediv from R6\[7:5\], counter from R14
/// - R3\[5:4\]: DigiDrum voice selector (1=A,2=B,3=C)
///   - Drum index from R8+voice low 5 bits
///   - Timer prediv from R8\[7:5\], counter from R15
pub fn decode_effects_ym5(registers: &[u8; 16]) -> Vec<EffectCommand> {
    let mut out = Vec::new();

    // SID
    let sid_code = (registers[1] >> 4) & 0x03; // 1..3 => voices A..C
    if sid_code != 0 {
        let voice = sid_code - 1;
        let prediv_idx = ((registers[6] >> 5) & 0x07) as usize;
        let count = registers[14] as u32;
        let prediv = MFP_PREDIV[prediv_idx];
        if prediv != 0 && count != 0 {
            let freq = MFP_CLOCK / (prediv * count);
            let volume = registers[8 + voice as usize] & 0x0F;
            out.push(EffectCommand::SidStart {
                voice,
                freq,
                volume,
            });
        }
    }

    // DigiDrum
    let drum_code = (registers[3] >> 4) & 0x03; // 1..3 => voices A..C
    if drum_code != 0 {
        let voice = drum_code - 1;
        let drum_num = registers[8 + voice as usize] & 0x1F;
        let prediv_idx = ((registers[8] >> 5) & 0x07) as usize;
        let count = registers[15] as u32;
        let prediv = MFP_PREDIV[prediv_idx];
        if prediv != 0 && count != 0 {
            let freq = MFP_CLOCK / (prediv * count);
            out.push(EffectCommand::DigiDrumStart {
                voice,
                drum_num,
                freq,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_effect_zero_code() {
        let decoder = Ym6EffectDecoder::new();
        let mut registers = [0u8; 16];
        registers[1] = 0x00; // No effect code

        let effects = decoder.decode_effects(&registers);
        assert_eq!(effects[0], EffectCommand::None);
    }

    #[test]
    fn test_no_effect_zero_counter() {
        let decoder = Ym6EffectDecoder::new();
        let mut registers = [0u8; 16];
        registers[1] = 0x10; // SID Voice A code
        registers[14] = 0x00; // Zero counter

        let effects = decoder.decode_effects(&registers);
        assert_eq!(effects[0], EffectCommand::None);
    }

    #[test]
    fn test_sid_voice_a() {
        let decoder = Ym6EffectDecoder::new();
        let mut registers = [0u8; 16];
        registers[1] = 0x10; // SID Voice A
        registers[6] = 0x20; // Prescaler Div4 (idx 1)
        registers[14] = 0x64; // Counter 100
        registers[8] = 0x0F; // Volume 15

        let effects = decoder.decode_effects(&registers);

        if let EffectCommand::SidStart {
            voice,
            freq,
            volume,
        } = effects[0]
        {
            assert_eq!(voice, 0); // Voice A
            assert_eq!(volume, 15);
            // Frequency: 2457600 / (4 * 100) = 6144 Hz
            assert_eq!(freq, 6144);
        } else {
            panic!("Expected SidStart, got {:?}", effects[0]);
        }
    }

    #[test]
    fn test_sync_buzzer() {
        let decoder = Ym6EffectDecoder::new();
        let mut registers = [0u8; 16];
        registers[3] = 0xD0; // Sync Buzzer (slot 2)
        registers[8] = 0x60; // Prescaler Div16 (idx 3, bits 7-5 = 011)
        registers[15] = 0x32; // Counter 50
        registers[13] = 0x08; // Envelope shape 8

        let effects = decoder.decode_effects(&registers);

        if let EffectCommand::SyncBuzzerStart { freq, env_shape } = effects[1] {
            assert_eq!(env_shape, 8);
            // Frequency: 2457600 / (16 * 50) = 3072 Hz
            assert_eq!(freq, 3072);
        } else {
            panic!("Expected SyncBuzzerStart, got {:?}", effects[1]);
        }
    }

    #[test]
    fn test_both_slots_active() {
        let decoder = Ym6EffectDecoder::new();
        let mut registers = [0u8; 16];

        // Slot 1: SID Voice B
        registers[1] = 0x20; // SID Voice B (code = 2)
        registers[6] = 0x60; // Prescaler Div10 (idx 2, bits 7-5 = 011)
        registers[14] = 0x78; // Counter 120
        registers[9] = 0x0A; // Volume B = 10 (r8 + voice, voice=1 → r9)

        // Slot 2: Sync Buzzer
        registers[3] = 0xE0; // Sync Buzzer (code = 14)
        registers[8] = 0xE0; // Prescaler Div200 (idx 7, bits 7-5 = 111)
        registers[15] = 0x40; // Counter 64
        registers[13] = 0x05; // Envelope shape 5

        let effects = decoder.decode_effects(&registers);

        // Slot 1 should be SID Voice B
        if let EffectCommand::SidStart {
            voice,
            freq,
            volume,
        } = effects[0]
        {
            assert_eq!(voice, 1); // Voice B
            assert_eq!(volume, 10);
            // Frequency: 2457600 / (16 * 120) = 1280 Hz (prescaler idx 3)
            assert_eq!(freq, 1280);
        } else {
            panic!("Slot 1: Expected SidStart, got {:?}", effects[0]);
        }

        // Slot 2 should be Sync Buzzer
        if let EffectCommand::SyncBuzzerStart { freq, env_shape } = effects[1] {
            assert_eq!(env_shape, 5);
            // Frequency: 2457600 / (200 * 64) = 191.69 Hz (truncated to 191)
            // prescaler idx 7 = 200
            assert_eq!(freq, MFP_CLOCK / (200 * 64));
        } else {
            panic!("Slot 2: Expected SyncBuzzerStart, got {:?}", effects[1]);
        }
    }
}
