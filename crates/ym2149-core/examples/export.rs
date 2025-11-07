//! YM to WAV/MP3 Export Tool
//!
//! Converts YM chiptune files to standard audio formats.
//!
//! # Usage
//!
//! Export to WAV (mono):
//! ```bash
//! cargo run --example export --features export-wav -- input.ym output.wav
//! ```
//!
//! Export to WAV (stereo with fade out):
//! ```bash
//! cargo run --example export --features export-wav -- input.ym output.wav --stereo --fade 2.0
//! ```
//!
//! Export to MP3:
//! ```bash
//! cargo run --example export --features export-mp3 -- input.ym output.mp3 --bitrate 320
//! ```
//!
//! Batch export directory:
//! ```bash
//! cargo run --example export --features export-wav -- --batch examples/ output_dir/
//! ```

use std::env;
use std::path::PathBuf;

#[cfg(any(feature = "export-wav", feature = "export-mp3"))]
use ym2149::export::ExportConfig;
#[cfg(feature = "export-mp3")]
use ym2149::export::export_to_mp3_with_config;
#[cfg(feature = "export-wav")]
use ym2149::export::export_to_wav_with_config;

fn print_usage() {
    println!("YM to Audio Export Tool");
    println!();
    println!("USAGE:");
    println!("  Single file export:");
    println!("    export [OPTIONS] <INPUT.ym> <OUTPUT.wav|mp3>");
    println!();
    println!("  Batch export:");
    println!("    export --batch [OPTIONS] <INPUT_DIR> <OUTPUT_DIR>");
    println!();
    println!("OPTIONS:");
    println!("  --stereo           Export as stereo (default: mono)");
    println!("  --no-normalize     Disable audio normalization");
    println!("  --fade <seconds>   Add fade out (e.g., --fade 2.0)");
    println!("  --bitrate <kbps>   MP3 bitrate (default: 192)");
    println!("  --batch            Batch convert all .ym files in directory");
    println!();
    println!("EXAMPLES:");
    println!("  export song.ym song.wav");
    println!("  export song.ym song.mp3 --bitrate 320");
    println!("  export song.ym output.wav --stereo --fade 2.0");
    println!("  export --batch examples/ output/");
}

#[derive(Default)]
struct ExportOptions {
    stereo: bool,
    normalize: bool,
    fade_out: f32,
    bitrate: u32,
    batch: bool,
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf, ExportOptions), String> {
    if args.is_empty() || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        std::process::exit(0);
    }

    let mut opts = ExportOptions {
        normalize: true, // Normalize by default
        bitrate: 192,    // Default MP3 bitrate
        ..Default::default()
    };

    let mut input = None;
    let mut output = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--stereo" => opts.stereo = true,
            "--no-normalize" => opts.normalize = false,
            "--batch" => opts.batch = true,
            "--fade" => {
                i += 1;
                if i >= args.len() {
                    return Err("--fade requires a value (seconds)".to_string());
                }
                opts.fade_out = args[i]
                    .parse()
                    .map_err(|_| "Invalid fade duration".to_string())?;
            }
            "--bitrate" => {
                i += 1;
                if i >= args.len() {
                    return Err("--bitrate requires a value (kbps)".to_string());
                }
                opts.bitrate = args[i].parse().map_err(|_| "Invalid bitrate".to_string())?;
            }
            arg if !arg.starts_with("--") => {
                if input.is_none() {
                    input = Some(PathBuf::from(arg));
                } else if output.is_none() {
                    output = Some(PathBuf::from(arg));
                } else {
                    return Err(format!("Unexpected argument: {}", arg));
                }
            }
            arg => return Err(format!("Unknown option: {}", arg)),
        }
        i += 1;
    }

    let input = input.ok_or("Missing input path")?;
    let output = output.ok_or("Missing output path")?;

    Ok((input, output, opts))
}

#[cfg(any(feature = "export-wav", feature = "export-mp3"))]
fn export_file(
    input_path: &Path,
    output_path: &Path,
    opts: &ExportOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n▶ Processing: {}", input_path.display());
    println!("  Output: {}", output_path.display());

    let start = Instant::now();

    // Load YM file
    let data = std::fs::read(input_path)?;
    let (mut player, info) = load_song(&data)?;

    println!("  Format: {}", info.format);
    println!("  Frames: {}", info.frame_count);
    println!(
        "  Samples: {} ({} per frame)",
        info.total_samples(),
        info.samples_per_frame
    );

    // Configure export
    let mut config = if opts.stereo {
        ExportConfig::stereo()
    } else {
        ExportConfig::default()
    };

    config = config.normalize(opts.normalize);

    if opts.fade_out > 0.0 {
        config = config.fade_out(opts.fade_out);
    }

    // Detect output format and export
    let extension = output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match extension.to_lowercase().as_str() {
        #[cfg(feature = "export-wav")]
        "wav" => {
            export_to_wav_with_config(&mut player, output_path, info, config)?;
        }
        #[cfg(feature = "export-mp3")]
        "mp3" => {
            export_to_mp3_with_config(&mut player, output_path, info, opts.bitrate, config)?;
        }
        _ => {
            return Err(format!("Unsupported output format: {}", extension).into());
        }
    }

    let elapsed = start.elapsed();
    println!("  ✓ Completed in {:.1}s", elapsed.as_secs_f32());

    Ok(())
}

#[cfg(any(feature = "export-wav", feature = "export-mp3"))]
fn batch_export(
    input_dir: &Path,
    output_dir: &Path,
    opts: &ExportOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    if !input_dir.is_dir() {
        return Err(format!("{} is not a directory", input_dir.display()).into());
    }

    // Create output directory
    fs::create_dir_all(output_dir)?;

    // Detect output format
    let output_ext = if cfg!(feature = "export-mp3") {
        "mp3"
    } else {
        "wav"
    };

    println!(
        "Batch export: {} → {}",
        input_dir.display(),
        output_dir.display()
    );
    println!("Output format: {}", output_ext.to_uppercase());
    println!();

    let mut count = 0;
    let mut errors = 0;

    // Find all .ym files
    for entry in fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("ym") {
            let output_name =
                path.file_stem().unwrap().to_string_lossy().to_string() + "." + output_ext;
            let output_path = output_dir.join(output_name);

            match export_file(&path, &output_path, opts) {
                Ok(_) => count += 1,
                Err(e) => {
                    eprintln!("  ✗ Error: {}", e);
                    errors += 1;
                }
            }
        }
    }

    println!();
    println!(
        "Batch export complete: {} successful, {} errors",
        count, errors
    );

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    #[cfg(not(any(feature = "export-wav", feature = "export-mp3")))]
    {
        eprintln!("Error: No export features enabled!");
        eprintln!("Build with --features export-wav or --features export-mp3");
        std::process::exit(1);
    }

    #[cfg(any(feature = "export-wav", feature = "export-mp3"))]
    {
        let (input, output, opts) = parse_args(&args)?;

        if opts.batch {
            batch_export(&input, &output, &opts)?;
        } else {
            export_file(&input, &output, &opts)?;
        }
    }

    Ok(())
}
