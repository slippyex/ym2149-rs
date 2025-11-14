//! AKS file format data structures
//!
//! Represents the Arkos Tracker 3 XML format in Rust.

use std::collections::HashMap;
use std::sync::Arc;

/// AKS song metadata
#[derive(Debug, Clone)]
pub struct SongMetadata {
    /// Song title
    pub title: String,
    /// Author name
    pub author: String,
    /// Composer name (for covers)
    pub composer: String,
    /// Comments
    pub comments: String,
    /// Creation date (ISO 8601)
    pub creation_date: String,
    /// Modification date (ISO 8601)
    pub modification_date: String,
}

/// PSG type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgType {
    /// AY-3-8910/8912
    AY,
    /// YM2149
    YM,
}

/// PSG mixing output configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixingOutput {
    /// ABC (normal)
    ABC,
    /// ACB
    ACB,
    /// BAC
    BAC,
    /// BCA
    BCA,
    /// CAB
    CAB,
    /// CBA
    CBA,
}

/// PSG configuration
#[derive(Debug, Clone)]
pub struct PsgConfig {
    /// PSG type (AY or YM)
    pub psg_type: PsgType,
    /// PSG frequency in Hz
    pub psg_frequency: u32,
    /// Reference frequency in Hz (usually 440.0)
    pub reference_frequency: f32,
    /// Sample player frequency in Hz
    pub sample_player_frequency: u32,
    /// Channel mixing output
    pub mixing_output: MixingOutput,
}

/// Note value (0-127, or 255 for no note)
pub type Note = u8;

/// Instrument type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentType {
    /// PSG instrument (tone/noise)
    Psg,
    /// Digi-drum sample
    Digi,
}

/// Channel link mode for instrument cells
///
/// This determines how the instrument cell's parameters are used:
/// - Software: tone period is calculated from note + arpeggio + pitch
/// - Hardware: hardware envelope period is used for buzzer sounds
/// - SoftToHard/HardToSoft: one is derived from the other using ratio
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLink {
    /// No software, no hardware envelope (silent or noise only)
    NoSoftwareNoHardware,
    /// Software envelope only (normal tone)
    SoftwareOnly,
    /// Hardware envelope only (buzzer)
    HardwareOnly,
    /// Both software and hardware (tone + buzzer)
    SoftwareAndHardware,
    /// Software calculated, hardware derived from software (using ratio)
    SoftwareToHardware,
    /// Hardware calculated, software derived from hardware (using ratio)
    HardwareToSoftware,
}

/// Instrument cell (FM synthesis parameters)
#[derive(Debug, Clone)]
pub struct InstrumentCell {
    /// Volume (0-15)
    pub volume: u8,
    /// Noise period (0-31)
    pub noise: u8,
    /// Primary period offset
    pub primary_period: i16,
    /// Primary arpeggio note in octave (0-11)
    pub primary_arpeggio_note_in_octave: u8,
    /// Primary arpeggio octave
    pub primary_arpeggio_octave: i8,
    /// Primary pitch offset
    pub primary_pitch: i16,
    /// Channel link mode
    pub link: ChannelLink,
    /// Hardware envelope ratio (0-15)
    pub ratio: u8,
    /// Hardware envelope shape (0-15)
    pub hardware_envelope: u8,
    /// Secondary period offset
    pub secondary_period: i16,
    /// Secondary arpeggio note in octave (0-11)
    pub secondary_arpeggio_note_in_octave: u8,
    /// Secondary arpeggio octave
    pub secondary_arpeggio_octave: i8,
    /// Secondary pitch offset
    pub secondary_pitch: i16,
    /// Is retrig (reset phase)
    pub is_retrig: bool,
}

/// Instrument definition
#[derive(Debug, Clone)]
pub struct Instrument {
    /// Instrument name
    pub name: String,
    /// Color (ARGB)
    pub color_argb: u32,
    /// Instrument type
    pub instrument_type: InstrumentType,
    /// Speed (playback rate divisor)
    pub speed: u8,
    /// Is retrig (reset on new note)
    pub is_retrig: bool,
    /// Loop start index
    pub loop_start_index: usize,
    /// End index
    pub end_index: usize,
    /// Is looping
    pub is_looping: bool,
    /// Is exported as SFX
    pub is_sfx_exported: bool,
    /// Instrument cells (envelope)
    pub cells: Vec<InstrumentCell>,
    /// Optional sample data if this is a digi instrument
    pub sample: Option<SampleInstrument>,
}

/// Sample instrument data
#[derive(Debug, Clone)]
pub struct SampleInstrument {
    /// Source sample rate in Hz
    pub frequency_hz: u32,
    /// Amplification ratio
    pub amplification_ratio: f32,
    /// Original filename (if provided)
    pub original_filename: Option<String>,
    /// Loop start index
    pub loop_start_index: usize,
    /// Loop end index
    pub end_index: usize,
    /// Whether the sample loops
    pub is_looping: bool,
    /// PCM data decoded to -1.0..1.0 range
    pub data: Arc<Vec<f32>>,
    /// Note used when triggering via event track (digidrums)
    pub digidrum_note: i32,
}

/// Effect in a cell
#[derive(Debug, Clone)]
pub struct Effect {
    /// Effect index in cell
    pub index: usize,
    /// Effect name (volume, arpeggio, etc.)
    pub name: String,
    /// Logical value
    pub logical_value: i32,
}

/// Cell in a track (one row)
#[derive(Debug, Clone)]
pub struct Cell {
    /// Cell index in track
    pub index: usize,
    /// Note (0-95, or 255 for none)
    pub note: Note,
    /// Instrument number
    pub instrument: usize,
    /// Whether an instrument was explicitly set on this cell
    pub instrument_present: bool,
    /// Effects in this cell
    pub effects: Vec<Effect>,
}

/// Track (one channel's data)
#[derive(Debug, Clone)]
pub struct Track {
    /// Track index
    pub index: usize,
    /// Cells in this track
    pub cells: Vec<Cell>,
}

/// Pattern cell (maps channel to track with transposition)
#[derive(Debug, Clone)]
pub struct PatternCell {
    /// Track number to use for this channel
    pub track_number: usize,
    /// Transposition in semitones
    pub transposition: i8,
}

/// Pattern (maps channels to tracks)
#[derive(Debug, Clone)]
pub struct Pattern {
    /// Pattern index
    pub index: usize,
    /// Track indexes (one per channel)
    pub track_indexes: Vec<usize>,
    /// Speed track index (for tempo changes)
    pub speed_track_index: usize,
    /// Event track index (for special events)
    pub event_track_index: usize,
    /// Color (ARGB)
    pub color_argb: u32,
}

/// Position (references a pattern with height and transpositions)
#[derive(Debug, Clone)]
pub struct Position {
    /// Pattern index to use
    pub pattern_index: usize,
    /// Pattern height (number of lines)
    pub height: usize,
    /// Marker name (for navigation)
    pub marker_name: String,
    /// Marker color (ARGB)
    pub marker_color: u32,
    /// Transpositions per channel (empty if no transposition)
    pub transpositions: Vec<i8>,
}

/// A subsong within an AKS file
#[derive(Debug, Clone)]
pub struct Subsong {
    /// Subsong name
    pub title: String,
    /// Initial speed
    pub initial_speed: u8,
    /// End position index
    pub end_position: usize,
    /// Loop start position index
    pub loop_start_position: usize,
    /// Replay frequency in Hz
    pub replay_frequency_hz: f32,
    /// PSG configurations (multi-PSG support!)
    pub psgs: Vec<PsgConfig>,
    /// Digi channel number
    pub digi_channel: usize,
    /// Highlight spacing
    pub highlight_spacing: usize,
    /// Secondary highlight
    pub secondary_highlight: usize,
    /// Positions (song structure)
    pub positions: Vec<Position>,
    /// Patterns (channel -> track mapping)
    pub patterns: Vec<Pattern>,
    /// Tracks (the actual note data) - HashMap keyed by track index
    pub tracks: HashMap<usize, Track>,
    /// Speed tracks (special tracks controlling tempo changes)
    pub speed_tracks: HashMap<usize, SpecialTrack>,
    /// Event tracks (triggering digidrums/samples)
    pub event_tracks: HashMap<usize, SpecialTrack>,
}

/// Special track cell (speed/event)
#[derive(Debug, Clone)]
pub struct SpecialCell {
    /// Cell index (line)
    pub index: usize,
    /// Value stored in this cell
    pub value: i32,
}

/// Special track definition
#[derive(Debug, Clone)]
pub struct SpecialTrack {
    /// Track index/number
    pub index: usize,
    /// Cells in this track
    pub cells: Vec<SpecialCell>,
}

impl SpecialTrack {
    /// Return the last cell at or before the requested line
    pub fn latest_value(&self, line: usize) -> Option<i32> {
        let mut best: Option<&SpecialCell> = None;
        for cell in &self.cells {
            if cell.index <= line {
                match best {
                    Some(prev) if prev.index >= cell.index => {}
                    _ => best = Some(cell),
                }
            }
        }

        best.map(|cell| cell.value)
    }
}

/// Arpeggio table entry
#[derive(Debug, Clone)]
pub struct Arpeggio {
    /// Arpeggio index
    pub index: usize,
    /// Arpeggio name
    pub name: String,
    /// Arpeggio values (note offsets)
    pub values: Vec<i8>,
    /// Speed (ticks per step)
    pub speed: u8,
    /// Loop start index
    pub loop_start: usize,
    /// End index
    pub end_index: usize,
    /// Leading zero count (shift)
    pub shift: usize,
}

/// Pitch table entry
#[derive(Debug, Clone)]
pub struct PitchTable {
    /// Pitch table index
    pub index: usize,
    /// Pitch table name
    pub name: String,
    /// Pitch values (period offsets)
    pub values: Vec<i16>,
    /// Speed (ticks per step)
    pub speed: u8,
    /// Loop start index
    pub loop_start: usize,
    /// End index
    pub end_index: usize,
    /// Leading zero count (shift)
    pub shift: usize,
}

/// Complete AKS song
#[derive(Debug, Clone)]
pub struct AksSong {
    /// Song metadata
    pub metadata: SongMetadata,
    /// Instruments
    pub instruments: Vec<Instrument>,
    /// Arpeggios (table arpeggios, index 0 is empty)
    pub arpeggios: Vec<Arpeggio>,
    /// Pitch tables (index 0 is empty)
    pub pitch_tables: Vec<PitchTable>,
    /// List of subsongs
    pub subsongs: Vec<Subsong>,
}

impl Default for SongMetadata {
    fn default() -> Self {
        Self {
            title: "Untitled".to_string(),
            author: "Unknown".to_string(),
            composer: String::new(),
            comments: String::new(),
            creation_date: String::new(),
            modification_date: String::new(),
        }
    }
}

impl Default for PsgConfig {
    fn default() -> Self {
        // Default to Atari ST configuration
        Self {
            psg_type: PsgType::YM,
            psg_frequency: 2_000_000,
            reference_frequency: 440.0,
            sample_player_frequency: 11025,
            mixing_output: MixingOutput::ABC,
        }
    }
}

impl PsgConfig {
    /// Create PSG config for Amstrad CPC
    pub fn cpc() -> Self {
        Self {
            psg_type: PsgType::AY,
            psg_frequency: 1_000_000,
            ..Default::default()
        }
    }

    /// Create PSG config for Atari ST
    pub fn atari_st() -> Self {
        Self {
            psg_type: PsgType::YM,
            psg_frequency: 2_000_000,
            ..Default::default()
        }
    }

    /// Create PSG config for ZX Spectrum
    pub fn spectrum() -> Self {
        Self {
            psg_type: PsgType::AY,
            psg_frequency: 1_773_400,
            ..Default::default()
        }
    }
}
