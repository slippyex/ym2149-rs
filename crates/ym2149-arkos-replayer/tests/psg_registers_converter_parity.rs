#![cfg(feature = "extended-tests")]

use ym2149_arkos_replayer::format::{ChannelLink, InstrumentCell};
use ym2149_arkos_replayer::psg_registers::{
    DEFAULT_HARDWARE_ENVELOPE, HARDWARE_VOLUME_VALUE, PsgRegisters,
};
use ym2149_arkos_replayer::psg_registers_converter::PsgRegistersConverter;

const DEFAULT_RATIO: u8 = 4;

fn build_no_soft_no_hard(volume: u8, noise: u8) -> InstrumentCell {
    InstrumentCell {
        volume,
        noise,
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
    }
}

fn build_soft_cell(
    volume: u8,
    noise: u8,
    arpeggio_note: i32,
    arpeggio_octave: i32,
    pitch: i32,
) -> InstrumentCell {
    InstrumentCell {
        volume,
        noise,
        link: ChannelLink::SoftwareOnly,
        primary_period: 0,
        primary_arpeggio_note_in_octave: arpeggio_note as u8,
        primary_arpeggio_octave: arpeggio_octave as i8,
        primary_pitch: pitch as i16,
        ratio: DEFAULT_RATIO,
        hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
        secondary_period: 0,
        secondary_arpeggio_note_in_octave: 0,
        secondary_arpeggio_octave: 0,
        secondary_pitch: 0,
        is_retrig: false,
    }
}

fn build_soft_cell_forced(volume: u8, noise: u8, period: i16) -> InstrumentCell {
    InstrumentCell {
        primary_period: period,
        ..build_soft_cell(volume, noise, 0, 0, 0)
    }
}

fn build_hard_only(
    noise: u8,
    arpeggio_note: i32,
    arpeggio_octave: i32,
    pitch: i32,
    envelope: u8,
    retrig: bool,
    hardware_period: i16,
) -> InstrumentCell {
    InstrumentCell {
        volume: 0,
        noise,
        link: ChannelLink::HardwareOnly,
        primary_period: hardware_period,
        primary_arpeggio_note_in_octave: arpeggio_note as u8,
        primary_arpeggio_octave: arpeggio_octave as i8,
        primary_pitch: pitch as i16,
        ratio: DEFAULT_RATIO,
        hardware_envelope: envelope,
        secondary_period: 0,
        secondary_arpeggio_note_in_octave: 0,
        secondary_arpeggio_octave: 0,
        secondary_pitch: 0,
        is_retrig: retrig,
    }
}

fn build_soft_to_hard(
    noise: u8,
    arpeggio_note: i32,
    arpeggio_octave: i32,
    pitch: i32,
    ratio: u8,
    envelope: u8,
    retrig: bool,
) -> InstrumentCell {
    InstrumentCell {
        volume: 0,
        noise,
        link: ChannelLink::SoftwareToHardware,
        primary_period: 0,
        primary_arpeggio_note_in_octave: arpeggio_note as u8,
        primary_arpeggio_octave: arpeggio_octave as i8,
        primary_pitch: pitch as i16,
        ratio,
        hardware_envelope: envelope,
        secondary_period: 0,
        secondary_arpeggio_note_in_octave: 0,
        secondary_arpeggio_octave: 0,
        secondary_pitch: 0,
        is_retrig: retrig,
    }
}

fn build_hard_to_soft(
    noise: u8,
    arpeggio_note: i32,
    arpeggio_octave: i32,
    pitch: i32,
    ratio: u8,
    envelope: u8,
    retrig: bool,
) -> InstrumentCell {
    InstrumentCell {
        link: ChannelLink::HardwareToSoftware,
        ..build_soft_to_hard(
            noise,
            arpeggio_note,
            arpeggio_octave,
            pitch,
            ratio,
            envelope,
            retrig,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn build_soft_and_hard(
    noise: u8,
    soft_note: i32,
    soft_octave: i32,
    soft_pitch: i32,
    ratio: u8,
    hardware_period: i16,
    hardware_note: i32,
    hardware_octave: i32,
    hardware_pitch: i32,
    envelope: u8,
    retrig: bool,
) -> InstrumentCell {
    InstrumentCell {
        volume: 0,
        noise,
        link: ChannelLink::SoftwareAndHardware,
        primary_period: 0,
        primary_arpeggio_note_in_octave: soft_note as u8,
        primary_arpeggio_octave: soft_octave as i8,
        primary_pitch: soft_pitch as i16,
        ratio,
        hardware_envelope: envelope,
        secondary_period: hardware_period,
        secondary_arpeggio_note_in_octave: hardware_note as u8,
        secondary_arpeggio_octave: hardware_octave as i8,
        secondary_pitch: hardware_pitch as i16,
        is_retrig: retrig,
    }
}

fn assert_cell(actual: &InstrumentCell, expected: &InstrumentCell) {
    assert_eq!(actual.link, expected.link, "link mismatch");
    assert_eq!(actual.volume, expected.volume, "volume mismatch");
    assert_eq!(actual.noise, expected.noise, "noise mismatch");
    assert_eq!(
        actual.primary_period, expected.primary_period,
        "primary period mismatch"
    );
    assert_eq!(
        actual.primary_arpeggio_note_in_octave, expected.primary_arpeggio_note_in_octave,
        "primary arp note mismatch"
    );
    assert_eq!(
        actual.primary_arpeggio_octave, expected.primary_arpeggio_octave,
        "primary arp octave mismatch"
    );
    assert_eq!(
        actual.primary_pitch, expected.primary_pitch,
        "primary pitch mismatch"
    );
    assert_eq!(actual.ratio, expected.ratio, "ratio mismatch");
    assert_eq!(
        actual.secondary_period, expected.secondary_period,
        "secondary period mismatch"
    );
    assert_eq!(
        actual.secondary_arpeggio_note_in_octave, expected.secondary_arpeggio_note_in_octave,
        "secondary arp note mismatch"
    );
    assert_eq!(
        actual.secondary_arpeggio_octave, expected.secondary_arpeggio_octave,
        "secondary arp octave mismatch"
    );
    assert_eq!(
        actual.secondary_pitch, expected.secondary_pitch,
        "secondary pitch mismatch"
    );
    assert_eq!(
        actual.hardware_envelope, expected.hardware_envelope,
        "hardware envelope mismatch"
    );
    assert_eq!(actual.is_retrig, expected.is_retrig, "retrig mismatch");
}

#[test]
fn encode_as_notes_no_soft_no_hard() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();

        let mut registers = PsgRegisters::new();
        register_list.push(registers.clone());

        registers = PsgRegisters::new();
        registers.set_volume(channel_index, 1);
        registers.set_noise(1);
        registers.set_mixer_noise_state(channel_index, true);
        register_list.push(registers.clone());

        registers.set_volume(channel_index, 15);
        registers.set_noise(31);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        assert_cell(&cells[0], &build_no_soft_no_hard(0, 0));
        assert_cell(&cells[1], &build_no_soft_no_hard(1, 1));
        assert_cell(&cells[2], &build_no_soft_no_hard(15, 31));
    }
}

#[test]
fn encode_as_notes_soft_only() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();

        let mut registers = PsgRegisters::new();
        registers.set_software_period(channel_index, 1804);
        registers.set_volume(channel_index, 10);
        registers.set_mixer_sound_state(channel_index, true);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_software_period(channel_index, 1703);
        registers.set_volume(channel_index, 15);
        registers.set_mixer_sound_state(channel_index, true);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_software_period(channel_index, 3608);
        registers.set_volume(channel_index, 1);
        registers.set_noise(18);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_software_period(channel_index, 1805);
        registers.set_volume(channel_index, 2);
        registers.set_noise(31);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_software_period(channel_index, 1911 - 10);
        registers.set_volume(channel_index, 14);
        registers.set_mixer_sound_state(channel_index, true);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        let expected = [
            build_soft_cell(10, 0, 0, 0, 0),
            build_soft_cell(15, 0, 1, 0, 0),
            build_soft_cell(1, 18, 0, -1, 0),
            build_soft_cell(2, 31, 0, 0, -1),
            build_soft_cell(14, 0, 11, -1, 10),
        ];

        for (idx, expected_cell) in expected.iter().enumerate() {
            assert_cell(&cells[idx], expected_cell);
        }
    }
}

#[test]
fn encode_as_notes_hard_only() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(1804);
        registers.set_hardware_envelope_and_retrig(8, false);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(1607);
        registers.set_hardware_envelope_and_retrig(10, false);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(31);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(1607);
        registers.set_hardware_envelope_and_retrig(12, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(1);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(2025 + 5);
        registers.set_hardware_envelope_and_retrig(12, true);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(2025 - 10);
        registers.set_hardware_envelope_and_retrig(15, false);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(17);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        assert_cell(&cells[0], &build_hard_only(0, 0, 0, 0, 8, false, 0));
        assert_cell(&cells[1], &build_hard_only(31, 2, 0, 0, 10, false, 0));
        assert_cell(&cells[2], &build_hard_only(1, 2, 0, 0, 12, true, 0));
        assert_cell(&cells[3], &build_hard_only(0, 10, -1, -5, 12, true, 0));
        assert_cell(&cells[4], &build_hard_only(17, 10, -1, 10, 15, false, 0));
    }
}

#[test]
fn encode_as_notes_soft_to_hard() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_software_period(channel_index, 956);
        registers.set_hardware_period(60);
        registers.set_hardware_envelope_and_retrig(8, false);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(12);
        registers.set_software_period(channel_index, 957);
        registers.set_hardware_period(60);
        registers.set_hardware_envelope_and_retrig(8, false);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        assert_cell(
            &cells[0],
            &build_soft_to_hard(0, 0, 0, 0, DEFAULT_RATIO, 8, false),
        );
        assert_cell(
            &cells[1],
            &build_soft_to_hard(12, 0, 0, -1, DEFAULT_RATIO, 8, false),
        );
    }
}

#[test]
fn encode_as_notes_soft_and_hard_ben_daglish() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_software_period(channel_index, 956);
        registers.set_hardware_period(4);
        registers.set_hardware_envelope_and_retrig(10, false);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);
        let expected = build_soft_and_hard(0, 0, 0, 0, 0, 4, 0, 0, 0, 10, false);
        assert_cell(&cells[0], &expected);
    }
}

#[test]
fn encode_as_notes_soft_and_hard() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_software_period(channel_index, 956);
        registers.set_hardware_period(716);
        registers.set_hardware_envelope_and_retrig(15, false);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        let expected = build_soft_and_hard(0, 0, 0, 0, 0, 0, 29 - 24, 0, 0, 15, false);

        assert_cell(&cells[0], &expected);
    }
}

#[test]
fn encode_as_notes_hard_to_soft() {
    for channel_index in 0..3 {
        let mut register_list = Vec::new();

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_hardware_period(60);
        registers.set_software_period(channel_index, 60 * 16);
        registers.set_hardware_envelope_and_retrig(12, false);
        register_list.push(registers);

        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_hardware_period(59);
        registers.set_software_period(channel_index, 59 * 16);
        registers.set_hardware_envelope_and_retrig(12, true);
        register_list.push(registers);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        assert_cell(
            &cells[0],
            &build_hard_to_soft(0, 0, 0, 0, DEFAULT_RATIO, 12, false),
        );
        assert_cell(
            &cells[1],
            &build_hard_to_soft(0, 0, 0, 1, DEFAULT_RATIO, 12, true),
        );
    }
}

#[test]
fn encode_as_notes_soft_only_high_pitch() {
    let periods = [0xef, 0xe7, 0xdf, 0xd7, 0xcf, 0xc7, 0xbf];
    for channel_index in 0..3 {
        let mut register_list = Vec::new();
        for &period in &periods {
            let mut registers = PsgRegisters::new();
            registers.set_software_period(channel_index, period);
            registers.set_volume(channel_index, 15);
            registers.set_mixer_sound_state(channel_index, true);
            register_list.push(registers);
        }

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, false);

        assert_cell(&cells[0], &build_soft_cell(15, 0, 0, 0, 0));
        assert_cell(&cells[1], &build_soft_cell(15, 0, 1, 0, -6));
        assert_cell(&cells[2], &build_soft_cell(15, 0, 1, 0, 2));
        assert_cell(&cells[3], &build_soft_cell(15, 0, 2, 0, -2));
        assert_cell(&cells[4], &build_soft_cell(15, 0, 2, 0, 6));
        assert_cell(&cells[5], &build_soft_cell(15, 0, 3, 0, 2));
        assert_cell(&cells[6], &build_soft_cell(15, 0, 4, 0, -1));
    }
}

#[test]
fn encode_as_forced_periods_no_soft_no_hard() {
    for channel_index in 0..3 {
        let register_list = vec![PsgRegisters::new()];
        let mut converter = PsgRegistersConverter::new(1_000_000, 2_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&register_list, channel_index, true);
        assert_cell(&cells[0], &build_no_soft_no_hard(0, 0));
    }
}

#[test]
fn encode_as_forced_periods_no_soft_no_hard_with_noise() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_volume(channel_index, 15);
        registers.set_noise(31);
        registers.set_software_period(channel_index, 1000);

        let mut converter = PsgRegistersConverter::new(1_000_000, 2_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_no_soft_no_hard(15, 31));
    }
}

#[test]
fn encode_as_forced_periods_soft_only() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_volume(channel_index, 10);
        registers.set_software_period(channel_index, 100);

        let mut converter = PsgRegistersConverter::new(1_000_000, 2_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_soft_cell_forced(10, 0, 200));
    }
}

#[test]
fn encode_as_forced_periods_soft_only_with_noise_downscale() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_volume(channel_index, 15);
        registers.set_noise(5);
        registers.set_software_period(channel_index, 1000);

        let mut converter = PsgRegistersConverter::new(2_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_soft_cell_forced(15, 2, 500));
    }
}

#[test]
fn encode_as_forced_periods_soft_only_with_noise_identity() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_volume(channel_index, 15);
        registers.set_noise(5);
        registers.set_software_period(channel_index, 1000);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_soft_cell_forced(15, 5, 1000));
    }
}

#[test]
fn encode_as_forced_periods_hard_only() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_hardware_period(500);
        registers.set_hardware_envelope_and_retrig(10, false);

        let mut converter = PsgRegistersConverter::new(1_000_000, 2_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_hard_only(0, 0, 0, 0, 10, false, 1000));
    }
}

#[test]
fn encode_as_forced_periods_hard_only_with_noise() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(31);
        registers.set_hardware_period(500);
        registers.set_hardware_envelope_and_retrig(14, true);

        let mut converter = PsgRegistersConverter::new(2_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        assert_cell(&cells[0], &build_hard_only(15, 0, 0, 0, 14, true, 250));
    }
}

#[test]
fn encode_as_forced_periods_soft_and_hard() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_software_period(channel_index, 500);
        registers.set_hardware_period(500);
        registers.set_hardware_envelope_and_retrig(8, false);

        let mut converter = PsgRegistersConverter::new(2_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        let mut expected = build_soft_and_hard(0, 0, 0, 0, 0, 250, 0, 0, 0, 8, false);
        expected.primary_period = 250;
        assert_cell(&cells[0], &expected);
    }
}

#[test]
fn encode_as_forced_periods_soft_and_hard_with_noise() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(31);
        registers.set_software_period(channel_index, 500);
        registers.set_hardware_period(500);
        registers.set_hardware_envelope_and_retrig(9, true);

        let mut converter = PsgRegistersConverter::new(1_000_000, 2_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        let mut expected = build_soft_and_hard(31, 0, 0, 0, 0, 1000, 0, 0, 0, 9, true);
        expected.primary_period = 1000;
        assert_cell(&cells[0], &expected);
    }
}

#[test]
fn encode_as_forced_periods_soft_and_hard_with_noise_identity() {
    for channel_index in 0..3 {
        let mut registers = PsgRegisters::new();
        registers.set_volume(channel_index, HARDWARE_VOLUME_VALUE);
        registers.set_mixer_sound_state(channel_index, true);
        registers.set_mixer_noise_state(channel_index, true);
        registers.set_noise(15);
        registers.set_software_period(channel_index, 500);
        registers.set_hardware_period(3000);
        registers.set_hardware_envelope_and_retrig(13, false);

        let mut converter = PsgRegistersConverter::new(1_000_000, 1_000_000, 440.0);
        let cells = converter.encode_as_instrument_cells(&[registers], channel_index, true);
        let mut expected = build_soft_and_hard(15, 0, 0, 0, 0, 3000, 0, 0, 0, 13, false);
        expected.primary_period = 500;
        assert_cell(&cells[0], &expected);
    }
}
