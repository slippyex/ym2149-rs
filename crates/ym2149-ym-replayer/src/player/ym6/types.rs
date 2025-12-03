//! YM6 file format types

use std::fmt;
use std::sync::Arc;

use super::super::format_profile::FormatMode;

/// Supported YM file formats handled by the loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YmFileFormat {
    /// YM2 format (Mad Max).
    Ym2,
    /// Legacy YM3 format without embedded metadata.
    Ym3,
    /// YM3 variant with loop information footer.
    Ym3b,
    /// YM4 format (metadata, optional digidrums, 14 registers).
    Ym4,
    /// YM5 format (metadata, digidrums, effect attributes).
    Ym5,
    /// YM6 format (metadata, extended effects).
    Ym6,
    /// YM Tracker format version 1.
    Ymt1,
    /// YM Tracker format version 2.
    Ymt2,
}

impl fmt::Display for YmFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            YmFileFormat::Ym2 => "YM2",
            YmFileFormat::Ym3 => "YM3",
            YmFileFormat::Ym3b => "YM3b",
            YmFileFormat::Ym4 => "YM4",
            YmFileFormat::Ym5 => "YM5",
            YmFileFormat::Ym6 => "YM6",
            YmFileFormat::Ymt1 => "YMT1",
            YmFileFormat::Ymt2 => "YMT2",
        };
        f.write_str(name)
    }
}

/// Summary information returned after loading file data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadSummary {
    /// Detected YM file format.
    pub format: YmFileFormat,
    /// Number of register frames in the song.
    pub frame_count: usize,
    /// Samples generated per frame (derived from frame rate).
    pub samples_per_frame: u32,
}

impl LoadSummary {
    /// Total number of audio samples encoded in the song.
    pub fn total_samples(&self) -> usize {
        self.frame_count
            .saturating_mul(self.samples_per_frame as usize)
    }
}

/// YM6 File Metadata
#[derive(Debug, Clone)]
pub struct Ym6Info {
    /// Song name
    pub song_name: String,
    /// Author name
    pub author: String,
    /// Song comment
    pub comment: String,
    /// Number of frames
    pub frame_count: u32,
    /// Frame rate (typically 50Hz)
    pub frame_rate: u16,
    /// Loop frame number
    pub loop_frame: u32,
    /// Master clock frequency
    pub master_clock: u32,
}

/// Parameters for initializing playback state
pub(in crate::player) struct PlaybackStateInit {
    pub frames: Vec<[u8; 16]>,
    pub loop_frame: Option<usize>,
    pub samples_per_frame: u32,
    pub digidrums: Vec<Arc<[u8]>>,
    pub attributes: u32,
    pub format_mode: FormatMode,
    pub info: Option<Ym6Info>,
}
