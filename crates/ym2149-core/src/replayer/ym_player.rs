//! YM6 File Player
//!
//! Plays back YM2-YM6 format chiptune files with proper VBL synchronization.

use super::effects_manager::EffectsManager;
use super::madmax_digidrums::{MADMAX_SAMPLE_RATE_BASE, MADMAX_SAMPLES};
use super::tracker_player::{
    TrackerFormat, TrackerLine, TrackerSample, TrackerState, deinterleave_tracker_bytes,
};
use super::{PlaybackController, PlaybackState, VblSync};
use crate::ym_parser::FormatParser;
use crate::ym_parser::{
    ATTR_LOOP_MODE, ATTR_STREAM_INTERLEAVED, Ym6Parser, YmParser,
    effects::{EffectCommand, Ym6EffectDecoder, decode_effects_ym5},
};
use crate::{Result, Ym2149, compression};
use std::fmt;

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

fn read_be_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        return Err("Unexpected end of data while reading u16".into());
    }
    let value = u16::from_be_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(value)
}

fn read_be_u32(data: &[u8], offset: &mut usize) -> Result<u32> {
    if *offset + 4 > data.len() {
        return Err("Unexpected end of data while reading u32".into());
    }
    let value = u32::from_be_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
    ]);
    *offset += 4;
    Ok(value)
}

fn read_c_string(data: &[u8], offset: &mut usize) -> Result<String> {
    if *offset >= data.len() {
        return Err("Unexpected end of data while reading string".into());
    }
    let start = *offset;
    while *offset < data.len() && data[*offset] != 0 {
        *offset += 1;
    }
    if *offset >= data.len() {
        return Err("Unterminated string in YM tracker data".into());
    }
    let string = String::from_utf8_lossy(&data[start..*offset]).to_string();
    *offset += 1; // Skip null terminator
    Ok(string)
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
struct PlaybackStateInit {
    frames: Vec<[u8; 16]>,
    loop_frame: Option<usize>,
    samples_per_frame: u32,
    digidrums: Vec<Vec<u8>>,
    attributes: u32,
    is_ym2_mode: bool,
    is_ym5_mode: bool,
    info: Option<Ym6Info>,
}

/// YM6 File Player
pub struct Ym6Player {
    /// PSG chip emulator
    chip: Ym2149,
    /// VBL synchronization
    vbl: VblSync,
    /// Playback state
    state: PlaybackState,
    /// Register frames (each frame contains 16 bytes of registers)
    frames: Vec<[u8; 16]>,
    /// Current frame index
    current_frame: usize,
    /// Samples generated so far in current frame
    samples_in_frame: u32,
    /// Samples per frame (calculated from frame rate)
    samples_per_frame: u32,
    /// Loop frame for looping playback (None = no loop)
    loop_point: Option<usize>,
    /// Song metadata
    info: Option<Ym6Info>,
    /// Digidrum sample bank (raw bytes from file)
    digidrums: Vec<Vec<u8>>,
    /// YM6 attributes bitfield (A_* flags)
    attributes: u32,
    /// Effect decoder for YM6 frames
    fx_decoder: Ym6EffectDecoder,
    is_ym2_mode: bool,
    is_ym5_mode: bool,
    is_ym6_mode: bool,
    /// Per-voice SID active flags
    sid_active: [bool; 3],
    /// Per-voice DigiDrum active flags
    drum_active: [bool; 3],
    active_drum_index: [Option<u8>; 3],
    active_drum_freq: [u32; 3],
    /// Effects manager for YM6 special effects
    effects: EffectsManager,
    /// Tracker playback state (for YMT1/YMT2 formats)
    tracker: Option<TrackerState>,
    /// Indicates if current song uses tracker mixing path
    is_tracker_mode: bool,
    /// Flag to track if first frame's registers have been pre-loaded
    first_frame_pre_loaded: bool,
    /// Cache previous R13 (envelope shape) to avoid redundant resets
    prev_r13: Option<u8>,
}

impl Ym6Player {
    /// Create a new YM6 player with empty song
    pub fn new() -> Self {
        Ym6Player {
            chip: Ym2149::new(),
            vbl: VblSync::default(),
            state: PlaybackState::Stopped,
            frames: Vec::new(),
            current_frame: 0,
            samples_in_frame: 0,
            samples_per_frame: 882, // Default for 44.1kHz at 50Hz
            loop_point: None,
            info: None,
            digidrums: Vec::new(),
            attributes: 0,
            fx_decoder: Ym6EffectDecoder::new(),
            is_ym2_mode: false,
            is_ym5_mode: false,
            is_ym6_mode: false,
            sid_active: [false; 3],
            drum_active: [false; 3],
            active_drum_index: [None; 3],
            active_drum_freq: [0; 3],
            effects: EffectsManager::new(44_100),
            tracker: None,
            is_tracker_mode: false,
            first_frame_pre_loaded: false,
            prev_r13: None,
        }
    }

    fn load_ym2(&mut self, data: &[u8]) -> Result<()> {
        if data.len() < 4 {
            return Err("YM2 file too small".into());
        }

        let payload = &data[4..];
        if payload.is_empty() || !payload.len().is_multiple_of(14) {
            return Err(
                format!("YM2 payload size {} is not a multiple of 14", payload.len()).into(),
            );
        }

        let frame_count = payload.len() / 14;
        let mut frames = Vec::with_capacity(frame_count);

        // YM2 data is interleaved by register (like YM3). Deinterleave.
        for j in 0..frame_count {
            let mut frame = [0u8; 16];
            for (k, fr) in frame.iter_mut().enumerate().take(14) {
                let idx = j + frame_count * k;
                *fr = payload[idx];
            }
            frames.push(frame);
        }

        let digidrums: Vec<Vec<u8>> = MADMAX_SAMPLES
            .iter()
            .map(|sample| sample.to_vec())
            .collect();

        let info = Ym6Info {
            song_name: String::new(),
            author: String::new(),
            comment: "YM2 (Mad Max) format".to_string(),
            frame_count: frame_count as u32,
            frame_rate: 50,
            loop_frame: 0,
            master_clock: 2_000_000,
        };

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: None,
            samples_per_frame: self.calculate_samples_per_frame(50),
            digidrums,
            attributes: 0,
            is_ym2_mode: true,
            is_ym5_mode: false,
            info: Some(info),
        });

        Ok(())
    }

    fn load_ym_tracker(&mut self, data: &[u8], format: TrackerFormat) -> Result<()> {
        if data.len() < 12 {
            return Err("Tracker file too small".into());
        }

        if &data[4..12] != b"LeOnArD!" {
            return Err("Invalid tracker signature".into());
        }

        let mut offset = 12;
        let nb_voice = read_be_u16(data, &mut offset)? as usize;
        if nb_voice == 0 || nb_voice > 8 {
            return Err(format!("Unsupported tracker voice count: {}", nb_voice).into());
        }

        let player_rate = read_be_u16(data, &mut offset)?;
        let total_frames = read_be_u32(data, &mut offset)? as usize;
        let loop_frame_raw = read_be_u32(data, &mut offset)? as usize;
        let digidrum_count = read_be_u16(data, &mut offset)? as usize;
        let mut attributes = read_be_u32(data, &mut offset)?;

        let song_name = read_c_string(data, &mut offset)?;
        let author = read_c_string(data, &mut offset)?;
        let comment = read_c_string(data, &mut offset)?;

        let mut samples = Vec::with_capacity(digidrum_count);
        for _ in 0..digidrum_count {
            let size = read_be_u16(data, &mut offset)? as usize;
            let (repeat_len, _flags) = match format {
                TrackerFormat::Ymt1 => (size, 0u16),
                TrackerFormat::Ymt2 => {
                    let rep = read_be_u16(data, &mut offset)? as usize;
                    let flag = read_be_u16(data, &mut offset)?;
                    (rep, flag)
                }
            };

            if offset + size > data.len() {
                return Err("Tracker digidrum data truncated".into());
            }

            let mut sample_data = vec![0u8; size];
            sample_data.copy_from_slice(&data[offset..offset + size]);
            offset += size;

            samples.push(TrackerSample {
                data: sample_data,
                repeat_len: repeat_len.min(size).max(1),
            });
        }

        if total_frames == 0 {
            return Err("Tracker song has zero frames".into());
        }

        let bytes_per_line = 4;
        let line_count = nb_voice
            .checked_mul(total_frames)
            .ok_or_else(|| "Tracker frame count overflow".to_string())?;
        let frame_bytes = line_count
            .checked_mul(bytes_per_line)
            .ok_or_else(|| "Tracker data size overflow".to_string())?;

        if offset + frame_bytes > data.len() {
            return Err("Tracker pattern data truncated".into());
        }

        let mut tracker_bytes = data[offset..offset + frame_bytes].to_vec();

        if (attributes & ATTR_STREAM_INTERLEAVED) != 0 {
            tracker_bytes = deinterleave_tracker_bytes(&tracker_bytes, nb_voice, total_frames);
            attributes &= !ATTR_STREAM_INTERLEAVED;
        }

        let freq_shift = match format {
            TrackerFormat::Ymt2 => {
                let shift = ((attributes >> 28) & 0x0F) as u8;
                attributes &= 0x0FFF_FFFF;
                shift
            }
            TrackerFormat::Ymt1 => 0,
        };

        let loop_enabled = (attributes & ATTR_LOOP_MODE) != 0;

        let mut lines = Vec::with_capacity(line_count);
        for chunk in tracker_bytes.chunks_exact(bytes_per_line) {
            lines.push(TrackerLine {
                note_on: chunk[0],
                volume: chunk[1],
                freq_high: chunk[2],
                freq_low: chunk[3],
            });
        }

        let loop_frame = if loop_frame_raw < total_frames {
            loop_frame_raw
        } else {
            0
        };

        let mut tracker_state = TrackerState::new(
            nb_voice,
            player_rate,
            total_frames,
            loop_frame,
            loop_enabled,
            freq_shift,
            samples,
            lines,
            44_100,
        );
        tracker_state.reset();

        self.tracker = Some(tracker_state);
        self.is_tracker_mode = true;
        self.frames.clear();
        self.digidrums.clear();
        self.current_frame = 0;
        self.samples_in_frame = 0;
        self.loop_point = None;
        self.samples_per_frame = if player_rate > 0 {
            (44_100 / u32::from(player_rate)).max(1)
        } else {
            0
        };
        self.attributes = attributes;
        self.is_ym2_mode = false;
        self.is_ym5_mode = false;
        self.sid_active = [false; 3];
        self.drum_active = [false; 3];
        self.active_drum_index = [None; 3];
        self.active_drum_freq = [0; 3];
        self.fx_decoder = Ym6EffectDecoder::new();
        self.effects = EffectsManager::new(44_100);

        let info = Ym6Info {
            song_name,
            author,
            comment,
            frame_count: total_frames as u32,
            frame_rate: player_rate,
            loop_frame: loop_frame_raw as u32,
            master_clock: 2_000_000,
        };
        self.info = Some(info);

        Ok(())
    }

    /// Load YM data (compressed or raw) and initialize playback state.
    pub fn load_data(&mut self, data: &[u8]) -> Result<LoadSummary> {
        let decompressed = compression::decompress_if_needed(data)?;
        self.load_decompressed(&decompressed)
    }

    fn load_decompressed(&mut self, data: &[u8]) -> Result<LoadSummary> {
        if data.len() < 4 {
            return Err("YM data too short".into());
        }

        let header = &data[0..4];
        let format = match header {
            b"YM2!" => {
                self.load_ym2(data)?;
                YmFileFormat::Ym2
            }
            b"YM3!" => {
                self.load_ym3_variant(data, false)?;
                YmFileFormat::Ym3
            }
            b"YM3b" => {
                self.load_ym3_variant(data, true)?;
                YmFileFormat::Ym3b
            }
            b"YM4!" => {
                self.load_ym4(data)?;
                YmFileFormat::Ym4
            }
            b"YM5!" => {
                self.load_ym5(data)?;
                YmFileFormat::Ym5
            }
            b"YM6!" => {
                self.load_ym6(data)?;
                YmFileFormat::Ym6
            }
            b"YMT1" => {
                self.load_ym_tracker(data, TrackerFormat::Ymt1)?;
                YmFileFormat::Ymt1
            }
            b"YMT2" => {
                self.load_ym_tracker(data, TrackerFormat::Ymt2)?;
                YmFileFormat::Ymt2
            }
            _ => return Err("Unsupported YM format".into()),
        };

        Ok(LoadSummary {
            format,
            frame_count: self.frame_count(),
            samples_per_frame: self.samples_per_frame.max(1),
        })
    }

    /// Load and parse YM6 file data
    pub fn load_ym6(&mut self, data: &[u8]) -> Result<()> {
        let parser = Ym6Parser;
        let (frames, header, metadata, digidrums) = parser.parse_full(data)?;

        let samples_per_frame = self.calculate_samples_per_frame(header.frame_rate);
        let info = Ym6Info {
            song_name: metadata.song_name,
            author: metadata.author,
            comment: metadata.comment,
            frame_count: header.frame_count,
            frame_rate: header.frame_rate,
            loop_frame: header.loop_frame,
            master_clock: header.master_clock,
        };

        let loop_point = self.normalize_loop_point(header.loop_frame, frames.len(), false);

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: loop_point,
            samples_per_frame,
            digidrums,
            attributes: header.attributes,
            is_ym2_mode: false,
            is_ym5_mode: false,
            info: Some(info),
        });
        self.is_ym6_mode = true;

        Ok(())
    }

    /// Load register frames directly (for testing or pre-parsed data)
    ///
    /// # Frame Rate Assumptions
    /// This method assumes **50Hz PAL frame rate** and calculates timing for 44.1kHz output:
    /// - Samples per frame: 882 (44100 / 50)
    /// - Duration calculation: uses 50Hz as default
    /// - No metadata is set
    ///
    /// # For YM6 Files
    /// **Do NOT use this method for YM6 files.** Use `load_ym6()` instead, which:
    /// - Parses the file's actual frame rate from the header
    /// - Extracts metadata (song name, author, comment)
    /// - Automatically calculates correct timing
    ///
    /// # For Custom Frame Rates
    /// If your frames use a different frame rate, call `set_samples_per_frame()` after loading
    /// with the correct value (e.g., 735 for 60Hz NTSC: `44100 / 60`).
    ///
    /// # Example
    /// ```ignore
    /// // For 60Hz NTSC data:
    /// player.load_frames(frames);
    /// player.set_samples_per_frame(735); // 44100 / 60
    /// ```
    pub fn load_frames(&mut self, frames: Vec<[u8; 16]>) {
        let samples_per_frame = self.samples_per_frame;
        let info = self.info.clone();

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: None,
            samples_per_frame,
            digidrums: Vec::new(),
            attributes: 0,
            is_ym2_mode: false,
            is_ym5_mode: false,
            info,
        });
    }

    /// Load and parse YM4 file data (frames + metadata, no timer effects)
    pub fn load_ym4(&mut self, data: &[u8]) -> Result<()> {
        let parser = YmParser::new();
        let (frames, metadata) = parser.parse_full(data)?;

        // YM4 typically 50Hz
        let frame_rate = metadata.player_freq.unwrap_or(50);
        let samples_per_frame = self.calculate_samples_per_frame(frame_rate);
        let frame_count = frames.len() as u32;
        let loop_point = self.normalize_loop_point(metadata.loop_frame, frames.len(), false);

        let info = Ym6Info {
            song_name: metadata.song_name,
            author: metadata.author,
            comment: metadata.comment,
            frame_count,
            frame_rate,
            loop_frame: metadata.loop_frame,
            master_clock: 2_000_000,
        };

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: loop_point,
            samples_per_frame,
            digidrums: Vec::new(),
            attributes: 0,
            is_ym2_mode: false,
            is_ym5_mode: false,
            info: Some(info),
        });
        self.is_ym6_mode = false;

        Ok(())
    }
    /// Load and parse YM5 file data (with digidrums and effects)
    pub fn load_ym5(&mut self, data: &[u8]) -> Result<()> {
        let parser = YmParser::new();
        let (frames, header, metadata, digidrums) = parser.parse_ym5_full_with_digidrums(data)?;

        // YM5 embeds player frequency in header.player_freq (Some)
        let frame_rate = header.player_freq.unwrap_or(50);
        let samples_per_frame = self.calculate_samples_per_frame(frame_rate);

        let info = Ym6Info {
            song_name: metadata.song_name,
            author: metadata.author,
            comment: metadata.comment,
            frame_count: header.frame_count as u32,
            frame_rate,
            loop_frame: header.loop_frame,
            master_clock: header.master_clock.unwrap_or(2_000_000),
        };

        let loop_point = self.normalize_loop_point(header.loop_frame, frames.len(), false);

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: loop_point,
            samples_per_frame,
            digidrums,
            attributes: header.attributes,
            is_ym2_mode: false,
            is_ym5_mode: true,
            info: Some(info),
        });

        Ok(())
    }

    /// Initialize playback state with common initialization logic
    ///
    /// This helper consolidates the common initialization pattern used by
    /// load_ym6(), load_ym4(), and load_ym5() methods.
    fn initialize_playback_state(&mut self, params: PlaybackStateInit) {
        let PlaybackStateInit {
            frames,
            loop_frame,
            samples_per_frame,
            digidrums,
            attributes,
            is_ym2_mode,
            is_ym5_mode,
            info,
        } = params;

        // Set frame data and reset playback position
        self.frames = frames;
        self.current_frame = 0;
        self.samples_in_frame = 0;

        // Reset effect state
        self.sid_active = [false; 3];
        self.drum_active = [false; 3];

        // Set playback parameters
        let frame_len = self.frames.len();
        self.loop_point =
            loop_frame.and_then(|frame| if frame < frame_len { Some(frame) } else { None });
        self.samples_per_frame = samples_per_frame;
        self.digidrums = digidrums;
        self.attributes = attributes;
        self.is_ym2_mode = is_ym2_mode;
        self.is_ym5_mode = is_ym5_mode;

        // Set metadata
        self.info = info;

        self.tracker = None;
        self.is_tracker_mode = false;

        // Enable ST-style color filter for authentic tone
        self.chip.set_color_filter(true);

        // Reset effect decoder to clear any previous state
        self.fx_decoder = Ym6EffectDecoder::new();

        // Reset effects manager with correct sample rate (44.1kHz)
        self.effects = EffectsManager::new(44_100);

        // Reset first frame pre-load flag
        self.first_frame_pre_loaded = false;
        // Clear R13 cache to ensure first shape write happens
        self.prev_r13 = None;
    }

    /// Enable Sync Buzzer effect with specific timer frequency
    ///
    /// Sync Buzzer is a timer-based effect that repeatedly retriggers the envelope
    /// to create a continuous buzzing sound. This is typically used with envelope
    /// shapes like 0x0D (Hold) or 0x0F (Hold-Sawtooth).
    ///
    /// # Arguments
    /// * `timer_freq` - Timer frequency in Hz (typical range: 4000-8000 Hz)
    ///
    /// # Example
    /// ```ignore
    /// // Enable Sync Buzzer at 6 kHz
    /// player.enable_sync_buzzer(6000)?;
    /// player.play()?;
    /// ```
    pub fn enable_sync_buzzer(&mut self, timer_freq: u32) -> Result<()> {
        if timer_freq == 0 {
            return Err("Sync Buzzer timer frequency must be > 0".into());
        }
        self.effects.sync_buzzer_start(timer_freq);
        Ok(())
    }

    /// Disable Sync Buzzer effect
    pub fn disable_sync_buzzer(&mut self) {
        self.effects.sync_buzzer_stop();
    }

    /// Set loop frame for looping playback
    pub fn set_loop_frame(&mut self, frame: usize) {
        if let Some(tracker) = self.tracker.as_mut() {
            if tracker.total_frames == 0 {
                tracker.loop_enabled = false;
                tracker.loop_frame = 0;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = 0;
                }
                return;
            }

            if frame < tracker.total_frames {
                tracker.loop_enabled = true;
                tracker.loop_frame = frame;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = frame as u32;
                }
            } else {
                tracker.loop_enabled = false;
                tracker.loop_frame = 0;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = 0;
                }
            }
        } else if frame < self.frames.len() {
            self.loop_point = Some(frame);
            if let Some(info) = self.info.as_mut() {
                info.loop_frame = frame as u32;
            }
        } else {
            self.loop_point = None;
            if let Some(info) = self.info.as_mut() {
                info.loop_frame = 0;
            }
        }
    }

    /// Get the number of frames
    pub fn frame_count(&self) -> usize {
        if let Some(tracker) = &self.tracker {
            tracker.total_frames
        } else {
            self.frames.len()
        }
    }

    #[allow(missing_docs)]
    pub fn samples_per_frame_value(&self) -> u32 {
        self.samples_per_frame
    }

    #[allow(missing_docs)]
    pub fn loop_point_value(&self) -> Option<usize> {
        if self.is_tracker_mode {
            self.tracker.as_ref().and_then(|tracker| {
                if tracker.loop_enabled {
                    Some(tracker.loop_frame)
                } else {
                    None
                }
            })
        } else {
            self.loop_point
        }
    }

    #[allow(missing_docs)]
    pub fn frames_clone(&self) -> Option<Vec<[u8; 16]>> {
        if self.is_tracker_mode {
            None
        } else {
            Some(self.frames.clone())
        }
    }

    #[allow(missing_docs)]
    pub fn is_tracker_mode(&self) -> bool {
        self.is_tracker_mode
    }

    /// Set samples per frame (default 882 for 44.1kHz at 50Hz)
    ///
    /// # Arguments
    /// * `samples` - Samples per frame; must be > 0 and <= 10000
    ///
    /// # Valid Range
    /// Typical values:
    /// - 441: 100Hz frame rate at 44.1kHz
    /// - 735: 60Hz (NTSC) at 44.1kHz
    /// - 882: 50Hz (PAL) at 44.1kHz
    /// - 1764: 25Hz at 44.1kHz
    ///
    /// # Errors
    /// Returns error if `samples` is 0 or exceeds 10000 (which would imply < 4.41Hz frame rate).
    pub fn set_samples_per_frame(&mut self, samples: u32) -> Result<()> {
        if samples == 0 {
            return Err("samples_per_frame cannot be zero".into());
        }
        if samples > 10000 {
            return Err(format!(
                "samples_per_frame {} exceeds reasonable limit of 10000 (implies < 4.41Hz frame rate)",
                samples
            ).into());
        }
        self.samples_per_frame = samples;
        // Reconfigure VBL with new timing
        self.vbl.reset();
        Ok(())
    }

    /// Calculate samples per frame based on frame rate and sample rate
    ///
    /// # Assumptions
    /// - Output sample rate: 44.1kHz (industry standard for audio)
    /// - Frame synchronization: uses integer division (frame-quantized timing)
    ///
    /// # Timing Precision
    /// This method uses integer division, which matches the original YM2149 hardware's
    /// frame-based timing model. The result is precise for standard frame rates:
    /// - 50Hz (PAL):   44100 / 50 = 882.0 samples/frame (exact)
    /// - 60Hz (NTSC):  44100 / 60 = 735.0 samples/frame (exact)
    /// - 100Hz:        44100 / 100 = 441.0 samples/frame (exact)
    ///
    /// For non-standard rates (e.g., 59Hz), the fractional part is discarded:
    /// - 59Hz: 44100 / 59 = 747 samples/frame (loses 0.457 samples/frame ≈ 36ms over 1 hour)
    ///
    /// For authentic YM2149 emulation, this frame-quantized behavior is correct,
    /// as the original hardware synchronized output to VBL interrupts, not sample clocks.
    ///
    /// # Panics
    /// Returns 0 if frame_rate is 0 (guard against division by zero).
    fn calculate_samples_per_frame(&self, frame_rate: u16) -> u32 {
        const SAMPLE_RATE: u32 = 44100;
        let rate = if frame_rate > 0 {
            frame_rate as u32
        } else {
            50
        };
        SAMPLE_RATE / rate
    }

    fn normalize_loop_point(
        &self,
        loop_frame: u32,
        frame_len: usize,
        treat_zero_as_loop: bool,
    ) -> Option<usize> {
        if frame_len == 0 {
            return None;
        }

        if loop_frame == 0 {
            if treat_zero_as_loop { Some(0) } else { None }
        } else {
            let idx = loop_frame as usize;
            if idx < frame_len { Some(idx) } else { None }
        }
    }

    fn extract_ym3b_loop_point(
        &self,
        data: &[u8],
        frame_count: usize,
        treat_zero_as_loop: bool,
    ) -> Option<usize> {
        if data.len() < 8 || frame_count == 0 {
            return None;
        }

        let payload_len = data.len() - 4;
        if payload_len < 4 {
            return None;
        }

        // YM3b stores loop frame as little-endian u32 footer
        let loop_raw = u32::from_be_bytes([
            data[data.len() - 4],
            data[data.len() - 3],
            data[data.len() - 2],
            data[data.len() - 1],
        ]);

        // Validate payload layout: header + frames + loop footer
        let frame_bytes = payload_len - 4;
        if !frame_bytes.is_multiple_of(14) {
            return None;
        }

        self.normalize_loop_point(loop_raw, frame_count, treat_zero_as_loop)
    }

    fn load_ym3_variant(&mut self, data: &[u8], has_loop_footer: bool) -> Result<()> {
        let parser = YmParser::new();
        let frames = if has_loop_footer {
            let mut normalized = data.to_vec();
            if let Some(header) = normalized.get_mut(3) {
                *header = b'!';
            }
            parser.parse(&normalized)?
        } else {
            parser.parse(data)?
        };
        let frame_count = frames.len();

        let loop_point = if has_loop_footer {
            self.extract_ym3b_loop_point(data, frame_count, true)
        } else {
            None
        };

        let info = Ym6Info {
            song_name: String::new(),
            author: String::new(),
            comment: String::new(),
            frame_count: frame_count as u32,
            frame_rate: 50,
            loop_frame: loop_point.unwrap_or(0) as u32,
            master_clock: 2_000_000,
        };

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: loop_point,
            samples_per_frame: self.calculate_samples_per_frame(50),
            digidrums: Vec::new(),
            attributes: 0,
            is_ym2_mode: false,
            is_ym5_mode: false,
            info: Some(info),
        });
        self.is_ym6_mode = false;

        Ok(())
    }

    /// Generate the next sample and advance playback
    pub fn generate_sample(&mut self) -> f32 {
        if self.state != PlaybackState::Playing {
            return 0.0;
        }

        if self.is_tracker_mode {
            return self.generate_tracker_sample();
        }

        if self.frames.is_empty() {
            return 0.0;
        }

        // Load registers for current frame (once per frame)
        if self.samples_in_frame == 0 {
            self.load_frame_registers();
        }

        // Update effects before clocking chip
        self.effects.tick(&mut self.chip);

        // Generate sample
        self.chip.clock();
        let sample = self.chip.get_sample();

        // Advance frame counter
        self.advance_frame();

        sample
    }

    /// Load and apply register values for the current frame
    fn load_frame_registers(&mut self) {
        let frame_to_load = self.current_frame;
        // Clone the frame data to avoid borrow checker issues
        let regs = self.frames[frame_to_load].clone();

        if self.is_ym2_mode {
            self.load_ym2_frame(&regs);
        } else {
            self.load_ymx_frame(&regs);
        }
    }

    /// Load YM2 (Mad Max) frame with special drum handling
    fn load_ym2_frame(&mut self, regs: &[u8; 16]) {
        // Reset effect state that is not used in YM2 playback
        self.effects.sync_buzzer_stop();
        for voice in 0..3 {
            if self.sid_active[voice] {
                self.effects.sid_stop(voice);
                self.sid_active[voice] = false;
            }
            if voice != 2 && self.drum_active[voice] {
                self.effects.digidrum_stop(voice);
                self.drum_active[voice] = false;
            }
        }

        // Write registers 0-10
        for (reg_idx, &val) in regs.iter().enumerate().take(11) {
            self.chip.write_register(reg_idx as u8, val);
        }

        // YM2 (Mad Max): if R13 != 0xFF, force envelope (R11), set R12=0 and R13=0x0A
        if regs[13] != 0xFF {
            self.chip.write_register(11, regs[11]);
            self.chip.write_register(12, 0);
            self.chip.write_register(13, 0x0A);
        }

        // Handle Mad Max DigiDrum on channel C
        if (regs[10] & 0x80) != 0 {
            let mixer = self.chip.read_register(0x07) | 0x24;
            self.chip.write_register(0x07, mixer);

            let sample_idx = (regs[10] & 0x7F) as usize;
            if let Some(sample) = self.digidrums.get(sample_idx).cloned() {
                let timer = regs[12] as u32;
                if timer > 0 {
                    let freq = (MADMAX_SAMPLE_RATE_BASE / 4) / timer;
                    if freq > 0 {
                        self.effects.digidrum_start(2, sample, freq);
                        self.drum_active[2] = true;
                        self.active_drum_index[2] = Some(sample_idx as u8);
                        self.active_drum_freq[2] = freq;
                    }
                }
            }
        } else if self.drum_active[2] {
            self.effects.digidrum_stop(2);
            self.drum_active[2] = false;
            self.active_drum_index[2] = None;
            self.active_drum_freq[2] = 0;
        }
    }

    /// Load YM5/YM6 frame with advanced effect support
    fn load_ymx_frame(&mut self, regs: &[u8; 16]) {
        // Write all registers; only gate R13 by sentinel 0xFF
        for r in 0u8..=15u8 {
            if r == 13 {
                let shape = regs[13];
                if shape != 0xFF {
                    self.chip.write_register(13, shape);
                }
            } else {
                self.chip.write_register(r, regs[r as usize]);
            }
        }

        // Decode effects based on format
        let cmds = self.decode_frame_effects(regs);

        // Apply effect commands
        self.apply_effect_intents(&cmds, regs);
    }

    /// Decode effect commands from frame registers
    fn decode_frame_effects(&self, regs: &[u8; 16]) -> Vec<EffectCommand> {
        if self.is_ym5_mode {
            decode_effects_ym5(regs)
        } else if self.is_ym6_mode {
            self.fx_decoder.decode_effects(regs).to_vec()
        } else {
            Vec::new()
        }
    }

    /// Apply decoded effect commands to the effects manager
    fn apply_effect_intents(&mut self, cmds: &[EffectCommand], regs: &[u8; 16]) {
        // Aggregate per-voice intents
        let mut sid_intent: [Option<(u32, u8)>; 3] = [None, None, None];
        let mut sid_sin_intent: [Option<(u32, u8)>; 3] = [None, None, None];
        let mut drum_intent: [Option<(u8, u32)>; 3] = [None, None, None];
        let mut sync_intent: Option<(u32, u8)> = None;

        for cmd in cmds.iter() {
            match *cmd {
                EffectCommand::None => {}
                EffectCommand::SidStart {
                    voice,
                    freq,
                    volume,
                } => {
                    if (voice as usize) < 3 {
                        sid_intent[voice as usize] = Some((freq, volume));
                    }
                }
                EffectCommand::SinusSidStart {
                    voice,
                    freq,
                    volume,
                } => {
                    if (voice as usize) < 3 {
                        sid_sin_intent[voice as usize] = Some((freq, volume));
                    }
                }
                EffectCommand::DigiDrumStart {
                    voice,
                    drum_num,
                    freq,
                } => {
                    if (voice as usize) < 3 {
                        drum_intent[voice as usize] = Some((drum_num, freq));
                    }
                }
                EffectCommand::SyncBuzzerStart { freq, env_shape } => {
                    sync_intent = Some((freq, env_shape));
                }
            }
        }

        // Apply Sync Buzzer
        self.apply_sync_buzzer_intent(sync_intent, regs);

        // Apply per-voice effects
        self.apply_voice_effects(sid_intent, sid_sin_intent, drum_intent);
    }

    /// Apply sync buzzer effect intent
    fn apply_sync_buzzer_intent(&mut self, sync_intent: Option<(u32, u8)>, regs: &[u8; 16]) {
        if let Some((freq, env_shape)) = sync_intent {
            if !self.effects.sync_buzzer_is_enabled() {
                // Respect YM6 sentinel: if R13==0xFF, do not change the shape
                if regs[13] != 0xFF {
                    self.chip.write_register(0x0D, env_shape & 0x0F);
                }
                self.effects.sync_buzzer_start(freq);
            }
        } else if self.effects.sync_buzzer_is_enabled() {
            self.effects.sync_buzzer_stop();
        }
    }

    /// Apply per-voice SID and DigiDrum effects
    fn apply_voice_effects(
        &mut self,
        sid_intent: [Option<(u32, u8)>; 3],
        sid_sin_intent: [Option<(u32, u8)>; 3],
        drum_intent: [Option<(u8, u32)>; 3],
    ) {
        for voice in 0..3 {
            // Handle DigiDrum
            if let Some((drum_idx, freq)) = drum_intent[voice] {
                if let Some(sample) = self.digidrums.get(drum_idx as usize) {
                    let should_restart = !self.drum_active[voice]
                        || self.active_drum_index[voice] != Some(drum_idx)
                        || self.active_drum_freq[voice] != freq;
                    if should_restart {
                        self.effects.digidrum_start(voice, sample.clone(), freq);
                        self.drum_active[voice] = true;
                        self.active_drum_index[voice] = Some(drum_idx);
                        self.active_drum_freq[voice] = freq;
                    }
                }
            } else if self.drum_active[voice] {
                self.effects.digidrum_stop(voice);
                self.drum_active[voice] = false;
                self.active_drum_index[voice] = None;
                self.active_drum_freq[voice] = 0;
            }

            // Handle SID
            if let Some((freq, volume)) = sid_sin_intent[voice] {
                self.effects.sid_sin_start(voice, freq, volume);
                self.sid_active[voice] = true;
            } else if let Some((freq, volume)) = sid_intent[voice] {
                self.effects.sid_start(voice, freq, volume);
                self.sid_active[voice] = true;
            } else if self.sid_active[voice] {
                self.effects.sid_stop(voice);
                self.sid_active[voice] = false;
            }
        }
    }

    /// Advance frame counter and handle looping
    fn advance_frame(&mut self) {
        self.samples_in_frame += 1;

        if self.samples_in_frame >= self.samples_per_frame {
            self.samples_in_frame = 0;

            // Handle looping
            if self.current_frame + 1 >= self.frames.len() {
                if let Some(loop_start) = self.loop_point {
                    self.current_frame = loop_start;
                } else {
                    self.state = PlaybackState::Stopped;
                }
            } else {
                self.current_frame += 1;
            }
        }
    }

    /// Generate a block of samples
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.generate_sample());
        }
        samples
    }

    fn generate_tracker_sample(&mut self) -> f32 {
        let tracker = match self.tracker.as_mut() {
            Some(state) => state,
            None => return 0.0,
        };

        if tracker.samples_per_step <= 0.0 {
            return 0.0;
        }

        while tracker.samples_until_update <= 0.0 {
            if !tracker.advance_frame() {
                self.state = PlaybackState::Stopped;
                return 0.0;
            }
            tracker.samples_until_update += tracker.samples_per_step;
        }

        let sample = tracker.mix_sample();
        tracker.samples_until_update -= 1.0;
        sample
    }

    /// Get the chip for direct manipulation
    pub fn get_chip_mut(&mut self) -> &mut Ym2149 {
        &mut self.chip
    }

    /// Get the chip (read-only)
    pub fn get_chip(&self) -> &Ym2149 {
        &self.chip
    }

    /// Mute or unmute a channel (0=A,1=B,2=C)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.chip.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.chip.is_channel_muted(channel)
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> usize {
        if let Some(tracker) = &self.tracker {
            if tracker.total_frames == 0 {
                0
            } else {
                tracker
                    .current_frame
                    .min(tracker.total_frames.saturating_sub(1))
            }
        } else {
            self.current_frame
        }
    }

    /// Get playback position as a percentage (0.0 to 1.0)
    pub fn get_playback_position(&self) -> f32 {
        if self.is_tracker_mode {
            if let Some(tracker) = &self.tracker {
                if tracker.total_frames == 0 {
                    0.0
                } else {
                    (tracker.current_frame.min(tracker.total_frames) as f32)
                        / (tracker.total_frames as f32)
                }
            } else {
                0.0
            }
        } else if self.frames.is_empty() {
            0.0
        } else {
            (self.current_frame as f32) / (self.frames.len() as f32)
        }
    }

    /// Get song duration in seconds
    ///
    /// Uses the actual frame rate from loaded YM6 file metadata if available,
    /// otherwise defaults to 50Hz (PAL standard). For frames loaded manually
    /// via `load_frames()`, the default 50Hz is used unless overridden with
    /// `set_samples_per_frame()`.
    pub fn get_duration_seconds(&self) -> f32 {
        if let Some(tracker) = &self.tracker {
            if tracker.player_rate == 0 {
                return 0.0;
            }
            return tracker.total_frames as f32 / f32::from(tracker.player_rate);
        }

        if self.frames.is_empty() {
            return 0.0;
        }

        let frame_rate = self
            .info
            .as_ref()
            .map(|info| info.frame_rate as u32)
            .unwrap_or(50);

        let total_frames = self.frames.len() as u32;
        total_frames as f32 / frame_rate as f32
    }

    /// Get song metadata if available
    pub fn info(&self) -> Option<&Ym6Info> {
        self.info.as_ref()
    }

    /// Set song metadata
    pub fn set_info(&mut self, info: Ym6Info) {
        self.info = Some(info);
    }

    /// Get current active effects status for visualization
    ///
    /// Returns tuple of (sync_buzzer_active, sid_active_per_voice, drum_active_per_voice)
    pub fn get_active_effects(&self) -> (bool, [bool; 3], [bool; 3]) {
        // Check if sync buzzer is active by looking for effect in current frame
        let sync_buzzer_active = if self.current_frame < self.frames.len() {
            let regs = &self.frames[self.current_frame];
            let cmds = self.fx_decoder.decode_effects(regs);
            cmds.iter()
                .any(|cmd| matches!(cmd, EffectCommand::SyncBuzzerStart { .. }))
        } else {
            false
        };

        (sync_buzzer_active, self.sid_active, self.drum_active)
    }

    /// Format playback information as human-readable string
    ///
    /// # Returns
    /// A formatted string containing song metadata (if available) and playback info
    ///
    /// # Example
    /// ```ignore
    /// println!("File Information:");
    /// println!("{}", player.format_info());
    /// ```
    pub fn format_info(&self) -> String {
        let duration = self.get_duration_seconds();
        let frame_count = self.frame_count();

        if let Some(info) = self.info() {
            format!(
                "  Song: {}\n  Author: {}\n  Comment: {}\n  Duration: {:.2}s ({} frames @ {}Hz)\n  Master Clock: {} Hz",
                info.song_name,
                info.author,
                info.comment,
                duration,
                frame_count,
                info.frame_rate,
                info.master_clock
            )
        } else {
            format!(
                "  Duration: {:.2}s ({} frames @ 50Hz)\n  Master Clock: 2,000,000 Hz",
                duration, frame_count
            )
        }
    }
}

impl Default for Ym6Player {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience helper to create and load a player from YM data.
pub fn load_song(data: &[u8]) -> Result<(Ym6Player, LoadSummary)> {
    let mut player = Ym6Player::new();
    let summary = player.load_data(data)?;
    Ok((player, summary))
}

impl PlaybackController for Ym6Player {
    fn play(&mut self) -> Result<()> {
        if self.is_tracker_mode {
            if let Some(tracker) = self.tracker.as_mut() {
                tracker.samples_until_update = 0.0;
                tracker.current_frame = tracker.current_frame.min(tracker.total_frames);
            }
            self.state = PlaybackState::Playing;
        } else if !self.frames.is_empty() {
            self.state = PlaybackState::Playing;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.state = PlaybackState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = PlaybackState::Stopped;
        self.current_frame = 0;
        self.samples_in_frame = 0;
        self.vbl.reset();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.reset();
        }
        Ok(())
    }

    fn state(&self) -> PlaybackState {
        self.state
    }
}

/// Type alias preserving the legacy `Player` name.
pub type Player = Ym6Player;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ym6_player_creation() {
        let player = Ym6Player::new();
        assert_eq!(player.state, PlaybackState::Stopped);
        assert_eq!(player.frame_count(), 0);
    }

    #[test]
    fn test_load_data_detects_ym3() {
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");
        data.extend_from_slice(&[0u8; 14 * 2]);

        let mut player = Ym6Player::new();
        let summary = player.load_data(&data).expect("YM3 load failed");

        assert_eq!(summary.format, YmFileFormat::Ym3);
        assert_eq!(summary.frame_count, 2);

        player.play().unwrap();
        let samples_needed = summary.samples_per_frame as usize * summary.frame_count;
        let _ = player.generate_samples(samples_needed + 1);
        assert_eq!(player.state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_load_data_detects_ym3b_loop() {
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3b");
        data.extend_from_slice(&[0u8; 14 * 2]);
        // Loop back to frame 1 (second frame)
        data.extend_from_slice(&1u32.to_be_bytes());

        let mut player = Ym6Player::new();
        let summary = player.load_data(&data).expect("YM3b load failed");

        assert_eq!(summary.format, YmFileFormat::Ym3b);
        assert_eq!(summary.frame_count, 2);

        player.play().unwrap();
        let samples_needed = summary.samples_per_frame as usize * summary.frame_count * 3;
        let _ = player.generate_samples(samples_needed);
        assert_eq!(player.state(), PlaybackState::Playing);
        assert!(player.get_current_frame() < summary.frame_count);
    }

    #[test]
    fn test_ym6_player_initialization() {
        // Test that a new player initializes with correct default state
        let player = Ym6Player::new();
        assert_eq!(player.frame_count(), 0, "New player should have 0 frames");
        assert_eq!(
            player.get_current_frame(),
            0,
            "New player should start at frame 0"
        );
        assert_eq!(
            player.state(),
            PlaybackState::Stopped,
            "New player should be stopped"
        );
    }

    #[test]
    fn test_ym6_player_frame_progression() {
        // Test that frame position advances correctly during playback
        let mut player = Ym6Player::new();
        // Create 10 frames of test data (16 bytes per frame for YM6)
        let test_frames = vec![[0x00u8; 16]; 10];
        player.load_frames(test_frames);

        player.play().unwrap();
        assert_eq!(player.state(), PlaybackState::Playing);

        // Advance through several frames
        let _ = player.generate_samples(4410); // ~0.1 seconds at 44.1kHz
        assert!(
            player.get_current_frame() <= 10,
            "Frame position should not exceed frame count"
        );
    }

    #[test]
    fn test_ym6_player_load_frames() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);
        assert_eq!(player.frame_count(), 10);
    }

    #[test]
    fn test_ym6_player_playback() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 5];
        player.load_frames(frames);
        player.play().unwrap();

        let samples = player.generate_samples(100);
        assert_eq!(samples.len(), 100);
    }

    #[test]
    fn test_ym6_player_duration() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 250]; // 250 frames at 50Hz = 5 seconds
        player.load_frames(frames);
        let duration = player.get_duration_seconds();
        assert!(duration > 4.9 && duration < 5.1);
    }

    #[test]
    fn test_ym6_player_looping() {
        let mut player = Ym6Player::new();
        let frames = vec![[0x42u8; 16]; 10];
        player.load_frames(frames);
        player.set_loop_frame(5);
        player.play().unwrap();

        // Generate enough samples to reach end and loop
        // Need more than 10 * 882 samples to reach end, then generate more
        let _ = player.generate_samples(10000);

        // After looping, we should be at or past frame 5
        // The exact frame depends on timing, so just check we're in the loop range
        assert!(player.get_current_frame() >= 5 && player.get_current_frame() < 10);
        assert_eq!(player.state, PlaybackState::Playing);
    }

    #[test]
    fn test_ym6_player_position() {
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 100];
        player.load_frames(frames);
        player.play().unwrap();

        let pos = player.get_playback_position();
        assert!((0.0..=1.0).contains(&pos));
    }

    #[test]
    fn test_ym6_player_load_ym6_with_metadata() {
        // Create a simple YM6 file with metadata
        let mut ym6_data = Vec::new();

        // Header
        ym6_data.extend_from_slice(b"YM6!"); // Magic (4 bytes)
        ym6_data.extend_from_slice(b"LeOnArD!"); // Signature (8 bytes)
        ym6_data.extend_from_slice(&(2u32).to_be_bytes()); // Frame count (4 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Attributes (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Digidrum count (2 bytes)
        ym6_data.extend_from_slice(&2000000u32.to_be_bytes()); // Master clock (4 bytes)
        ym6_data.extend_from_slice(&50u16.to_be_bytes()); // Frame rate (2 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Loop frame (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Extra data size (2 bytes)

        // Metadata: song name
        ym6_data.extend_from_slice(b"Test Song\0");

        // Metadata: author
        ym6_data.extend_from_slice(b"Test Author\0");

        // Metadata: comment
        ym6_data.extend_from_slice(b"Test Comment\0");

        // Frame data (2 frames, 16 bytes each)
        ym6_data.extend_from_slice(&[0u8; 16]);
        ym6_data.extend_from_slice(&[1u8; 16]);

        // End marker
        ym6_data.extend_from_slice(b"End!");

        // Load and verify
        let mut player = Ym6Player::new();
        assert!(player.load_ym6(&ym6_data).is_ok());

        // Check metadata was populated
        let info = player.info();
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.song_name, "Test Song");
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.comment, "Test Comment");
        assert_eq!(info.frame_count, 2);
        assert_eq!(info.frame_rate, 50);
        assert_eq!(info.master_clock, 2000000);

        // Check frames were loaded
        assert_eq!(player.frame_count(), 2);

        // Check samples per frame was calculated correctly
        // 44100 / 50 = 882 samples per frame
        player.play().unwrap();
        let samples = player.generate_samples(882);
        assert_eq!(samples.len(), 882);
        assert_eq!(player.get_current_frame(), 1); // Should have advanced to frame 1
    }

    #[test]
    fn test_ym6_player_duration_with_custom_frame_rate() {
        // Test with 60Hz NTSC frame rate to verify duration calculation uses actual frame rate
        let mut ym6_data = Vec::new();

        // Header
        ym6_data.extend_from_slice(b"YM6!"); // Magic (4 bytes)
        ym6_data.extend_from_slice(b"LeOnArD!"); // Signature (8 bytes)
        ym6_data.extend_from_slice(&(300u32).to_be_bytes()); // 300 frames (4 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Attributes (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Digidrum count (2 bytes)
        ym6_data.extend_from_slice(&2000000u32.to_be_bytes()); // Master clock (4 bytes)
        ym6_data.extend_from_slice(&60u16.to_be_bytes()); // Frame rate: 60Hz NTSC (2 bytes)
        ym6_data.extend_from_slice(&0u32.to_be_bytes()); // Loop frame (4 bytes)
        ym6_data.extend_from_slice(&0u16.to_be_bytes()); // Extra data size (2 bytes)

        // Metadata
        ym6_data.extend_from_slice(b"Test NTSC\0");
        ym6_data.extend_from_slice(b"Author\0");
        ym6_data.extend_from_slice(b"Comment\0");

        // Frame data (300 frames, 16 bytes each)
        ym6_data.extend_from_slice(&vec![0u8; 300 * 16]);

        // End marker
        ym6_data.extend_from_slice(b"End!");

        // Load and verify
        let mut player = Ym6Player::new();
        assert!(player.load_ym6(&ym6_data).is_ok());

        // Verify metadata was populated with correct frame rate
        let info = player.info().unwrap();
        assert_eq!(info.frame_rate, 60);

        // Verify duration is calculated correctly: 300 frames at 60Hz = 5.0 seconds
        let duration = player.get_duration_seconds();
        assert!(
            (duration - 5.0).abs() < 0.01,
            "Expected ~5.0s, got {}",
            duration
        );

        // Verify samples per frame was calculated for 60Hz: 44100 / 60 = 735 samples
        // Generate 735 samples (1 frame) and verify we advance to frame 1
        player.play().unwrap();
        let samples = player.generate_samples(735);
        assert_eq!(samples.len(), 735);
        assert_eq!(player.get_current_frame(), 1);
    }

    #[test]
    fn test_ym6_player_duration_default_frame_rate() {
        // Test that manually loaded frames default to 50Hz for duration calculation
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 250]; // 250 frames at 50Hz = 5.0 seconds
        player.load_frames(frames);

        let duration = player.get_duration_seconds();
        assert!(
            (duration - 5.0).abs() < 0.01,
            "Expected ~5.0s, got {}",
            duration
        );
    }

    #[test]
    fn test_sync_buzzer_enable() {
        // Test enabling Sync Buzzer effect
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Should succeed with valid frequency
        assert!(player.enable_sync_buzzer(6000).is_ok());

        // Verify effects manager has sync buzzer enabled
        assert!(player.effects.sync_buzzer_is_enabled());
    }

    #[test]
    fn test_sync_buzzer_disable() {
        // Test disabling Sync Buzzer effect
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Enable then disable
        assert!(player.enable_sync_buzzer(6000).is_ok());
        player.disable_sync_buzzer();

        // Verify effects manager has sync buzzer disabled
        assert!(!player.effects.sync_buzzer_is_enabled());
    }

    #[test]
    fn test_sync_buzzer_zero_frequency_error() {
        // Test that zero frequency is rejected
        let mut player = Ym6Player::new();
        let frames = vec![[0u8; 16]; 10];
        player.load_frames(frames);

        // Should fail with zero frequency
        let result = player.enable_sync_buzzer(0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("frequency must be > 0")
        );
    }

    #[test]
    fn test_sync_buzzer_with_playback() {
        // Test that Sync Buzzer works during playback
        let mut player = Ym6Player::new();

        // Create a simple test frame with envelope shape 0x0F (Hold-Sawtooth)
        let mut frame = [0u8; 16];
        frame[13] = 0x0F; // Register R13: envelope shape = Hold-Sawtooth
        frame[8] = 0x0F; // Register R8: amplitude with envelope
        frame[7] = 0xBE; // Register R7: mixer - enable channel A tone

        let frames = vec![frame; 100];
        player.load_frames(frames);

        // Enable Sync Buzzer
        assert!(player.enable_sync_buzzer(6000).is_ok());

        // Play and generate some samples
        player.play().unwrap();
        let samples = player.generate_samples(1000);

        assert_eq!(samples.len(), 1000);
        // Samples should be valid (not NaN or Inf)
        for sample in samples {
            assert!(sample.is_finite());
        }
    }
}
