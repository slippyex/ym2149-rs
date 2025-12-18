//! DC offset removal filter
//!
//! The YM2149 output has a DC offset that varies with the audio content.
//! This filter uses a running average to remove it.

/// History buffer size (2048 samples = ~20ms at 44.1kHz)
const HISTORY_SIZE_BITS: usize = 11;
const HISTORY_SIZE: usize = 1 << HISTORY_SIZE_BITS;

/// DC offset removal filter using a running average
///
/// This filter maintains a circular buffer of recent samples and subtracts
/// the running average to center the output around zero.
#[derive(Clone)]
pub struct DcFilter {
    /// Circular buffer of recent samples
    buffer: Box<[u16; HISTORY_SIZE]>,
    /// Current write position in buffer
    position: usize,
    /// Running sum of all samples in buffer
    running_sum: u32,
}

impl DcFilter {
    /// Create a new DC filter
    pub fn new() -> Self {
        Self {
            buffer: Box::new([0; HISTORY_SIZE]),
            position: 0,
            running_sum: 0,
        }
    }

    /// Process a sample and return the DC-adjusted value
    ///
    /// # Arguments
    ///
    /// * `sample` - Input sample (unsigned 16-bit)
    ///
    /// # Returns
    ///
    /// DC-adjusted sample (signed 16-bit)
    #[inline]
    pub fn process(&mut self, sample: u16) -> i16 {
        // Remove old sample from sum
        self.running_sum -= self.buffer[self.position] as u32;
        // Add new sample to sum
        self.running_sum += sample as u32;
        // Store new sample
        self.buffer[self.position] = sample;

        // Advance position with wraparound
        self.position = (self.position + 1) & (HISTORY_SIZE - 1);

        // Compute DC offset as average
        let dc_offset = self.running_sum >> HISTORY_SIZE_BITS;

        // Return sample with DC removed
        (sample as i32 - dc_offset as i32) as i16
    }

    /// Reset the filter state
    pub fn reset(&mut self) {
        self.buffer.fill(0);
        self.position = 0;
        self.running_sum = 0;
    }
}

impl Default for DcFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DcFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DcFilter")
            .field("position", &self.position)
            .field("running_sum", &self.running_sum)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_filter_removes_offset() {
        let mut filter = DcFilter::new();

        // Feed constant DC value
        let dc_value = 1000u16;
        for _ in 0..HISTORY_SIZE * 2 {
            filter.process(dc_value);
        }

        // After warmup, output should be near zero
        let output = filter.process(dc_value);
        assert!(
            output.abs() < 10,
            "DC filter should remove constant offset, got {output}"
        );
    }

    #[test]
    fn test_dc_filter_preserves_ac() {
        let mut filter = DcFilter::new();

        // Warmup with mid-range value
        for _ in 0..HISTORY_SIZE * 2 {
            filter.process(500);
        }

        // Apply step change
        let output = filter.process(1500);

        // Should see significant positive deviation
        assert!(
            output > 100,
            "DC filter should pass AC component, got {output}"
        );
    }

    #[test]
    fn test_dc_filter_reset() {
        let mut filter = DcFilter::new();

        // Process some samples
        for i in 0..100 {
            filter.process(i as u16 * 100);
        }

        // Reset
        filter.reset();

        assert_eq!(filter.position, 0);
        assert_eq!(filter.running_sum, 0);
    }
}
