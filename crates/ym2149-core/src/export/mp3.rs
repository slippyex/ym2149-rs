//! MP3 file export functionality using LAME encoder

use super::{apply_fade_out, normalize_samples, ExportConfig};
use crate::replayer::{LoadSummary, PlaybackController, Ym6Player};
use crate::Result;
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

    // Generate all samples
    println!(
        "Rendering {} frames ({:.1}s)...",
        info.frame_count,
        total_samples as f32 / config.sample_rate as f32
    );
    let mut samples = player.generate_samples(total_samples);

    // Apply post-processing
    if config.normalize {
        println!("Normalizing audio...");
        normalize_samples(&mut samples);
    }

    if config.fade_out_duration > 0.0 {
        println!("Applying {:.1}s fade out...", config.fade_out_duration);
        apply_fade_out(&mut samples, config.fade_out_duration, config.sample_rate);
    }

    // Convert f32 to i16
    let samples_i16: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    println!("Encoding to MP3 ({} kbps)...", bitrate);

    // Create LAME encoder
    let mut encoder = Builder::new().ok_or("Failed to create LAME encoder")?;

    encoder
        .set_sample_rate(config.sample_rate)
        .map_err(|e| format!("Failed to set sample rate: {:?}", e))?;

    encoder
        .set_num_channels(config.channels as u8)
        .map_err(|e| format!("Failed to set channels: {:?}", e))?;

    encoder
        .set_brate(mp3lame_encoder::Bitrate::Kbps(bitrate as u16))
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

    // Encode samples in chunks
    const CHUNK_SIZE: usize = 4096;

    for chunk in samples_i16.chunks(CHUNK_SIZE * config.channels as usize) {
        let encoded = if config.channels == 1 {
            let mono = MonoPcm(chunk);
            encoder
                .encode(mono)
                .map_err(|e| format!("Encoding failed: {:?}", e))?
        } else {
            // Interleaved stereo
            let stereo = InterleavedPcm(chunk);
            encoder
                .encode(stereo)
                .map_err(|e| format!("Encoding failed: {:?}", e))?
        };

        output_file
            .write_all(encoded)
            .map_err(|e| format!("Failed to write MP3 data: {}", e))?;
    }

    // Flush encoder
    let flushed = encoder
        .flush::<FlushNoGap>()
        .map_err(|e| format!("Failed to flush encoder: {:?}", e))?;

    output_file
        .write_all(flushed)
        .map_err(|e| format!("Failed to write final MP3 data: {}", e))?;

    println!("MP3 export complete!");
    Ok(())
}
