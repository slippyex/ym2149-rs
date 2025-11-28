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

use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use ym2149::{AudioDevice, RingBuffer, StreamConfig};
use ym2149_gist_replayer::{GistPlayer, GistSound, TICK_RATE};

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
    println!("▶ Playing {}", sound_label);
    println!(
        "   Duration: {} ticks = {:.2} seconds",
        sound.duration,
        GistPlayer::sound_duration_seconds(&sound)
    );
    println!(
        "   Tone: {} ({})",
        sound.initial_freq,
        if sound.initial_freq >= 0 {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "   Noise: {} ({})",
        sound.initial_noise_freq,
        if sound.initial_noise_freq >= 0 {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "   Volume: {}, vol_phase: {}",
        sound.initial_volume, sound.vol_phase
    );
    println!(
        "   Vol envelope: attack={:08X}, decay={:08X}, sustain={:08X}, release={:08X}",
        sound.vol_attack as u32,
        sound.vol_decay as u32,
        sound.vol_sustain as u32,
        sound.vol_release as u32
    );
    println!(
        "   Vol LFO: limit={:08X}, step={:08X}, delay={}",
        sound.vol_lfo_limit as u32, sound.vol_lfo_step as u32, sound.vol_lfo_delay
    );
    println!("   Freq phase: {}", sound.freq_env_phase);
    println!(
        "   Freq envelope: attack={:08X}, attack_target={:08X}",
        sound.freq_attack as u32, sound.freq_attack_target as u32
    );
    println!(
        "   Freq LFO: limit={:08X}, step={:08X}, delay={}",
        sound.freq_lfo_limit as u32, sound.freq_lfo_step as u32, sound.freq_lfo_delay
    );
    println!();
    println!("Setting up audio device...\n");

    // Setup audio streaming
    let config = StreamConfig::default();
    let buffer = Arc::new(parking_lot::Mutex::new(
        RingBuffer::new(config.ring_buffer_size).unwrap(),
    ));

    let audio_device =
        match AudioDevice::new(config.sample_rate, config.channels, Arc::clone(&buffer)) {
            Ok(device) => device,
            Err(e) => {
                eprintln!("Failed to initialize audio device: {}", e);
                eprintln!("Make sure your audio system is working.");
                return;
            }
        };

    // Create player with matching sample rate
    let mut player = GistPlayer::with_sample_rate(config.sample_rate as u32);

    // Start sound
    if player.play_sound(&sound, None, None).is_none() {
        eprintln!("Failed to start GIST sound");
        return;
    }

    println!("Playing at {}Hz tick rate...\n", TICK_RATE);

    let tail_samples = (config.sample_rate / 2) as usize; // 500ms tail after sound ends
    let mut samples_buffer = vec![0.0f32; 512];

    // Play sound
    while player.is_playing() {
        player.generate_samples_into(&mut samples_buffer);

        // Write to ring buffer, waiting if buffer is full
        loop {
            let written = buffer.lock().write(&samples_buffer);
            if written == samples_buffer.len() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    // Play tail silence so the sound fades out properly
    let mut remaining_tail = tail_samples;
    while remaining_tail > 0 {
        let to_generate = remaining_tail.min(512);
        player.generate_samples_into(&mut samples_buffer[..to_generate]);

        loop {
            let written = buffer.lock().write(&samples_buffer[..to_generate]);
            if written == to_generate {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        remaining_tail -= to_generate;
    }

    // Wait for audio buffer to drain completely
    while !buffer.lock().is_empty() {
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("✓ Done.\n");

    drop(audio_device);
}

fn print_usage() {
    println!("GIST PSG Sound Player");
    println!();
    println!("Usage: cargo run --example player -- <file.snd>");
    println!();
    println!("Arguments:");
    println!("  file.snd  - Path to a GIST .snd sound effect file");
    println!();
    println!("GIST sounds are played at 200Hz (Atari ST Timer C rate).");
    println!();
    println!("Example sound files can be found in: examples/gist/");
}
