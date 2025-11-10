//! YM File Format Parsers
//!
//! Comprehensive support for Atari ST YM format versions:
//! - YM3: Simple register dump format (4 bytes header + 14 bytes/frame)
//! - YM3b: YM3 with loop support (last 4 bytes = loop frame)
//! - YM4: Adds metadata, digi-drum samples, variable header (26 bytes)
//! - YM5: Extends YM4 with chip/player frequency info (34 bytes header)
//! - YM6: (handled by separate ym6.rs module)
//!
//! Most YM files are LHA-compressed, but these parsers handle uncompressed data.

use super::{ATTR_DRUM_4BIT, FormatParser, decode_4bit_digidrum};
use crate::Result;

/// Type alias for full YM parse result: frames, header, metadata, digidrums
pub type YmParseResult = (Vec<[u8; 16]>, YmHeader, YmMetadata, Vec<Vec<u8>>);

/// Common metadata extracted from YM4/YM5 files
#[derive(Debug, Clone)]
pub struct YmMetadata {
    /// Song title/name
    pub song_name: String,
    /// Composer/musician name
    pub author: String,
    /// Additional information or notes about the song
    pub comment: String,
    /// Frame number where playback should loop (0 = no loop)
    pub loop_frame: u32,
    /// Master clock frequency in Hz (YM5 only, None for YM4)
    /// Typical value: 2,000,000 Hz for Atari ST
    pub master_clock: Option<u32>,
    /// Player/VBL frequency in Hz (YM5 only, None for YM4)
    /// Typical values: 50 Hz (PAL), 60 Hz (NTSC)
    pub player_freq: Option<u16>,
}

/// Header information for YM4/YM5 formats
pub struct YmHeader {
    /// Total number of VBL frames in the song
    pub frame_count: usize,
    /// File attributes bitfield (interleaved, loop, etc.)
    pub attributes: u32,
    /// Number of DigiDrum samples included
    pub digidrum_count: u16,
    /// Frame index where song loops back (0 = no loop)
    pub loop_frame: u32,
    /// Master clock frequency in Hz (if specified in file)
    pub master_clock: Option<u32>,
    /// Playback frequency in Hz (if specified in file)
    pub player_freq: Option<u16>,
    /// Size of extra data section in bytes
    pub extra_data_size: u16,
    /// Offset where song body data begins
    pub body_start_offset: usize,
}

const MAX_REASONABLE_FRAMES: u32 = 100_000;

/// YM Format Parser - auto-detects version and parses accordingly
pub struct YmParser;

impl YmParser {
    /// Create a new YM parser
    pub fn new() -> Self {
        YmParser
    }

    /// Parse YM5 format with digidrum samples and return frames, header, metadata, digidrums
    pub fn parse_ym5_full_with_digidrums(&self, data: &[u8]) -> Result<YmParseResult> {
        let mut header = Self::parse_ym5_header(data)?;
        let mut offset = header.body_start_offset;

        // Skip extra data section first (for format compatibility)
        offset = offset
            .checked_add(header.extra_data_size as usize)
            .ok_or("YM5 extra data offset overflow")?;
        if offset > data.len() {
            return Err("YM5 truncated in extra data section".into());
        }

        // Collect digidrum samples
        let mut digidrums: Vec<Vec<u8>> = Vec::new();
        for _ in 0..header.digidrum_count {
            if offset + 4 > data.len() {
                return Err("Incomplete YM5 digidrum size field".into());
            }
            let sample_size = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset = offset
                .checked_add(4)
                .ok_or("YM5 digidrum data offset overflow")?;
            if offset.checked_add(sample_size).is_none() || offset + sample_size > data.len() {
                return Err("Incomplete YM5 digidrum data".into());
            }
            let mut sample = data[offset..offset + sample_size].to_vec();
            if (header.attributes & ATTR_DRUM_4BIT) != 0 {
                sample = decode_4bit_digidrum(&sample);
            }
            digidrums.push(sample);
            offset += sample_size;
        }

        if (header.attributes & ATTR_DRUM_4BIT) != 0 {
            header.attributes &= !ATTR_DRUM_4BIT;
        }

        // Parse metadata strings
        let (mut metadata, new_offset) = Self::parse_metadata_strings(data, offset)?;
        offset = new_offset;

        // Add metadata from header
        metadata.loop_frame = header.loop_frame;
        metadata.master_clock = header.master_clock;
        metadata.player_freq = header.player_freq;

        // Parse frames
        let is_interleaved = (header.attributes & 1) != 0;
        let frames = match Self::parse_frame_data(
            data,
            offset,
            header.frame_count,
            is_interleaved,
            "YM5",
            16,
        ) {
            Ok(frames) => frames,
            Err(_) => {
                Self::parse_frame_data(data, offset, header.frame_count, is_interleaved, "YM5", 14)?
            }
        };

        Ok((frames, header, metadata, digidrums))
    }

    /// Check if data looks like a YM file (magic header)
    pub fn is_ym_format(data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        let magic = &data[0..4];
        magic == b"YM3!" || magic == b"YM4!" || magic == b"YM5!"
    }

    /// Detect YM format version
    fn detect_version(data: &[u8]) -> Result<&'static str> {
        if data.len() < 4 {
            return Err("Data too short for YM format detection".into());
        }
        match &data[0..4] {
            b"YM3!" => Ok("YM3"),
            b"YM4!" => Ok("YM4"),
            b"YM5!" => Ok("YM5"),
            _ => Err("Not a recognized YM format".into()),
        }
    }

    /// Parse YM4 or YM5 format, returning both frames and metadata
    /// Returns error for YM3/YM3b (which don't have metadata)
    pub fn parse_full(&self, data: &[u8]) -> Result<(Vec<[u8; 16]>, YmMetadata)> {
        if !Self::is_ym_format(data) {
            return Err("Not a valid YM file format".into());
        }

        let version = Self::detect_version(data)?;

        match version {
            "YM3" | "YM3b" => {
                Err("YM3 format does not contain metadata. Use parse() instead.".into())
            }
            "YM4" => Self::parse_ym4_full(data),
            "YM5" => Self::parse_ym5_full(data),
            _ => Err(format!("Unsupported YM version: {}", version).into()),
        }
    }

    /// Parse YM3 format (simplest - just register data)
    fn parse_ym3(data: &[u8]) -> Result<Vec<[u8; 16]>> {
        if data.len() < 4 {
            return Err("YM3 file too small".into());
        }

        // YM3 format: 4-byte header "YM3!" + 14 bytes per frame
        let payload = &data[4..];

        if !payload.len().is_multiple_of(14) {
            return Err(format!("YM3 data size {} is not multiple of 14", payload.len()).into());
        }

        let frame_count = payload.len() / 14;
        let mut frames = Vec::with_capacity(frame_count);

        // YM3 streams are interleaved by register: all R0 for all frames, then R1, etc.
        // Deinterleave to sequential frames
        for j in 0..frame_count {
            let mut frame = [0u8; 16];
            for (k, fr) in frame.iter_mut().enumerate().take(14) {
                let idx = j + frame_count * k;
                *fr = payload[idx];
            }
            frames.push(frame);
        }

        Ok(frames)
    }

    /// Parse YM3b format (YM3 with loop support) and validate loop frame
    fn parse_ym3b(data: &[u8]) -> Result<Vec<[u8; 16]>> {
        if data.len() < 8 {
            // At least header + loop frame DWORD
            return Err("YM3b file too small".into());
        }

        // Everything except last 4 bytes is frame data
        let payload_end = data.len() - 4;
        let payload = &data[4..payload_end];

        if !payload.len().is_multiple_of(14) {
            return Err(format!("YM3b data size {} is not multiple of 14", payload.len()).into());
        }

        let frame_count = payload.len() / 14;

        // Validate loop frame from last 4 bytes
        let loop_frame = u32::from_be_bytes([
            data[data.len() - 4],
            data[data.len() - 3],
            data[data.len() - 2],
            data[data.len() - 1],
        ]);

        // Loop frame must be within valid range
        // A loop frame >= frame_count is invalid and suggests this is actually plain YM3
        if loop_frame >= frame_count as u32 {
            return Err("Invalid YM3b loop frame (exceeds frame count)".into());
        }

        let mut frames = Vec::with_capacity(frame_count);

        // Convert 14-register format to 16-register format
        for j in 0..frame_count {
            let mut frame = [0u8; 16];
            for (k, fr) in frame.iter_mut().enumerate().take(14) {
                let idx = j + frame_count * k;
                *fr = payload[idx];
            }
            frames.push(frame);
        }

        // Note: Loop frame info is available in last 4 bytes but we don't preserve it
        // Could store in player metadata if needed (requires API changes to FormatParser trait)

        Ok(frames)
    }

    /// Parse YM4 header (26 bytes fixed size)
    fn parse_ym4_header(data: &[u8]) -> Result<YmHeader> {
        if data.len() < 26 {
            return Err("YM4 file too small for header".into());
        }

        // Verify header
        if &data[0..4] != b"YM4!" {
            return Err("Invalid YM4 magic".into());
        }
        if &data[4..12] != b"LeOnArD!" {
            return Err("Invalid YM4 signature".into());
        }

        let frame_count = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;

        // Validate frame count
        if frame_count == 0 {
            return Err("YM4 has zero frames".into());
        }
        if frame_count as u32 > MAX_REASONABLE_FRAMES {
            return Err(format!(
                "YM4 frame count {} exceeds limit of {}",
                frame_count, MAX_REASONABLE_FRAMES
            )
            .into());
        }

        let attributes = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let digidrum_count = u16::from_be_bytes([data[20], data[21]]);
        let loop_frame = u32::from_be_bytes([data[22], data[23], data[24], data[25]]);

        Ok(YmHeader {
            frame_count,
            attributes,
            digidrum_count,
            loop_frame,
            master_clock: None,
            player_freq: None,
            extra_data_size: 0,
            body_start_offset: 26,
        })
    }

    /// Parse YM5 header (34 bytes fixed size, extends YM4)
    fn parse_ym5_header(data: &[u8]) -> Result<YmHeader> {
        if data.len() < 34 {
            return Err("YM5 file too small for header".into());
        }

        // Verify header
        if &data[0..4] != b"YM5!" {
            return Err("Invalid YM5 magic".into());
        }
        if &data[4..12] != b"LeOnArD!" {
            return Err("Invalid YM5 signature".into());
        }

        let frame_count = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;

        // Validate frame count
        if frame_count == 0 {
            return Err("YM5 has zero frames".into());
        }
        if frame_count as u32 > MAX_REASONABLE_FRAMES {
            return Err(format!(
                "YM5 frame count {} exceeds limit of {}",
                frame_count, MAX_REASONABLE_FRAMES
            )
            .into());
        }

        let attributes = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let digidrum_count = u16::from_be_bytes([data[20], data[21]]);
        let master_clock = u32::from_be_bytes([data[22], data[23], data[24], data[25]]);
        let player_freq = u16::from_be_bytes([data[26], data[27]]);
        let loop_frame = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);
        let extra_data_size = u16::from_be_bytes([data[32], data[33]]);

        Ok(YmHeader {
            frame_count,
            attributes,
            digidrum_count,
            loop_frame,
            master_clock: Some(master_clock),
            player_freq: Some(player_freq),
            extra_data_size,
            body_start_offset: 34,
        })
    }

    /// Skip digidrum samples with overflow protection
    fn parse_digidrum_section(
        data: &[u8],
        mut offset: usize,
        count: u16,
        format_name: &str,
    ) -> Result<usize> {
        for _ in 0..count {
            offset = offset
                .checked_add(4)
                .ok_or(format!("{} digidrum offset overflow", format_name))?;

            if offset > data.len() {
                return Err(format!("{} truncated in digidrum size header", format_name).into());
            }

            let sample_size = u32::from_be_bytes([
                data[offset - 4],
                data[offset - 3],
                data[offset - 2],
                data[offset - 1],
            ]) as usize;

            offset = offset
                .checked_add(sample_size)
                .ok_or(format!("{} digidrum data offset overflow", format_name))?;

            if offset > data.len() {
                return Err(format!("{} truncated in digidrum data", format_name).into());
            }
        }
        Ok(offset)
    }

    /// Parse metadata strings (title, author, comment)
    fn parse_metadata_strings(data: &[u8], mut offset: usize) -> Result<(YmMetadata, usize)> {
        let mut strings = Vec::new();

        for _ in 0..3 {
            let mut s = String::new();
            while offset < data.len() && data[offset] != 0 {
                s.push(data[offset] as char);
                offset = offset
                    .checked_add(1)
                    .ok_or("Metadata string offset overflow")?;
            }
            offset = offset
                .checked_add(1)
                .ok_or("Metadata null terminator offset overflow")?;
            strings.push(s);
        }

        let metadata = YmMetadata {
            song_name: strings.first().cloned().unwrap_or_default(),
            author: strings.get(1).cloned().unwrap_or_default(),
            comment: strings.get(2).cloned().unwrap_or_default(),
            loop_frame: 0, // Will be set by caller
            master_clock: None,
            player_freq: None,
        };

        Ok((metadata, offset))
    }

    /// Parse frame data (handles both interleaved and non-interleaved formats)
    ///
    /// `registers_per_frame` is 14 for YM3/YM4 and 16 for YM5 (and YM6 handled elsewhere).
    fn parse_frame_data(
        data: &[u8],
        offset: usize,
        frame_count: usize,
        is_interleaved: bool,
        format_name: &str,
        registers_per_frame: usize,
    ) -> Result<Vec<[u8; 16]>> {
        let frame_data_size = frame_count
            .checked_mul(registers_per_frame)
            .ok_or(format!("{} frame data size overflow", format_name))?;

        let end_offset = offset
            .checked_add(frame_data_size)
            .ok_or(format!("{} frame data offset overflow", format_name))?;

        if end_offset > data.len() {
            return Err(format!("{} truncated in frame data", format_name).into());
        }

        let frame_data = &data[offset..end_offset];
        let mut frames = vec![[0u8; 16]; frame_count];

        if is_interleaved {
            // Interleaved: all R0, then all R1, etc.
            for reg in 0..registers_per_frame {
                for (frame_idx, frame) in frames.iter_mut().enumerate() {
                    let idx = reg
                        .checked_mul(frame_count)
                        .and_then(|i| i.checked_add(frame_idx))
                        .ok_or(format!("{} interleaved index overflow", format_name))?;
                    if reg < 16 {
                        frame[reg] = frame_data[idx];
                    }
                }
            }
        } else {
            // Non-interleaved: N bytes sequential per frame
            for (frame_idx, frame) in frames.iter_mut().enumerate() {
                let start = frame_idx
                    .checked_mul(registers_per_frame)
                    .ok_or(format!("{} non-interleaved index overflow", format_name))?;
                let _end = start
                    .checked_add(registers_per_frame)
                    .ok_or(format!("{} non-interleaved range overflow", format_name))?;
                // Copy available registers up to 16
                let copy_len = registers_per_frame.min(16);
                frame[..copy_len].copy_from_slice(&frame_data[start..start + copy_len]);
            }
        }

        Ok(frames)
    }

    /// Parse YM4 format - frames only
    fn parse_ym4(data: &[u8]) -> Result<Vec<[u8; 16]>> {
        let (_frames, _metadata) = Self::parse_ym4_full(data)?;
        Ok(_frames)
    }

    /// Parse YM4 format - frames and metadata
    fn parse_ym4_full(data: &[u8]) -> Result<(Vec<[u8; 16]>, YmMetadata)> {
        let header = Self::parse_ym4_header(data)?;
        let mut offset = header.body_start_offset;

        // Skip digidrum samples
        offset = Self::parse_digidrum_section(data, offset, header.digidrum_count, "YM4")?;

        // Parse metadata strings
        let (mut metadata, new_offset) = Self::parse_metadata_strings(data, offset)?;
        offset = new_offset;

        // Add metadata from header
        metadata.loop_frame = header.loop_frame;
        metadata.master_clock = header.master_clock;
        metadata.player_freq = header.player_freq;

        // Parse frame data
        let is_interleaved = (header.attributes & 1) != 0;
        // YM4 uses 14 registers per frame
        let frames =
            Self::parse_frame_data(data, offset, header.frame_count, is_interleaved, "YM4", 14)?;

        Ok((frames, metadata))
    }

    /// Parse YM5 format - frames only
    fn parse_ym5(data: &[u8]) -> Result<Vec<[u8; 16]>> {
        let (frames, _metadata) = Self::parse_ym5_full(data)?;
        Ok(frames)
    }

    /// Parse YM5 format - frames and metadata
    fn parse_ym5_full(data: &[u8]) -> Result<(Vec<[u8; 16]>, YmMetadata)> {
        // Reuse the with-digidrums parser and drop the extras
        let (frames, _header, metadata, _drums) =
            YmParser::new().parse_ym5_full_with_digidrums(data)?;
        Ok((frames, metadata))
    }
}

impl Default for YmParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for YmParser {
    fn parse(&self, data: &[u8]) -> Result<Vec<[u8; 16]>> {
        if !Self::is_ym_format(data) {
            return Err("Not a valid YM file format".into());
        }

        let version = Self::detect_version(data)?;

        match version {
            "YM3" => {
                // Check if it's actually YM3b by trying to parse as YM3b
                // YM3b has exactly: header (4 bytes) + (frame_count * 14) bytes + loop frame (4 bytes)
                // Real validation happens in parse_ym3b which checks loop frame validity
                if data.len() >= 8 {
                    let payload_end = data.len() - 4;
                    // Check if removing the last 4 bytes leaves a payload divisible by 14
                    if payload_end > 4 && (payload_end - 4).is_multiple_of(14) {
                        // Could be YM3b, try parsing as YM3b first
                        // parse_ym3b will validate the loop frame is within range
                        if let Ok(frames) = Self::parse_ym3b(data) {
                            return Ok(frames);
                        }
                    }
                }
                Self::parse_ym3(data)
            }
            "YM4" => Self::parse_ym4(data),
            "YM5" => Self::parse_ym5(data),
            _ => Err(format!("Unsupported YM version: {}", version).into()),
        }
    }

    fn name(&self) -> &str {
        "YM Format Parser (YM3/YM3b/YM4/YM5)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ym_magic_detection() {
        assert!(YmParser::is_ym_format(b"YM3!data"));
        assert!(YmParser::is_ym_format(b"YM4!data"));
        assert!(YmParser::is_ym_format(b"YM5!data"));
        assert!(!YmParser::is_ym_format(b"YM6!data"));
        assert!(!YmParser::is_ym_format(b"XX"));
    }

    #[test]
    fn test_ym3_parser_creation() {
        let parser = YmParser::new();
        assert!(parser.name().contains("YM3"));
    }

    #[test]
    fn test_ym3_parsing() {
        // Create minimal YM3 file: 4-byte header + 14 bytes per frame
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");

        // Add one frame of register data
        for i in 0..14 {
            data.push(i as u8);
        }

        let parser = YmParser;
        let result = parser.parse(&data);
        assert!(result.is_ok());

        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);

        // Check that registers 0-13 were loaded correctly
        for (i, frame_byte) in frames[0].iter().enumerate().take(14) {
            assert_eq!(*frame_byte, i as u8);
        }

        // Check that registers 14-15 are zero-padded
        assert_eq!(frames[0][14], 0);
        assert_eq!(frames[0][15], 0);
    }

    #[test]
    fn test_ym4_minimum_header() {
        // Create minimal YM4 file with valid header but no frame data
        let mut data = Vec::new();
        data.extend_from_slice(b"YM4!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(1u32).to_be_bytes()); // 1 frame
        data.extend_from_slice(&0u32.to_be_bytes()); // attributes
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame

        // Add metadata (3 empty strings)
        data.extend([0; 3]); // null terminators for each string

        // Add frame data (14 bytes for 1 frame)
        data.extend_from_slice(&[0u8; 14]);

        let parser = YmParser;
        let result = parser.parse(&data);
        assert!(result.is_ok());

        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_ym5_minimum_header() {
        // Create minimal YM5 file
        let mut data = Vec::new();
        data.extend_from_slice(b"YM5!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(1u32).to_be_bytes()); // 1 frame
        data.extend_from_slice(&0u32.to_be_bytes()); // attributes
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&2000000u32.to_be_bytes()); // master clock
        data.extend_from_slice(&50u16.to_be_bytes()); // player freq
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame
        data.extend_from_slice(&0u16.to_be_bytes()); // no extra data

        // Add metadata
        data.extend([0; 3]);

        // Add frame data (YM5 has 16 registers per frame)
        data.extend_from_slice(&[0u8; 16]);

        let parser = YmParser;
        let result = parser.parse(&data);
        assert!(result.is_ok());

        let frames = result.unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_ym3_multiple_frames() {
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");

        let frame_count = 3;

        // YM3 stores data interleaved by register: R0 for all frames, then R1, etc.
        for reg in 0..14u8 {
            for frame_idx in 0..frame_count {
                data.push((reg << 4) | (frame_idx as u8));
            }
        }

        let parser = YmParser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), frame_count);

        // Verify frame 1 has correct values
        for (reg, frame_byte) in frames[1].iter().enumerate().take(14) {
            let expected = ((reg as u8) << 4) | 1;
            assert_eq!(
                *frame_byte, expected,
                "Frame 1 register {} mismatch",
                reg
            );
        }
    }

    #[test]
    fn test_ym4_non_interleaved_with_values() {
        // Create YM4 file with non-interleaved frame data (3 frames)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM4!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(3u32).to_be_bytes()); // 3 frames
        data.extend_from_slice(&0u32.to_be_bytes()); // attributes (non-interleaved)
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame

        // Add metadata (3 empty strings)
        data.extend([0; 3]);

        // Add frame data - non-interleaved (14 bytes per frame, sequential)
        for frame_idx in 0..3 {
            for reg in 0..14 {
                data.push((frame_idx * 14 + reg) as u8);
            }
        }

        let parser = YmParser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 3);

        // Verify frame 1 has correct sequential values
        for (reg, frame_byte) in frames[1].iter().enumerate().take(14) {
            assert_eq!(
                *frame_byte,
                (14 + reg) as u8,
                "Frame 1 register {} mismatch",
                reg
            );
        }

        // Verify registers 14-15 are zero-padded
        assert_eq!(frames[1][14], 0);
        assert_eq!(frames[1][15], 0);
    }

    #[test]
    fn test_ym4_interleaved_with_values() {
        // Create YM4 file with interleaved frame data (3 frames)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM4!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(3u32).to_be_bytes()); // 3 frames
        data.extend_from_slice(&1u32.to_be_bytes()); // attributes (interleaved)
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame

        // Add metadata (3 empty strings)
        data.extend([0; 3]);

        // Add frame data - interleaved (all R0, then all R1, etc.)
        for reg in 0..14 {
            for frame_idx in 0..3 {
                data.push((reg * 3 + frame_idx) as u8); // Unique value per position
            }
        }

        let parser = YmParser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 3);

        // Verify interleaved data was parsed correctly
        // Frame 0, Register 0 should be at position 0
        assert_eq!(frames[0][0], 0, "Frame 0 Reg 0 should be 0");
        // Frame 1, Register 0 should be at position 1
        assert_eq!(frames[1][0], 1, "Frame 1 Reg 0 should be 1");
        // Frame 2, Register 0 should be at position 2
        assert_eq!(frames[2][0], 2, "Frame 2 Reg 0 should be 2");

        // Frame 0, Register 1 should be at position 3
        assert_eq!(frames[0][1], 3, "Frame 0 Reg 1 should be 3");
        // Frame 1, Register 1 should be at position 4
        assert_eq!(frames[1][1], 4, "Frame 1 Reg 1 should be 4");
    }

    #[test]
    fn test_ym5_interleaved_with_values() {
        // Create YM5 file with interleaved frame data (2 frames)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM5!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(2u32).to_be_bytes()); // 2 frames
        data.extend_from_slice(&1u32.to_be_bytes()); // attributes (interleaved)
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&2000000u32.to_be_bytes()); // master clock
        data.extend_from_slice(&50u16.to_be_bytes()); // player freq
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame
        data.extend_from_slice(&0u16.to_be_bytes()); // no extra data

        // Add metadata (3 empty strings)
        data.extend([0; 3]);

        // Add frame data - interleaved (YM5 has 16 registers per frame)
        for reg in 0..16 {
            for frame_idx in 0..2 {
                data.push((reg * 2 + frame_idx) as u8);
            }
        }

        let parser = YmParser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 2);

        // Verify interleaved parsing
        assert_eq!(frames[0][0], 0);
        assert_eq!(frames[1][0], 1);
        assert_eq!(frames[0][1], 2);
        assert_eq!(frames[1][1], 3);
    }

    #[test]
    fn test_ym4_parse_full_with_metadata() {
        // Create YM4 file with metadata strings
        let mut data = Vec::new();
        data.extend_from_slice(b"YM4!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(1u32).to_be_bytes()); // 1 frame
        data.extend_from_slice(&0u32.to_be_bytes()); // non-interleaved
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame

        // Add metadata strings
        data.extend_from_slice(b"Test Song\0");
        data.extend_from_slice(b"Test Author\0");
        data.extend_from_slice(b"Test Comment\0");

        // Add frame data (YM5 has 16 registers per frame)
        data.extend_from_slice(&[0u8; 16]);

        let parser = YmParser;
        let result = parser.parse_full(&data);
        assert!(result.is_ok());

        let (_frames, metadata) = result.unwrap();
        assert_eq!(metadata.song_name, "Test Song");
        assert_eq!(metadata.author, "Test Author");
        assert_eq!(metadata.comment, "Test Comment");
        assert_eq!(metadata.master_clock, None); // YM4 doesn't have master_clock
        assert_eq!(metadata.player_freq, None);
    }

    #[test]
    fn test_ym5_parse_full_with_metadata() {
        // Create YM5 file with metadata strings and frequency info
        let mut data = Vec::new();
        data.extend_from_slice(b"YM5!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&(1u32).to_be_bytes()); // 1 frame
        data.extend_from_slice(&0u32.to_be_bytes()); // non-interleaved
        data.extend_from_slice(&0u16.to_be_bytes()); // no digidrum
        data.extend_from_slice(&2000000u32.to_be_bytes()); // master clock
        data.extend_from_slice(&50u16.to_be_bytes()); // player freq
        data.extend_from_slice(&0u32.to_be_bytes()); // loop frame
        data.extend_from_slice(&0u16.to_be_bytes()); // no extra data

        // Add metadata strings
        data.extend_from_slice(b"YM5 Song\0");
        data.extend_from_slice(b"YM5 Author\0");
        data.extend_from_slice(b"YM5 Comment\0");

        // Add frame data
        data.extend_from_slice(&[0u8; 14]);

        let parser = YmParser;
        let result = parser.parse_full(&data);
        assert!(result.is_ok());

        let (_frames, metadata) = result.unwrap();
        assert_eq!(metadata.song_name, "YM5 Song");
        assert_eq!(metadata.author, "YM5 Author");
        assert_eq!(metadata.comment, "YM5 Comment");
        assert_eq!(metadata.master_clock, Some(2000000));
        assert_eq!(metadata.player_freq, Some(50));
    }

    #[test]
    fn test_ym3b_valid_loop_frame() {
        // Create a valid YM3b file with loop frame = 0 (points to first frame)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");

        // Add 3 frames
        for frame_idx in 0..3 {
            for reg in 0..14 {
                data.push((frame_idx * 14 + reg) as u8);
            }
        }

        // Add valid loop frame (0, pointing to first frame)
        data.extend_from_slice(&0u32.to_be_bytes());

        let parser = YmParser;
        let result = parser.parse(&data);
        assert!(result.is_ok());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn test_ym3b_max_loop_frame() {
        // Create a YM3b file with loop frame = frame_count - 1 (valid max)
        let mut data = Vec::new();
        data.extend_from_slice(b"YM3!");

        // Add 5 frames
        for frame_idx in 0..5 {
            for reg in 0..14 {
                data.push((frame_idx * 14 + reg) as u8);
            }
        }

        // Add valid loop frame (4, pointing to last frame)
        data.extend_from_slice(&4u32.to_be_bytes());

        let parser = YmParser;
        let result = parser.parse(&data);
        assert!(result.is_ok());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 5);
    }
}
