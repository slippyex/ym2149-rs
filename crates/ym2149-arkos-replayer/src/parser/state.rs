//! Parser state machine and builder types for AKS XML parsing.
//!
//! This module contains the state tracking types used during XML parsing,
//! including the parse state enum and temporary builder structs for
//! constructing complex data structures.

use crate::error::{ArkosError, Result};
use crate::format::SampleInstrument;
use std::sync::Arc;

/// Default digidrum note (octave 6, middle C)
pub const DEFAULT_DIGIDRUM_NOTE: i32 = 12 * 6;

/// Format version detected from AKS file
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatVersion {
    /// Legacy format (version 1.x - Arkos Tracker 2)
    Legacy,
    /// Modern format (version 3.x - Arkos Tracker 3)
    Modern,
}

/// Tracks the current position in the XML hierarchy during parsing.
///
/// The parser uses this enum to maintain context as it traverses the XML tree,
/// allowing it to correctly interpret element values based on their position
/// in the document structure.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseState {
    /// At root level of document
    Root,
    /// Inside `<instruments>` block
    Instruments,
    /// Inside single `<instrument>`
    Instrument,
    /// Inside instrument's `<autoSpread>` block (ignored)
    InstrumentAutoSpread,
    /// Inside instrument's `<cells>` block
    InstrumentCells,
    /// Inside single instrument cell
    InstrumentCell,
    /// Inside `<arpeggios>` block
    Arpeggios,
    /// Inside single `<arpeggio>` or `<expression>`
    Arpeggio,
    /// Inside legacy arpeggio cell (note/octave format)
    LegacyArpeggioCell,
    /// Inside `<pitchTables>` block
    PitchTables,
    /// Inside single pitch table
    PitchTable,
    /// Inside `<subsongs>` block
    Subsongs,
    /// Inside single `<subsong>`
    Subsong,
    /// Inside subsong's `<psgs>` block
    SubsongPsgs,
    /// Inside single PSG configuration
    SubsongPsg,
    /// Inside `<patterns>` block
    SubsongPatterns,
    /// Inside single `<pattern>`
    Pattern,
    /// Inside pattern cell (track reference)
    PatternCell,
    /// Inside pattern's `<trackIndexes>` block
    PatternTrackIndexes,
    /// Inside pattern's speed track index
    PatternSpeedTrackIndex,
    /// Inside pattern's event track index
    PatternEventTrackIndex,
    /// Inside `<speedTracks>` block
    SpeedTracks,
    /// Inside single speed track
    SpeedTrack,
    /// Inside speed track cell
    SpeedCell,
    /// Inside `<eventTracks>` block
    EventTracks,
    /// Inside single event track
    EventTrack,
    /// Inside event track cell
    EventCell,
    /// Inside `<tracks>` block
    SubsongTracks,
    /// Inside single `<track>`
    Track,
    /// Inside track `<cell>`
    Cell,
    /// Inside cell `<effect>`
    Effect,
}

/// Distinguishes between modern and legacy effect containers in XML
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContainer {
    /// Modern format: `<effect>` element
    Modern,
    /// Legacy format: `<effectAndValue>` element
    Legacy,
}

/// Builder for constructing [`SampleInstrument`] from parsed XML data.
///
/// Collects sample instrument fields during parsing and validates
/// required fields before building the final struct.
///
/// # Example
///
/// ```ignore
/// let mut builder = SampleInstrumentBuilder::default();
/// builder.frequency_hz = 22050;
/// builder.data = Some(vec![0.0, 0.5, -0.5]);
/// let sample = builder.build()?;
/// ```
#[derive(Debug, Clone)]
pub struct SampleInstrumentBuilder {
    /// Sample playback frequency in Hz (default: 44100)
    pub frequency_hz: u32,
    /// Volume amplification ratio (default: 1.0)
    pub amplification_ratio: f32,
    /// Original filename if available
    pub original_filename: Option<String>,
    /// Loop start index in samples
    pub loop_start_index: usize,
    /// End index in samples
    pub end_index: usize,
    /// Whether the sample loops
    pub is_looping: bool,
    /// PCM sample data (required for build)
    pub data: Option<Vec<f32>>,
    /// Note value for digidrum playback
    pub digidrum_note: i32,
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

impl SampleInstrumentBuilder {
    /// Consumes the builder and creates a [`SampleInstrument`].
    ///
    /// # Errors
    ///
    /// Returns [`ArkosError::InvalidFormat`] if sample data is missing.
    pub fn build(self) -> Result<SampleInstrument> {
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

/// Extracts the local name from XML element bytes, stripping any namespace prefix.
///
/// # Example
///
/// ```ignore
/// assert_eq!(local_name_from_bytes(b"aks:instrument"), "instrument");
/// assert_eq!(local_name_from_bytes(b"song"), "song");
/// ```
#[inline]
pub fn local_name_from_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
