//! Player tests (requires `extended-tests` feature).

#![cfg(all(test, feature = "extended-tests"))]

use super::*;
use super::psg_output::frames_to_registers;
use crate::parser::load_aks;
use std::path::PathBuf;
use ym2149_ym_replayer::parser::ym6::Ym6Parser;

fn normalize_ym_registers(frame: &[u8], prev: &mut [u8; 16]) -> [u8; 16] {
    let mut regs = [0u8; 16];
    let len = frame.len().min(16);
    regs[..len].copy_from_slice(&frame[..len]);
    if regs[13] == 0xFF {
        regs[13] = prev[13];
    }
    *prev = regs;
    regs
}

fn data_path(file: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "examples",
        "arkos",
        file,
    ]
    .iter()
    .collect()
}

#[test]
fn doclands_first_tick_contains_hardware_channel() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "examples",
        "arkos",
        "Doclands - Pong Cracktro (YM).aks",
    ]
    .iter()
    .collect();
    let data = std::fs::read(path).expect("load Doclands .aks");
    let song = load_aks(&data).expect("parse Doclands");
    let mut player = ArkosPlayer::new(song, 0).expect("player init");

    let frames = player.capture_tick_frames();
    assert!(
        frames.iter().any(|frame| frame.psg.volume == 16),
        "expected at least one hardware channel on first tick"
    );
    assert!(
        frames.iter().any(|frame| frame.psg.noise > 0),
        "expected noise activity on first tick, got {:?}",
        frames
            .iter()
            .map(|frame| frame.psg.noise)
            .collect::<Vec<_>>()
    );
}

#[test]
fn at2_first_tick_is_silent() {
    let aks_path = data_path("Excellence in Art 2018 - Just add cream.aks");
    let song_data = std::fs::read(aks_path).expect("read AT2 AKS");
    let song = load_aks(&song_data).expect("parse AT2 AKS");
    let mut player = ArkosPlayer::new(song, 0).expect("init AT2 player");

    assert!(
        !player.song.subsongs[player.subsong_index]
            .positions
            .is_empty(),
        "AT2 subsong should have synthesized positions"
    );
    let subsong = &player.song.subsongs[player.subsong_index];
    let (cell, _) = tick::resolve_cell(subsong, 0, 0, 0);
    let cell = cell.expect("expected a cell at first line/channel for AT2 pattern 0");
    assert!(
        cell.instrument_present && cell.instrument == usize::MAX,
        "legacy cells with instrument 0 should be marked as RST sentinel"
    );
    let frames = player.capture_tick_frames();
    assert!(
        frames.iter().all(|frame| frame.psg.volume == 0),
        "expected volume 0 on first tick before the buzzer is primed, got {:?}",
        frames
    );
}

#[test]
#[ignore]
fn doclands_matches_reference_ym() {
    let ym_path = data_path("Doclands - Pong Cracktro (YM).ym");
    let ym_data = std::fs::read(ym_path).expect("read reference YM");
    let parser = Ym6Parser;
    let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse YM");

    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let mut player = ArkosPlayer::new(song, 0).expect("init player");

    let frame_limit = ym_frames.len();
    let mut prev_expected = [0u8; 16];
    let mut prev_actual = [0u8; 16];
    prev_actual[13] = 8;
    for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
        let expected = normalize_ym_registers(frame, &mut prev_expected);
        let frames = player.capture_tick_frames();
        let actual = frames_to_registers(0, &frames, &mut prev_actual);
        if idx < 8 {
            println!(
                "Frame {idx}: expected {:?} actual {:?}",
                &expected[..14],
                &actual[..14]
            );
        }
        if expected[..14] != actual[..14] {
            println!(
                "Context at position {} line {}:",
                player.current_position, player.current_line
            );
            if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                && let Some(pos) = subsong.positions.get(player.current_position)
                && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
            {
                println!("  pattern track indexes {:?}", pattern.track_indexes);
            }
            for ch in 0..player.channel_players.len() {
                if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                    && let Some(pos) = subsong.positions.get(player.current_position)
                    && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                    && let Some(track_idx) = pattern.track_indexes.get(ch)
                {
                    println!("  ch{ch} uses track {}", track_idx);
                    if let Some(track) = subsong.tracks.get(track_idx) {
                        let indices: Vec<_> = track.cells.iter().map(|c| c.index).collect();
                        println!("    track cells {:?}", indices);
                    }
                }
                let ctx = player
                    .effect_context
                    .line_context(player.current_position, ch, player.current_line)
                    .cloned();
                println!("  ch{ch} ctx {:?}", ctx);
            }
            println!("Frame {} mismatch:", idx);
            println!("  current speed {}", player.current_speed);
            println!("  Expected regs: {:?}", &expected[..14]);
            println!("  Actual regs:   {:?}", &actual[..14]);
            for (channel, frame) in frames.iter().enumerate().take(3) {
                let out = &frame.psg;
                println!(
                    "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                    channel,
                    out.volume,
                    out.noise,
                    out.sound_open,
                    out.software_period,
                    out.hardware_period,
                    out.hardware_envelope,
                    frame.sample
                );
                let state = player.channel_players[channel].debug_state();
                println!("    state: {:?}", state);
            }
            panic!(
                "Register mismatch at frame {} (of {})",
                idx, header.frame_count
            );
        }
    }
}

#[test]
#[ignore]
fn lop_ears_matches_reference_ym() {
    let ym_path = data_path("Andy Severn - Lop Ears.ym");
    let ym_data = std::fs::read(ym_path).expect("read Lop Ears YM");
    let parser = Ym6Parser;
    let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse Lop Ears YM");

    let aks_path = data_path("Andy Severn - Lop Ears.aks");
    let song_data = std::fs::read(aks_path).expect("read Lop Ears AKS");
    let song = load_aks(&song_data).expect("parse Lop Ears AKS");
    #[cfg(test)]
    {
        let inst = &song.instruments[1];
        let links: Vec<_> = inst
            .cells
            .iter()
            .enumerate()
            .map(|(idx, cell)| (idx, format!("{:?}", cell.link)))
            .collect();
        println!("lop ears instrument1 links {:?}", links);
        if let Some(track) = song.subsongs[0].tracks.get(&8) {
            println!(
                "lop ears track8 cells {:?}",
                track.cells.iter().map(|c| c.index).collect::<Vec<_>>()
            );
        }
    }
    let mut player = ArkosPlayer::new(song, 0).expect("init Lop Ears player");

    let frame_limit = ym_frames.len();
    let mut prev_expected = [0u8; 16];
    let mut prev_actual = [0u8; 16];
    prev_actual[13] = 8;
    for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
        let expected = normalize_ym_registers(frame, &mut prev_expected);
        let frames = player.capture_tick_frames();
        let actual = frames_to_registers(0, &frames, &mut prev_actual);
        if idx < 8 {
            continue;
        }
        if expected[..14] != actual[..14] {
            println!(
                "Context at position {} line {}:",
                player.current_position, player.current_line
            );
            for ch in 0..player.channel_players.len() {
                if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                    && let Some(pos) = subsong.positions.get(player.current_position)
                    && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                    && let Some(track_idx) = pattern.track_indexes.get(ch)
                {
                    println!("  ch{ch} uses track {}", track_idx);
                }
                let ctx = player
                    .effect_context
                    .line_context(player.current_position, ch, player.current_line)
                    .cloned();
                println!("  ch{ch} ctx {:?}", ctx);
            }
            println!("Frame {} mismatch:", idx);
            println!("  current speed {}", player.current_speed);
            println!("  Expected regs: {:?}", &expected[..14]);
            println!("  Actual regs:   {:?}", &actual[..14]);
            for (channel, frame) in frames.iter().enumerate().take(3) {
                let out = &frame.psg;
                println!(
                    "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                    channel,
                    out.volume,
                    out.noise,
                    out.sound_open,
                    out.software_period,
                    out.hardware_period,
                    out.hardware_envelope,
                    frame.sample
                );
            }
            panic!(
                "Lop Ears mismatch at frame {} (of {})",
                idx, header.frame_count
            );
        }
    }
}

#[test]
#[ignore]
fn debug_doclands_first_cell() {
    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let subsong = &song.subsongs[0];
    for channel_idx in 0..3 {
        if let Some((track_index, cell)) = subsong
            .positions
            .first()
            .and_then(|position| subsong.patterns.get(position.pattern_index))
            .and_then(|pattern| {
                pattern
                    .track_indexes
                    .get(channel_idx)
                    .and_then(|track_index| {
                        subsong
                            .tracks
                            .get(track_index)
                            .map(|track| (track_index, track))
                    })
            })
            .and_then(|(track_index, track)| {
                track
                    .cells
                    .iter()
                    .find(|c| c.index == 0)
                    .map(|cell| (*track_index, cell))
            })
        {
            println!(
                "Channel {channel_idx}: track {track_index} note {} instrument {} effects {:?}",
                cell.note, cell.instrument, cell.effects
            );
            if cell.instrument < song.instruments.len() {
                let instrument = &song.instruments[cell.instrument];
                if !instrument.cells.is_empty() {
                    let inst_cell = &instrument.cells[0];
                    println!(
                        "  instrument cell volume {} noise {} link {:?} prim_arp_note {} prim_arp_oct {} prim_pitch {} sec_arp_note {} sec_arp_oct {} sec_pitch {} forced_sw {} forced_hw {}",
                        inst_cell.volume,
                        inst_cell.noise,
                        inst_cell.link,
                        inst_cell.primary_arpeggio_note_in_octave,
                        inst_cell.primary_arpeggio_octave,
                        inst_cell.primary_pitch,
                        inst_cell.secondary_arpeggio_note_in_octave,
                        inst_cell.secondary_arpeggio_octave,
                        inst_cell.secondary_pitch,
                        inst_cell.primary_period,
                        inst_cell.secondary_period
                    );
                }
            }
        } else {
            println!("Channel {channel_idx}: No cell at position 0, line 0");
        }
    }
}

#[test]
#[ignore]
fn debug_doclands_pattern_tracks() {
    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let subsong = &song.subsongs[0];
    for (pattern_idx, pattern) in subsong.patterns.iter().enumerate().take(4) {
        println!("Pattern {pattern_idx}: {:?}", pattern.track_indexes);
    }
    for (pos_idx, position) in subsong.positions.iter().enumerate().take(4) {
        println!(
            "Position {pos_idx}: pattern {} height {}",
            position.pattern_index, position.height
        );
    }
}

#[test]
#[ignore]
fn debug_doclands_track_cells() {
    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let subsong = &song.subsongs[0];
    for (track_idx, track) in subsong.tracks.iter() {
        println!("Track {track_idx}");
        for cell in track.cells.iter().take(8) {
            println!(
                "  line {:>2}: note {:>3} instrument {} effects {:?}",
                cell.index, cell.note, cell.instrument, cell.effects
            );
        }
    }
}

#[test]
#[ignore]
fn debug_doclands_first_frames_registers() {
    let ym_path = data_path("Doclands - Pong Cracktro (YM).ym");
    let ym_data = std::fs::read(ym_path).expect("read reference YM");
    let parser = Ym6Parser;
    let (ym_frames, _, _, _) = parser.parse_full(&ym_data).expect("parse YM");

    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let mut player = ArkosPlayer::new(song, 0).expect("init doc player");

    let mut prev_expected = [0u8; 16];
    let mut prev_actual = [0u8; 16];
    prev_actual[13] = 8;

    for (idx, frame) in ym_frames.iter().take(16).enumerate() {
        let expected = normalize_ym_registers(frame, &mut prev_expected);
        let frames = player.capture_tick_frames();
        let actual = frames_to_registers(0, &frames, &mut prev_actual);
        println!(
            "#{idx:02} pos {} line {} speed {} exp {:?} act {:?}",
            player.current_position,
            player.current_line,
            player.current_speed,
            &expected[..14],
            &actual[..14]
        );
        for (ch, frame) in frames.iter().enumerate().take(3) {
            let out = &frame.psg;
            let state = player.channel_players[ch].debug_state();
            println!(
                "   ch{} vol {:>2} noise {:>2} period {:>4} inst {:?}",
                ch, out.volume, out.noise, out.software_period, state
            );
        }
    }
}

#[test]
#[ignore]
fn debug_doclands_speed_tracks() {
    let aks_path = data_path("Doclands - Pong Cracktro (YM).aks");
    let song_data = std::fs::read(aks_path).expect("read Doclands AKS");
    let song = load_aks(&song_data).expect("parse Doclands AKS");
    let subsong = &song.subsongs[0];
    println!(
        "speed tracks available: {:?}",
        subsong.speed_tracks.keys().collect::<Vec<_>>()
    );
    let pattern = &subsong.patterns[subsong.positions[0].pattern_index];
    println!("pattern0 speed idx {}", pattern.speed_track_index);
    if let Some(track) = subsong.speed_tracks.get(&pattern.speed_track_index) {
        println!("track cells {:?}", track.cells);
    } else {
        println!("track not found!");
    }
}

#[test]
#[ignore]
fn at2_matches_reference_ym() {
    let ym_path = data_path("Excellence in Art 2018 - Just add cream.ym");
    let ym_data = std::fs::read(ym_path).expect("read AT2 YM");
    let parser = Ym6Parser;
    let (ym_frames, header, _, _) = parser.parse_full(&ym_data).expect("parse AT2 YM");

    let aks_path = data_path("Excellence in Art 2018 - Just add cream.aks");
    let song_data = std::fs::read(aks_path).expect("read AT2 AKS");
    let song = load_aks(&song_data).expect("parse AT2 AKS");
    let mut player = ArkosPlayer::new(song, 0).expect("init AT2 player");

    let frame_limit = ym_frames.len();
    let mut prev_expected = [0u8; 16];
    let mut prev_actual = [0u8; 16];
    prev_actual[13] = 8;
    for (idx, frame) in ym_frames.iter().take(frame_limit).enumerate() {
        let expected = normalize_ym_registers(frame, &mut prev_expected);
        let frames = player.capture_tick_frames();
        let actual = frames_to_registers(0, &frames, &mut prev_actual);
        if expected[..14] != actual[..14] {
            println!(
                "AT2 mismatch at frame {} position {} line {}",
                idx, player.current_position, player.current_line
            );
            if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                && let Some(pos) = subsong.positions.get(player.current_position)
            {
                if let Some(pattern) = subsong.patterns.get(pos.pattern_index) {
                    println!(
                        "  pattern {} track idx {:?} speed {} event {}",
                        pattern.index,
                        pattern.track_indexes,
                        pattern.speed_track_index,
                        pattern.event_track_index
                    );
                }
                println!("  position transpositions {:?}", pos.transpositions);
            }
            for ch in 0..player.channel_players.len().min(3) {
                if let Some(subsong) = player.song.subsongs.get(player.subsong_index)
                    && let Some(pos) = subsong.positions.get(player.current_position)
                    && let Some(pattern) = subsong.patterns.get(pos.pattern_index)
                    && let Some(track_idx) = pattern.track_indexes.get(ch)
                {
                    println!("  ch{ch} uses track {}", track_idx);
                }
                let ctx = player
                    .effect_context
                    .line_context(player.current_position, ch, player.current_line)
                    .cloned();
                println!("  ch{ch} ctx {:?}", ctx);
            }
            println!("Frame {} mismatch:", idx);
            println!("  current speed {}", player.current_speed);
            println!("  Expected regs: {:?}", &expected[..14]);
            println!("  Actual regs:   {:?}", &actual[..14]);
            for (channel, frame) in frames.iter().enumerate().take(3) {
                let out = &frame.psg;
                println!(
                    "  Ch{}: vol {:>2} noise {:>2} sound_open {} period {:>4} hw_period {:>4} env {:>2} sample {:?}",
                    channel,
                    out.volume,
                    out.noise,
                    out.sound_open,
                    out.software_period,
                    out.hardware_period,
                    out.hardware_envelope,
                    frame.sample
                );
                let state = player.channel_players[channel].debug_state();
                println!("    state: {:?}", state);
            }
            panic!(
                "AT2 Register mismatch at frame {} (of {})",
                idx, header.frame_count
            );
        }
    }
}
