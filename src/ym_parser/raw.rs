//! Raw Register Dump Format Parser
//!
//! Parses raw sequences of PSG register writes.
//! Each frame is 16 bytes representing registers R0-R15.

use super::FormatParser;
use crate::Result;

/// Raw Register Dump Parser
pub struct RawParser;

impl RawParser {
    /// Create a new raw parser
    pub fn new() -> Self {
        RawParser
    }

    /// Parse raw register frames
    /// Expects data as a sequence of 16-byte register frames
    pub fn parse_frames(data: &[u8]) -> Result<Vec<[u8; 16]>> {
        if !data.len().is_multiple_of(16) {
            return Err(format!(
                "Data length {} is not a multiple of 16 (expected register frames)",
                data.len()
            )
            .into());
        }

        let num_frames = data.len() / 16;
        let mut frames = Vec::with_capacity(num_frames);

        for i in 0..num_frames {
            let frame_start = i * 16;
            let frame_end = frame_start + 16;
            let mut frame = [0u8; 16];
            frame.copy_from_slice(&data[frame_start..frame_end]);
            frames.push(frame);
        }

        Ok(frames)
    }
}

impl Default for RawParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for RawParser {
    fn parse(&self, data: &[u8]) -> Result<Vec<[u8; 16]>> {
        Self::parse_frames(data)
    }

    fn name(&self) -> &str {
        "Raw Register Dump Parser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_parser_creation() {
        let parser = RawParser::new();
        assert_eq!(parser.name(), "Raw Register Dump Parser");
    }

    #[test]
    fn test_parse_single_frame() {
        let data = vec![0u8; 16];
        let frames = RawParser::parse_frames(&data).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], [0u8; 16]);
    }

    #[test]
    fn test_parse_multiple_frames() {
        let data = vec![0u8; 48]; // 3 frames
        let frames = RawParser::parse_frames(&data).unwrap();
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn test_parse_invalid_length() {
        let data = vec![0u8; 17]; // Not a multiple of 16
        let result = RawParser::parse_frames(&data);
        assert!(result.is_err());
    }
}
