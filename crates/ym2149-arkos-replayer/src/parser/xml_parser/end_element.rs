//! Handles XML end element events during AKS parsing.

use std::collections::HashMap;
use crate::error::Result;
use crate::format::*;

use super::super::state::*;

/// Handles XML end element events.
///
/// Finalizes temporary builders and adds completed structures to their
/// parent containers. Updates the parse state machine.
#[allow(clippy::too_many_arguments)]
pub fn handle_end_element(
    name: &str,
    current_state: &mut ParseState,
    format_version: FormatVersion,
    instruments: &mut Vec<Instrument>,
    arpeggios: &mut Vec<Arpeggio>,
    pitch_tables: &mut Vec<PitchTable>,
    subsongs: &mut Vec<Subsong>,
    current_instrument: &mut Option<Instrument>,
    current_instrument_cell: &mut Option<InstrumentCell>,
    current_sample_builder: &mut Option<SampleInstrumentBuilder>,
    current_arpeggio: &mut Option<Arpeggio>,
    legacy_arpeggio_note: i32,
    legacy_arpeggio_octave: i32,
    current_pitch_table: &mut Option<PitchTable>,
    current_subsong: &mut Option<Subsong>,
    current_psg: &mut Option<PsgConfig>,
    current_pattern: &mut Option<Pattern>,
    current_pattern_track_indexes: &mut Vec<usize>,
    current_pattern_transpositions: &mut Vec<i8>,
    current_pattern_height: usize,
    current_pattern_cell_track_number: &mut Option<usize>,
    current_pattern_cell_transposition: i8,
    current_track: &mut Option<Track>,
    current_cell: &mut Option<Cell>,
    current_effect: &mut Option<Effect>,
    current_effect_container: &mut Option<EffectContainer>,
    current_speed_track: &mut Option<SpecialTrack>,
    current_event_track: &mut Option<SpecialTrack>,
    current_special_cell: &mut Option<SpecialCell>,
    legacy_arpeggio_map: &mut HashMap<usize, usize>,
    legacy_pitch_map: &mut HashMap<usize, usize>,
) -> Result<()> {
    match name {
        "trackIndexes" if *current_state == ParseState::PatternTrackIndexes => {
            *current_state = ParseState::Pattern;
        }
        "patternCell" if *current_state == ParseState::PatternCell => {
            if let Some(track_index) = current_pattern_cell_track_number.take() {
                current_pattern_track_indexes.push(track_index);
                current_pattern_transpositions.push(current_pattern_cell_transposition);
            }
            *current_state = ParseState::Pattern;
        }
        "speedTrackIndex" if *current_state == ParseState::PatternSpeedTrackIndex => {
            *current_state = ParseState::Pattern;
        }
        "eventTrackIndex" if *current_state == ParseState::PatternEventTrackIndex => {
            *current_state = ParseState::Pattern;
        }
        "pattern" => {
            if let Some(mut pattern) = current_pattern.take() {
                pattern.track_indexes = current_pattern_track_indexes.clone();
                if let Some(subsong) = current_subsong {
                    let pattern_index = subsong.patterns.len();
                    let height = current_pattern_height;
                    pattern.index = pattern_index;
                    subsong.patterns.push(pattern);
                    if format_version == FormatVersion::Legacy {
                        let mut transpositions = current_pattern_transpositions.clone();
                        if transpositions.len() < current_pattern_track_indexes.len() {
                            transpositions
                                .resize(current_pattern_track_indexes.len(), 0);
                        }
                        subsong.positions.push(Position {
                            pattern_index,
                            height,
                            marker_name: String::new(),
                            marker_color: 0,
                            transpositions,
                        });
                    }
                }
            }
            current_pattern_track_indexes.clear();
            current_pattern_transpositions.clear();
            *current_state = ParseState::SubsongPatterns;
        }
        "patterns" if *current_state == ParseState::SubsongPatterns => {
            *current_state = ParseState::Subsong;
        }
        "position" | "positions" => {}
        "cell" | "fmInstrumentCell" if *current_state == ParseState::InstrumentCell => {
            if let (Some(cell), Some(instr)) =
                (current_instrument_cell.take(), current_instrument.as_mut())
            {
                instr.cells.push(cell);
            }
            *current_state = if format_version == FormatVersion::Legacy {
                ParseState::Instrument
            } else {
                ParseState::InstrumentCells
            };
        }
        "cells" if *current_state == ParseState::InstrumentCells => {
            *current_state = ParseState::Instrument;
        }
        "autoSpread" if *current_state == ParseState::InstrumentAutoSpread => {
            *current_state = ParseState::Instrument;
        }
        "instrument" | "fmInstrument" if *current_state == ParseState::Instrument => {
            if let Some(mut instr) = current_instrument.take() {
                if instr.instrument_type == InstrumentType::Digi
                    && let Some(builder) = current_sample_builder.take()
                {
                    let mut finalized_builder = builder;
                    finalized_builder.loop_start_index = instr.loop_start_index;
                    finalized_builder.end_index = instr.end_index;
                    finalized_builder.is_looping = instr.is_looping;
                    instr.sample = Some(finalized_builder.build()?);
                }
                instruments.push(instr);
            }
            *current_state = ParseState::Instruments;
        }
        "instruments" | "fmInstruments" if *current_state == ParseState::Instruments => {
            *current_state = ParseState::Root;
        }
        "expression" if *current_state == ParseState::Arpeggio => {
            if let Some(arp) = current_arpeggio.take() {
                if format_version == FormatVersion::Legacy {
                    legacy_arpeggio_map.insert(arp.index, arpeggios.len());
                }
                arpeggios.push(arp);
            }
            *current_state = ParseState::Arpeggios;
        }
        "arpeggio" if *current_state == ParseState::Arpeggio => {
            if let Some(arp) = current_arpeggio.take() {
                if format_version == FormatVersion::Legacy {
                    legacy_arpeggio_map.insert(arp.index, arpeggios.len());
                }
                arpeggios.push(arp);
            }
            *current_state = ParseState::Arpeggios;
        }
        "arpeggioCell" if *current_state == ParseState::LegacyArpeggioCell => {
            if let Some(arp) = current_arpeggio.as_mut() {
                let value = (legacy_arpeggio_octave * 12 + legacy_arpeggio_note)
                    .clamp(i8::MIN as i32, i8::MAX as i32)
                    as i8;
                arp.values.push(value);
            }
            *current_state = ParseState::Arpeggio;
        }
        "arpeggios" if *current_state == ParseState::Arpeggios => {
            *current_state = ParseState::Root;
        }
        "expression" if *current_state == ParseState::PitchTable => {
            if let Some(pitch) = current_pitch_table.take() {
                if format_version == FormatVersion::Legacy {
                    legacy_pitch_map.insert(pitch.index, pitch_tables.len());
                }
                pitch_tables.push(pitch);
            }
            *current_state = ParseState::PitchTables;
        }
        "pitch" if *current_state == ParseState::PitchTable => {
            if let Some(pitch) = current_pitch_table.take() {
                if format_version == FormatVersion::Legacy {
                    legacy_pitch_map.insert(pitch.index, pitch_tables.len());
                }
                pitch_tables.push(pitch);
            }
            *current_state = ParseState::PitchTables;
        }
        "pitchs" | "pitchTables" if *current_state == ParseState::PitchTables => {
            *current_state = ParseState::Root;
        }
        "psg" | "psgMetadata" if *current_state == ParseState::SubsongPsg => {
            if let (Some(psg), Some(subsong)) =
                (current_psg.take(), current_subsong.as_mut())
            {
                subsong.psgs.push(psg);
            }
            *current_state = if format_version == FormatVersion::Legacy {
                ParseState::Subsong
            } else {
                ParseState::SubsongPsgs
            };
        }
        "psgs" if *current_state == ParseState::SubsongPsgs => {
            *current_state = ParseState::Subsong;
        }
        "effect" if *current_state == ParseState::Effect => {
            if *current_effect_container == Some(EffectContainer::Modern) {
                if let (Some(eff), Some(cell)) =
                    (current_effect.take(), current_cell.as_mut())
                {
                    cell.effects.push(eff);
                }
                *current_effect_container = None;
                *current_state = ParseState::Cell;
            }
        }
        "effectAndValue" if *current_state == ParseState::Effect => {
            if let (Some(eff), Some(cell)) =
                (current_effect.take(), current_cell.as_mut())
            {
                cell.effects.push(eff);
            }
            *current_effect_container = None;
            *current_state = ParseState::Cell;
        }
        "cell" if *current_state == ParseState::Cell => {
            if let (Some(cell), Some(track)) =
                (current_cell.take(), current_track.as_mut())
            {
                track.cells.push(cell);
            }
            *current_state = ParseState::Track;
        }
        "track" if *current_state == ParseState::Track => {
            if let (Some(track), Some(subsong)) =
                (current_track.take(), current_subsong.as_mut())
            {
                let track_index = track.index;
                subsong.tracks.insert(track_index, track);
            }
            *current_state = ParseState::SubsongTracks;
        }
        "cell" | "speedCell" if *current_state == ParseState::SpeedCell => {
            if let (Some(cell), Some(track)) =
                (current_special_cell.take(), current_speed_track.as_mut())
            {
                track.cells.push(cell);
            }
            *current_state = ParseState::SpeedTrack;
        }
        "cell" | "eventCell" if *current_state == ParseState::EventCell => {
            if let (Some(cell), Some(track)) =
                (current_special_cell.take(), current_event_track.as_mut())
            {
                track.cells.push(cell);
            }
            *current_state = ParseState::EventTrack;
        }
        "speedTrack" if *current_state == ParseState::SpeedTrack => {
            if let (Some(track), Some(subsong)) =
                (current_speed_track.take(), current_subsong.as_mut())
            {
                subsong.speed_tracks.insert(track.index, track);
            }
            *current_state = ParseState::SpeedTracks;
        }
        "eventTrack" if *current_state == ParseState::EventTrack => {
            if let (Some(track), Some(subsong)) =
                (current_event_track.take(), current_subsong.as_mut())
            {
                subsong.event_tracks.insert(track.index, track);
            }
            *current_state = ParseState::EventTracks;
        }
        "speedTracks" if *current_state == ParseState::SpeedTracks => {
            *current_state = ParseState::Subsong;
        }
        "eventTracks" if *current_state == ParseState::EventTracks => {
            *current_state = ParseState::Subsong;
        }
        "tracks" if *current_state == ParseState::SubsongTracks => {
            *current_state = ParseState::Subsong;
        }
        "subsong" => {
            if let Some(subsong) = current_subsong.take() {
                subsongs.push(subsong);
            }
            *current_state = ParseState::Subsongs;
        }
        "subsongs" if *current_state == ParseState::Subsongs => {
            *current_state = ParseState::Root;
        }
        _ => {}
    }

    Ok(())
}
