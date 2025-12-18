//! GIST PSG Sound Player
//!
//! Plays GIST sound effects using the YM2149 chip emulator with real-time audio.
//! GIST (Graphics, Images, Sound, Text) was a popular Atari ST graphics/sound tool.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example player -- <file.snd>
//! ```

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use ym2149_gist_replayer::{GistPlayer, GistSound};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_usage();
        return;
    }

    // Load .snd file from disk
    let path = Path::new(&args[1]);
    let (sound, sound_label): (GistSound, String) = match GistSound::load(path) {
        Ok(snd) => (
            snd,
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ),
        Err(e) => {
            eprintln!("Failed to load {}: {}", args[1], e);
            return;
        }
    };

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║            GIST PSG Sound Player (YM2149)                ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");
    println!("Playing: {sound_label}");
    println!(
        "Duration: {} ticks = {:.2} seconds",
        sound.duration,
        GistPlayer::sound_duration_seconds(&sound)
    );
    println!();

    // Setup audio with cpal
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            eprintln!("No audio output device available");
            return;
        }
    };

    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0;

    // Create player
    let player = Arc::new(Mutex::new(GistPlayer::with_sample_rate(sample_rate)));
    let player_clone = Arc::clone(&player);
    let finished = Arc::new(AtomicBool::new(false));
    let finished_clone = Arc::clone(&finished);

    // Start sound
    {
        let mut p = player.lock();
        if p.play_sound(&sound, None, None).is_none() {
            eprintln!("Failed to start GIST sound");
            return;
        }
    }

    // Build audio stream
    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut p = player_clone.lock();
                if p.is_playing() {
                    p.generate_samples_into(data);
                } else {
                    data.fill(0.0);
                    finished_clone.store(true, Ordering::Relaxed);
                }
            },
            |err| eprintln!("Audio error: {err}"),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    // Wait for playback to finish
    while !finished.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }

    // Short tail for fade-out
    std::thread::sleep(Duration::from_millis(100));

    println!("Done.\n");
}

fn print_usage() {
    println!("GIST PSG Sound Player");
    println!();
    println!("Usage: cargo run --example player -- <file.snd>");
    println!();
    println!("Arguments:");
    println!("  file.snd  - Path to a GIST .snd sound effect file");
    println!();
    println!("Example sound files can be found in: examples/gist/");
}
