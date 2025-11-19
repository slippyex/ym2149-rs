//! Interactive YM2149 Chip Demo with Audio Output
//!
//! Demonstrates direct use of the YM2149 chip emulator with real-time audio playback.
//! Simple demo that plays a 440 Hz tone (A4) for 5 seconds.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example chip_demo -p ym2149 --features streaming
//! ```

use std::sync::Arc;
use std::time::Duration;
use ym2149::{AudioDevice, RingBuffer, StreamConfig, Ym2149};

fn main() {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   YM2149 PSG Chip Demo with Audio Output (A4 Tone)     ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("This demo showcases the YM2149 chip's capabilities:");
    println!("  • Cycle-accurate tone generation");
    println!("  • Real-time audio output");
    println!("  • Playing 440 Hz (A4) for 5 seconds\n");

    println!("Setting up audio device...");

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

    // Create chip and configure for A4 tone (440 Hz)
    let mut chip = Ym2149::new();

    println!("\n▶ Playing A4 tone (440 Hz)...\n");

    // Configure mixer: enable tone A, disable noise
    chip.write_register(0x07, 0x3E);

    // Set frequency for A4 (440 Hz)
    // Period = 2_000_000 / (16 * 440) = 284 = 0x011C
    chip.write_register(0x00, 0x1C); // Low byte
    chip.write_register(0x01, 0x01); // High byte

    // Set volume to maximum
    chip.write_register(0x08, 0x0F);

    // Generate and play samples for 5 seconds
    let total_samples = config.sample_rate * 5; // 5 seconds
    let mut samples_generated = 0;
    let mut samples_buffer = Vec::with_capacity(512);

    println!("Generating {} samples ({} seconds)...", total_samples, 5);

    while samples_generated < total_samples as usize {
        samples_buffer.clear();
        for _ in 0..512.min(total_samples as usize - samples_generated) {
            chip.clock();
            samples_buffer.push(chip.get_sample());
        }

        // Write to ring buffer
        buffer.lock().write(&samples_buffer);
        samples_generated += samples_buffer.len();

        // Small sleep to prevent CPU spinning
        std::thread::sleep(Duration::from_micros(100));
    }

    println!("✓ Playback complete!\n");

    drop(audio_device);

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete!                       ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("This demo showed:");
    println!("  ✓ Direct YM2149 chip emulation");
    println!("  ✓ Real-time audio output");
    println!("  ✓ Tone generation");
    println!("\nFor YM file playback, use:");
    println!("  cargo run -p ym2149-ym-replayer-cli -- path/to/song.ym\n");
}
