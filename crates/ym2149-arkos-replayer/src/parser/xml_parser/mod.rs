//! Core XML parsing logic for AKS files.
//!
//! Contains the main parsing loop and element handlers for converting
//! Arkos Tracker XML into Rust data structures.
//!
//! # Module Structure
//!
//! - [`start_element`] - Handles XML start element events
//! - [`text_content`] - Handles text content between elements
//! - [`end_element`] - Handles XML end element events

mod start_element;
mod text_content;
mod end_element;

use std::collections::HashMap;
use crate::error::{ArkosError, Result};
use crate::format::*;
use quick_xml::Reader;
use quick_xml::events::Event;

use super::state::*;

/// Parses plain XML AKS data into an [`AksSong`].
///
/// This is the main parsing function that processes the entire XML document,
/// handling both legacy (Format 1.x) and modern (Format 3.x) structures.
///
/// # State Machine
///
/// The parser maintains a state stack via [`ParseState`] to track position
/// in the XML hierarchy. Element content is accumulated in `current_text`
/// and processed when the closing tag is encountered.
///
/// # Memory Management
///
/// Complex structures (instruments, arpeggios, subsongs) are built incrementally
/// using temporary variables (`current_instrument`, `current_arpeggio`, etc.)
/// and finalized when their closing tags are reached.
pub fn parse_aks_xml(data: &[u8]) -> Result<AksSong> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut metadata = SongMetadata::default();
    let mut instruments = Vec::new();
    let mut arpeggios = Vec::new();
    let mut pitch_tables = Vec::new();
    let mut subsongs = Vec::new();
    let mut format_version = FormatVersion::Modern;
    let mut legacy_defaults_inserted = false;

    let mut current_state = ParseState::Root;
    let mut current_text = String::new();

    // Temporary storage for building complex structures
    let mut current_instrument: Option<Instrument> = None;
    let mut current_instrument_cell: Option<InstrumentCell> = None;
    let mut current_sample_builder: Option<SampleInstrumentBuilder> = None;
    let mut current_arpeggio: Option<Arpeggio> = None;
    let mut legacy_arpeggio_note: i32 = 0;
    let mut legacy_arpeggio_octave: i32 = 0;
    let mut current_pitch_table: Option<PitchTable> = None;
    let mut current_subsong: Option<Subsong> = None;
    let mut current_psg: Option<PsgConfig> = None;
    let mut current_pattern: Option<Pattern> = None;
    let mut current_pattern_track_indexes: Vec<usize> = Vec::new();
    let mut current_pattern_transpositions: Vec<i8> = Vec::new();
    let mut current_pattern_cell_track_number: Option<usize> = None;
    let mut current_pattern_cell_transposition: i8 = 0;
    let mut current_pattern_height: usize = 64;
    let mut current_track: Option<Track> = None;
    let mut current_cell: Option<Cell> = None;
    let mut current_effect: Option<Effect> = None;
    let mut current_effect_container: Option<EffectContainer> = None;
    let mut current_speed_track: Option<SpecialTrack> = None;
    let mut current_event_track: Option<SpecialTrack> = None;
    let mut current_special_cell: Option<SpecialCell> = None;
    let mut legacy_arpeggio_map: HashMap<usize, usize> = HashMap::new();
    let mut legacy_pitch_map: HashMap<usize, usize> = HashMap::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = local_name_from_bytes(e.name().local_name().as_ref());

                start_element::handle_start_element(
                    &name,
                    &mut current_state,
                    format_version,
                    &mut current_instrument,
                    &mut current_instrument_cell,
                    &mut current_sample_builder,
                    &mut current_arpeggio,
                    &mut legacy_arpeggio_note,
                    &mut legacy_arpeggio_octave,
                    &mut current_pitch_table,
                    &mut current_subsong,
                    &mut current_psg,
                    &mut current_pattern,
                    &mut current_pattern_track_indexes,
                    &mut current_pattern_transpositions,
                    &mut current_pattern_cell_track_number,
                    &mut current_pattern_cell_transposition,
                    &mut current_pattern_height,
                    &mut current_track,
                    &mut current_cell,
                    &mut current_effect,
                    &mut current_effect_container,
                    &mut current_speed_track,
                    &mut current_event_track,
                    &mut current_special_cell,
                    &mut reader,
                    &mut buf,
                )?;

                current_text.clear();
            }

            Ok(Event::Text(e)) => {
                current_text = e.unescape()?.to_string();
            }

            Ok(Event::End(e)) => {
                let name = local_name_from_bytes(e.name().local_name().as_ref());

                text_content::handle_text_content(
                    &current_state,
                    &name,
                    &current_text,
                    format_version,
                    &mut metadata,
                    &mut format_version,
                    &mut legacy_defaults_inserted,
                    &mut arpeggios,
                    &mut pitch_tables,
                    &mut current_instrument,
                    &mut current_instrument_cell,
                    &mut current_sample_builder,
                    &mut current_arpeggio,
                    &mut legacy_arpeggio_note,
                    &mut legacy_arpeggio_octave,
                    &mut current_pitch_table,
                    &mut current_subsong,
                    &mut current_psg,
                    &mut current_pattern,
                    &mut current_pattern_track_indexes,
                    &mut current_pattern_height,
                    &mut current_pattern_cell_track_number,
                    &mut current_pattern_cell_transposition,
                    &mut current_track,
                    &mut current_cell,
                    &mut current_effect,
                    &mut current_effect_container,
                    &mut current_speed_track,
                    &mut current_event_track,
                    &mut current_special_cell,
                    &legacy_arpeggio_map,
                    &legacy_pitch_map,
                )?;

                end_element::handle_end_element(
                    &name,
                    &mut current_state,
                    format_version,
                    &mut instruments,
                    &mut arpeggios,
                    &mut pitch_tables,
                    &mut subsongs,
                    &mut current_instrument,
                    &mut current_instrument_cell,
                    &mut current_sample_builder,
                    &mut current_arpeggio,
                    legacy_arpeggio_note,
                    legacy_arpeggio_octave,
                    &mut current_pitch_table,
                    &mut current_subsong,
                    &mut current_psg,
                    &mut current_pattern,
                    &mut current_pattern_track_indexes,
                    &mut current_pattern_transpositions,
                    current_pattern_height,
                    &mut current_pattern_cell_track_number,
                    current_pattern_cell_transposition,
                    &mut current_track,
                    &mut current_cell,
                    &mut current_effect,
                    &mut current_effect_container,
                    &mut current_speed_track,
                    &mut current_event_track,
                    &mut current_special_cell,
                    &mut legacy_arpeggio_map,
                    &mut legacy_pitch_map,
                )?;

                current_text.clear();
            }

            Ok(Event::Eof) => break,
            Err(e) => return Err(ArkosError::from(e)),
            _ => {}
        }

        buf.clear();
    }

    Ok(AksSong {
        format: match format_version {
            FormatVersion::Legacy => SongFormat::Legacy,
            FormatVersion::Modern => SongFormat::Modern,
        },
        metadata,
        instruments,
        arpeggios,
        pitch_tables,
        subsongs,
    })
}
