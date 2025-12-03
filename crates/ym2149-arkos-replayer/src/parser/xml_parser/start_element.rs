//! Handles XML start element events during AKS parsing.

use crate::error::Result;
use crate::format::*;
use quick_xml::Reader;
use std::collections::HashMap;

use super::super::helpers::*;
use super::super::state::*;

/// Handles XML start element events.
///
/// Updates the parse state and initializes temporary builders based on
/// the element being entered.
#[allow(clippy::too_many_arguments)]
pub fn handle_start_element<R: std::io::BufRead>(
    name: &str,
    current_state: &mut ParseState,
    format_version: FormatVersion,
    current_instrument: &mut Option<Instrument>,
    current_instrument_cell: &mut Option<InstrumentCell>,
    _current_sample_builder: &mut Option<SampleInstrumentBuilder>,
    current_arpeggio: &mut Option<Arpeggio>,
    legacy_arpeggio_note: &mut i32,
    legacy_arpeggio_octave: &mut i32,
    current_pitch_table: &mut Option<PitchTable>,
    current_subsong: &mut Option<Subsong>,
    current_psg: &mut Option<PsgConfig>,
    current_pattern: &mut Option<Pattern>,
    current_pattern_track_indexes: &mut Vec<usize>,
    current_pattern_transpositions: &mut Vec<i8>,
    current_pattern_cell_track_number: &mut Option<usize>,
    current_pattern_cell_transposition: &mut i8,
    current_pattern_height: &mut usize,
    current_track: &mut Option<Track>,
    current_cell: &mut Option<Cell>,
    current_effect: &mut Option<Effect>,
    current_effect_container: &mut Option<EffectContainer>,
    current_speed_track: &mut Option<SpecialTrack>,
    current_event_track: &mut Option<SpecialTrack>,
    current_special_cell: &mut Option<SpecialCell>,
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<()> {
    match name {
        // Top-level sections
        "instruments" | "fmInstruments" => *current_state = ParseState::Instruments,
        "arpeggios" => *current_state = ParseState::Arpeggios,
        "pitchs" | "pitchTables" | "pitches" => *current_state = ParseState::PitchTables,
        "subsongs" => *current_state = ParseState::Subsongs,

        // Instrument handling
        "instrument" | "fmInstrument" if *current_state == ParseState::Instruments => {
            *current_state = ParseState::Instrument;
            *current_instrument = Some(Instrument {
                name: String::new(),
                color_argb: 0,
                instrument_type: InstrumentType::Psg,
                speed: 0,
                is_retrig: false,
                loop_start_index: 0,
                end_index: 0,
                is_looping: false,
                is_sfx_exported: false,
                cells: Vec::new(),
                sample: None,
            });
        }
        "cells" if *current_state == ParseState::Instrument => {
            *current_state = ParseState::InstrumentCells;
        }
        "autoSpread" if *current_state == ParseState::Instrument => {
            *current_state = ParseState::InstrumentAutoSpread;
        }
        "cell" | "fmInstrumentCell"
            if *current_state == ParseState::InstrumentCells
                || (*current_state == ParseState::Instrument
                    && format_version == FormatVersion::Legacy) =>
        {
            *current_state = ParseState::InstrumentCell;
            *current_instrument_cell = Some(InstrumentCell {
                volume: 0,
                noise: 0,
                primary_period: 0,
                primary_arpeggio_note_in_octave: 0,
                primary_arpeggio_octave: 0,
                primary_pitch: 0,
                link: ChannelLink::NoSoftwareNoHardware,
                ratio: 4,
                hardware_envelope: 8,
                secondary_period: 0,
                secondary_arpeggio_note_in_octave: 0,
                secondary_arpeggio_octave: 0,
                secondary_pitch: 0,
                is_retrig: false,
            });
        }

        // Arpeggio handling
        "expression" if *current_state == ParseState::Arpeggios => {
            *current_state = ParseState::Arpeggio;
            *current_arpeggio = Some(Arpeggio {
                index: 0,
                name: String::new(),
                values: Vec::new(),
                speed: 0,
                loop_start: 0,
                end_index: 0,
                shift: 0,
            });
        }
        "arpeggio" if *current_state == ParseState::Arpeggios => {
            *current_state = ParseState::Arpeggio;
            *current_arpeggio = Some(Arpeggio {
                index: 0,
                name: String::new(),
                values: Vec::new(),
                speed: 0,
                loop_start: 0,
                end_index: 0,
                shift: 0,
            });
        }
        "arpeggioCell" if *current_state == ParseState::Arpeggio => {
            *current_state = ParseState::LegacyArpeggioCell;
            *legacy_arpeggio_note = 0;
            *legacy_arpeggio_octave = 0;
        }

        // Pitch table handling
        "expression" if *current_state == ParseState::PitchTables => {
            *current_state = ParseState::PitchTable;
            *current_pitch_table = Some(PitchTable {
                index: 0,
                name: String::new(),
                values: Vec::new(),
                speed: 0,
                loop_start: 0,
                end_index: 0,
                shift: 0,
            });
        }
        "pitch" if *current_state == ParseState::PitchTables => {
            *current_state = ParseState::PitchTable;
            *current_pitch_table = Some(PitchTable {
                index: 0,
                name: String::new(),
                values: Vec::new(),
                speed: 0,
                loop_start: 0,
                end_index: 0,
                shift: 0,
            });
        }

        // Subsong handling
        "subsong" => {
            *current_state = ParseState::Subsong;
            *current_subsong = Some(Subsong {
                title: String::new(),
                initial_speed: 6,
                end_position: 0,
                loop_start_position: 0,
                replay_frequency_hz: 50.0,
                psgs: Vec::new(),
                digi_channel: 0,
                highlight_spacing: 4,
                secondary_highlight: 4,
                positions: Vec::new(),
                patterns: Vec::new(),
                tracks: HashMap::new(),
                speed_tracks: HashMap::new(),
                event_tracks: HashMap::new(),
            });
        }
        "psgs" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::SubsongPsgs;
        }
        "psg" if *current_state == ParseState::SubsongPsgs => {
            *current_state = ParseState::SubsongPsg;
            *current_psg = Some(PsgConfig::default());
        }
        "psgMetadata" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::SubsongPsg;
            *current_psg = Some(PsgConfig::default());
        }
        "positions" => {
            if let Some(subsong) = current_subsong {
                subsong.positions = parse_positions_block(reader, buf)?;
            } else {
                skip_block(reader, buf, "positions")?;
            }
            *current_state = ParseState::Subsong;
        }

        // Speed/Event tracks
        "speedTracks" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::SpeedTracks;
        }
        "eventTracks" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::EventTracks;
        }
        "speedTrack" if *current_state == ParseState::SpeedTracks => {
            *current_state = ParseState::SpeedTrack;
            *current_speed_track = Some(SpecialTrack {
                index: 0,
                cells: Vec::new(),
            });
        }
        "eventTrack" if *current_state == ParseState::EventTracks => {
            *current_state = ParseState::EventTrack;
            *current_event_track = Some(SpecialTrack {
                index: 0,
                cells: Vec::new(),
            });
        }

        // Patterns
        "patterns" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::SubsongPatterns;
        }
        "pattern" if *current_state == ParseState::SubsongPatterns => {
            *current_state = ParseState::Pattern;
            *current_pattern = Some(Pattern {
                index: 0,
                track_indexes: Vec::new(),
                speed_track_index: 0,
                event_track_index: 0,
                color_argb: 0,
            });
            current_pattern_track_indexes.clear();
            current_pattern_transpositions.clear();
            *current_pattern_height = 64;
        }
        "patternCell" if *current_state == ParseState::Pattern => {
            *current_state = ParseState::PatternCell;
            *current_pattern_cell_track_number = None;
            *current_pattern_cell_transposition = 0;
        }
        "trackIndexes" if *current_state == ParseState::Pattern => {
            *current_state = ParseState::PatternTrackIndexes;
        }
        "speedTrackIndex" if *current_state == ParseState::Pattern => {
            *current_state = ParseState::PatternSpeedTrackIndex;
        }
        "eventTrackIndex" if *current_state == ParseState::Pattern => {
            *current_state = ParseState::PatternEventTrackIndex;
        }

        // Tracks
        "tracks" if *current_state == ParseState::Subsong => {
            *current_state = ParseState::SubsongTracks;
        }
        "track" if *current_state == ParseState::SubsongTracks => {
            *current_state = ParseState::Track;
            *current_track = Some(Track {
                index: 0,
                cells: Vec::new(),
            });
        }
        "cell" if *current_state == ParseState::Track => {
            *current_state = ParseState::Cell;
            *current_cell = Some(Cell {
                index: 0,
                note: 255, // No note
                instrument: 0,
                instrument_present: false,
                effects: Vec::new(),
            });
        }
        "cell" | "speedCell" if *current_state == ParseState::SpeedTrack => {
            *current_state = ParseState::SpeedCell;
            *current_special_cell = Some(SpecialCell { index: 0, value: 0 });
        }
        "cell" | "eventCell" if *current_state == ParseState::EventTrack => {
            *current_state = ParseState::EventCell;
            *current_special_cell = Some(SpecialCell { index: 0, value: 0 });
        }

        // Effects
        "effect" | "effectAndValue" if *current_state == ParseState::Cell => {
            *current_state = ParseState::Effect;
            *current_effect_container = Some(if name == "effectAndValue" {
                EffectContainer::Legacy
            } else {
                EffectContainer::Modern
            });
            *current_effect = Some(Effect {
                index: 0,
                name: String::new(),
                logical_value: 0,
            });
        }
        _ => {}
    }

    Ok(())
}
