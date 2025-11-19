//! Convert PSG register streams back into Arkos instrument cells (parity with AT3)
#![allow(missing_docs)]

use crate::format::{ChannelLink, InstrumentCell};
use crate::psg::{AbsoluteNote, PsgPeriodTable, split_note};
use crate::psg_registers::{
    ChannelOutputRegisters, DEFAULT_HARDWARE_ENVELOPE, MAX_HARDWARE_PERIOD, MAX_NOISE,
    MAX_SOFTWARE_PERIOD, PsgRegisters, SoundType,
};

const DEFAULT_RATIO: u8 = 4;
const THRESHOLD_BEN_DAGLISH_EFFECT: u16 = 10;
const RATIO_SHIFT_TOLERANCE: i32 = 2;

#[derive(Clone)]
struct NoteShift {
    absolute_note: i32,
    relative_note: i32,
    shift: i32,
}

impl NoteShift {
    fn note_in_octave_and_octave(&self) -> (i32, i32) {
        split_note(self.relative_note)
    }

    fn shift(&self) -> i32 {
        self.shift
    }

    fn absolute_note(&self) -> i32 {
        self.absolute_note
    }
}

/// Converts raw PSG register streams into Arkos instrument cells (mirrors AT3 logic)
pub struct PsgRegistersConverter {
    source_psg_frequency_hz: f32,
    target_psg_frequency_hz: f32,
    period_table: PsgPeriodTable,
    current_soft_base_note: Option<i32>,
    current_hard_base_note: Option<i32>,
}

impl PsgRegistersConverter {
    /// Create a converter for a given source/target PSG pair
    pub fn new(
        source_psg_frequency_hz: u32,
        target_psg_frequency_hz: u32,
        reference_frequency_hz: f32,
    ) -> Self {
        Self {
            source_psg_frequency_hz: source_psg_frequency_hz as f32,
            target_psg_frequency_hz: target_psg_frequency_hz as f32,
            period_table: PsgPeriodTable::new(
                target_psg_frequency_hz as f64,
                reference_frequency_hz as f64,
            ),
            current_soft_base_note: None,
            current_hard_base_note: None,
        }
    }

    /// Convert a register list into a sequence of instrument cells (notes or forced periods)
    pub fn encode_as_instrument_cells(
        &mut self,
        register_list: &[PsgRegisters],
        channel_index: usize,
        encode_as_forced_periods: bool,
    ) -> Vec<InstrumentCell> {
        self.current_soft_base_note = None;
        self.current_hard_base_note = None;

        register_list
            .iter()
            .map(|registers| {
                let mut corrected = registers.clone();
                corrected.correct();
                let converted = self.convert_periods(&corrected);
                let mut converted = converted;

                if converted.get_software_period(channel_index) == 0 {
                    converted.set_software_period(channel_index, 1);
                }
                if converted.get_mixer_noise_state(channel_index) && converted.get_noise() == 0 {
                    converted.set_noise(1);
                }

                let channel_registers =
                    ChannelOutputRegisters::from_registers(channel_index, &converted);
                if encode_as_forced_periods {
                    self.encode_as_cell_force_periods(&channel_registers)
                } else {
                    self.encode_as_cell_notes(&channel_registers)
                }
            })
            .collect()
    }

    fn convert_periods(&self, registers: &PsgRegisters) -> PsgRegisters {
        if (self.source_psg_frequency_hz - self.target_psg_frequency_hz).abs() < f32::EPSILON {
            return registers.clone();
        }

        let mut output = registers.clone();
        for channel in 0..crate::psg_registers::CHANNEL_COUNT {
            let period = registers.get_software_period(channel);
            let converted = self.convert_period(period as u32, MAX_SOFTWARE_PERIOD as u32);
            output.set_software_period(channel, converted as u16);
        }

        let hardware_period = registers.get_hardware_period();
        output.set_hardware_period(
            self.convert_period(hardware_period as u32, MAX_HARDWARE_PERIOD as u32) as u16,
        );

        let noise = registers.get_noise();
        let converted_noise = self.convert_period(noise as u32, MAX_NOISE as u32) as u8;
        output.set_noise(converted_noise);

        output
    }

    fn convert_period(&self, period: u32, maximum: u32) -> u32 {
        if period == 0 {
            return 0;
        }
        if (self.source_psg_frequency_hz - self.target_psg_frequency_hz).abs() < f32::EPSILON {
            return period.min(maximum);
        }
        let ratio = self.target_psg_frequency_hz / self.source_psg_frequency_hz;
        let converted = (period as f32 * ratio) as i32;
        converted.clamp(1, maximum as i32) as u32
    }

    fn encode_as_cell_force_periods(&self, registers: &ChannelOutputRegisters) -> InstrumentCell {
        match registers.sound_type() {
            SoundType::NoSoftwareNoHardware => InstrumentCell {
                volume: registers.volume().min(15),
                noise: registers.noise(),
                link: ChannelLink::NoSoftwareNoHardware,
                primary_period: 0,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                ratio: DEFAULT_RATIO,
                hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
                secondary_period: 0,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: false,
            },
            SoundType::SoftwareOnly => InstrumentCell {
                volume: registers.volume().min(15),
                noise: registers.noise(),
                link: ChannelLink::SoftwareOnly,
                primary_period: registers.software_period() as i16,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                ratio: DEFAULT_RATIO,
                hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
                secondary_period: 0,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: false,
            },
            SoundType::HardwareOnly => InstrumentCell {
                volume: 0,
                noise: registers.noise(),
                link: ChannelLink::HardwareOnly,
                primary_period: registers.hardware_period() as i16,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                ratio: DEFAULT_RATIO,
                hardware_envelope: registers.hardware_envelope(),
                secondary_period: 0,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: registers.retrig(),
            },
            SoundType::SoftwareAndHardware => InstrumentCell {
                volume: 0,
                noise: registers.noise(),
                link: ChannelLink::SoftwareAndHardware,
                primary_period: registers.software_period() as i16,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                ratio: 0,
                hardware_envelope: registers.hardware_envelope(),
                secondary_period: registers.hardware_period() as i16,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: registers.retrig(),
            },
        }
    }

    fn encode_as_cell_notes(&mut self, registers: &ChannelOutputRegisters) -> InstrumentCell {
        match registers.sound_type() {
            SoundType::NoSoftwareNoHardware => InstrumentCell {
                volume: registers.volume().min(15),
                noise: registers.noise(),
                link: ChannelLink::NoSoftwareNoHardware,
                primary_period: 0,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                ratio: DEFAULT_RATIO,
                hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
                secondary_period: 0,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: false,
            },
            SoundType::SoftwareOnly => {
                let absolute = self
                    .period_table
                    .find_note_and_shift(registers.software_period() as i32);
                let note_shift = self.find_note_and_shift_to_encode(absolute, true);
                let (note_in_octave, octave) = note_shift.note_in_octave_and_octave();
                InstrumentCell {
                    volume: registers.volume().min(15),
                    noise: registers.noise(),
                    link: ChannelLink::SoftwareOnly,
                    primary_period: 0,
                    primary_arpeggio_note_in_octave: note_in_octave as u8,
                    primary_arpeggio_octave: octave as i8,
                    primary_pitch: note_shift.shift() as i16,
                    ratio: DEFAULT_RATIO,
                    hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
                    secondary_period: 0,
                    secondary_arpeggio_note_in_octave: 0,
                    secondary_arpeggio_octave: 0,
                    secondary_pitch: 0,
                    is_retrig: false,
                }
            }
            SoundType::HardwareOnly => {
                let absolute = self
                    .period_table
                    .find_note_and_shift(registers.hardware_period() as i32);
                let note_shift = self.find_note_and_shift_to_encode(absolute, false);
                let (note_in_octave, octave) = note_shift.note_in_octave_and_octave();
                InstrumentCell {
                    volume: 0,
                    noise: registers.noise(),
                    link: ChannelLink::HardwareOnly,
                    primary_period: 0,
                    primary_arpeggio_note_in_octave: note_in_octave as u8,
                    primary_arpeggio_octave: octave as i8,
                    primary_pitch: note_shift.shift() as i16,
                    ratio: DEFAULT_RATIO,
                    hardware_envelope: registers.hardware_envelope(),
                    secondary_period: 0,
                    secondary_arpeggio_note_in_octave: 0,
                    secondary_arpeggio_octave: 0,
                    secondary_pitch: 0,
                    is_retrig: registers.retrig(),
                }
            }
            SoundType::SoftwareAndHardware => self.generate_soft_and_hard_cell(registers),
        }
    }

    fn generate_soft_and_hard_cell(
        &mut self,
        registers: &ChannelOutputRegisters,
    ) -> InstrumentCell {
        let software_abs = self
            .period_table
            .find_note_and_shift(registers.software_period() as i32);
        let hardware_abs = self
            .period_table
            .find_note_and_shift(registers.hardware_period() as i32);
        let software_note = self.find_note_and_shift_to_encode(software_abs, true);
        let hardware_note = self.find_note_and_shift_to_encode(hardware_abs, false);

        let (link, ratio) = self.find_link(registers, &software_note, &hardware_note);
        let noise = registers.noise();
        let hardware_envelope = registers.hardware_envelope();
        let retrig = registers.retrig();

        match link {
            ChannelLink::SoftwareToHardware => {
                let (note_in_octave, octave) = software_note.note_in_octave_and_octave();
                InstrumentCell {
                    volume: 0,
                    noise,
                    link,
                    primary_period: 0,
                    primary_arpeggio_note_in_octave: note_in_octave as u8,
                    primary_arpeggio_octave: octave as i8,
                    primary_pitch: software_note.shift() as i16,
                    ratio,
                    hardware_envelope,
                    secondary_period: 0,
                    secondary_arpeggio_note_in_octave: 0,
                    secondary_arpeggio_octave: 0,
                    secondary_pitch: 0,
                    is_retrig: retrig,
                }
            }
            ChannelLink::HardwareToSoftware => {
                let (note_in_octave, octave) = hardware_note.note_in_octave_and_octave();
                InstrumentCell {
                    volume: 0,
                    noise,
                    link,
                    primary_period: 0,
                    primary_arpeggio_note_in_octave: note_in_octave as u8,
                    primary_arpeggio_octave: octave as i8,
                    primary_pitch: hardware_note.shift() as i16,
                    ratio,
                    hardware_envelope,
                    secondary_period: 0,
                    secondary_arpeggio_note_in_octave: 0,
                    secondary_arpeggio_octave: 0,
                    secondary_pitch: 0,
                    is_retrig: retrig,
                }
            }
            ChannelLink::SoftwareAndHardware => {
                if registers.hardware_period() < THRESHOLD_BEN_DAGLISH_EFFECT {
                    let (note_in_octave, octave) = software_note.note_in_octave_and_octave();
                    return InstrumentCell {
                        volume: 0,
                        noise,
                        link,
                        primary_period: 0,
                        primary_arpeggio_note_in_octave: note_in_octave as u8,
                        primary_arpeggio_octave: octave as i8,
                        primary_pitch: software_note.shift() as i16,
                        ratio: 0,
                        hardware_envelope,
                        secondary_period: registers.hardware_period() as i16,
                        secondary_arpeggio_note_in_octave: 0,
                        secondary_arpeggio_octave: 0,
                        secondary_pitch: 0,
                        is_retrig: retrig,
                    };
                }

                let hardware_arpeggio =
                    hardware_note.absolute_note() - software_note.absolute_note();
                let (hw_note_in_octave, hw_octave) = split_note(hardware_arpeggio);
                let (sw_note_in_octave, sw_octave) = software_note.note_in_octave_and_octave();

                InstrumentCell {
                    volume: 0,
                    noise,
                    link,
                    primary_period: 0,
                    primary_arpeggio_note_in_octave: sw_note_in_octave as u8,
                    primary_arpeggio_octave: sw_octave as i8,
                    primary_pitch: software_note.shift() as i16,
                    ratio: 0,
                    hardware_envelope,
                    secondary_period: 0,
                    secondary_arpeggio_note_in_octave: hw_note_in_octave as u8,
                    secondary_arpeggio_octave: hw_octave as i8,
                    secondary_pitch: hardware_note.shift() as i16,
                    is_retrig: retrig,
                }
            }
            _ => unreachable!(),
        }
    }

    fn find_link(
        &self,
        registers: &ChannelOutputRegisters,
        software_note: &NoteShift,
        _hardware_note: &NoteShift,
    ) -> (ChannelLink, u8) {
        let (soft_ratio, soft_shift) = self.find_ratio_and_shift_soft_to_hard(
            registers.software_period(),
            registers.hardware_period(),
        );
        let (hard_ratio, hard_shift) = self.find_ratio_and_shift_hard_to_soft(
            registers.software_period(),
            registers.hardware_period(),
        );

        let software_perfect = software_note.shift() == 0;

        if soft_ratio < 0 && hard_ratio < 0 {
            return (ChannelLink::SoftwareAndHardware, DEFAULT_RATIO);
        }

        let mut soft_preferred = soft_ratio >= 0;
        let mut hard_preferred = hard_ratio >= 0;

        if soft_preferred && hard_preferred {
            if software_perfect {
                hard_preferred = false;
            } else {
                soft_preferred = false;
            }
        }

        if soft_preferred {
            return (ChannelLink::SoftwareToHardware, soft_ratio as u8);
        }
        if hard_preferred {
            return (ChannelLink::HardwareToSoftware, hard_ratio as u8);
        }

        if soft_shift.abs() <= hard_shift.abs() {
            (ChannelLink::SoftwareToHardware, soft_ratio.max(0) as u8)
        } else {
            (ChannelLink::HardwareToSoftware, hard_ratio.max(0) as u8)
        }
    }

    fn find_ratio_and_shift_soft_to_hard(
        &self,
        software_period: u16,
        hardware_period: u16,
    ) -> (i32, i32) {
        if software_period < hardware_period {
            return (-1, 0);
        }
        let mut current = software_period as i32;
        for ratio in 0..8 {
            let shift = current - hardware_period as i32;
            if shift.abs() <= RATIO_SHIFT_TOLERANCE {
                return (ratio, shift);
            }
            current /= 2;
        }
        (-1, 0)
    }

    fn find_ratio_and_shift_hard_to_soft(
        &self,
        software_period: u16,
        hardware_period: u16,
    ) -> (i32, i32) {
        if software_period < hardware_period {
            return (-1, 0);
        }
        let mut current = hardware_period as i32;
        for ratio in 0..8 {
            let shift = current - software_period as i32;
            if shift.abs() <= RATIO_SHIFT_TOLERANCE {
                return (ratio, shift);
            }
            current *= 2;
        }
        (-1, 0)
    }

    fn find_note_and_shift_to_encode(
        &mut self,
        absolute: AbsoluteNote,
        software: bool,
    ) -> NoteShift {
        let base = if software {
            &mut self.current_soft_base_note
        } else {
            &mut self.current_hard_base_note
        };
        if base.is_none() {
            *base = Some(absolute.note_index);
        }
        let relative_note = absolute.note_index - base.unwrap();
        NoteShift {
            absolute_note: absolute.note_index,
            relative_note,
            shift: absolute.shift,
        }
    }
}
