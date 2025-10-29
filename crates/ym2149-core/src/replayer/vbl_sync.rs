//! VBL (Vertical Blanking) Synchronization
//!
//! Manages 50Hz synchronization for PAL ATARI ST display refresh cycle.
//! The VBL interrupt is a critical timing reference for music playback.

use super::TimingConfig;

/// VBL Synchronization Manager
#[derive(Debug, Clone)]
pub struct VblSync {
    /// Timing configuration
    config: TimingConfig,
    /// Current sample count within VBL period
    sample_count: u32,
    /// Total VBL periods (for tracking playback position)
    vbl_count: u64,
    /// Samples until next VBL
    samples_until_vbl: u32,
}

impl VblSync {
    /// Create a new VBL synchronizer
    pub fn new(config: TimingConfig) -> Self {
        let samples_per_vbl = config.samples_per_vbl();
        VblSync {
            config,
            sample_count: 0,
            vbl_count: 0,
            samples_until_vbl: samples_per_vbl,
        }
    }

    /// Clock the synchronizer by one sample
    /// Returns true if VBL interrupt should occur
    pub fn clock(&mut self) -> bool {
        self.sample_count += 1;
        self.samples_until_vbl = self.samples_until_vbl.saturating_sub(1);

        if self.samples_until_vbl == 0 {
            // VBL interrupt occurred
            self.vbl_count += 1;
            self.samples_until_vbl = self.config.samples_per_vbl();
            true
        } else {
            false
        }
    }

    /// Get the current sample count within VBL period
    pub fn get_sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Get the total VBL count
    pub fn get_vbl_count(&self) -> u64 {
        self.vbl_count
    }

    /// Get samples until next VBL
    pub fn get_samples_until_vbl(&self) -> u32 {
        self.samples_until_vbl
    }

    /// Reset the synchronizer
    pub fn reset(&mut self) {
        self.sample_count = 0;
        self.vbl_count = 0;
        self.samples_until_vbl = self.config.samples_per_vbl();
    }

    /// Get the configuration
    pub fn get_config(&self) -> &TimingConfig {
        &self.config
    }

    /// Set the configuration
    pub fn set_config(&mut self, config: TimingConfig) {
        self.config = config;
        self.samples_until_vbl = config.samples_per_vbl();
    }

    /// Get elapsed time in seconds
    pub fn get_elapsed_time(&self) -> f64 {
        (self.sample_count as f64) / (self.config.sample_rate as f64)
    }

    /// Get playback position in VBL frames
    pub fn get_playback_frame(&self) -> u32 {
        (self.vbl_count as u32) + (self.sample_count / self.config.samples_per_vbl())
    }
}

impl Default for VblSync {
    fn default() -> Self {
        Self::new(TimingConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vbl_sync_creation() {
        let vbl = VblSync::default();
        assert_eq!(vbl.get_vbl_count(), 0);
    }

    #[test]
    fn test_vbl_interrupt() {
        let config = TimingConfig {
            sample_rate: 44100,
            vbl_frequency: 50.0,
            psg_clock_frequency: 2_000_000,
        };
        let mut vbl = VblSync::new(config);
        let samples_per_vbl = config.samples_per_vbl();

        // Clock until VBL should occur
        let mut vbl_triggered = false;
        for _ in 0..samples_per_vbl {
            if vbl.clock() {
                vbl_triggered = true;
                break;
            }
        }

        assert!(vbl_triggered);
        assert_eq!(vbl.get_vbl_count(), 1);
    }
}
