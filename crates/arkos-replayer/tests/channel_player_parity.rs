use std::collections::HashMap;
use std::sync::Arc;

use arkos_replayer::channel_player::{ChannelPlayer, SampleCommand};
use arkos_replayer::format::{
    AksSong, Arpeggio, Cell, ChannelLink, Effect, Instrument, InstrumentCell, InstrumentType, Note,
    Pattern, Position, PsgConfig, PsgType, Subsong, Track,
};
fn empty_instrument() -> Instrument {
    Instrument {
        name: "Empty".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed: 0,
        is_retrig: false,
        loop_start_index: 0,
        end_index: 0,
        is_looping: true,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume: 0,
            noise: 0,
            primary_period: 0,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::NoSoftwareNoHardware,
            ratio: 0,
            hardware_envelope: 0,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: None,
    }
}

fn soft_only_instrument(volume: u8, speed: u8, looping: bool) -> Instrument {
    Instrument {
        name: "SoftOnly".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed,
        is_retrig: false,
        loop_start_index: 0,
        end_index: 0,
        is_looping: looping,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume,
            noise: 0,
            primary_period: 0,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::SoftwareOnly,
            ratio: 0,
            hardware_envelope: 0,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: None,
    }
}

fn volume_fade_instrument() -> Instrument {
    Instrument {
        name: "VolFade".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed: 0,
        is_retrig: false,
        loop_start_index: 0,
        end_index: 0,
        is_looping: true,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume: 15,
            noise: 0,
            primary_period: 100,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::SoftwareOnly,
            ratio: 0,
            hardware_envelope: 0,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: None,
    }
}

fn decaying_soft_instrument(volumes: &[u8], speed: u8, looping: bool) -> Instrument {
    let mut cells = Vec::new();
    for &vol in volumes {
        cells.push(InstrumentCell {
            volume: vol,
            noise: 0,
            primary_period: 0,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::SoftwareOnly,
            ratio: 0,
            hardware_envelope: 0,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        });
    }

    Instrument {
        name: "Decay".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed,
        is_retrig: false,
        loop_start_index: 0,
        end_index: cells.len().saturating_sub(1),
        is_looping: looping,
        is_sfx_exported: false,
        cells,
        sample: None,
    }
}

fn build_song_with_instruments(instruments: Vec<Instrument>) -> Arc<AksSong> {
    build_song_with_data(instruments, Vec::new())
}

fn build_song_with_data(instruments: Vec<Instrument>, extra_arps: Vec<Arpeggio>) -> Arc<AksSong> {
    let mut tracks = HashMap::new();
    tracks.insert(
        0,
        Track {
            index: 0,
            cells: Vec::new(),
        },
    );

    let subsong = Subsong {
        title: "Test".into(),
        initial_speed: 6,
        end_position: 0,
        loop_start_position: 0,
        replay_frequency_hz: 50.0,
        psgs: vec![PsgConfig {
            psg_type: PsgType::AY,
            psg_frequency: 1_000_000,
            reference_frequency: 440.0,
            sample_player_frequency: 11_025,
            mixing_output: arkos_replayer::format::MixingOutput::ABC,
        }],
        digi_channel: 0,
        highlight_spacing: 4,
        secondary_highlight: 4,
        positions: vec![Position {
            pattern_index: 0,
            height: 64,
            marker_name: String::new(),
            marker_color: 0,
            transpositions: vec![0, 0, 0],
        }],
        patterns: vec![Pattern {
            index: 0,
            track_indexes: vec![0, 0, 0],
            speed_track_index: 0,
            event_track_index: 0,
            color_argb: 0,
        }],
        tracks,
        speed_tracks: HashMap::new(),
        event_tracks: HashMap::new(),
    };

    let mut arpeggios = vec![Arpeggio {
        index: 0,
        name: "Empty".into(),
        values: vec![0],
        speed: 0,
        loop_start: 0,
        end_index: 0,
        shift: 0,
    }];
    arpeggios.extend(extra_arps);

    Arc::new(AksSong {
        metadata: arkos_replayer::format::SongMetadata::default(),
        instruments,
        arpeggios,
        pitch_tables: Vec::new(),
        subsongs: vec![subsong],
    })
}

fn soft_to_hard_instrument(
    ratio: u8,
    hardware_envelope: u8,
    instrument_retrig: bool,
) -> Instrument {
    soft_to_hard_instrument_with_period(ratio, hardware_envelope, instrument_retrig, None)
}

fn soft_to_hard_instrument_with_period(
    ratio: u8,
    hardware_envelope: u8,
    instrument_retrig: bool,
    forced_period: Option<i16>,
) -> Instrument {
    Instrument {
        name: "SoftToHard".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed: 0,
        is_retrig: instrument_retrig,
        loop_start_index: 0,
        end_index: 0,
        is_looping: true,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume: 15,
            noise: 0,
            primary_period: forced_period.unwrap_or(0),
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::SoftwareToHardware,
            ratio,
            hardware_envelope,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: None,
    }
}

fn sample_instrument(frequency_hz: u32, digidrum_note: i32) -> Instrument {
    use arkos_replayer::format::SampleInstrument;
    Instrument {
        name: "Sample".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Digi,
        speed: 0,
        is_retrig: false,
        loop_start_index: 0,
        end_index: 0,
        is_looping: false,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume: 0,
            noise: 0,
            primary_period: 0,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::NoSoftwareNoHardware,
            ratio: 0,
            hardware_envelope: 0,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: 0,
            secondary_arpeggio_octave: 0,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: Some(SampleInstrument {
            frequency_hz,
            amplification_ratio: 1.0,
            original_filename: None,
            loop_start_index: 0,
            end_index: 15,
            is_looping: false,
            data: Arc::new(vec![0.1; 16]),
            digidrum_note,
        }),
    }
}

fn hardware_only_instrument_with_arp(
    ratio: u8,
    hardware_envelope: u8,
    secondary_note_in_octave: u8,
    secondary_octave: i8,
) -> Instrument {
    Instrument {
        name: "HardOnlyArp".into(),
        color_argb: 0,
        instrument_type: InstrumentType::Psg,
        speed: 0,
        is_retrig: false,
        loop_start_index: 0,
        end_index: 0,
        is_looping: true,
        is_sfx_exported: false,
        cells: vec![InstrumentCell {
            volume: 0,
            noise: 0,
            primary_period: 0,
            primary_arpeggio_note_in_octave: 0,
            primary_arpeggio_octave: 0,
            primary_pitch: 0,
            link: ChannelLink::HardwareOnly,
            ratio,
            hardware_envelope,
            secondary_period: 0,
            secondary_arpeggio_note_in_octave: secondary_note_in_octave,
            secondary_arpeggio_octave: secondary_octave,
            secondary_pitch: 0,
            is_retrig: false,
        }],
        sample: None,
    }
}

fn test_cell(note: Note, instrument_index: usize) -> Cell {
    Cell {
        index: 0,
        note,
        instrument: instrument_index,
        instrument_present: true,
        effects: Vec::new(),
    }
}

fn cell_with_effect(note: Note, instrument_index: usize, name: &str, value: i32) -> Cell {
    let mut cell = test_cell(note, instrument_index);
    cell.effects.push(Effect {
        index: 0,
        name: name.into(),
        logical_value: value,
    });
    cell
}

fn effect_only_cell(name: &str, value: i32) -> Cell {
    Cell {
        index: 0,
        note: 255,
        instrument: 0,
        instrument_present: false,
        effects: vec![Effect {
            index: 0,
            name: name.into(),
            logical_value: value,
        }],
    }
}

fn run_single_tick(
    player: &mut ChannelPlayer,
    cell: Option<&Cell>,
    is_first_tick: bool,
    still_within_line: bool,
) -> arkos_replayer::channel_player::ChannelFrame {
    player.play_frame(cell, 0, is_first_tick, still_within_line)
}

fn run_volume_case(
    player: &mut ChannelPlayer,
    speed: usize,
    events: &[(usize, Cell)],
    expected_volumes: &[u8],
) {
    let mut tick = 0;
    let mut event_idx = 0;
    for (iteration, expected_volume) in expected_volumes.iter().enumerate() {
        let cell = if tick == 0 && event_idx < events.len() && events[event_idx].0 == iteration {
            let cell_ref = &events[event_idx].1;
            event_idx += 1;
            Some(cell_ref)
        } else {
            None
        };

        let frame = run_single_tick(player, cell, tick == 0, tick < speed);
        let output = frame.psg;
        assert_eq!(output.volume, *expected_volume);
        tick = (tick + 1) % speed;
    }
}

fn compute_period(psg_frequency: f32, reference_frequency: f32, note: Note) -> u16 {
    if note == 255 {
        return 0;
    }

    const START_OCTAVE: i32 = -3;
    const NOTES_IN_OCTAVE: i32 = 12;
    let octave = (note as i32 / NOTES_IN_OCTAVE) + START_OCTAVE;
    let note_in_octave = (note as i32 % NOTES_IN_OCTAVE) + 1;
    let frequency = (reference_frequency as f64)
        * 2f64.powf((octave as f64) + ((note_in_octave as f64 - 10.0) / 12.0));
    let period = ((psg_frequency as f64 / 8.0) / frequency).round();
    period.max(0.0).min(4095.0) as u16
}

fn expected_hardware_period(period: u16, ratio: u8) -> u16 {
    if ratio == 0 {
        return period;
    }

    let mut value = period as u32;
    let mut remainder = false;
    for _ in 0..ratio {
        remainder = (value & 1) != 0;
        value >>= 1;
    }
    if remainder {
        value += 1;
    }
    value.min(u16::MAX as u32) as u16
}

#[test]
fn channel_player_nothing() {
    let song = build_song_with_instruments(vec![empty_instrument()]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    for idx in 0..30 {
        let frame = run_single_tick(&mut player, None, idx == 0, idx < 6);
        assert!(matches!(frame.sample, SampleCommand::None));
        let output = frame.psg;
        assert_eq!(output.volume, 0);
        assert_eq!(output.noise, 0);
        assert!(!output.sound_open);
        assert_eq!(output.software_period, 0);
        assert_eq!(output.hardware_period, 0);
        assert_eq!(output.hardware_envelope, 0);
        assert!(!output.hardware_retrig);
    }
}

#[test]
fn channel_player_soft_to_hard_ratio4() {
    let instrument = soft_to_hard_instrument(4, 8, false);
    // Instruments vector: index 0 unused placeholder, index 1 = actual instrument.
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let cell = test_cell((4 * 12) as Note, 1);

    for tick in 0..30 {
        let frame = if tick == 0 {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, true)
        };

        assert!(matches!(frame.sample, SampleCommand::None));
        let output = frame.psg;
        assert_eq!(output.volume, 16);
        assert!(output.sound_open);
        assert_eq!(output.noise, 0);
        assert_eq!(output.hardware_envelope, 8);
        assert_eq!(output.software_period, 239);
        assert_eq!(output.hardware_period, 15);
        assert!(!output.hardware_retrig);
    }
}

#[test]
fn channel_player_soft_to_hard_retrig_first_tick_only() {
    let instrument = soft_to_hard_instrument(5, 10, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);
    let cell = test_cell((4 * 12) as Note, 1);

    for tick in 0..10 {
        let frame = if tick == 0 {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, true)
        };

        assert!(matches!(frame.sample, SampleCommand::None));
        let output = frame.psg;
        assert_eq!(output.volume, 16);
        assert!(output.sound_open);
        assert_eq!(output.hardware_envelope, 10);
        assert_eq!(output.software_period, 239);
        assert_eq!(output.hardware_period, 7);
        assert_eq!(output.hardware_retrig, tick == 0);
    }
}

#[test]
fn channel_player_soft_to_hard_pitch_up_effect() {
    let ratio = 4;
    let instrument = soft_to_hard_instrument(ratio, 10, false);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);
    let cell = cell_with_effect((4 * 12) as Note, 1, "pitchUp", 0x100);

    let base_period: u16 = 239;
    let expected_periods: Vec<u16> = (1..=10).map(|i| base_period - i as u16).collect();

    for (tick, expected_soft_period) in expected_periods.iter().enumerate() {
        let frame = if tick == 0 {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, true)
        };

        assert!(matches!(frame.sample, SampleCommand::None));
        let output = frame.psg;
        assert_eq!(output.volume, 16);
        assert!(output.sound_open);
        assert_eq!(output.noise, 0);
        assert_eq!(output.hardware_envelope, 10);
        assert_eq!(output.software_period, *expected_soft_period);
        let expected_hw = expected_hardware_period(*expected_soft_period, ratio);
        assert_eq!(output.hardware_period, expected_hw);
        assert!(!output.hardware_retrig);
    }
}

#[test]
fn channel_player_soft_to_hard_pitch_down_effect() {
    let ratio = 2;
    let instrument = soft_to_hard_instrument(ratio, 12, false);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);
    let cell = cell_with_effect((4 * 12) as Note, 1, "pitchDown", 0x80);

    let base_period: u16 = 239;
    let expected_offsets = [0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];
    for (tick, offset) in expected_offsets.iter().enumerate() {
        let frame = if tick == 0 {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, true)
        };

        assert!(matches!(frame.sample, SampleCommand::None));
        let output = frame.psg;
        assert_eq!(output.volume, 16);
        assert!(output.sound_open);
        assert_eq!(output.noise, 0);
        assert_eq!(output.hardware_envelope, 12);
        let expected_soft = base_period + *offset as u16;
        assert_eq!(output.software_period, expected_soft);
        let expected_hw = expected_hardware_period(expected_soft, ratio);
        assert_eq!(output.hardware_period, expected_hw);
        assert!(!output.hardware_retrig);
    }
}

#[test]
fn channel_player_high_pitched_hard_only_with_arp_overflow() {
    let instrument = hardware_only_instrument_with_arp(4, 8, 0, 4);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let cell = test_cell(((6 * 12) + 7) as Note, 1);

    for iteration in 0..20 {
        let frame = if iteration == 0 {
            run_single_tick(&mut player, Some(&cell), true, false)
        } else {
            run_single_tick(&mut player, None, false, false)
        };

        let output = frame.psg;
        assert_eq!(output.volume, 16);
        assert!(!output.sound_open);
        assert_eq!(output.noise, 0);
        assert_eq!(output.hardware_envelope, 8);
        assert_eq!(output.hardware_period, 2);
        assert!(!output.hardware_retrig);
    }
}

#[test]
fn channel_player_glide_up_soft_only() {
    let instrument = soft_only_instrument(15, 0, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let base_cell = test_cell(((1 * 12) + 9) as Note, 1);
    let glide_cell = cell_with_effect(((2 * 12) + 2) as Note, 1, "pitchGlide", 0x0FFF);

    let expected_periods: &[u16] = &[
        1136, 1136, 1136, 1136, 1136, 1136, 1120, 1104, 1088, 1072, 1056, 1040, 1024, 1008, 992,
        976, 960, 944, 928, 912, 896, 880, 864, 851, 851, 851, 851, 851, 851, 851,
    ];
    let speed = 6;
    let mut tick = 0;

    for (idx, expected) in expected_periods.iter().enumerate() {
        let cell = if tick == 0 {
            if idx == 0 {
                Some(&base_cell)
            } else if idx == speed {
                Some(&glide_cell)
            } else {
                None
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        assert_eq!(frame.psg.software_period, *expected);
        assert_eq!(frame.psg.volume, 15);
        assert!(frame.psg.sound_open);
        assert_eq!(frame.psg.noise, 0);
        assert_eq!(frame.psg.hardware_period, 0);
        assert_eq!(frame.psg.hardware_envelope, 0);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_glide_down_soft_only() {
    let instrument = soft_only_instrument(15, 0, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let base_cell = test_cell(((2 * 12) + 9) as Note, 1);
    let glide_cell = cell_with_effect(((2 * 12) + 2) as Note, 1, "pitchGlide", 0x0FFF);

    let expected_periods: &[u16] = &[
        568, 568, 568, 568, 568, 568, 583, 599, 615, 631, 647, 663, 679, 695, 711, 727, 743, 759,
        775, 791, 807, 823, 839, 851, 851, 851, 851, 851, 851, 851,
    ];
    let speed = 6;
    let mut tick = 0;

    for (idx, expected) in expected_periods.iter().enumerate() {
        let cell = if tick == 0 {
            if idx == 0 {
                Some(&base_cell)
            } else if idx == speed {
                Some(&glide_cell)
            } else {
                None
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        assert_eq!(frame.psg.software_period, *expected);
        assert_eq!(frame.psg.volume, 15);
        assert!(frame.psg.sound_open);
        assert_eq!(frame.psg.noise, 0);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_psg_stopped_by_sample_instrument() {
    let psg_instrument = soft_to_hard_instrument_with_period(4, 8, false, Some(100));
    let sample_instr = sample_instrument(11_025, 12 * 6);
    let song = build_song_with_instruments(vec![empty_instrument(), psg_instrument, sample_instr]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let psg_cell = test_cell(((5 * 12) + 3) as Note, 1);
    let sample_cell = test_cell((6 * 12) as Note, 2);

    let speed = 10;
    let sample_cell_index = 2;
    let sample_iteration_index = sample_cell_index * speed;
    let total_iterations = 5 * speed;
    let mut tick = 0;

    for iteration in 0..total_iterations {
        let cell = if tick == 0 {
            match iteration / speed {
                0 => Some(&psg_cell),
                idx if idx == sample_cell_index => Some(&sample_cell),
                _ => None,
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        let output = frame.psg;

        if iteration < sample_iteration_index {
            assert_eq!(output.volume, 16);
            assert!(output.sound_open);
        } else {
            assert_eq!(output.volume, 0);
            assert!(!output.sound_open);
        }
        assert_eq!(output.software_period, 100);

        if iteration == sample_iteration_index {
            assert!(matches!(frame.sample, SampleCommand::Play(_)));
        }

        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_decaying_software_sound() {
    let base_volumes = [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
    let expected_volumes = [
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let instrument = decaying_soft_instrument(&base_volumes, 0, false);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let note = (4 * 12) as Note;
    let expected_period = compute_period(1_000_000.0, 440.0, note);
    let cell = test_cell(note, 1);
    let speed = 6;
    let mut tick = 0;
    let mut posted = false;

    for expected_volume in expected_volumes.iter() {
        let send_cell = !posted && tick == 0;
        if send_cell {
            posted = true;
        }
        let frame = if send_cell {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, tick < speed)
        };

        let output = frame.psg;
        assert_eq!(output.volume, *expected_volume);
        assert_eq!(output.noise, 0);
        assert_eq!(output.software_period, expected_period);
        assert_eq!(output.hardware_period, 0);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_decaying_software_sound_with_instrument_speed() {
    let base_volumes = [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
    let mut expected_volumes = Vec::new();
    for &value in &base_volumes {
        expected_volumes.extend(std::iter::repeat(value).take(3));
    }
    expected_volumes.extend(std::iter::repeat(0).take(5));
    let instrument = decaying_soft_instrument(&base_volumes, 2, false);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let note = ((3 * 12) + 5) as Note;
    let expected_period = compute_period(1_000_000.0, 440.0, note);
    let cell = test_cell(note, 1);
    let speed = 6;
    let mut tick = 0;
    let mut posted = false;

    for &expected_volume in &expected_volumes {
        let send_cell = !posted && tick == 0;
        if send_cell {
            posted = true;
        }
        let frame = if send_cell {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, tick < speed)
        };

        let output = frame.psg;
        assert_eq!(output.volume, expected_volume);
        assert_eq!(output.noise, 0);
        assert_eq!(output.hardware_period, 0);
        assert_eq!(output.software_period, expected_period);

        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_software_sound_with_arpeggio_effect() {
    let volumes = [15, 14, 13, 12, 11, 10];
    let mut instrument = decaying_soft_instrument(&volumes, 0, true);
    instrument.loop_start_index = 1;
    instrument.end_index = 5;
    instrument.is_looping = true;
    if let Some(first) = instrument.cells.first_mut() {
        first.primary_arpeggio_octave = 1;
    }
    let arpeggio = Arpeggio {
        index: 1,
        name: "arp".into(),
        values: vec![0, 3, 7],
        speed: 0,
        loop_start: 0,
        end_index: 2,
        shift: 0,
    };
    let song = build_song_with_data(vec![empty_instrument(), instrument], vec![arpeggio]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let cell = cell_with_effect((3 * 12) as Note, 1, "arpeggioTable", 1);
    let expected_volumes = [
        15, 14, 13, 12, 11, 10, 14, 13, 12, 11, 10, 14, 13, 12, 11, 10, 14, 13, 12, 11, 10,
    ];
    let expected_periods = [
        239, 402, 319, 478, 402, 319, 478, 402, 319, 478, 402, 319, 478, 402, 319, 478, 402, 319,
        478, 402, 319,
    ];

    let speed = 6;
    let mut tick = 0;
    for (idx, expected_volume) in expected_volumes.iter().enumerate() {
        let frame = if idx == 0 {
            run_single_tick(&mut player, Some(&cell), true, true)
        } else {
            run_single_tick(&mut player, None, false, tick < speed)
        };

        let output = frame.psg;
        assert_eq!(output.volume, *expected_volume);
        assert_eq!(output.noise, 0);
        assert_eq!(output.software_period, expected_periods[idx]);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_pitch_up_software_sound() {
    let instrument = soft_only_instrument(15, 0, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let base_cell = test_cell(((3 * 12) + 9) as Note, 1);
    let pitch_up_cell = effect_only_cell("pitchUp", 0x0380);
    let pitch_reset_cell = effect_only_cell("pitchUp", 0x0000);

    let expected_periods = [
        284, 284, 284, 284, 284, 284, 280, 277, 273, 270, 266, 263, 259, 256, 252, 249, 245, 242,
        238, 235, 231, 228, 224, 221, 221, 221, 221, 221, 221, 221,
    ];

    let speed = 6;
    let mut tick = 0;
    for (idx, expected_period) in expected_periods.iter().enumerate() {
        let cell = if tick == 0 {
            match idx / speed {
                0 => Some(&base_cell),
                1 => Some(&pitch_up_cell),
                4 => Some(&pitch_reset_cell),
                _ => None,
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        let output = frame.psg;
        println!("glide idx {idx} period {}", output.software_period);
        assert_eq!(output.software_period, *expected_period);
        assert_eq!(output.volume, 15);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_pitch_down_software_sound() {
    let instrument = soft_only_instrument(15, 0, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let base_cell = test_cell(((3 * 12) + 9) as Note, 1);
    let pitch_down_cell = effect_only_cell("pitchDown", 0x0380);
    let pitch_reset_cell = effect_only_cell("pitchDown", 0x0000);

    let expected_periods = [
        284, 284, 284, 284, 284, 284, 287, 291, 294, 298, 301, 305, 308, 312, 315, 319, 322, 326,
        329, 333, 336, 340, 343, 347, 347, 347, 347, 347, 347, 347,
    ];

    let speed = 6;
    let mut tick = 0;
    for (idx, expected_period) in expected_periods.iter().enumerate() {
        let cell = if tick == 0 {
            match idx / speed {
                0 => Some(&base_cell),
                1 => Some(&pitch_down_cell),
                4 => Some(&pitch_reset_cell),
                _ => None,
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        let output = frame.psg;
        assert_eq!(output.software_period, *expected_period);
        assert_eq!(output.volume, 15);
        tick = (tick + 1) % speed;
    }
}

#[test]
fn channel_player_glide_up_then_down_soft_sound() {
    let instrument = soft_only_instrument(15, 0, true);
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let cell_d5 = test_cell(((5 * 12) + 3) as Note, 1);
    let cell_e5_glide = cell_with_effect(((5 * 12) + 5) as Note, 1, "pitchGlide", 0x0200);
    let cell_c5_glide = cell_with_effect(((5 * 12) + 1) as Note, 1, "pitchGlide", 0x0200);

    let expected_periods = [
        100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 98, 96, 94, 92, 90, 89, 89, 89, 89, 89,
        91, 93, 95, 97, 99, 101, 103, 105, 107, 109, 111, 113, 113, 113, 113, 113, 113, 113, 113,
        113,
    ];

    let speed = 10;
    let mut tick = 0;
    let mut posted_e = false;
    let mut posted_c = false;
    let mut actual_periods = Vec::new();
    for (idx, _) in expected_periods.iter().enumerate() {
        let cell = if tick == 0 {
            match idx / speed {
                0 => Some(&cell_d5),
                1 if !posted_e => {
                    posted_e = true;
                    Some(&cell_e5_glide)
                }
                2 if !posted_c => {
                    posted_c = true;
                    Some(&cell_c5_glide)
                }
                _ => None,
            }
        } else {
            None
        };

        let frame = run_single_tick(&mut player, cell, tick == 0, tick < speed);
        let output = frame.psg;
        actual_periods.push(output.software_period);
        tick = (tick + 1) % speed;
    }

    assert_eq!(actual_periods, expected_periods);
}

#[test]
fn channel_player_reset_stops_volume_out() {
    let instrument = volume_fade_instrument();
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let events = vec![
        (
            0,
            cell_with_effect(((5 * 12) + 3) as Note, 1, "volumeOut", 0x0080),
        ),
        (12, effect_only_cell("reset", 0x0000)),
    ];
    let expected = [
        14, 14, 13, 13, 12, 12, 11, 11, 10, 10, 9, 9, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    ];
    run_volume_case(&mut player, 6, &events, &expected);
}

#[test]
fn channel_player_reset_with_inverted_volume() {
    let instrument = volume_fade_instrument();
    let song = build_song_with_instruments(vec![empty_instrument(), instrument]);
    let mut player = ChannelPlayer::new(0, Arc::clone(&song), 1_000_000.0, 440.0, 11_025.0);

    let events = vec![
        (
            0,
            cell_with_effect(((5 * 12) + 3) as Note, 1, "volumeOut", 0x0080),
        ),
        (12, effect_only_cell("reset", 0x0002)),
    ];
    let expected = [
        14, 14, 13, 13, 12, 12, 11, 11, 10, 10, 9, 9, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    ];
    run_volume_case(&mut player, 6, &events, &expected);
}
