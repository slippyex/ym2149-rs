//! Handles XML text content during AKS parsing.

use crate::effects::EffectType;
use crate::error::{ArkosError, Result};
use crate::format::*;
use crate::psg::split_note;
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;

use super::super::state::*;

/// Handles text content for the current element.
///
/// Called when text content is found between element tags. Updates the
/// appropriate field based on current parse state and element name.
#[allow(clippy::too_many_arguments)]
pub fn handle_text_content(
    current_state: &ParseState,
    name: &str,
    current_text: &str,
    format_version: FormatVersion,
    metadata: &mut SongMetadata,
    format_version_out: &mut FormatVersion,
    legacy_defaults_inserted: &mut bool,
    arpeggios: &mut Vec<Arpeggio>,
    pitch_tables: &mut Vec<PitchTable>,
    current_instrument: &mut Option<Instrument>,
    current_instrument_cell: &mut Option<InstrumentCell>,
    current_sample_builder: &mut Option<SampleInstrumentBuilder>,
    current_arpeggio: &mut Option<Arpeggio>,
    legacy_arpeggio_note: &mut i32,
    legacy_arpeggio_octave: &mut i32,
    current_pitch_table: &mut Option<PitchTable>,
    current_subsong: &mut Option<Subsong>,
    current_psg: &mut Option<PsgConfig>,
    current_pattern: &mut Option<Pattern>,
    current_pattern_track_indexes: &mut Vec<usize>,
    current_pattern_height: &mut usize,
    current_pattern_cell_track_number: &mut Option<usize>,
    current_pattern_cell_transposition: &mut i8,
    current_track: &mut Option<Track>,
    current_cell: &mut Option<Cell>,
    current_effect: &mut Option<Effect>,
    current_effect_container: &mut Option<EffectContainer>,
    current_speed_track: &mut Option<SpecialTrack>,
    current_event_track: &mut Option<SpecialTrack>,
    current_special_cell: &mut Option<SpecialCell>,
    legacy_arpeggio_map: &HashMap<usize, usize>,
    legacy_pitch_map: &HashMap<usize, usize>,
) -> Result<()> {
    match (current_state.clone(), name) {
        // Format version detection
        (ParseState::Root, "formatVersion") => {
            let trimmed = current_text.trim();
            *format_version_out = if trimmed.starts_with('1') {
                FormatVersion::Legacy
            } else {
                FormatVersion::Modern
            };
            if *format_version_out == FormatVersion::Legacy && !*legacy_defaults_inserted {
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
                *legacy_defaults_inserted = true;
            }
        }

        // Metadata
        (ParseState::Root, "title") => metadata.title = current_text.to_string(),
        (ParseState::Root, "author") => metadata.author = current_text.to_string(),
        (ParseState::Root, "composer") => metadata.composer = current_text.to_string(),
        (ParseState::Root, "comment") => metadata.comments = current_text.to_string(),

        // Instrument fields
        (ParseState::Instrument, "name") => {
            if let Some(instr) = current_instrument {
                instr.name = current_text.to_string();
            }
        }
        (ParseState::Instrument, "colorArgb") => {
            if let Some(instr) = current_instrument {
                instr.color_argb = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Instrument, "type") => {
            if let Some(instr) = current_instrument {
                instr.instrument_type = match current_text.to_lowercase().as_str() {
                    "psg" => InstrumentType::Psg,
                    "digi" | "sample" => InstrumentType::Digi,
                    _ => InstrumentType::Psg,
                };

                if instr.instrument_type == InstrumentType::Digi {
                    *current_sample_builder = Some(SampleInstrumentBuilder::default());
                } else {
                    *current_sample_builder = None;
                }
            }
        }
        (ParseState::Instrument, "speed") => {
            if let Some(instr) = current_instrument {
                instr.speed = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Instrument, "isRetrig") => {
            if let Some(instr) = current_instrument {
                instr.is_retrig = current_text == "true";
            }
        }
        (ParseState::Instrument, "loopStartIndex") => {
            if let Some(instr) = current_instrument {
                instr.loop_start_index = current_text.parse().unwrap_or(0);
                if instr.instrument_type == InstrumentType::Digi
                    && let Some(builder) = current_sample_builder
                {
                    builder.loop_start_index = instr.loop_start_index;
                }
            }
        }
        (ParseState::Instrument, "endIndex") => {
            if let Some(instr) = current_instrument {
                instr.end_index = current_text.parse().unwrap_or(0);
                if instr.instrument_type == InstrumentType::Digi
                    && let Some(builder) = current_sample_builder
                {
                    builder.end_index = instr.end_index;
                }
            }
        }
        (ParseState::Instrument, "isLooping") => {
            if let Some(instr) = current_instrument {
                instr.is_looping = current_text == "true";
                if instr.instrument_type == InstrumentType::Digi
                    && let Some(builder) = current_sample_builder
                {
                    builder.is_looping = instr.is_looping;
                }
            }
        }
        (ParseState::InstrumentAutoSpread, _) => {
            // Ignore loop/index fields inside autoSpread blocks
        }
        (ParseState::Instrument, "isSfxExported") => {
            if let Some(instr) = current_instrument {
                instr.is_sfx_exported = current_text == "true";
            }
        }
        (ParseState::Instrument, "frequencyHz") => {
            if let (Some(builder), Some(instr)) =
                (current_sample_builder.as_mut(), current_instrument.as_ref())
                && instr.instrument_type == InstrumentType::Digi
            {
                builder.frequency_hz = current_text.parse().unwrap_or(44_100);
            }
        }
        (ParseState::Instrument, "amplificationRatio") => {
            if let (Some(builder), Some(instr)) =
                (current_sample_builder.as_mut(), current_instrument.as_ref())
                && instr.instrument_type == InstrumentType::Digi
            {
                builder.amplification_ratio = current_text.parse().unwrap_or(1.0);
            }
        }
        (ParseState::Instrument, "originalFilename") => {
            if let (Some(builder), Some(instr)) =
                (current_sample_builder.as_mut(), current_instrument.as_ref())
                && instr.instrument_type == InstrumentType::Digi
            {
                builder.original_filename = Some(current_text.to_string());
            }
        }
        (ParseState::Instrument, "digiNote") => {
            if let (Some(builder), Some(instr)) =
                (current_sample_builder.as_mut(), current_instrument.as_ref())
                && instr.instrument_type == InstrumentType::Digi
            {
                builder.digidrum_note = current_text.parse().unwrap_or(DEFAULT_DIGIDRUM_NOTE);
            }
        }
        (ParseState::Instrument, "sampleUnsigned8BitsBase64") => {
            if let (Some(builder), Some(instr)) =
                (current_sample_builder.as_mut(), current_instrument.as_ref())
                && instr.instrument_type == InstrumentType::Digi
            {
                let sanitized: String = current_text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                let decoded = general_purpose::STANDARD
                    .decode(sanitized)
                    .map_err(|e| ArkosError::InvalidFormat(format!("Invalid sample data: {e}")))?;
                let pcm: Vec<f32> = decoded
                    .into_iter()
                    .map(|byte| (byte as f32 - 128.0) / 128.0)
                    .collect();
                builder.data = Some(pcm);
            }
        }

        // Instrument cell fields
        (ParseState::InstrumentCell, "volume") => {
            if let Some(cell) = current_instrument_cell {
                cell.volume = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "noise") => {
            if let Some(cell) = current_instrument_cell {
                cell.noise = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "primaryPeriod")
        | (ParseState::InstrumentCell, "softwarePeriod") => {
            if let Some(cell) = current_instrument_cell {
                cell.primary_period = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "primaryArpeggioNoteInOctave") => {
            if let Some(cell) = current_instrument_cell {
                cell.primary_arpeggio_note_in_octave = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "softwareArpeggio") => {
            if let Some(cell) = current_instrument_cell {
                let value = current_text.parse().unwrap_or(0);
                let (note_in_octave, octave) = split_note(value);
                cell.primary_arpeggio_note_in_octave = note_in_octave as u8;
                cell.primary_arpeggio_octave = octave as i8;
            }
        }
        (ParseState::InstrumentCell, "primaryArpeggioOctave") => {
            if let Some(cell) = current_instrument_cell {
                cell.primary_arpeggio_octave = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "primaryPitch")
        | (ParseState::InstrumentCell, "softwarePitch") => {
            if let Some(cell) = current_instrument_cell {
                cell.primary_pitch = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "link") => {
            if let Some(cell) = current_instrument_cell {
                cell.link = match current_text {
                    "noSoftwareNoHardware" | "noSoftNoHard" => ChannelLink::NoSoftwareNoHardware,
                    "softwareOnly" | "softOnly" => ChannelLink::SoftwareOnly,
                    "hardwareOnly" | "hardOnly" => ChannelLink::HardwareOnly,
                    "softwareAndHardware" | "softAndHard" => ChannelLink::SoftwareAndHardware,
                    "softwareToHardware" | "softToHard" => ChannelLink::SoftwareToHardware,
                    "hardwareToSoftware" | "hardToSoft" => ChannelLink::HardwareToSoftware,
                    _ => ChannelLink::NoSoftwareNoHardware,
                };
            }
        }
        (ParseState::InstrumentCell, "ratio") => {
            if let Some(cell) = current_instrument_cell {
                cell.ratio = current_text.parse().unwrap_or(4);
            }
        }
        (ParseState::InstrumentCell, "hardwareEnvelope")
        | (ParseState::InstrumentCell, "hardwareCurve") => {
            if let Some(cell) = current_instrument_cell {
                cell.hardware_envelope = current_text.parse().unwrap_or(8);
            }
        }
        (ParseState::InstrumentCell, "secondaryPeriod")
        | (ParseState::InstrumentCell, "hardwarePeriod") => {
            if let Some(cell) = current_instrument_cell {
                cell.secondary_period = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "secondaryArpeggioNoteInOctave") => {
            if let Some(cell) = current_instrument_cell {
                cell.secondary_arpeggio_note_in_octave = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "hardwareArpeggio") => {
            if let Some(cell) = current_instrument_cell {
                let value = current_text.parse().unwrap_or(0);
                let (note_in_octave, octave) = split_note(value);
                cell.secondary_arpeggio_note_in_octave = note_in_octave as u8;
                cell.secondary_arpeggio_octave = octave as i8;
            }
        }
        (ParseState::InstrumentCell, "secondaryArpeggioOctave") => {
            if let Some(cell) = current_instrument_cell {
                cell.secondary_arpeggio_octave = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "secondaryPitch")
        | (ParseState::InstrumentCell, "hardwarePitch") => {
            if let Some(cell) = current_instrument_cell {
                cell.secondary_pitch = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::InstrumentCell, "isRetrig") => {
            if let Some(cell) = current_instrument_cell {
                cell.is_retrig = current_text == "true";
            }
        }

        // Arpeggio fields
        (ParseState::Arpeggio, "index") => {
            if let Some(arp) = current_arpeggio {
                arp.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Arpeggio, "name") => {
            if let Some(arp) = current_arpeggio {
                arp.name = current_text.to_string();
            }
        }
        (ParseState::Arpeggio, "speed") => {
            if let Some(arp) = current_arpeggio {
                arp.speed = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Arpeggio, "loopStartIndex") => {
            if let Some(arp) = current_arpeggio {
                arp.loop_start = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Arpeggio, "endIndex") => {
            if let Some(arp) = current_arpeggio {
                arp.end_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Arpeggio, "shift") => {
            if let Some(arp) = current_arpeggio {
                arp.shift = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Arpeggio, "value") => {
            if let Some(arp) = current_arpeggio {
                let value = current_text.parse().unwrap_or(0);
                arp.values.push(value);
            }
        }
        (ParseState::LegacyArpeggioCell, "note") => {
            *legacy_arpeggio_note = current_text.parse().unwrap_or(0);
        }
        (ParseState::LegacyArpeggioCell, "octave") => {
            *legacy_arpeggio_octave = current_text.parse().unwrap_or(0);
        }

        // Pitch table fields
        (ParseState::PitchTable, "index") => {
            if let Some(pitch) = current_pitch_table {
                pitch.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PitchTable, "name") => {
            if let Some(pitch) = current_pitch_table {
                pitch.name = current_text.to_string();
            }
        }
        (ParseState::PitchTable, "speed") => {
            if let Some(pitch) = current_pitch_table {
                pitch.speed = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PitchTable, "loopStartIndex") => {
            if let Some(pitch) = current_pitch_table {
                pitch.loop_start = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PitchTable, "endIndex") => {
            if let Some(pitch) = current_pitch_table {
                pitch.end_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PitchTable, "shift") => {
            if let Some(pitch) = current_pitch_table {
                pitch.shift = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PitchTable, "value") => {
            if let Some(pitch) = current_pitch_table {
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
            *current_pattern_height = current_text.parse().unwrap_or(64);
        }
        (ParseState::PatternSpeedTrackIndex, "trackIndex") => {
            if let Some(pat) = current_pattern {
                pat.speed_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PatternEventTrackIndex, "trackIndex") => {
            if let Some(pat) = current_pattern {
                pat.event_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PatternSpeedTrackIndex, "speedTrackNumber") => {
            if let Some(pat) = current_pattern {
                pat.speed_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PatternEventTrackIndex, "eventTrackNumber") => {
            if let Some(pat) = current_pattern {
                pat.event_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::PatternCell, "trackNumber") | (ParseState::PatternCell, "trackIndex") => {
            *current_pattern_cell_track_number = Some(current_text.parse().unwrap_or(0));
        }
        (ParseState::PatternCell, "transposition") => {
            *current_pattern_cell_transposition = current_text.parse::<i32>().unwrap_or(0) as i8;
        }
        (ParseState::Pattern, "colorArgb") => {
            if let Some(pat) = current_pattern {
                pat.color_argb = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Pattern, "speedTrackNumber") => {
            if let Some(pat) = current_pattern {
                pat.speed_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Pattern, "eventTrackNumber") => {
            if let Some(pat) = current_pattern {
                pat.event_track_index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::SpeedTrack, "number") | (ParseState::SpeedTrack, "index") => {
            if let Some(track) = current_speed_track {
                track.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::EventTrack, "number") | (ParseState::EventTrack, "index") => {
            if let Some(track) = current_event_track {
                track.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::SpeedCell, "index") | (ParseState::EventCell, "index") => {
            if let Some(cell) = current_special_cell {
                cell.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::SpeedCell, "value") | (ParseState::EventCell, "value") => {
            if let Some(cell) = current_special_cell {
                cell.value = current_text.parse().unwrap_or(0);
            }
        }

        // Subsong fields
        (ParseState::Subsong, "title") => {
            if let Some(s) = current_subsong {
                s.title = current_text.to_string();
            }
        }
        (ParseState::Subsong, "initialSpeed") => {
            if let Some(s) = current_subsong {
                s.initial_speed = current_text.parse().unwrap_or(6);
            }
        }
        (ParseState::Subsong, "endPosition") | (ParseState::Subsong, "endIndex") => {
            if let Some(s) = current_subsong {
                s.end_position = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Subsong, "loopStartPosition") | (ParseState::Subsong, "loopStartIndex") => {
            if let Some(s) = current_subsong {
                s.loop_start_position = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Subsong, "replayFrequencyHz") | (ParseState::Subsong, "replayFrequency") => {
            if let Some(s) = current_subsong {
                s.replay_frequency_hz = current_text.parse().unwrap_or(50.0);
            }
        }
        (ParseState::Subsong, "digiChannel") => {
            if let Some(s) = current_subsong {
                s.digi_channel = current_text.parse().unwrap_or(0);
            }
        }

        // PSG fields
        (ParseState::SubsongPsg, "type") => {
            if let Some(psg) = current_psg {
                psg.psg_type = match current_text.to_lowercase().as_str() {
                    "ym" => PsgType::YM,
                    "ay" => PsgType::AY,
                    _ => PsgType::YM,
                };
            }
        }
        (ParseState::SubsongPsg, "frequencyHz") | (ParseState::SubsongPsg, "psgFrequency") => {
            if let Some(psg) = current_psg {
                psg.psg_frequency = current_text.parse().unwrap_or(2_000_000);
            }
        }
        (ParseState::SubsongPsg, "referenceFrequencyHz")
        | (ParseState::SubsongPsg, "referenceFrequency") => {
            if let Some(psg) = current_psg {
                psg.reference_frequency = current_text.parse().unwrap_or(440.0);
            }
        }
        (ParseState::SubsongPsg, "samplePlayerFrequencyHz")
        | (ParseState::SubsongPsg, "samplePlayerFrequency") => {
            if let Some(psg) = current_psg {
                psg.sample_player_frequency = current_text.parse().unwrap_or(8000);
            }
        }
        (ParseState::SubsongPsg, "mixingOutput") => {
            if let Some(psg) = current_psg {
                psg.mixing_output = match current_text {
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
            if let Some(t) = current_track {
                t.index = current_text.parse().unwrap_or(0);
            }
        }

        // Cell fields
        (ParseState::Cell, "index") => {
            if let Some(c) = current_cell {
                c.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Cell, "note") => {
            if let Some(c) = current_cell {
                c.note = current_text.parse().unwrap_or(255);
            }
        }
        (ParseState::Cell, "instrument") => {
            if let Some(c) = current_cell {
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
            if let Some(eff) = current_effect {
                eff.index = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Effect, "name") => {
            if let Some(eff) = current_effect {
                eff.name = current_text.to_string();
            }
        }
        (ParseState::Effect, "effect") => {
            if *current_effect_container == Some(EffectContainer::Legacy)
                && let Some(eff) = current_effect
            {
                eff.name = current_text.to_string();
            }
        }
        (ParseState::Effect, "logicalValue") => {
            if let Some(eff) = current_effect {
                eff.logical_value = current_text.parse().unwrap_or(0);
            }
        }
        (ParseState::Effect, "hexValue") => {
            if let Some(eff) = current_effect {
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
                                    if let Some(&mapped) = legacy_pitch_map.get(&(value as usize)) {
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

    Ok(())
}
