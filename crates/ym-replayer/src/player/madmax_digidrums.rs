//! Mad Max (YM2) prebuilt digi-drum sample bank

/// Sample rate base for Mad Max digi-drums
pub const MADMAX_SAMPLE_RATE_BASE: u32 = 2_457_600;

/// Binary sample data embedded at compile time
const MADMAX_SAMPLES_BIN: &[u8] = include_bytes!("madmax_samples.bin");

/// Lazily parsed sample bank
static MADMAX_SAMPLES_LAZY: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

/// Get reference to the Mad Max sample bank
///
/// This parses the embedded binary data on first access and caches the result.
/// Format: 1 byte (count) + for each sample: 2 bytes (u16 LE length) + data
pub const MADMAX_SAMPLES: MadMaxSampleBank = MadMaxSampleBank;

pub struct MadMaxSampleBank;

impl MadMaxSampleBank {
    /// Get a sample by index (used in tests)
    #[cfg(test)]
    pub fn get(&self, index: usize) -> Option<&Vec<u8>> {
        let samples = MADMAX_SAMPLES_LAZY.get_or_init(|| parse_madmax_samples());
        samples.get(index)
    }

    /// Get the number of samples (used in tests)
    #[cfg(test)]
    pub fn len(&self) -> usize {
        let samples = MADMAX_SAMPLES_LAZY.get_or_init(|| parse_madmax_samples());
        samples.len()
    }

    /// Check if the sample bank is empty (used in tests)
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over all samples
    pub fn iter(&self) -> impl Iterator<Item = &Vec<u8>> {
        let samples = MADMAX_SAMPLES_LAZY.get_or_init(parse_madmax_samples);
        samples.iter()
    }
}

/// Parse the binary sample data
fn parse_madmax_samples() -> Vec<Vec<u8>> {
    let mut samples = Vec::new();
    let data = MADMAX_SAMPLES_BIN;

    // MADMAX_SAMPLES_BIN is a compile-time constant from include_bytes!()
    // No need to check if it's empty as it's always the same size

    // Read number of samples
    let count = data[0] as usize;
    let mut offset = 1;

    // Parse each sample
    for _ in 0..count {
        if offset + 2 > data.len() {
            break;
        }

        // Read length (u16 little-endian)
        let len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        if offset + len > data.len() {
            break;
        }

        // Extract sample data
        let sample = data[offset..offset + len].to_vec();
        samples.push(sample);
        offset += len;
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_madmax_samples_count() {
        assert_eq!(MADMAX_SAMPLES.len(), 40);
    }

    #[test]
    fn test_madmax_samples_not_empty() {
        assert!(!MADMAX_SAMPLES.is_empty());
    }

    #[test]
    fn test_madmax_sample_access() {
        // Test that we can access samples
        assert!(MADMAX_SAMPLES.get(0).is_some());
        assert!(MADMAX_SAMPLES.get(39).is_some());
        assert!(MADMAX_SAMPLES.get(40).is_none());
    }

    #[test]
    fn test_madmax_samples_have_data() {
        // Verify samples are not empty
        for (i, sample) in MADMAX_SAMPLES.iter().enumerate() {
            assert!(!sample.is_empty(), "Sample {} should not be empty", i);
        }
    }
}
