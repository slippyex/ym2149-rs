//! Audio export functionality for YM2149 playback
//!
//! This module provides utilities to export YM file playback to various audio formats:
//! - WAV (uncompressed PCM)
//! - MP3 (LAME-encoded)
//!
//! # Examples
//!
//! ## Export to WAV
//!
//! ```no_run
//! use ym2149::export::export_to_wav;
//! use ym2149::replayer::load_song;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("song.ym")?;
//! let (mut player, _) = load_song(&data)?;
//!
//! export_to_wav(&mut player, "output.wav")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Export to MP3
//!
//! ```no_run
//! use ym2149::export::export_to_mp3;
//! use ym2149::replayer::load_song;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("song.ym")?;
//! let (mut player, _) = load_song(&data)?;
//!
//! export_to_mp3(&mut player, "output.mp3", 192)?; // 192 kbps
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "export-wav")]
mod wav;
#[cfg(feature = "export-wav")]
pub use wav::*;

#[cfg(feature = "export-mp3")]
mod mp3;
#[cfg(feature = "export-mp3")]
pub use mp3::*;

/// Export configuration options
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Sample rate for export (default: 44100 Hz)
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo)
    pub channels: u16,
    /// Whether to normalize audio to prevent clipping
    pub normalize: bool,
    /// Fade out duration in seconds (0 = no fade)
    pub fade_out_duration: f32,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            channels: 1,
            normalize: true,
            fade_out_duration: 0.0,
        }
    }
}

impl ExportConfig {
    /// Create config for stereo export
    pub fn stereo() -> Self {
        Self {
            channels: 2,
            ..Default::default()
        }
    }

    /// Create config with custom sample rate
    pub fn with_sample_rate(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }

    /// Enable normalization to prevent clipping
    pub fn normalize(mut self, enable: bool) -> Self {
        self.normalize = enable;
        self
    }

    /// Add fade out at the end
    pub fn fade_out(mut self, duration_seconds: f32) -> Self {
        self.fade_out_duration = duration_seconds;
        self
    }
}

/// Apply normalization to audio samples
fn normalize_samples(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }

    // Find peak amplitude
    let peak = samples
        .iter()
        .map(|s| s.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(1.0);

    // Normalize if peak > 0.95 (leave some headroom)
    if peak > 0.95 {
        let scale = 0.95 / peak;
        for sample in samples.iter_mut() {
            *sample *= scale;
        }
    }
}

/// Apply fade out to the end of audio samples
fn apply_fade_out(samples: &mut [f32], fade_duration: f32, sample_rate: u32) {
    if fade_duration <= 0.0 || samples.is_empty() {
        return;
    }

    let fade_samples = (fade_duration * sample_rate as f32) as usize;
    let start_fade = samples.len().saturating_sub(fade_samples);

    for (i, sample) in samples.iter_mut().enumerate().skip(start_fade) {
        let progress = (i - start_fade) as f32 / fade_samples as f32;
        let fade_factor = 1.0 - progress;
        *sample *= fade_factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_samples() {
        let mut samples = vec![0.5, 1.5, -1.2, 0.8];
        normalize_samples(&mut samples);

        // Check that peak is now <= 0.95
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak <= 0.96); // Allow small floating point error
    }

    #[test]
    fn test_fade_out() {
        let mut samples = vec![1.0; 1000];
        apply_fade_out(&mut samples, 0.1, 44100); // 100ms fade

        // First samples should be unchanged
        assert_eq!(samples[0], 1.0);
        // Last sample should be near 0
        assert!(samples[999].abs() < 0.01);
    }

    #[test]
    fn test_export_config_builder() {
        let config = ExportConfig::stereo().normalize(false).fade_out(2.0);

        assert_eq!(config.channels, 2);
        assert!(!config.normalize);
        assert_eq!(config.fade_out_duration, 2.0);
    }
}
