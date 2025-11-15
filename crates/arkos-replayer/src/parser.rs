//! AKS XML parser for Format 3.0
//!
//! Parses Arkos Tracker 3 XML files into Rust data structures.

use std::collections::HashMap;
use std::sync::Arc;

use crate::effects::EffectType;
use crate::error::{ArkosError, Result};
use crate::format::*;
use crate::psg::split_note;
use base64::{Engine as _, engine::general_purpose};
use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormatVersion {
    Legacy,
    Modern,
}

/// Load an AKS file from bytes
///
/// Automatically detects whether the file is:
/// - Plain XML (test files)
/// - ZIP-compressed XML (production/packaged files)
///
/// # Arguments
///
/// * `data` - AKS file data (XML or ZIP)
pub fn load_aks(data: &[u8]) -> Result<AksSong> {
    // Check if it's a ZIP file (magic bytes: PK\x03\x04)
    if data.len() >= 4 && &data[0..2] == b"PK" {
        // ZIP-compressed AKS file
        return load_aks_zip(data);
    }

    // Plain XML AKS file
    load_aks_xml(data)
}

/// Load a ZIP-compressed AKS file
fn load_aks_zip(data: &[u8]) -> Result<AksSong> {
    use std::io::{Cursor, Read};
    use zip::ZipArchive;

    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ArkosError::InvalidFormat(format!("Not a valid ZIP file: {}", e)))?;

    // AKS ZIP files contain a single .aks file with the same name
    if archive.len() != 1 {
        return Err(ArkosError::InvalidFormat(format!(
            "Expected 1 file in ZIP, found {}",
            archive.len()
        )));
    }

    let mut file = archive
        .by_index(0)
        .map_err(|e| ArkosError::InvalidFormat(format!("Cannot read ZIP entry: {}", e)))?;

    let mut xml_data = Vec::new();
    file.read_to_end(&mut xml_data)
        .map_err(|e| ArkosError::IoError(e))?;

    load_aks_xml(&xml_data)
}

/// Parse state for tracking where we are in the XML
#[derive(Debug, Clone, PartialEq)]
enum ParseState {
    Root,
    Instruments,
    Instrument,
    InstrumentAutoSpread,
    InstrumentCells,
    InstrumentCell,
    Arpeggios,
    Arpeggio,
    LegacyArpeggioCell,
    PitchTables,
    PitchTable,
    Subsongs,
    Subsong,
    SubsongPsgs,
    SubsongPsg,
    SubsongPatterns,
    Pattern,
    PatternCell,
    PatternTrackIndexes,
    PatternSpeedTrackIndex,
    PatternEventTrackIndex,
    SpeedTracks,
    SpeedTrack,
    SpeedCell,
    EventTracks,
    EventTrack,
    EventCell,
    SubsongTracks,
    Track,
    Cell,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectContainer {
    Modern,
    Legacy,
}

#[derive(Debug, Clone)]
struct SampleInstrumentBuilder {
    frequency_hz: u32,
    amplification_ratio: f32,
    original_filename: Option<String>,
    loop_start_index: usize,
    end_index: usize,
    is_looping: bool,
    data: Option<Vec<f32>>,
    digidrum_note: i32,
}

impl Default for SampleInstrumentBuilder {
    fn default() -> Self {
        Self {
            frequency_hz: 44_100,
            amplification_ratio: 1.0,
            original_filename: None,
            loop_start_index: 0,
            end_index: 0,
            is_looping: false,
            data: None,
            digidrum_note: DEFAULT_DIGIDRUM_NOTE,
        }
    }
}

const DEFAULT_DIGIDRUM_NOTE: i32 = 12 * 6;

impl SampleInstrumentBuilder {
    fn build(self) -> Result<SampleInstrument> {
        let data = self
            .data
            .ok_or_else(|| ArkosError::InvalidFormat("Missing sample data".to_string()))?;

        Ok(SampleInstrument {
            frequency_hz: self.frequency_hz,
            amplification_ratio: if self.amplification_ratio == 0.0 {
                1.0
            } else {
                self.amplification_ratio
            },
            original_filename: self.original_filename,
            loop_start_index: self.loop_start_index,
            end_index: self.end_index,
            is_looping: self.is_looping,
            data: Arc::new(data),
            digidrum_note: self.digidrum_note,
        })
    }
}

fn local_name_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn parse_positions_block<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
) -> Result<Vec<Position>> {
    let mut positions = Vec::new();
    let mut current_position: Option<Position> = None;
    let mut current_field: Option<String> = None;

    loop {
        buf.clear();
        match reader.read_event_into(buf)? {
            Event::Start(e) => {
                let name = local_name_from_bytes(e.name().local_name().as_ref());
                match name.as_str() {
                    "position" => {
                        current_position = Some(Position {
                            pattern_index: 0,
                            height: 64,
                            marker_name: String::new(),
                            marker_color: 0,
                            transpositions: Vec::new(),
                        });
                    }
                    "patternIndex" | "height" | "markerName" | "markerColor" | "transposition" => {
                        current_field = Some(name);
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let name = local_name_from_bytes(e.name().local_name().as_ref());
                match name.as_str() {
                    "position" => {
                        if let Some(pos) = current_position.take() {
                            positions.push(pos);
                        }
                    }
                    "transpositions" => {}
                    _ => {}
                }
            }
            Event::Text(e) => {
                if let (Some(field), Some(pos)) =
                    (current_field.as_deref(), current_position.as_mut())
                {
                    let text = e.unescape()?.to_string();
                    match field {
                        "patternIndex" => pos.pattern_index = text.parse().unwrap_or(0),
                        "height" => pos.height = text.parse().unwrap_or(64),
                        "markerName" => pos.marker_name = text,
                        "markerColor" => pos.marker_color = text.parse().unwrap_or(0),
                        "transposition" => pos.transpositions.push(text.parse().unwrap_or(0)),
                        _ => {}
                    }
                }
            }
            Event::End(e) => {
                let name = local_name_from_bytes(e.name().local_name().as_ref());
                match name.as_str() {
                    "position" => {
                        if let Some(pos) = current_position.take() {
                            positions.push(pos);
                        }
                    }
                    "patternIndex" | "height" | "markerName" | "markerColor" | "transposition" => {
                        current_field = None;
                    }
                    "positions" => break,
                    _ => {}
                }
            }
            Event::Eof => {
                return Err(ArkosError::InvalidFormat(
                    "Unexpected EOF while parsing positions".to_string(),
                ));
            }
            _ => {}
        }
    }

    Ok(positions)
}

fn skip_block<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    tag: &str,
) -> Result<()> {
    let mut depth = 1;
    loop {
        buf.clear();
        match reader.read_event_into(buf)? {
            Event::Start(e) => {
                if e.name().local_name().as_ref() == tag.as_bytes() {
                    depth += 1;
                }
            }
            Event::End(e) => {
                if e.name().local_name().as_ref() == tag.as_bytes() {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            Event::Eof => {
                return Err(ArkosError::InvalidFormat(format!(
                    "Unexpected EOF while skipping {} block",
                    tag
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Load a plain XML AKS file (Format 3.0)
fn load_aks_xml(data: &[u8]) -> Result<AksSong> {
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
                // Use local_name() to strip namespace prefix (aks:element -> element)
                let name = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();
                // State transitions
                match name.as_str() {
                    // Note: Format 1.0 uses "aks:" prefix, Format 3.0 might not
                    "instruments" | "fmInstruments" => current_state = ParseState::Instruments,
                    "arpeggios" => current_state = ParseState::Arpeggios,
                    "pitchs" | "pitchTables" | "pitches" => current_state = ParseState::PitchTables,
                    "instrument" | "fmInstrument" if current_state == ParseState::Instruments => {
                        current_state = ParseState::Instrument;
                        current_instrument = Some(Instrument {
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
                    "cells" if current_state == ParseState::Instrument => {
                        current_state = ParseState::InstrumentCells
                    }
                    "autoSpread" if current_state == ParseState::Instrument => {
                        current_state = ParseState::InstrumentAutoSpread;
                    }
                    "cell" | "fmInstrumentCell"
                        if current_state == ParseState::InstrumentCells
                            || (current_state == ParseState::Instrument
                                && format_version == FormatVersion::Legacy) =>
                    {
                        current_state = ParseState::InstrumentCell;
                        current_instrument_cell = Some(InstrumentCell {
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
                    "expression" if current_state == ParseState::Arpeggios => {
                        current_state = ParseState::Arpeggio;
                        current_arpeggio = Some(Arpeggio {
                            index: 0,
                            name: String::new(),
                            values: Vec::new(),
                            speed: 0,
                            loop_start: 0,
                            end_index: 0,
                            shift: 0,
                        });
                    }
                    "arpeggio" if current_state == ParseState::Arpeggios => {
                        current_state = ParseState::Arpeggio;
                        current_arpeggio = Some(Arpeggio {
                            index: 0,
                            name: String::new(),
                            values: Vec::new(),
                            speed: 0,
                            loop_start: 0,
                            end_index: 0,
                            shift: 0,
                        });
                    }
                    "arpeggioCell" if current_state == ParseState::Arpeggio => {
                        current_state = ParseState::LegacyArpeggioCell;
                        legacy_arpeggio_note = 0;
                        legacy_arpeggio_octave = 0;
                    }
                    "expression" if current_state == ParseState::PitchTables => {
                        current_state = ParseState::PitchTable;
                        current_pitch_table = Some(PitchTable {
                            index: 0,
                            name: String::new(),
                            values: Vec::new(),
                            speed: 0,
                            loop_start: 0,
                            end_index: 0,
                            shift: 0,
                        });
                    }
                    "pitch" if current_state == ParseState::PitchTables => {
                        current_state = ParseState::PitchTable;
                        current_pitch_table = Some(PitchTable {
                            index: 0,
                            name: String::new(),
                            values: Vec::new(),
                            speed: 0,
                            loop_start: 0,
                            end_index: 0,
                            shift: 0,
                        });
                    }
                    "subsongs" => current_state = ParseState::Subsongs,
                    "subsong" => {
                        current_state = ParseState::Subsong;
                        current_subsong = Some(Subsong {
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
                    "psgs" if current_state == ParseState::Subsong => {
                        current_state = ParseState::SubsongPsgs
                    }
                    "psg" if current_state == ParseState::SubsongPsgs => {
                        current_state = ParseState::SubsongPsg;
                        current_psg = Some(PsgConfig::default());
                    }
                    "psgMetadata" if current_state == ParseState::Subsong => {
                        current_state = ParseState::SubsongPsg;
                        current_psg = Some(PsgConfig::default());
                    }
                    "positions" => {
                        if let Some(ref mut subsong) = current_subsong {
                            subsong.positions = parse_positions_block(&mut reader, &mut buf)?;
                        } else {
                            skip_block(&mut reader, &mut buf, "positions")?;
                        }
                        current_state = ParseState::Subsong;
                    }
                    "speedTracks" if current_state == ParseState::Subsong => {
                        current_state = ParseState::SpeedTracks;
                    }
                    "eventTracks" if current_state == ParseState::Subsong => {
                        current_state = ParseState::EventTracks;
                    }
                    "speedTrack" if current_state == ParseState::SpeedTracks => {
                        current_state = ParseState::SpeedTrack;
                        current_speed_track = Some(SpecialTrack {
                            index: 0,
                            cells: Vec::new(),
                        });
                    }
                    "eventTrack" if current_state == ParseState::EventTracks => {
                        current_state = ParseState::EventTrack;
                        current_event_track = Some(SpecialTrack {
                            index: 0,
                            cells: Vec::new(),
                        });
                    }
                    "patterns" if current_state == ParseState::Subsong => {
                        current_state = ParseState::SubsongPatterns
                    }
                    "pattern" if current_state == ParseState::SubsongPatterns => {
                        current_state = ParseState::Pattern;
                        current_pattern = Some(Pattern {
                            index: 0,
                            track_indexes: Vec::new(),
                            speed_track_index: 0,
                            event_track_index: 0,
                            color_argb: 0,
                        });
                        current_pattern_track_indexes.clear();
                        current_pattern_transpositions.clear();
                        current_pattern_height = 64;
                    }
                    "patternCell" if current_state == ParseState::Pattern => {
                        current_state = ParseState::PatternCell;
                        current_pattern_cell_track_number = None;
                        current_pattern_cell_transposition = 0;
                    }
                    "trackIndexes" if current_state == ParseState::Pattern => {
                        current_state = ParseState::PatternTrackIndexes
                    }
                    "speedTrackIndex" if current_state == ParseState::Pattern => {
                        current_state = ParseState::PatternSpeedTrackIndex
                    }
                    "eventTrackIndex" if current_state == ParseState::Pattern => {
                        current_state = ParseState::PatternEventTrackIndex
                    }
                    "tracks" if current_state == ParseState::Subsong => {
                        current_state = ParseState::SubsongTracks
                    }
                    "track" if current_state == ParseState::SubsongTracks => {
                        current_state = ParseState::Track;
                        current_track = Some(Track {
                            index: 0,
                            cells: Vec::new(),
                        });
                    }
                    "cell" if current_state == ParseState::Track => {
                        current_state = ParseState::Cell;
                        current_cell = Some(Cell {
                            index: 0,
                            note: 255, // No note
                            instrument: 0,
                            instrument_present: false,
                            effects: Vec::new(),
                        });
                    }
                    "cell" | "speedCell" if current_state == ParseState::SpeedTrack => {
                        current_state = ParseState::SpeedCell;
                        current_special_cell = Some(SpecialCell { index: 0, value: 0 });
                    }
                    "cell" | "eventCell" if current_state == ParseState::EventTrack => {
                        current_state = ParseState::EventCell;
                        current_special_cell = Some(SpecialCell { index: 0, value: 0 });
                    }
                    "effect" | "effectAndValue" if current_state == ParseState::Cell => {
                        current_state = ParseState::Effect;
                        current_effect_container = Some(if name == "effectAndValue" {
                            EffectContainer::Legacy
                        } else {
                            EffectContainer::Modern
                        });
                        current_effect = Some(Effect {
                            index: 0,
                            name: String::new(),
                            logical_value: 0,
                        });
                    }
                    _ => {}
                }

                current_text.clear();
            }

            Ok(Event::Text(e)) => {
                current_text = e.unescape()?.to_string();
            }

            Ok(Event::End(e)) => {
                // Use local_name() to strip namespace prefix (aks:element -> element)
                let name = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();

                // Handle text content based on current element
                match (current_state.clone(), name.as_str()) {
                    // Metadata
                    (ParseState::Root, "formatVersion") => {
                        let trimmed = current_text.trim();
                        format_version = if trimmed.starts_with('1') {
                            FormatVersion::Legacy
                        } else {
                            FormatVersion::Modern
                        };
                        if format_version == FormatVersion::Legacy && !legacy_defaults_inserted {
                            arpeggios.push(Arpeggio {
                                index: 0,
                                name: String::new(),
                                values: vec![0],
                                speed: 0,
                                loop_start: 0,
                                end_index: 0,
                                shift: 0,
                            });
                            pitch_tables.push(PitchTable {
                                index: 0,
                                name: String::new(),
                                values: vec![0],
                                speed: 0,
                                loop_start: 0,
                                end_index: 0,
                                shift: 0,
                            });
                            legacy_defaults_inserted = true;
                        }
                    }
                    (ParseState::Root, "title") => metadata.title = current_text.clone(),
                    (ParseState::Root, "author") => metadata.author = current_text.clone(),
                    (ParseState::Root, "composer") => metadata.composer = current_text.clone(),
                    (ParseState::Root, "comment") => metadata.comments = current_text.clone(),

                    // Instrument fields
                    (ParseState::Instrument, "name") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.name = current_text.clone();
                        }
                    }
                    (ParseState::Instrument, "colorArgb") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.color_argb = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Instrument, "type") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.instrument_type = match current_text.to_lowercase().as_str() {
                                "psg" => InstrumentType::Psg,
                                "digi" | "sample" => InstrumentType::Digi,
                                _ => InstrumentType::Psg,
                            };

                            if instr.instrument_type == InstrumentType::Digi {
                                current_sample_builder = Some(SampleInstrumentBuilder::default());
                            } else {
                                current_sample_builder = None;
                            }
                        }
                    }
                    (ParseState::Instrument, "speed") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.speed = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Instrument, "isRetrig") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.is_retrig = current_text == "true";
                        }
                    }
                    (ParseState::Instrument, "loopStartIndex") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.loop_start_index = current_text.parse().unwrap_or(0);
                            if instr.instrument_type == InstrumentType::Digi {
                                if let Some(ref mut builder) = current_sample_builder {
                                    builder.loop_start_index = instr.loop_start_index;
                                }
                            }
                        }
                    }
                    (ParseState::Instrument, "endIndex") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.end_index = current_text.parse().unwrap_or(0);
                            if instr.instrument_type == InstrumentType::Digi {
                                if let Some(ref mut builder) = current_sample_builder {
                                    builder.end_index = instr.end_index;
                                }
                            }
                        }
                    }
                    (ParseState::Instrument, "isLooping") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.is_looping = current_text == "true";
                            if instr.instrument_type == InstrumentType::Digi {
                                if let Some(ref mut builder) = current_sample_builder {
                                    builder.is_looping = instr.is_looping;
                                }
                            }
                        }
                    }
                    (ParseState::InstrumentAutoSpread, _) => {
                        // Ignore loop/index fields inside autoSpread blocks
                    }
                    (ParseState::Instrument, "isSfxExported") => {
                        if let Some(ref mut instr) = current_instrument {
                            instr.is_sfx_exported = current_text == "true";
                        }
                    }
                    (ParseState::Instrument, "frequencyHz") => {
                        if let (Some(ref mut builder), Some(ref instr)) =
                            (current_sample_builder.as_mut(), current_instrument.as_ref())
                        {
                            if instr.instrument_type == InstrumentType::Digi {
                                builder.frequency_hz = current_text.parse().unwrap_or(44_100);
                            }
                        }
                    }
                    (ParseState::Instrument, "amplificationRatio") => {
                        if let (Some(ref mut builder), Some(ref instr)) =
                            (current_sample_builder.as_mut(), current_instrument.as_ref())
                        {
                            if instr.instrument_type == InstrumentType::Digi {
                                builder.amplification_ratio = current_text.parse().unwrap_or(1.0);
                            }
                        }
                    }
                    (ParseState::Instrument, "originalFilename") => {
                        if let (Some(ref mut builder), Some(ref instr)) =
                            (current_sample_builder.as_mut(), current_instrument.as_ref())
                        {
                            if instr.instrument_type == InstrumentType::Digi {
                                builder.original_filename = Some(current_text.clone());
                            }
                        }
                    }
                    (ParseState::Instrument, "digiNote") => {
                        if let (Some(ref mut builder), Some(ref instr)) =
                            (current_sample_builder.as_mut(), current_instrument.as_ref())
                        {
                            if instr.instrument_type == InstrumentType::Digi {
                                builder.digidrum_note =
                                    current_text.parse().unwrap_or(DEFAULT_DIGIDRUM_NOTE);
                            }
                        }
                    }
                    (ParseState::Instrument, "sampleUnsigned8BitsBase64") => {
                        if let (Some(builder), Some(instr)) =
                            (current_sample_builder.as_mut(), current_instrument.as_ref())
                        {
                            if instr.instrument_type == InstrumentType::Digi {
                                let sanitized: String = current_text
                                    .chars()
                                    .filter(|c| !c.is_whitespace())
                                    .collect();
                                let decoded =
                                    general_purpose::STANDARD.decode(sanitized).map_err(|e| {
                                        ArkosError::InvalidFormat(format!(
                                            "Invalid sample data: {e}"
                                        ))
                                    })?;
                                let pcm: Vec<f32> = decoded
                                    .into_iter()
                                    .map(|byte| (byte as f32 - 128.0) / 128.0)
                                    .collect();
                                builder.data = Some(pcm);
                            }
                        }
                    }

                    // Instrument cell fields
                    (ParseState::InstrumentCell, "volume") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.volume = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "noise") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.noise = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "primaryPeriod")
                    | (ParseState::InstrumentCell, "softwarePeriod") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.primary_period = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "primaryArpeggioNoteInOctave") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.primary_arpeggio_note_in_octave =
                                current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "softwareArpeggio") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            let value = current_text.parse().unwrap_or(0);
                            let (note_in_octave, octave) = split_note(value);
                            cell.primary_arpeggio_note_in_octave = note_in_octave as u8;
                            cell.primary_arpeggio_octave = octave as i8;
                        }
                    }
                    (ParseState::InstrumentCell, "primaryArpeggioOctave") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.primary_arpeggio_octave = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "primaryPitch")
                    | (ParseState::InstrumentCell, "softwarePitch") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.primary_pitch = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "link") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.link = match current_text.as_str() {
                                "noSoftwareNoHardware" | "noSoftNoHard" => {
                                    ChannelLink::NoSoftwareNoHardware
                                }
                                "softwareOnly" | "softOnly" => ChannelLink::SoftwareOnly,
                                "hardwareOnly" | "hardOnly" => ChannelLink::HardwareOnly,
                                "softwareAndHardware" | "softAndHard" => {
                                    ChannelLink::SoftwareAndHardware
                                }
                                "softwareToHardware" | "softToHard" => {
                                    ChannelLink::SoftwareToHardware
                                }
                                "hardwareToSoftware" | "hardToSoft" => {
                                    ChannelLink::HardwareToSoftware
                                }
                                _ => ChannelLink::NoSoftwareNoHardware,
                            };
                        }
                    }
                    (ParseState::InstrumentCell, "ratio") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.ratio = current_text.parse().unwrap_or(4);
                        }
                    }
                    (ParseState::InstrumentCell, "hardwareEnvelope")
                    | (ParseState::InstrumentCell, "hardwareCurve") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.hardware_envelope = current_text.parse().unwrap_or(8);
                        }
                    }
                    (ParseState::InstrumentCell, "secondaryPeriod")
                    | (ParseState::InstrumentCell, "hardwarePeriod") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.secondary_period = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "secondaryArpeggioNoteInOctave") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.secondary_arpeggio_note_in_octave =
                                current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "hardwareArpeggio") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            let value = current_text.parse().unwrap_or(0);
                            let (note_in_octave, octave) = split_note(value);
                            cell.secondary_arpeggio_note_in_octave = note_in_octave as u8;
                            cell.secondary_arpeggio_octave = octave as i8;
                        }
                    }
                    (ParseState::InstrumentCell, "secondaryArpeggioOctave") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.secondary_arpeggio_octave = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "secondaryPitch")
                    | (ParseState::InstrumentCell, "hardwarePitch") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.secondary_pitch = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::InstrumentCell, "isRetrig") => {
                        if let Some(ref mut cell) = current_instrument_cell {
                            cell.is_retrig = current_text == "true";
                        }
                    }

                    // Arpeggio fields
                    (ParseState::Arpeggio, "index") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Arpeggio, "name") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.name = current_text.clone();
                        }
                    }
                    (ParseState::Arpeggio, "speed") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.speed = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Arpeggio, "loopStartIndex") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.loop_start = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Arpeggio, "endIndex") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.end_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Arpeggio, "shift") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            arp.shift = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Arpeggio, "value") => {
                        if let Some(ref mut arp) = current_arpeggio {
                            let value = current_text.parse().unwrap_or(0);
                            arp.values.push(value);
                        }
                    }
                    (ParseState::LegacyArpeggioCell, "note") => {
                        legacy_arpeggio_note = current_text.parse().unwrap_or(0);
                    }
                    (ParseState::LegacyArpeggioCell, "octave") => {
                        legacy_arpeggio_octave = current_text.parse().unwrap_or(0);
                    }

                    // Pitch table fields
                    (ParseState::PitchTable, "index") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PitchTable, "name") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.name = current_text.clone();
                        }
                    }
                    (ParseState::PitchTable, "speed") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.speed = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PitchTable, "loopStartIndex") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.loop_start = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PitchTable, "endIndex") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.end_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PitchTable, "shift") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            pitch.shift = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PitchTable, "value") => {
                        if let Some(ref mut pitch) = current_pitch_table {
                            let value = current_text.parse().unwrap_or(0);
                            pitch.values.push(value);
                        }
                    }

                    // Pattern fields
                    (ParseState::PatternTrackIndexes, "trackIndex") => {
                        let track_index = current_text.parse().unwrap_or(0);
                        current_pattern_track_indexes.push(track_index);
                    }
                    (ParseState::Pattern, "height") => {
                        current_pattern_height = current_text.parse().unwrap_or(64);
                    }
                    (ParseState::PatternSpeedTrackIndex, "trackIndex") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.speed_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PatternEventTrackIndex, "trackIndex") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.event_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PatternSpeedTrackIndex, "speedTrackNumber") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.speed_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PatternEventTrackIndex, "eventTrackNumber") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.event_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::PatternCell, "trackNumber")
                    | (ParseState::PatternCell, "trackIndex") => {
                        current_pattern_cell_track_number = Some(current_text.parse().unwrap_or(0));
                    }
                    (ParseState::PatternCell, "transposition") => {
                        current_pattern_cell_transposition =
                            current_text.parse::<i32>().unwrap_or(0) as i8;
                    }
                    (ParseState::Pattern, "colorArgb") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.color_argb = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Pattern, "speedTrackNumber") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.speed_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Pattern, "eventTrackNumber") => {
                        if let Some(ref mut pat) = current_pattern {
                            pat.event_track_index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::SpeedTrack, "number") | (ParseState::SpeedTrack, "index") => {
                        if let Some(ref mut track) = current_speed_track {
                            track.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::EventTrack, "number") | (ParseState::EventTrack, "index") => {
                        if let Some(ref mut track) = current_event_track {
                            track.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::SpeedCell, "index") | (ParseState::EventCell, "index") => {
                        if let Some(ref mut cell) = current_special_cell {
                            cell.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::SpeedCell, "value") | (ParseState::EventCell, "value") => {
                        if let Some(ref mut cell) = current_special_cell {
                            cell.value = current_text.parse().unwrap_or(0);
                        }
                    }

                    // Subsong fields
                    (ParseState::Subsong, "title") => {
                        if let Some(ref mut s) = current_subsong {
                            s.title = current_text.clone();
                        }
                    }
                    (ParseState::Subsong, "initialSpeed") => {
                        if let Some(ref mut s) = current_subsong {
                            s.initial_speed = current_text.parse().unwrap_or(6);
                        }
                    }
                    (ParseState::Subsong, "endPosition") | (ParseState::Subsong, "endIndex") => {
                        if let Some(ref mut s) = current_subsong {
                            s.end_position = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Subsong, "loopStartPosition")
                    | (ParseState::Subsong, "loopStartIndex") => {
                        if let Some(ref mut s) = current_subsong {
                            s.loop_start_position = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Subsong, "replayFrequencyHz")
                    | (ParseState::Subsong, "replayFrequency") => {
                        if let Some(ref mut s) = current_subsong {
                            s.replay_frequency_hz = current_text.parse().unwrap_or(50.0);
                        }
                    }
                    (ParseState::Subsong, "digiChannel") => {
                        if let Some(ref mut s) = current_subsong {
                            s.digi_channel = current_text.parse().unwrap_or(0);
                        }
                    }

                    // PSG fields
                    (ParseState::SubsongPsg, "type") => {
                        if let Some(ref mut psg) = current_psg {
                            psg.psg_type = match current_text.to_lowercase().as_str() {
                                "ym" => PsgType::YM,
                                "ay" => PsgType::AY,
                                _ => PsgType::YM,
                            };
                        }
                    }
                    (ParseState::SubsongPsg, "frequencyHz")
                    | (ParseState::SubsongPsg, "psgFrequency") => {
                        if let Some(ref mut psg) = current_psg {
                            psg.psg_frequency = current_text.parse().unwrap_or(2_000_000);
                        }
                    }
                    (ParseState::SubsongPsg, "referenceFrequencyHz")
                    | (ParseState::SubsongPsg, "referenceFrequency") => {
                        if let Some(ref mut psg) = current_psg {
                            psg.reference_frequency = current_text.parse().unwrap_or(440.0);
                        }
                    }
                    (ParseState::SubsongPsg, "samplePlayerFrequencyHz")
                    | (ParseState::SubsongPsg, "samplePlayerFrequency") => {
                        if let Some(ref mut psg) = current_psg {
                            psg.sample_player_frequency = current_text.parse().unwrap_or(11025);
                        }
                    }
                    (ParseState::SubsongPsg, "mixingOutput") => {
                        if let Some(ref mut psg) = current_psg {
                            psg.mixing_output = match current_text.as_str() {
                                "ABC" => MixingOutput::ABC,
                                "ACB" => MixingOutput::ACB,
                                "BAC" => MixingOutput::BAC,
                                "BCA" => MixingOutput::BCA,
                                "CAB" => MixingOutput::CAB,
                                "CBA" => MixingOutput::CBA,
                                _ => MixingOutput::ABC,
                            };
                        }
                    }

                    // Track fields
                    (ParseState::Track, "index") | (ParseState::Track, "number") => {
                        if let Some(ref mut t) = current_track {
                            t.index = current_text.parse().unwrap_or(0);
                        }
                    }

                    // Cell fields
                    (ParseState::Cell, "index") => {
                        if let Some(ref mut c) = current_cell {
                            c.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Cell, "note") => {
                        if let Some(ref mut c) = current_cell {
                            c.note = current_text.parse().unwrap_or(255);
                        }
                    }
                    (ParseState::Cell, "instrument") => {
                        if let Some(ref mut c) = current_cell {
                            let parsed = current_text.parse::<i32>().unwrap_or(-1);
                            if format_version == FormatVersion::Legacy {
                                if parsed <= 0 {
                                    c.instrument = usize::MAX;
                                    c.instrument_present = true;
                                } else {
                                    c.instrument = (parsed - 1) as usize;
                                    c.instrument_present = true;
                                }
                            } else if parsed >= 0 {
                                c.instrument = parsed as usize;
                                c.instrument_present = true;
                            } else {
                                c.instrument_present = false;
                            }
                        }
                    }

                    // Effect fields
                    (ParseState::Effect, "index") => {
                        if let Some(ref mut eff) = current_effect {
                            eff.index = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Effect, "name") => {
                        if let Some(ref mut eff) = current_effect {
                            eff.name = current_text.clone();
                        }
                    }
                    (ParseState::Effect, "effect") => {
                        if current_effect_container == Some(EffectContainer::Legacy) {
                            if let Some(ref mut eff) = current_effect {
                                eff.name = current_text.clone();
                            }
                        }
                    }
                    (ParseState::Effect, "logicalValue") => {
                        if let Some(ref mut eff) = current_effect {
                            eff.logical_value = current_text.parse().unwrap_or(0);
                        }
                    }
                    (ParseState::Effect, "hexValue") => {
                        if let Some(ref mut eff) = current_effect {
                            let trimmed = current_text.trim();
                            let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
                            if let Ok(mut value) = i32::from_str_radix(hex, 16) {
                                if format_version == FormatVersion::Legacy {
                                    let effect_type = EffectType::from_name(&eff.name);
                                    value = effect_type.decode_legacy_value(value);
                                    if value > 0 {
                                        match effect_type {
                                            EffectType::ArpeggioTable => {
                                                if let Some(&mapped) =
                                                    legacy_arpeggio_map.get(&(value as usize))
                                                {
                                                    value = mapped as i32;
                                                }
                                            }
                                            EffectType::PitchTable => {
                                                if let Some(&mapped) =
                                                    legacy_pitch_map.get(&(value as usize))
                                                {
                                                    value = mapped as i32;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                eff.logical_value = value;
                            }
                        }
                    }

                    _ => {}
                }

                // State transitions on closing tags
                match name.as_str() {
                    "trackIndexes" if current_state == ParseState::PatternTrackIndexes => {
                        current_state = ParseState::Pattern;
                    }
                    "patternCell" if current_state == ParseState::PatternCell => {
                        if let Some(track_index) = current_pattern_cell_track_number.take() {
                            current_pattern_track_indexes.push(track_index);
                            current_pattern_transpositions.push(current_pattern_cell_transposition);
                        }
                        current_state = ParseState::Pattern;
                    }
                    "speedTrackIndex" if current_state == ParseState::PatternSpeedTrackIndex => {
                        current_state = ParseState::Pattern;
                    }
                    "eventTrackIndex" if current_state == ParseState::PatternEventTrackIndex => {
                        current_state = ParseState::Pattern;
                    }
                    "pattern" => {
                        if let Some(mut pattern) = current_pattern.take() {
                            // Set the track_indexes from the accumulated list
                            pattern.track_indexes = current_pattern_track_indexes.clone();
                            // Pattern index is implicit (position in vector)
                            if let Some(ref mut subsong) = current_subsong {
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
                        current_state = ParseState::SubsongPatterns;
                    }
                    "patterns" if current_state == ParseState::SubsongPatterns => {
                        current_state = ParseState::Subsong;
                    }
                    "position" => {}
                    "positions" => {}
                    "cell" | "fmInstrumentCell" if current_state == ParseState::InstrumentCell => {
                        if let (Some(cell), Some(ref mut instr)) =
                            (current_instrument_cell.take(), current_instrument.as_mut())
                        {
                            instr.cells.push(cell);
                        }
                        current_state = if format_version == FormatVersion::Legacy {
                            ParseState::Instrument
                        } else {
                            ParseState::InstrumentCells
                        };
                    }
                    "cells" if current_state == ParseState::InstrumentCells => {
                        current_state = ParseState::Instrument;
                    }
                    "autoSpread" if current_state == ParseState::InstrumentAutoSpread => {
                        current_state = ParseState::Instrument;
                    }
                    "instrument" | "fmInstrument" if current_state == ParseState::Instrument => {
                        if let Some(mut instr) = current_instrument.take() {
                            if instr.instrument_type == InstrumentType::Digi {
                                if let Some(builder) = current_sample_builder.take() {
                                    let mut finalized_builder = builder;
                                    finalized_builder.loop_start_index = instr.loop_start_index;
                                    finalized_builder.end_index = instr.end_index;
                                    finalized_builder.is_looping = instr.is_looping;
                                    instr.sample = Some(finalized_builder.build()?);
                                }
                            }
                            instruments.push(instr);
                        }
                        current_state = ParseState::Instruments;
                    }
                    "instruments" | "fmInstruments" if current_state == ParseState::Instruments => {
                        current_state = ParseState::Root;
                    }
                    "expression" if current_state == ParseState::Arpeggio => {
                        if let Some(arp) = current_arpeggio.take() {
                            if format_version == FormatVersion::Legacy {
                                legacy_arpeggio_map.insert(arp.index, arpeggios.len());
                            }
                            arpeggios.push(arp);
                        }
                        current_state = ParseState::Arpeggios;
                    }
                    "arpeggio" if current_state == ParseState::Arpeggio => {
                        if let Some(arp) = current_arpeggio.take() {
                            if format_version == FormatVersion::Legacy {
                                legacy_arpeggio_map.insert(arp.index, arpeggios.len());
                            }
                            arpeggios.push(arp);
                        }
                        current_state = ParseState::Arpeggios;
                    }
                    "arpeggioCell" if current_state == ParseState::LegacyArpeggioCell => {
                        if let Some(arp) = current_arpeggio.as_mut() {
                            let value = (legacy_arpeggio_octave * 12 + legacy_arpeggio_note)
                                .clamp(i8::MIN as i32, i8::MAX as i32)
                                as i8;
                            arp.values.push(value);
                        }
                        current_state = ParseState::Arpeggio;
                    }
                    "arpeggios" if current_state == ParseState::Arpeggios => {
                        current_state = ParseState::Root;
                    }
                    "expression" if current_state == ParseState::PitchTable => {
                        if let Some(pitch) = current_pitch_table.take() {
                            if format_version == FormatVersion::Legacy {
                                legacy_pitch_map.insert(pitch.index, pitch_tables.len());
                            }
                            pitch_tables.push(pitch);
                        }
                        current_state = ParseState::PitchTables;
                    }
                    "pitch" if current_state == ParseState::PitchTable => {
                        if let Some(pitch) = current_pitch_table.take() {
                            if format_version == FormatVersion::Legacy {
                                legacy_pitch_map.insert(pitch.index, pitch_tables.len());
                            }
                            pitch_tables.push(pitch);
                        }
                        current_state = ParseState::PitchTables;
                    }
                    "pitchs" | "pitchTables" if current_state == ParseState::PitchTables => {
                        current_state = ParseState::Root;
                    }
                    "psg" | "psgMetadata" if current_state == ParseState::SubsongPsg => {
                        if let (Some(psg), Some(ref mut subsong)) =
                            (current_psg.take(), current_subsong.as_mut())
                        {
                            subsong.psgs.push(psg);
                        }
                        current_state = if format_version == FormatVersion::Legacy {
                            ParseState::Subsong
                        } else {
                            ParseState::SubsongPsgs
                        };
                    }
                    "psgs" if current_state == ParseState::SubsongPsgs => {
                        current_state = ParseState::Subsong;
                    }
                    "effect" if current_state == ParseState::Effect => {
                        if current_effect_container == Some(EffectContainer::Modern) {
                            if let (Some(eff), Some(ref mut cell)) =
                                (current_effect.take(), current_cell.as_mut())
                            {
                                cell.effects.push(eff);
                            }
                            current_effect_container = None;
                            current_state = ParseState::Cell;
                        }
                    }
                    "effectAndValue" if current_state == ParseState::Effect => {
                        if let (Some(eff), Some(ref mut cell)) =
                            (current_effect.take(), current_cell.as_mut())
                        {
                            cell.effects.push(eff);
                        }
                        current_effect_container = None;
                        current_state = ParseState::Cell;
                    }
                    "cell" if current_state == ParseState::Cell => {
                        if let (Some(cell), Some(ref mut track)) =
                            (current_cell.take(), current_track.as_mut())
                        {
                            track.cells.push(cell);
                        }
                        current_state = ParseState::Track;
                    }
                    "track" if current_state == ParseState::Track => {
                        if let (Some(track), Some(ref mut subsong)) =
                            (current_track.take(), current_subsong.as_mut())
                        {
                            let track_index = track.index;
                            subsong.tracks.insert(track_index, track);
                        }
                        current_state = ParseState::SubsongTracks;
                    }
                    "cell" | "speedCell" if current_state == ParseState::SpeedCell => {
                        if let (Some(cell), Some(ref mut track)) =
                            (current_special_cell.take(), current_speed_track.as_mut())
                        {
                            track.cells.push(cell);
                        }
                        current_state = ParseState::SpeedTrack;
                    }
                    "cell" | "eventCell" if current_state == ParseState::EventCell => {
                        if let (Some(cell), Some(ref mut track)) =
                            (current_special_cell.take(), current_event_track.as_mut())
                        {
                            track.cells.push(cell);
                        }
                        current_state = ParseState::EventTrack;
                    }
                    "speedTrack" if current_state == ParseState::SpeedTrack => {
                        if let (Some(track), Some(ref mut subsong)) =
                            (current_speed_track.take(), current_subsong.as_mut())
                        {
                            subsong.speed_tracks.insert(track.index, track);
                        }
                        current_state = ParseState::SpeedTracks;
                    }
                    "eventTrack" if current_state == ParseState::EventTrack => {
                        if let (Some(track), Some(ref mut subsong)) =
                            (current_event_track.take(), current_subsong.as_mut())
                        {
                            subsong.event_tracks.insert(track.index, track);
                        }
                        current_state = ParseState::EventTracks;
                    }
                    "speedTracks" if current_state == ParseState::SpeedTracks => {
                        current_state = ParseState::Subsong;
                    }
                    "eventTracks" if current_state == ParseState::EventTracks => {
                        current_state = ParseState::Subsong;
                    }
                    "tracks" if current_state == ParseState::SubsongTracks => {
                        current_state = ParseState::Subsong;
                    }
                    "subsong" => {
                        if let Some(subsong) = current_subsong.take() {
                            subsongs.push(subsong);
                        }
                        current_state = ParseState::Subsongs;
                    }
                    "subsongs" if current_state == ParseState::Subsongs => {
                        current_state = ParseState::Root;
                    }
                    _ => {}
                }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format_3_metadata() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song xmlns:aks="https://www.julien-nevo.com/arkostracker/ArkosTrackerSong">
  <formatVersion>3.0</formatVersion>
  <title>Test Song</title>
  <author>Test Author</author>
  <composer>Test Composer</composer>
  <comment>Test Comment</comment>
</song>"#;

        let song = load_aks(xml.as_bytes()).unwrap();
        assert_eq!(song.metadata.title, "Test Song");
        assert_eq!(song.metadata.author, "Test Author");
        assert_eq!(song.metadata.composer, "Test Composer");
    }

    #[test]
    fn test_parse_subsong_with_psg() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Test</title>
  <subsongs>
    <subsong>
      <title>Main</title>
      <initialSpeed>6</initialSpeed>
      <replayFrequencyHz>50</replayFrequencyHz>
      <psgs>
        <psg>
          <type>ym</type>
          <frequencyHz>2000000</frequencyHz>
          <referenceFrequencyHz>440</referenceFrequencyHz>
          <samplePlayerFrequencyHz>11025</samplePlayerFrequencyHz>
          <mixingOutput>ABC</mixingOutput>
        </psg>
      </psgs>
    </subsong>
  </subsongs>
</song>"#;

        let song = load_aks(xml.as_bytes()).unwrap();
        assert_eq!(song.subsongs.len(), 1);
        assert_eq!(song.subsongs[0].title, "Main");
        assert_eq!(song.subsongs[0].psgs.len(), 1);
        assert_eq!(song.subsongs[0].psgs[0].psg_frequency, 2_000_000);
    }

    #[cfg(feature = "extended-tests")]
    #[test]
    fn test_parse_patterns() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Test</title>
  <subsongs>
    <subsong>
      <title>Main</title>
      <positions>
        <position>
          <patternIndex>0</patternIndex>
          <height>32</height>
          <markerName>Start</markerName>
          <markerColor>4282400896</markerColor>
          <transpositions/>
        </position>
        <position>
          <patternIndex>1</patternIndex>
          <height>64</height>
          <markerName></markerName>
          <markerColor>4282400896</markerColor>
          <transpositions/>
        </position>
      </positions>
      <patterns>
        <pattern>
          <trackIndexes>
            <trackIndex>0</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>1</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>2</trackIndex>
          </trackIndexes>
          <speedTrackIndex>
            <trackIndex>0</trackIndex>
          </speedTrackIndex>
          <eventTrackIndex>
            <trackIndex>0</trackIndex>
          </eventTrackIndex>
          <colorArgb>4286611584</colorArgb>
        </pattern>
        <pattern>
          <trackIndexes>
            <trackIndex>3</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>4</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>5</trackIndex>
          </trackIndexes>
          <speedTrackIndex>
            <trackIndex>1</trackIndex>
          </speedTrackIndex>
          <eventTrackIndex>
            <trackIndex>1</trackIndex>
          </eventTrackIndex>
          <colorArgb>4286611584</colorArgb>
        </pattern>
      </patterns>
      <psgs>
        <psg>
          <type>ym</type>
        </psg>
      </psgs>
    </subsong>
  </subsongs>
</song>"#;

        let song = load_aks(xml.as_bytes()).unwrap();
        assert_eq!(song.subsongs.len(), 1);

        let subsong = &song.subsongs[0];

        // Check positions
        assert_eq!(subsong.positions.len(), 2);
        assert_eq!(subsong.positions[0].pattern_index, 0);
        assert_eq!(subsong.positions[0].height, 32);
        assert_eq!(subsong.positions[0].marker_name, "Start");
        assert_eq!(subsong.positions[1].pattern_index, 1);
        assert_eq!(subsong.positions[1].height, 64);

        // Check patterns
        assert_eq!(subsong.patterns.len(), 2);

        let pattern0 = &subsong.patterns[0];
        assert_eq!(pattern0.index, 0);
        assert_eq!(pattern0.track_indexes, vec![0, 1, 2]);
        assert_eq!(pattern0.speed_track_index, 0);
        assert_eq!(pattern0.event_track_index, 0);
        assert_eq!(pattern0.color_argb, 4286611584);

        let pattern1 = &subsong.patterns[1];
        assert_eq!(pattern1.index, 1);
        assert_eq!(pattern1.track_indexes, vec![3, 4, 5]);
        assert_eq!(pattern1.speed_track_index, 1);
        assert_eq!(pattern1.event_track_index, 1);
    }

    #[cfg(feature = "extended-tests")]
    #[test]
    fn test_load_real_aks_file() {
        // Load a real AKS file to ensure everything parses correctly
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arkos/Doclands - Pong Cracktro (YM).aks");
        let data = std::fs::read(&path).expect("Doclands test AKS file missing");
        let song = load_aks(&data).expect("failed to parse Doclands AKS file");

        assert!(song.subsongs.len() > 0, "expected subsongs in {:?}", path);
        let subsong = &song.subsongs[0];
        eprintln!(
            "doclands subsong debug: positions {} patterns {} tracks {}",
            subsong.positions.len(),
            subsong.patterns.len(),
            subsong.tracks.len()
        );
        assert!(subsong.psgs.len() > 0);
        assert!(subsong.positions.len() > 0);
        assert!(subsong.patterns.len() > 0);
    }

    #[cfg(feature = "extended-tests")]
    #[test]
    fn test_load_real_at2_file() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
        let data = std::fs::read(&path).expect("AT2 test AKS file missing");
        let song = load_aks(&data).expect("failed to parse AT2 AKS file");

        assert!(!song.subsongs.is_empty(), "expected subsongs in AT2 song");
        let subsong = &song.subsongs[0];
        assert!(subsong.psgs.len() > 0);
        assert!(subsong.positions.len() > 0);
        assert!(subsong.patterns.len() > 0);
        assert!(
            subsong.speed_tracks.contains_key(&0),
            "expected speed track 0 in AT2 song"
        );
    }

    #[cfg(feature = "extended-tests")]
    #[test]
    fn test_at2_patterns_cover_all_channels() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
        let data = std::fs::read(&path).expect("AT2 test AKS file missing");
        let song = load_aks(&data).expect("failed to parse AT2 AKS file");

        let subsong = &song.subsongs[0];
        let channel_count = subsong.psgs.len() * 3;
        assert!(channel_count > 0, "song should define at least one PSG");

        for pattern in &subsong.patterns {
            assert_eq!(
                pattern.track_indexes.len(),
                channel_count,
                "pattern {} should provide a track index per channel",
                pattern.index
            );
        }
    }

    #[cfg(feature = "extended-tests")]
    #[test]
    fn test_at2_track_effects_have_names() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
        let data = std::fs::read(&path).expect("AT2 test AKS file missing");
        let song = load_aks(&data).expect("failed to parse AT2 AKS file");

        let subsong = &song.subsongs[0];
        let track = subsong.tracks.get(&1).expect("missing track 1");
        let cell = track
            .cells
            .iter()
            .find(|c| c.index == 0)
            .expect("missing cell 0");
        println!(
            "track1 cell0 instrument_present={} instrument={} format instruments={}",
            cell.instrument_present,
            cell.instrument,
            song.instruments.len()
        );
        assert!(
            !cell.effects.is_empty(),
            "expected effects parsed for track 1 cell 0"
        );
        let effect = &cell.effects[0];
        assert_eq!(effect.name, "volume");
        assert_eq!(effect.logical_value, 0xF);

        let arp_track = subsong.tracks.get(&22).expect("missing track 22");
        let arp_cell = arp_track
            .cells
            .iter()
            .find(|c| c.index == 0)
            .expect("missing track 22 cell 0");
        if let Some(vol_effect) = arp_cell.effects.iter().find(|eff| eff.name == "volume") {
            println!("track22 volume effect {}", vol_effect.logical_value);
        }
        let arp_effect = arp_cell
            .effects
            .iter()
            .find(|eff| eff.name == "arpeggioTable")
            .expect("missing arpeggioTable effect");
        assert_eq!(arp_effect.logical_value, 2);
    }
}
