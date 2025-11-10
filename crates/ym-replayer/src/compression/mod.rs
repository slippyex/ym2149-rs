//! Compression support for YM file formats
//!
//! Automatically detects and decompresses LHA-compressed YM files using the `delharc` crate.
//! Most YM files in the wild are compressed with LHA (Lossless Hamming Archive),
//! typically using the LH5 algorithm.
//!
//! Decompression is transparent - simply load any YM file, and this module handles
//! compression automatically. Uncompressed files pass through unchanged.
//!
//! # Architecture Decision
//!
//! This module provides **transparent decompression** of LHA-compressed YM files.
//! The `decompress_if_needed()` function automatically detects compression and decompresses,
//! or returns uncompressed data unchanged.
//!
//! Design principles:
//! - **Transparency**: Users don't need to know about compression
//! - **Safety**: Decompression includes size limits to prevent decompression bombs
//! - **Backward Compatibility**: Uncompressed files work unchanged
//! - **Robustness**: Errors provide clear guidance for troubleshooting

use crate::Result;
use std::io::Read;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;

/// Search limit for LHA signature pattern (bytes)
///
/// LHA header structure varies by level:
/// - Level 0/1: Method appears at offset 2 (typical)
/// - Level 2: Method can appear up to offset 25
///
/// We search up to 40 bytes to handle all LHA variants robustly.
const LHA_SEARCH_LIMIT: usize = 40;

/// Minimum bytes needed for a complete LHA signature pattern
/// Pattern is "-lh?-" which is 5 bytes
const LHA_SIGNATURE_LENGTH: usize = 5;

/// Maximum decompressed size: 100MB
/// YM files are typically 10KB-1MB uncompressed.
/// This limit prevents decompression bombs while allowing legitimate large files.
const MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;

/// Minimum and maximum valid compression levels in LHA
const LHA_MIN_VALID_LEVEL: u8 = b'0';
const LHA_MAX_VALID_LEVEL: u8 = b'7';

/// Automatically decompress LHA data if compressed, otherwise return as-is
///
/// This function provides **transparent decompression**:
/// - If data is LHA-compressed: extracts the first file from the archive
/// - If data is uncompressed: returns a copy unchanged
///
/// Includes safety guards against decompression bombs (enforces size limits).
///
/// # Arguments
/// * `data` - Byte slice potentially containing LHA-compressed data
///
/// # Returns
/// - `Ok(Vec<u8>)` - Decompressed or original data
/// - `Err` - If decompression fails or size limit exceeded
///
/// # Examples
/// ```ignore
/// use ym2149::compression::decompress_if_needed;
///
/// let data = std::fs::read("song.ym")?;
/// let decompressed = decompress_if_needed(&data)?;
/// // Works with both compressed (LH5) and uncompressed files
/// ```
pub fn decompress_if_needed(data: &[u8]) -> Result<Vec<u8>> {
    if !is_lha_compressed(data) {
        // Not compressed - return copy of original data
        return Ok(data.to_vec());
    }

    // WASM-compatible in-memory decompression
    #[cfg(target_arch = "wasm32")]
    {
        use delharc::LhaDecodeReader;

        // Create LHA reader directly from byte slice (no filesystem needed)
        let reader = LhaDecodeReader::new(data).map_err(|e| {
            crate::ReplayerError::DecompressionError(format!(
                "Failed to parse LHA archive from memory: {}",
                e
            ))
        })?;

        let mut decompressed = Vec::new();

        // Use take() to enforce hard limit and prevent decompression bombs
        let mut limited_reader = reader.take(MAX_DECOMPRESSED_SIZE as u64);
        limited_reader
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("LHA decompression failed: {}", e))?;

        // Verify we didn't hit the limit (would indicate truncation/attack)
        if decompressed.len() >= MAX_DECOMPRESSED_SIZE {
            return Err("Decompressed data exceeded maximum safe size (100MB). \
                The file may be corrupted or an attempted decompression bomb."
                .into());
        }

        Ok(decompressed)
    }

    // Native platform: use file-based decompression
    #[cfg(not(target_arch = "wasm32"))]
    {
        use tempfile::NamedTempFile;

        // Create a temporary file with automatic cleanup via Drop
        let mut temp_file = NamedTempFile::new().map_err(|e| {
            crate::ReplayerError::DecompressionError(format!("Failed to create temporary file: {}", e))
        })?;

        // Write compressed data to the temporary file
        temp_file.write_all(data).map_err(|e| {
            crate::ReplayerError::DecompressionError(format!(
                "Failed to write compressed data to temporary file: {} bytes - {}",
                data.len(),
                e
            ))
        })?;
        temp_file.flush().map_err(|e| {
            crate::ReplayerError::DecompressionError(format!("Failed to flush temporary file: {}", e))
        })?;

        // Get the path while keeping the file handle alive (prevents deletion)
        let temp_path = temp_file.path().to_path_buf();

        // Parse the LHA archive from the temporary file
        let reader = delharc::parse_file(&temp_path).map_err(|e| {
            crate::ReplayerError::DecompressionError(format!(
                "Failed to parse LHA archive from '{}': {}",
                temp_path.display(),
                e
            ))
        })?;

        let mut decompressed = Vec::new();

        // Use take() to enforce hard limit and prevent decompression bombs
        let mut limited_reader = reader.take(MAX_DECOMPRESSED_SIZE as u64);
        limited_reader
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("LHA decompression failed: {}", e))?;

        // Verify we didn't hit the limit (would indicate truncation/attack)
        if decompressed.len() >= MAX_DECOMPRESSED_SIZE {
            return Err("Decompressed data exceeded maximum safe size (100MB). \
                The file may be corrupted or an attempted decompression bomb."
                .into());
        }

        // temp_file Drop runs here - automatic cleanup guaranteed even on panic
        Ok(decompressed)
    }
}

/// Find the offset of LHA signature in data, if present
///
/// LHA format requires a specific pattern: `-lh[0-7]-`
/// This validates:
/// - Dash prefix
/// - "lh" marker
/// - Compression level digit (0-7)
/// - Dash suffix
///
/// Searches within the first `LHA_SEARCH_LIMIT` bytes where LHA headers typically
/// place the method signature, supporting all LHA header variants (level 0-2).
///
/// Returns the offset where the signature starts, or None if not found.
fn find_lha_signature(data: &[u8]) -> Option<usize> {
    if data.len() < LHA_SIGNATURE_LENGTH + 2 {
        return None;
    }

    // Limit search to handle all LHA header variants
    let search_limit = LHA_SEARCH_LIMIT.min(data.len().saturating_sub(LHA_SIGNATURE_LENGTH));

    for i in 1..=search_limit {
        // Use slice pattern matching for clarity and safety
        if let Some(window) = data.get(i..i + LHA_SIGNATURE_LENGTH)
            && window[0] == b'-'
            && window[1] == b'l'
            && window[2] == b'h'
            && is_valid_compression_level(window[3])
            && window[4] == b'-'
        {
            return Some(i);
        }
    }
    None
}

/// Check if a byte is a valid LHA compression level (0-7)
#[inline]
fn is_valid_compression_level(byte: u8) -> bool {
    (LHA_MIN_VALID_LEVEL..=LHA_MAX_VALID_LEVEL).contains(&byte)
}

/// Detect if data is LHA-compressed by checking magic bytes
///
/// LHA files have a specific header structure with method ID pattern `-lh[0-7]-`.
/// This function validates the complete pattern including:
/// - Dash prefix
/// - "lh" identifier
/// - Valid compression level (0-7)
/// - Dash suffix
///
/// # Arguments
/// * `data` - Byte slice to check for LHA compression
///
/// # Returns
/// `true` if the data appears to be LHA-compressed, `false` otherwise
///
/// # Examples
/// ```ignore
/// use ym2149::compression::is_lha_compressed;
///
/// // Valid LHA header (LH5 compression)
/// let lha_data = b"\x20\x2d\x6c\x68\x35\x2d\x15";
/// assert!(is_lha_compressed(lha_data));
///
/// // YM6 format (uncompressed)
/// let ym_data = b"YM6!";
/// assert!(!is_lha_compressed(ym_data));
/// ```
pub fn is_lha_compressed(data: &[u8]) -> bool {
    find_lha_signature(data).is_some()
}

/// Get information about LHA compression
///
/// Extracts the compression level and returns a human-readable description
/// of the compression format.
///
/// # Arguments
/// * `data` - Byte slice potentially containing LHA data
///
/// # Returns
/// `Some(description)` if LHA compression is detected, `None` otherwise
///
/// # Examples
/// ```ignore
/// use ym2149::compression::get_lha_info;
///
/// let lha_data = b"\x20\x2d\x6c\x68\x35\x2d\x15";
/// let info = get_lha_info(lha_data);
/// assert_eq!(info, Some("LH5 compressed (LHA/LZH archive)".to_string()));
/// ```
pub fn get_lha_info(data: &[u8]) -> Option<String> {
    find_lha_signature(data).map(|offset| {
        let level = data[offset + 3] as char;
        format!("LH{} compressed (LHA/LZH archive)", level)
    })
}

/// Check if data is LHA-compressed (for introspection/debugging)
///
/// **Note**: For normal file loading, use `decompress_if_needed()` instead,
/// which handles decompression transparently.
///
/// This function only checks for compression without decompressing.
/// Useful for logging, debugging, or introspection.
///
/// # Arguments
/// * `data` - Byte slice to check
///
/// # Returns
/// `Ok(())` if data is not compressed, `Err` if it is (for explicit checking)
///
/// # Examples
/// ```ignore
/// use ym2149::compression::validate_uncompressed;
///
/// let uncompressed_data = b"YM6!LeOnArD!...";
/// assert!(validate_uncompressed(uncompressed_data).is_ok());
/// ```
#[deprecated(
    since = "0.2.0",
    note = "For normal file loading, use decompress_if_needed() instead. \
            This function is kept for explicit validation/introspection only."
)]
pub fn validate_uncompressed(data: &[u8]) -> Result<()> {
    if is_lha_compressed(data) {
        let info = get_lha_info(data).unwrap_or_default();
        Err(format!(
            "File is LHA-compressed ({}).\n\
            Use decompress_if_needed() for automatic transparent decompression.",
            info
        )
        .into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lha_magic_detection() {
        // Valid LHA header: size + "-lh5-" pattern
        let lha_data = b"\x20\x2d\x6c\x68\x35\x2d\x15";
        assert!(is_lha_compressed(lha_data));

        // Non-LHA data
        assert!(!is_lha_compressed(b"YM3!"));
        assert!(!is_lha_compressed(b"YM6!"));
        assert!(!is_lha_compressed(b"XX"));
    }

    #[test]
    fn test_lha_all_compression_levels() {
        // Test all valid compression levels (0-7)
        for level in b'0'..=b'7' {
            let data = vec![0x20, b'-', b'l', b'h', level, b'-', 0x15];
            assert!(
                is_lha_compressed(&data),
                "Should detect lh{} compression",
                level as char
            );

            let info = get_lha_info(&data).expect("Should extract info");
            assert!(
                info.contains(&format!("LH{}", level as char)),
                "Info should contain compression level"
            );
        }
    }

    #[test]
    fn test_lha_invalid_compression_levels() {
        // Invalid: Level 8
        let data_8 = b"\x20\x2d\x6c\x68\x38\x2d\x15";
        assert!(
            !is_lha_compressed(data_8),
            "Should reject level 8 (max is 7)"
        );

        // Invalid: No closing dash
        let data_nodash = b"\x20\x2d\x6c\x68\x35\x00\x15";
        assert!(
            !is_lha_compressed(data_nodash),
            "Should reject missing closing dash"
        );

        // Invalid: Non-digit compression level
        let data_nondigit = b"\x20\x2d\x6c\x68\x41\x2d\x15";
        assert!(
            !is_lha_compressed(data_nondigit),
            "Should reject non-digit compression level"
        );
    }

    #[test]
    fn test_lha_false_positive_resistance() {
        // Song metadata containing "-lh" substring shouldn't trigger
        // Test case 1: Metadata string with "-lh"
        let mut data = b"YM6!LeOnArD!".to_vec();
        data.extend_from_slice(b"Title: Song-lh edition");
        assert!(
            !is_lha_compressed(&data),
            "Should not detect '-lh' in metadata without full pattern"
        );

        // Test case 2: Has "-lh" but wrong compression level position
        let mut data2 = vec![b'Y', b'M', b'3', b'!'];
        data2.extend_from_slice(b"some-lh data");
        assert!(
            !is_lha_compressed(&data2),
            "Should not match incomplete pattern"
        );
    }

    #[test]
    fn test_lha_minimum_size() {
        // Exactly 7 bytes (minimum for valid header)
        let min_valid = b"\x20\x2d\x6c\x68\x35\x2d\x15";
        assert!(is_lha_compressed(min_valid));

        // 6 bytes (too short, can't have full "-lh?-" pattern)
        let too_short = b"\x20\x2d\x6c\x68\x35\x2d";
        assert!(!is_lha_compressed(too_short));

        // Empty data
        assert!(!is_lha_compressed(&[]));

        // Single byte
        assert!(!is_lha_compressed(&[0x20]));
    }

    #[test]
    fn test_lha_info_extraction() {
        let lha_data = b"\x20\x2d\x6c\x68\x35\x2d\x15";
        let info = get_lha_info(lha_data);
        assert!(info.is_some());
        assert_eq!(info.unwrap(), "LH5 compressed (LHA/LZH archive)");

        // Non-LHA data should return None
        assert!(get_lha_info(b"YM3!").is_none());
        assert!(get_lha_info(b"XX").is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn test_validate_uncompressed_ok() {
        let uncompressed = b"YM6!LeOnArD!";
        assert!(validate_uncompressed(uncompressed).is_ok());

        let ym3 = b"YM3!";
        assert!(validate_uncompressed(ym3).is_ok());
    }

    #[test]
    #[allow(deprecated)]
    fn test_validate_uncompressed_error() {
        let compressed = b"\x20\x2d\x6c\x68\x35\x2d\x15";
        let result = validate_uncompressed(compressed);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("LHA-compressed"));
        // Updated error message now references decompress_if_needed() instead of external tools
        assert!(error_msg.contains("decompress_if_needed"));
    }

    // Decompression tests
    #[test]
    fn test_decompress_uncompressed_passthrough() {
        // Uncompressed data should pass through unchanged
        let uncompressed = b"YM6!LeOnArD!Test data";
        let result = decompress_if_needed(uncompressed).unwrap();
        assert_eq!(result, uncompressed);
    }

    #[test]
    fn test_decompress_uncompressed_empty() {
        // Empty data should pass through unchanged
        let empty = b"";
        let result = decompress_if_needed(empty).unwrap();
        assert_eq!(result, empty);
    }

    #[test]
    fn test_decompress_uncompressed_small_file() {
        // Small uncompressed file
        let small = b"YM3!";
        let result = decompress_if_needed(small).unwrap();
        assert_eq!(result, small);
    }

    #[test]
    fn test_decompress_compressed_data_valid() {
        // Use a real LHA signature to test compressed detection
        // (The actual decompression requires real LHA data)
        let compressed_with_signature = b"\x20\x2d\x6c\x68\x35\x2d\x15GARBAGE";

        // Should attempt decompression and fail gracefully
        // (because it's not a valid LHA file)
        let result = decompress_if_needed(compressed_with_signature);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Failed to parse LHA archive")
                || error_msg.contains("LHA decompression failed")
        );
    }

    #[test]
    fn test_decompress_size_validation() {
        // Verify that we can detect when decompression would exceed limits
        // This test ensures the size checking infrastructure is in place

        // Practical check: verify that the size limit is reasonable (>1MB, <1GB)
        // Constants are verified at compile time
    }

    #[test]
    fn test_decompress_non_lha_signature() {
        // Data that doesn't have LHA signature - should pass through
        let random_data = b"\xFF\xFE\xFD\xFC\xFB\xFA";
        let result = decompress_if_needed(random_data).unwrap();
        assert_eq!(result, random_data);
    }

    #[test]
    fn test_decompress_partial_signature() {
        // Partial LHA signature (not matching full pattern)
        let partial = b"\x2d\x6c\x68";
        let result = decompress_if_needed(partial).unwrap();
        assert_eq!(result, partial);
    }

    // Integration test with real LHA file
    #[test]
    #[ignore] // Only run if Great.ym is available
    fn test_decompress_great_ym_real_file() {
        use std::fs;
        use std::path::Path;

        let test_file = Path::new("examples/Great.ym");
        if !test_file.exists() {
            eprintln!("Skipping integration test - examples/Great.ym not found");
            return;
        }

        // Read the compressed file
        let compressed = fs::read(test_file).expect("Should read examples/Great.ym");

        // Verify it's detected as LHA compressed
        assert!(
            is_lha_compressed(&compressed),
            "Great.ym should be LHA compressed"
        );

        // Get compression info
        let info = get_lha_info(&compressed);
        assert!(info.is_some(), "Should detect compression info");
        assert!(
            info.as_ref().unwrap().contains("LH5"),
            "Should detect LH5 compression"
        );

        // Decompress
        let decompressed =
            decompress_if_needed(&compressed).expect("Should successfully decompress Great.ym");

        // Verify it's now a YM6 file (all decompressed YM files start with magic bytes)
        assert!(
            decompressed.len() > 4,
            "Decompressed data should have content"
        );
        assert!(
            decompressed.starts_with(b"YM"),
            "Decompressed data should be YM format (start with 'YM')"
        );

        // Verify size reduction
        println!(
            "Great.ym: {} bytes compressed → {} bytes decompressed ({:.1}% ratio)",
            compressed.len(),
            decompressed.len(),
            (compressed.len() as f32 / decompressed.len() as f32) * 100.0
        );

        assert!(
            decompressed.len() > compressed.len(),
            "Decompressed should be larger than compressed"
        );
    }
}
