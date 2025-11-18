//! MP3 file export functionality using LAME encoder

use super::{ExportConfig, apply_fade_out, normalize_samples};
use crate::Result;
use crate::{LoadSummary, PlaybackController, Ym6Player};
use std::path::Path;

/// Export YM playback to MP3 file with default bitrate (192 kbps)
///
/// Renders the entire song to an MP3 file using the LAME encoder.
///
/// # Arguments
///
/// * `player` - YM player instance (will be played from current position to end)
/// * `output_path` - Path where the MP3 file will be written
///
/// # Examples
///
/// ```no_run
/// use ym2149::export::export_to_mp3_default;
/// use ym2149::replayer::load_song;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = std::fs::read("song.ym")?;
/// let (mut player, _) = load_song(&data)?;
///
/// export_to_mp3_default(&mut player, "output.mp3")?;
/// # Ok(())
/// # }
/// ```
pub fn export_to_mp3_default<P: AsRef<Path>>(
    player: &mut Ym6Player,
    info: LoadSummary,
    output_path: P,
) -> Result<()> {
    export_to_mp3(player, output_path, info, 192)
}

/// Export YM playback to MP3 file with custom bitrate
///
/// # Arguments
///
/// * `player` - YM player instance
/// * `output_path` - Path where the MP3 file will be written
/// * `bitrate` - MP3 bitrate in kbps (e.g., 128, 192, 320)
///
/// # Examples
///
/// ```no_run
/// use ym2149::export::export_to_mp3;
/// use ym2149::replayer::load_song;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = std::fs::read("song.ym")?;
/// let (mut player, _) = load_song(&data)?;
///
/// export_to_mp3(&mut player, "output.mp3", 320)?; // High quality
/// # Ok(())
/// # }
/// ```
pub fn export_to_mp3<P: AsRef<Path>>(
    player: &mut Ym6Player,
    output_path: P,
    info: LoadSummary,
    bitrate: u32,
) -> Result<()> {
    export_to_mp3_with_config(player, output_path, info, bitrate, ExportConfig::default())
}

/// Export YM playback to MP3 file with custom configuration
///
/// # Arguments
///
/// * `player` - YM player instance
/// * `output_path` - Path where the MP3 file will be written
/// * `bitrate` - MP3 bitrate in kbps (e.g., 128, 192, 320)
/// * `config` - Export configuration (sample rate, channels, normalization, etc.)
///
/// # Examples
///
/// ```no_run
/// use ym2149::export::{export_to_mp3_with_config, ExportConfig};
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
/// export_to_mp3_with_config(&mut player, "output.mp3", 192, config)?;
/// # Ok(())
/// # }
/// ```
pub fn export_to_mp3_with_config<P: AsRef<Path>>(
    player: &mut Ym6Player,
    output_path: P,
    info: LoadSummary,
    bitrate: u32,
    config: ExportConfig,
) -> Result<()> {
    use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm, MonoPcm};
    use std::fs::File;
    use std::io::Write;

    // Ensure player is playing
    player.play()?;

    // Calculate total samples needed
    let total_samples = info.total_samples();

    println!(
        "Rendering {} frames ({:.1}s) to MP3 ({} kbps)...",
        info.frame_count,
        total_samples as f32 / config.sample_rate as f32,
        bitrate
    );

    // Create LAME encoder
    let mut encoder = Builder::new().ok_or("Failed to create LAME encoder")?;

    encoder
        .set_sample_rate(config.sample_rate)
        .map_err(|e| format!("Failed to set sample rate: {:?}", e))?;

    encoder
        .set_num_channels(config.channels as u8)
        .map_err(|e| format!("Failed to set channels: {:?}", e))?;

    // Map requested bitrate to nearest supported value
    let bitrate_enum = match bitrate {
        0..=64 => mp3lame_encoder::Bitrate::Kbps64,
        65..=96 => mp3lame_encoder::Bitrate::Kbps96,
        97..=128 => mp3lame_encoder::Bitrate::Kbps128,
        129..=192 => mp3lame_encoder::Bitrate::Kbps192,
        193..=256 => mp3lame_encoder::Bitrate::Kbps256,
        _ => mp3lame_encoder::Bitrate::Kbps320,
    };

    encoder
        .set_brate(bitrate_enum)
        .map_err(|e| format!("Failed to set bitrate: {:?}", e))?;

    encoder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| format!("Failed to set quality: {:?}", e))?;

    let mut encoder = encoder
        .build()
        .map_err(|e| format!("Failed to build encoder: {:?}", e))?;

    // Open output file
    let mut output_file = File::create(output_path.as_ref())
        .map_err(|e| format!("Failed to create MP3 file: {}", e))?;

    // Streaming generation with buffer reuse (avoids loading entire song into RAM)
    const SAMPLES_PER_CHUNK: usize = 4096;
    let mut sample_buffer_f32 = vec![0.0f32; SAMPLES_PER_CHUNK * config.channels as usize];
    let mut sample_buffer_i16 = vec![0i16; SAMPLES_PER_CHUNK * config.channels as usize];
    let mut mp3_buffer = vec![0u8; SAMPLES_PER_CHUNK * 4];

    let mut samples_generated = 0;
    let mut all_samples_for_postprocess = if config.normalize || config.fade_out_duration > 0.0 {
        // Need full buffer for normalization/fade-out
        Vec::with_capacity(total_samples)
    } else {
        Vec::new()
    };

    // Generate and encode in chunks
    while samples_generated < total_samples {
        let samples_to_generate = (total_samples - samples_generated).min(SAMPLES_PER_CHUNK * config.channels as usize);
        let buffer_slice = &mut sample_buffer_f32[..samples_to_generate];

        // Generate samples using zero-allocation API
        player.generate_samples_into(buffer_slice);

        // If we need post-processing, collect all samples first
        if config.normalize || config.fade_out_duration > 0.0 {
            all_samples_for_postprocess.extend_from_slice(buffer_slice);
            samples_generated += samples_to_generate;
            continue;
        }

        // Convert f32 to i16 in-place (reuse buffer)
        for (i, &sample) in buffer_slice.iter().enumerate() {
            sample_buffer_i16[i] = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }

        // Encode chunk
        let i16_slice = &sample_buffer_i16[..samples_to_generate];
        let encoded_size = if config.channels == 1 {
            let mono = MonoPcm(i16_slice);
            encoder
                .encode(mono, mp3_buffer.as_mut_slice())
                .map_err(|e| format!("Encoding failed: {:?}", e))?
        } else {
            let stereo = InterleavedPcm(i16_slice);
            encoder
                .encode(stereo, mp3_buffer.as_mut_slice())
                .map_err(|e| format!("Encoding failed: {:?}", e))?
        };

        // Verify encoder contract
        if encoded_size > mp3_buffer.len() {
            return Err(format!(
                "MP3 encoder violated contract: encoded {} bytes into {}-byte buffer",
                encoded_size,
                mp3_buffer.len()
            )
            .into());
        }

        output_file
            .write_all(&mp3_buffer[..encoded_size])
            .map_err(|e| format!("Failed to write MP3 data: {}", e))?;

        samples_generated += samples_to_generate;
    }

    // Post-processing path: normalize and/or fade-out, then encode
    if !all_samples_for_postprocess.is_empty() {
        if config.normalize {
            println!("Normalizing audio...");
            normalize_samples(&mut all_samples_for_postprocess);
        }

        if config.fade_out_duration > 0.0 {
            println!("Applying {:.1}s fade out...", config.fade_out_duration);
            apply_fade_out(&mut all_samples_for_postprocess, config.fade_out_duration, config.sample_rate);
        }

        // Encode post-processed samples in chunks
        for chunk_f32 in all_samples_for_postprocess.chunks(SAMPLES_PER_CHUNK * config.channels as usize) {
            // Convert to i16
            for (i, &sample) in chunk_f32.iter().enumerate() {
                sample_buffer_i16[i] = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            }

            let i16_slice = &sample_buffer_i16[..chunk_f32.len()];
            let encoded_size = if config.channels == 1 {
                let mono = MonoPcm(i16_slice);
                encoder
                    .encode(mono, mp3_buffer.as_mut_slice())
                    .map_err(|e| format!("Encoding failed: {:?}", e))?
            } else {
                let stereo = InterleavedPcm(i16_slice);
                encoder
                    .encode(stereo, mp3_buffer.as_mut_slice())
                    .map_err(|e| format!("Encoding failed: {:?}", e))?
            };

            if encoded_size > mp3_buffer.len() {
                return Err(format!(
                    "MP3 encoder violated contract: encoded {} bytes into {}-byte buffer",
                    encoded_size,
                    mp3_buffer.len()
                )
                .into());
            }

            output_file
                .write_all(&mp3_buffer[..encoded_size])
                .map_err(|e| format!("Failed to write MP3 data: {}", e))?;
        }
    }

    // Flush encoder - get remaining encoded data
    const MAX_FLUSH_SIZE: usize = 7200; // Maximum flush buffer size per mp3lame spec
    let mut flush_buffer = vec![0u8; MAX_FLUSH_SIZE];
    let flushed_size = encoder
        .flush::<FlushNoGap>(flush_buffer.as_mut_slice())
        .map_err(|e| format!("Failed to flush encoder: {:?}", e))?;

    // Verify encoder contract: flushed_size must not exceed buffer capacity
    if flushed_size > flush_buffer.len() {
        return Err(format!(
            "MP3 encoder violated contract during flush: flushed {} bytes into {}-byte buffer",
            flushed_size,
            flush_buffer.len()
        )
        .into());
    }

    output_file
        .write_all(&flush_buffer[..flushed_size])
        .map_err(|e| format!("Failed to write final MP3 data: {}", e))?;

    println!("MP3 export complete!");
    Ok(())
}
