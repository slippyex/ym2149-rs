//! XML parsing helper functions.
//!
//! Utility functions for common parsing operations like skipping blocks,
//! parsing positions, and reading text content.

use super::state::local_name_from_bytes;
use crate::error::{ArkosError, Result};
use crate::format::Position;
use quick_xml::Reader;
use quick_xml::events::Event;

/// Parses the `<positions>` block from subsong XML.
///
/// Extracts position entries with pattern index, height, markers,
/// and transpositions for each position in the song sequence.
///
/// # Arguments
///
/// * `reader` - XML reader positioned inside `<positions>` element
/// * `buf` - Reusable buffer for XML parsing
///
/// # Returns
///
/// Vector of [`Position`] entries in sequence order.
///
/// # Errors
///
/// Returns [`ArkosError::InvalidFormat`] on unexpected EOF or malformed XML.
pub fn parse_positions_block<R: std::io::BufRead>(
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

/// Skips over an entire XML block without processing its contents.
///
/// Useful for ignoring optional or unsupported elements while maintaining
/// proper depth tracking.
///
/// # Arguments
///
/// * `reader` - XML reader positioned at the start of the block
/// * `buf` - Reusable buffer for XML parsing
/// * `tag` - Name of the tag to skip (tracks nested occurrences)
///
/// # Errors
///
/// Returns [`ArkosError::InvalidFormat`] on unexpected EOF.
pub fn skip_block<R: std::io::BufRead>(
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
