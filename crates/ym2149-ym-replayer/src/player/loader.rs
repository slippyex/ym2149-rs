//! YM File Loading and Format Detection
//!
//! This module handles loading of all YM file formats (YM2-YM6, YMT1/YMT2),
//! format detection, decompression, and initialization of playback state.

use super::format_profile::{FormatMode, create_profile};
use super::madmax_digidrums::MADMAX_SAMPLES;
use super::tracker_player::{
    TrackerFormat, TrackerLine, TrackerSample, TrackerState, deinterleave_tracker_bytes,
};
use super::ym_player::Ym6PlayerGeneric;
use super::ym6::{LoadSummary, PlaybackStateInit, Ym6Info, YmFileFormat};
use super::ym6::{read_be_u16, read_be_u32, read_c_string};
use crate::parser::FormatParser;
use crate::parser::{ATTR_LOOP_MODE, ATTR_STREAM_INTERLEAVED, Ym6Parser, YmParser};
use crate::{Result, compression};
use ym2149::Ym2149Backend;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Load YM data (compressed or raw) and initialize playback state.
    pub fn load_data(&mut self, data: &[u8]) -> Result<LoadSummary> {
        let decompressed = compression::decompress_if_needed(data)?;
        self.load_decompressed(&decompressed)
    }

    /// Load decompressed YM data and detect format
    pub(in crate::player) fn load_decompressed(&mut self, data: &[u8]) -> Result<LoadSummary> {
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
            samples_per_frame: self.sequencer.samples_per_frame().max(1),
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
            format_mode: FormatMode::Ym6,
            info: Some(info),
        });

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
            format_mode: FormatMode::Ym5,
            info: Some(info),
        });

        Ok(())
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
            format_mode: FormatMode::Basic,
            info: Some(info),
        });

        Ok(())
    }

    /// Load YM3 or YM3b format
    pub(in crate::player) fn load_ym3_variant(
        &mut self,
        data: &[u8],
        has_loop_footer: bool,
    ) -> Result<()> {
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
            format_mode: FormatMode::Basic,
            info: Some(info),
        });

        Ok(())
    }

    /// Load YM2 (Mad Max) format
    pub(in crate::player) fn load_ym2(&mut self, data: &[u8]) -> Result<()> {
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
            format_mode: FormatMode::Ym2,
            info: Some(info),
        });

        Ok(())
    }

    /// Load YM Tracker format (YMT1 or YMT2)
    pub(in crate::player) fn load_ym_tracker(
        &mut self,
        data: &[u8],
        format: TrackerFormat,
    ) -> Result<()> {
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
        self.sequencer.clear();
        self.format_profile = create_profile(FormatMode::Basic);
        self.digidrums.clear();
        self.sequencer.set_loop_point(None);
        let tracker_samples_per_frame = if player_rate > 0 {
            (44_100 / u32::from(player_rate)).max(1)
        } else {
            1
        };
        self.sequencer
            .set_samples_per_frame(tracker_samples_per_frame);
        self.attributes = attributes;
        self.effects.reset();
        self.effects.set_sample_rate(44_100);

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
        let samples_per_frame = self.sequencer.samples_per_frame();
        let info = self.info.clone();

        self.initialize_playback_state(PlaybackStateInit {
            frames,
            loop_frame: None,
            samples_per_frame,
            digidrums: Vec::new(),
            attributes: 0,
            format_mode: FormatMode::Basic,
            info,
        });
    }

    /// Initialize playback state with common initialization logic
    ///
    /// This helper consolidates the common initialization pattern used by
    /// load_ym6(), load_ym4(), and load_ym5() methods.
    pub(in crate::player) fn initialize_playback_state(&mut self, params: PlaybackStateInit) {
        let PlaybackStateInit {
            frames,
            loop_frame,
            samples_per_frame,
            digidrums,
            attributes,
            format_mode,
            info,
        } = params;

        // Set frame data and reset playback position
        self.sequencer.load_frames(frames);
        self.format_profile = create_profile(format_mode);

        // Reset effect state
        self.effects.reset();

        // Set playback parameters
        self.sequencer.set_loop_point(loop_frame);
        self.sequencer.set_samples_per_frame(samples_per_frame);
        self.digidrums = digidrums;
        self.attributes = attributes;

        // Set metadata
        self.info = info;

        self.tracker = None;
        self.is_tracker_mode = false;

        // Enable ST-style color filter for authentic tone
        self.chip.set_color_filter(true);

        // Reset effects manager with correct sample rate (44.1kHz)
        self.effects.set_sample_rate(44_100);

        // Reset first frame pre-load flag
        self.first_frame_pre_loaded = false;
        // Clear R13 cache to ensure first shape write happens
        self.prev_r13 = None;
    }

    /// Normalize loop point to valid range
    pub(in crate::player) fn normalize_loop_point(
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

    /// Extract loop point from YM3b footer
    pub(in crate::player) fn extract_ym3b_loop_point(
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
    pub(in crate::player) fn calculate_samples_per_frame(&self, frame_rate: u16) -> u32 {
        const SAMPLE_RATE: u32 = 44100;
        let rate = if frame_rate > 0 {
            frame_rate as u32
        } else {
            50
        };
        SAMPLE_RATE / rate
    }
}
