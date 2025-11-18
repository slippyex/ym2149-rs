//! WAV file export functionality

use super::{ExportConfig, apply_fade_out, normalize_samples};
use crate::Result;
use crate::{LoadSummary, PlaybackController, Ym6Player};
use std::path::Path;

/// Export YM playback to WAV file
///
/// Renders the entire song to a WAV file with the specified configuration.
///
/// # Arguments
///
/// * `player` - YM player instance (will be played from current position to end)
/// * `output_path` - Path where the WAV file will be written
///
/// # Examples
///
/// ```no_run
/// use ym2149::export::export_to_wav;
/// use ym2149::replayer::load_song;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = std::fs::read("song.ym")?;
/// let (mut player, _) = load_song(&data)?;
///
/// export_to_wav(&mut player, "output.wav")?;
/// # Ok(())
/// # }
/// ```
pub fn export_to_wav<P: AsRef<Path>>(
    player: &mut Ym6Player,
    info: LoadSummary,
    output_path: P,
) -> Result<()> {
    export_to_wav_with_config(player, output_path, info, ExportConfig::default())
}

/// Export YM playback to WAV file with custom configuration
///
/// # Arguments
///
/// * `player` - YM player instance
/// * `output_path` - Path where the WAV file will be written
/// * `config` - Export configuration (sample rate, channels, normalization, etc.)
///
/// # Examples
///
/// ```no_run
/// use ym2149::export::{export_to_wav_with_config, ExportConfig};
/// use ym2149::replayer::load_song;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = std::fs::read("song.ym")?;
/// let (mut player, _) = load_song(&data)?;
///
/// let config = ExportConfig::stereo()
///     .normalize(true)
///     .fade_out(2.0);
///
/// export_to_wav_with_config(&mut player, "output.wav", config)?;
/// # Ok(())
/// # }
/// ```
pub fn export_to_wav_with_config<P: AsRef<Path>>(
    player: &mut Ym6Player,
    output_path: P,
    info: LoadSummary,
    config: ExportConfig,
) -> Result<()> {
    // Ensure player is playing
    player.play()?;

    // Calculate total samples needed
    let total_samples = info.total_samples();

    println!(
        "Rendering {} frames ({:.1}s) to WAV...",
        info.frame_count,
        total_samples as f32 / config.sample_rate as f32
    );

    // If we need post-processing (normalize/fade-out), collect all samples
    if config.normalize || config.fade_out_duration > 0.0 {
        let mut samples = player.generate_samples(total_samples);

        if config.normalize {
            println!("Normalizing audio...");
            normalize_samples(&mut samples);
        }

        if config.fade_out_duration > 0.0 {
            println!("Applying {:.1}s fade out...", config.fade_out_duration);
            apply_fade_out(&mut samples, config.fade_out_duration, config.sample_rate);
        }

        // Convert to stereo if needed
        let final_samples = if config.channels == 2 {
            println!("Converting to stereo...");
            mono_to_stereo(&samples)
        } else {
            samples
        };

        // Write WAV file
        println!("Writing WAV file to {}...", output_path.as_ref().display());
        write_wav_file(
            output_path.as_ref(),
            &final_samples,
            config.sample_rate,
            config.channels,
        )?;
    } else {
        // Streaming path: generate and write in chunks (memory-efficient)
        println!("Writing WAV file to {}...", output_path.as_ref().display());
        write_wav_file_streaming(
            player,
            output_path.as_ref(),
            total_samples,
            config.sample_rate,
            config.channels,
        )?;
    }

    println!("Export complete!");
    Ok(())
}

/// Convert mono samples to stereo (duplicate each sample)
fn mono_to_stereo(mono: &[f32]) -> Vec<f32> {
    let mut stereo = Vec::with_capacity(mono.len() * 2);
    for &sample in mono {
        stereo.push(sample);
        stereo.push(sample);
    }
    stereo
}

/// Write samples to WAV file
fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32, channels: u16) -> Result<()> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    // Convert f32 samples to i16
    for &sample in samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Write samples to WAV file using streaming (memory-efficient for large files)
fn write_wav_file_streaming(
    player: &mut Ym6Player,
    path: &Path,
    total_samples: usize,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    // Generate and write samples in chunks (zero-allocation hot path)
    const SAMPLES_PER_CHUNK: usize = 4096;
    let mut sample_buffer = vec![0.0f32; SAMPLES_PER_CHUNK];
    let mut samples_written = 0;

    while samples_written < total_samples {
        let samples_to_generate = (total_samples - samples_written).min(SAMPLES_PER_CHUNK);
        let buffer_slice = &mut sample_buffer[..samples_to_generate];

        // Generate samples using zero-allocation API
        player.generate_samples_into(buffer_slice);

        // Convert f32 to i16 and write
        for &sample in buffer_slice.iter() {
            let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;

            // Write for each channel
            for _ in 0..channels {
                writer
                    .write_sample(sample_i16)
                    .map_err(|e| format!("Failed to write sample: {}", e))?;
            }
        }

        samples_written += samples_to_generate;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mono_to_stereo() {
        let mono = vec![0.1, 0.2, 0.3];
        let stereo = mono_to_stereo(&mono);

        assert_eq!(stereo.len(), 6);
        assert_eq!(stereo, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
    }
}
