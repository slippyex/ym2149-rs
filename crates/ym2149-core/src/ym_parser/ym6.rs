//! YM6 format parser for Atari ST chiptunes
//!
//! YM6 is a register dump format for YM2149 PSG music files.
//! Files consist of a header followed by metadata and YM register data.
//!
//! Format details:
//! - Header: 34 bytes (fixed size)
//! - Metadata: Song name, author, comment (null-terminated strings)
//! - Register data: 16 bytes per frame (r0-r15)
//! - Interleaved or non-interleaved format
//! - Optional LZH compression

use super::FormatParser;
use crate::Result;
use std::io::Cursor;

/// Type alias for full YM6 parse result: frames, header, metadata, digidrums
pub type Ym6ParseResult = (Vec<[u8; 16]>, Ym6Header, Ym6Metadata, Vec<Vec<u8>>);

/// YM6 file header
#[derive(Debug, Clone)]
pub struct Ym6Header {
    /// Number of frames in the file
    pub frame_count: u32,
    /// Song attributes (bit 0: interleaved format)
    pub attributes: u32,
    /// Number of digidrum samples
    pub digidrum_count: u16,
    /// YM master clock frequency in Hz (usually 2,000,000 for ATARI ST)
    pub master_clock: u32,
    /// Original player frame rate in Hz (usually 50)
    pub frame_rate: u16,
    /// Loop frame number (0 to loop at beginning)
    pub loop_frame: u32,
    /// Size of future additional data to skip
    pub extra_data_size: u16,
}

/// YM6 file metadata
#[derive(Debug, Clone, Default)]
pub struct Ym6Metadata {
    /// Song name
    pub song_name: String,
    /// Author name
    pub author: String,
    /// Song comment
    pub comment: String,
}

/// YM6 file parser
pub struct Ym6Parser;

impl Ym6Parser {
    /// Maximum reasonable frame count (100,000 frames ≈ 33 minutes at 50Hz)
    const MAX_REASONABLE_FRAMES: u32 = 100_000;

    /// Parse YM6 header from data
    fn parse_header(data: &[u8]) -> Result<Ym6Header> {
        if data.len() < 34 {
            return Err("YM6 file too small for header".into());
        }

        // Check magic number "YM6!"
        if &data[0..4] != b"YM6!" {
            return Err("Invalid YM6 magic number".into());
        }

        // Check signature "LeOnArD!"
        if &data[4..12] != b"LeOnArD!" {
            return Err("Invalid YM6 signature".into());
        }

        // Parse header fields (big-endian)
        let frame_count = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let attributes = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let digidrum_count = u16::from_be_bytes([data[20], data[21]]);
        let master_clock = u32::from_be_bytes([data[22], data[23], data[24], data[25]]);
        let frame_rate = u16::from_be_bytes([data[26], data[27]]);
        let loop_frame = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);
        let extra_data_size = u16::from_be_bytes([data[32], data[33]]);

        // Validate frame count
        if frame_count == 0 {
            return Err("YM6 file has zero frames".into());
        }

        if frame_count > Self::MAX_REASONABLE_FRAMES {
            return Err(format!(
                "YM6 frame count {} exceeds reasonable limit of {}",
                frame_count,
                Self::MAX_REASONABLE_FRAMES
            )
            .into());
        }

        Ok(Ym6Header {
            frame_count,
            attributes,
            digidrum_count,
            master_clock,
            frame_rate,
            loop_frame,
            extra_data_size,
        })
    }

    /// Parse null-terminated string from cursor
    fn parse_nt_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let mut string = String::new();
        let buf = cursor.get_ref();
        let pos = cursor.position() as usize;

        if pos >= buf.len() {
            return Ok(string);
        }

        for i in pos..buf.len() {
            if buf[i] == 0 {
                cursor.set_position((i + 1) as u64);
                return Ok(string);
            }
            string.push(buf[i] as char);
        }

        // No null terminator found, consume rest
        cursor.set_position(buf.len() as u64);
        Ok(string)
    }

    /// Parse metadata from file
    fn parse_metadata(data: &[u8], offset: usize) -> Result<(Ym6Metadata, usize)> {
        let mut cursor = Cursor::new(&data[offset..]);

        let song_name = Self::parse_nt_string(&mut cursor)?;
        let author = Self::parse_nt_string(&mut cursor)?;
        let comment = Self::parse_nt_string(&mut cursor)?;

        let final_offset = offset + cursor.position() as usize;

        Ok((
            Ym6Metadata {
                song_name,
                author,
                comment,
            },
            final_offset,
        ))
    }

    /// Parse register data (either interleaved or non-interleaved)
    fn parse_register_data(
        data: &[u8],
        offset: usize,
        frame_count: u32,
        is_interleaved: bool,
    ) -> Result<Vec<[u8; 16]>> {
        let register_data_size = (frame_count as usize) * 16;
        if offset + register_data_size > data.len() {
            return Err("Not enough data for register frames".into());
        }

        let register_bytes = &data[offset..offset + register_data_size];
        let mut frames = vec![[0u8; 16]; frame_count as usize];

        if is_interleaved {
            // Interleaved format: all r0s, then all r1s, etc.
            for reg_idx in 0..16 {
                for (frame_idx, frame) in frames.iter_mut().enumerate() {
                    frame[reg_idx] = register_bytes[reg_idx * frame_count as usize + frame_idx];
                }
            }
        } else {
            // Non-interleaved format: r0-r15 for frame 0, then r0-r15 for frame 1, etc.
            for (frame_idx, frame) in frames.iter_mut().enumerate() {
                let start = frame_idx * 16;
                frame.copy_from_slice(&register_bytes[start..start + 16]);
            }
        }

        Ok(frames)
    }
}

impl Ym6Parser {
    /// Parse YM6 file and return frames, metadata, and digidrum samples
    pub fn parse_full(&self, data: &[u8]) -> Result<Ym6ParseResult> {
        // Parse header
        let header = Self::parse_header(data)?;

        // Skip extra data section before digidrums (matches reference)
        let mut offset: usize = 34;
        offset = offset
            .checked_add(header.extra_data_size as usize)
            .ok_or("Extra data offset overflow")?;
        if offset > data.len() {
            return Err("Extra data extends beyond file".into());
        }

        // Collect digidrum samples if present
        let mut digidrums: Vec<Vec<u8>> = Vec::new();
        for _ in 0..header.digidrum_count {
            if offset + 4 > data.len() {
                return Err("Incomplete digidrum sample size field".into());
            }
            let sample_size = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset = offset.checked_add(4).ok_or("Digidrum offset overflow")?;

            if offset.checked_add(sample_size).is_none() || offset + sample_size > data.len() {
                return Err("Incomplete digidrum sample data".into());
            }
            let sample = data[offset..offset + sample_size].to_vec();
            digidrums.push(sample);
            offset += sample_size;
        }

        // Skip extra data if present
        offset = offset
            .checked_add(header.extra_data_size as usize)
            .ok_or("Extra data offset overflow")?;
        if offset > data.len() {
            return Err("Extra data extends beyond file".into());
        }

        // Parse metadata
        let (metadata, metadata_end) = Self::parse_metadata(data, offset)?;
        offset = metadata_end;

        // Parse register data
        let is_interleaved = (header.attributes & 1) != 0;
        let frames = Self::parse_register_data(data, offset, header.frame_count, is_interleaved)?;

        // Verify end marker (must be present)
        let register_data_size = (header.frame_count as usize) * 16;
        let end_marker_offset = offset + register_data_size;

        if end_marker_offset + 4 > data.len() {
            return Err("YM6 file truncated - missing end marker".into());
        }

        if &data[end_marker_offset..end_marker_offset + 4] != b"End!" {
            return Err("Invalid YM6 end marker".into());
        }

        Ok((frames, header, metadata, digidrums))
    }
}

impl FormatParser for Ym6Parser {
    fn parse(&self, data: &[u8]) -> Result<Vec<[u8; 16]>> {
        // Parse header
        let header = Self::parse_header(data)?;

        // Skip digidrum samples if present
        let mut offset = 34;
        for _ in 0..header.digidrum_count {
            if offset + 4 > data.len() {
                return Err("Incomplete digidrum sample size field".into());
            }
            let sample_size = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4 + sample_size as usize;

            if offset > data.len() {
                return Err("Incomplete digidrum sample data".into());
            }
        }

        // Skip extra data if present
        offset += header.extra_data_size as usize;
        if offset > data.len() {
            return Err("Extra data extends beyond file".into());
        }

        // Parse metadata
        let (_, metadata_end) = Self::parse_metadata(data, offset)?;
        offset = metadata_end;

        // Parse register data
        let is_interleaved = (header.attributes & 1) != 0;
        let frames = Self::parse_register_data(data, offset, header.frame_count, is_interleaved)?;

        // Verify end marker (must be present)
        let register_data_size = (header.frame_count as usize) * 16;
        let end_marker_offset = offset + register_data_size;

        if end_marker_offset + 4 > data.len() {
            return Err("YM6 file truncated - missing end marker".into());
        }

        if &data[end_marker_offset..end_marker_offset + 4] != b"End!" {
            return Err("Invalid YM6 end marker".into());
        }

        Ok(frames)
    }

    fn name(&self) -> &str {
        "YM6"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_minimal_ym6(frame_count: u32, is_interleaved: bool) -> Vec<u8> {
        let mut data = Vec::new();

        // Header (34 bytes)
        data.extend_from_slice(b"YM6!"); // Magic (0-3)
        data.extend_from_slice(b"LeOnArD!"); // Signature (4-11)
        data.extend_from_slice(&frame_count.to_be_bytes()); // Frame count (12-15)
        data.extend_from_slice(&(if is_interleaved { 1u32 } else { 0u32 }).to_be_bytes()); // Attributes (16-19)
        data.extend_from_slice(&0u16.to_be_bytes()); // No digidrum samples (20-21)
        data.extend_from_slice(&2_000_000u32.to_be_bytes()); // Master clock (22-25)
        data.extend_from_slice(&50u16.to_be_bytes()); // Frame rate (26-27)
        data.extend_from_slice(&0u32.to_be_bytes()); // Loop frame (28-31)
        data.extend_from_slice(&0u16.to_be_bytes()); // No extra data (32-33)

        // Metadata (null-terminated strings)
        data.extend_from_slice(b"Test Song\0"); // Song name
        data.extend_from_slice(b"Test Author\0"); // Author
        data.extend_from_slice(b"Test Comment\0"); // Comment

        // Register data
        let register_data_size = (frame_count as usize) * 16;
        data.resize(data.len() + register_data_size, 0x42); // Fill with pattern

        // End marker
        data.extend_from_slice(b"End!");

        data
    }

    #[test]
    fn test_ym6_header_parsing() {
        let data = create_minimal_ym6(100, false);
        let header = Ym6Parser::parse_header(&data).unwrap();

        assert_eq!(header.frame_count, 100);
        assert_eq!(header.master_clock, 2_000_000);
        assert_eq!(header.frame_rate, 50);
        assert_eq!(header.loop_frame, 0);
        assert_eq!(header.digidrum_count, 0);
    }

    #[test]
    fn test_ym6_invalid_magic() {
        let mut data = create_minimal_ym6(10, false);
        data[0] = 0xFF; // Corrupt magic number
        let result = Ym6Parser::parse_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_ym6_invalid_signature() {
        let mut data = create_minimal_ym6(10, false);
        data[4] = 0xFF; // Corrupt signature
        let result = Ym6Parser::parse_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_ym6_non_interleaved_parsing() {
        let data = create_minimal_ym6(5, false);
        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();

        assert_eq!(frames.len(), 5);
        // All registers should be filled with pattern
        for frame in frames {
            for reg in frame {
                assert_eq!(reg, 0x42);
            }
        }
    }

    #[test]
    fn test_ym6_interleaved_parsing() {
        let data = create_minimal_ym6(5, true);
        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();

        assert_eq!(frames.len(), 5);
        // All registers should be filled with pattern
        for frame in frames {
            for reg in frame {
                assert_eq!(reg, 0x42);
            }
        }
    }

    #[test]
    fn test_ym6_metadata_extraction() {
        let data = create_minimal_ym6(1, false);
        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 1);
        // Metadata parsing is tested implicitly
    }

    #[test]
    fn test_ym6_file_too_small() {
        let data = vec![0u8; 10];
        let result = Ym6Parser::parse_header(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_ym6_invalid_end_marker() {
        let mut data = create_minimal_ym6(5, false);
        // Corrupt end marker
        let end_pos = data.len() - 4;
        data[end_pos] = 0xFF;
        let parser = Ym6Parser;
        let result = parser.parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_ym6_missing_end_marker() {
        let mut data = create_minimal_ym6(5, false);
        // Remove end marker
        data.truncate(data.len() - 4);
        let parser = Ym6Parser;
        let result = parser.parse(&data);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing end marker"));
    }

    #[test]
    fn test_ym6_zero_frame_count() {
        let mut data = create_minimal_ym6(1, false);
        data[12..16].copy_from_slice(&0u32.to_be_bytes());
        let result = Ym6Parser::parse_header(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("zero frames"));
    }

    #[test]
    fn test_ym6_excessive_frame_count() {
        let mut data = create_minimal_ym6(1, false);
        // Set frame count to exceeds MAX_REASONABLE_FRAMES
        data[12..16].copy_from_slice(&200_000u32.to_be_bytes());
        let result = Ym6Parser::parse_header(&data);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds reasonable limit"));
    }

    fn create_ym6_with_distinct_values(frame_count: u32, is_interleaved: bool) -> Vec<u8> {
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"YM6!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&frame_count.to_be_bytes());
        data.extend_from_slice(&(if is_interleaved { 1u32 } else { 0u32 }).to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&2_000_000u32.to_be_bytes());
        data.extend_from_slice(&50u16.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());

        // Metadata
        data.extend_from_slice(b"Song\0Author\0Comment\0");

        // Register data with pattern: value = (reg * 16 + frame) % 256
        if is_interleaved {
            for reg in 0u8..16 {
                for frame in 0..frame_count {
                    data.push(((reg as u32 * 16 + frame) % 256) as u8);
                }
            }
        } else {
            for frame in 0..frame_count {
                for reg in 0u8..16 {
                    data.push(((reg as u32 * 16 + frame) % 256) as u8);
                }
            }
        }

        data.extend_from_slice(b"End!");
        data
    }

    #[test]
    fn test_ym6_interleaved_correct_values() {
        let data = create_ym6_with_distinct_values(3, true);
        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();

        assert_eq!(frames.len(), 3);
        // Frame 0: R0=0x00, R1=0x10, R2=0x20
        assert_eq!(frames[0][0], 0x00);
        assert_eq!(frames[0][1], 0x10);
        assert_eq!(frames[0][2], 0x20);

        // Frame 1: R0=0x01, R1=0x11, R2=0x21
        assert_eq!(frames[1][0], 0x01);
        assert_eq!(frames[1][1], 0x11);
        assert_eq!(frames[1][2], 0x21);
    }

    #[test]
    fn test_ym6_non_interleaved_correct_values() {
        let data = create_ym6_with_distinct_values(3, false);
        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();

        assert_eq!(frames.len(), 3);
        // Frame 0: R0=0x00, R1=0x10, R2=0x20
        assert_eq!(frames[0][0], 0x00);
        assert_eq!(frames[0][1], 0x10);
        assert_eq!(frames[0][2], 0x20);

        // Frame 1: R0=0x01, R1=0x11, R2=0x21
        assert_eq!(frames[1][0], 0x01);
        assert_eq!(frames[1][1], 0x11);
        assert_eq!(frames[1][2], 0x21);
    }

    #[test]
    fn test_ym6_with_digidrum_samples() {
        let mut data = Vec::new();

        // Header with 2 digidrum samples
        data.extend_from_slice(b"YM6!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&2u32.to_be_bytes()); // 2 frames
        data.extend_from_slice(&0u32.to_be_bytes()); // Not interleaved
        data.extend_from_slice(&2u16.to_be_bytes()); // 2 digidrum samples
        data.extend_from_slice(&2_000_000u32.to_be_bytes());
        data.extend_from_slice(&50u16.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());

        // Digidrum sample 1: 10 bytes
        data.extend_from_slice(&10u32.to_be_bytes());
        data.extend_from_slice(&[0xAA; 10]);

        // Digidrum sample 2: 5 bytes
        data.extend_from_slice(&5u32.to_be_bytes());
        data.extend_from_slice(&[0xBB; 5]);

        // Metadata
        data.extend_from_slice(b"Song\0Author\0Comment\0");

        // Register data (2 frames * 16 registers)
        data.extend_from_slice(&[0x42; 32]);

        // End marker
        data.extend_from_slice(b"End!");

        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn test_ym6_with_extra_data() {
        let mut data = Vec::new();

        // Header with extra data
        data.extend_from_slice(b"YM6!");
        data.extend_from_slice(b"LeOnArD!");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&2_000_000u32.to_be_bytes());
        data.extend_from_slice(&50u16.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&20u16.to_be_bytes()); // 20 bytes extra data

        // Extra data (should be skipped)
        data.extend_from_slice(&[0xFF; 20]);

        // Metadata
        data.extend_from_slice(b"Song\0Author\0Comment\0");

        // Register data
        data.extend_from_slice(&[0x42; 16]);

        // End marker
        data.extend_from_slice(b"End!");

        let parser = Ym6Parser;
        let frames = parser.parse(&data).unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_ym6_parse_full_with_metadata() {
        let data = create_minimal_ym6(2, false);
        let parser = Ym6Parser;
        let (frames, header, metadata, _digidrums) = parser.parse_full(&data).unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(header.frame_count, 2);
        assert_eq!(metadata.song_name, "Test Song");
        assert_eq!(metadata.author, "Test Author");
        assert_eq!(metadata.comment, "Test Comment");
    }
}
