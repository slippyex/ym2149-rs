# ym2149 - YM2149 PSG Emulator Core

Cycle-accurate Yamaha YM2149 PSG (Atari ST) emulator in Rust with comprehensive YM file format support, real-time streaming, audio export, and an experimental softsynth.

[![Crates.io](https://img.shields.io/crates/v/ym2149.svg)](https://crates.io/crates/ym2149)
[![Docs.rs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

## Features

### Emulation
- **Integer-accurate YM2149 core** - Cycle-accurate envelope generator, LFSR noise, mixer, and color filter
- **Complete PSG emulation** - All 3 channels, noise generator, envelope shapes
- **Hardware-accurate timing** - VBL synchronization, MFP timer integration
- **DC offset compensation** - Moving-average filter for clean output

### File Format Support
- **YM2-YM6 formats** - Full support for all YM format variants
- **Mad Max digi-drums** (YM2) - Special effects and sample playback
- **YM5/YM6 effects** - SID voices, Sync Buzzer, DigiDrums
- **LHA compression** - Automatic decompression of compressed YM files
- **Metadata extraction** - Song title, author, comments

### Audio Output
- **Real-time streaming** - Low-latency rodio-based audio output
- **Buffer-based rendering** - Generate samples for offline processing
- **WAV export** - High-quality uncompressed PCM export
- **MP3 export** - LAME-encoded compressed audio export
- **Configurable sample rates** - 44.1kHz, 48kHz, or custom

### Advanced Features
- **Experimental softsynth** - Alternative synthesis engine with biquad filters
- **Terminal visualization** - Real-time waveform and spectrum display
- **Modular architecture** - Enable only the features you need via feature flags

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ym2149 = { version = "0.6", features = ["emulator", "ym-format", "replayer"] }

# Optional features:
# - "streaming": Real-time audio output (rodio)
# - "export-wav": WAV file export
# - "export-mp3": MP3 file export (requires lame)
# - "softsynth": Experimental synthesizer
# - "visualization": Terminal UI helpers
```

## Quick Start

### Basic Playback

```rust
use ym2149::replayer::load_song;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load and play YM file
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    println!("Format: {}, Frames: {}", info.format, info.frame_count);

    player.play()?;
    let samples = player.generate_samples(44_100); // 1 second at 44.1kHz

    Ok(())
}
```

### Real-Time Streaming

```rust
use ym2149::replayer::load_song;
use ym2149::streaming::{StreamConfig, RealtimePlayer, AudioDevice};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    player.play()?;

    // Set up audio streaming
    let cfg = StreamConfig::low_latency(44_100);
    let stream = RealtimePlayer::new(cfg)?;
    let _dev = AudioDevice::new(cfg.sample_rate, cfg.channels, stream.get_buffer())?;

    // Stream audio
    let total_samples = info.total_samples();
    let samples = player.generate_samples(total_samples);
    stream.write_blocking(&samples);

    Ok(())
}
```

### Export to WAV

```rust
use ym2149::replayer::load_song;
use ym2149::export::{export_to_wav_with_config, ExportConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    // Configure export
    let config = ExportConfig::stereo()
        .normalize(true)
        .fade_out(2.0); // 2 second fade

    export_to_wav_with_config(&mut player, "output.wav", info, config)?;

    Ok(())
}
```

### Export to MP3

```rust
use ym2149::replayer::load_song;
use ym2149::export::{export_to_mp3_with_config, ExportConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    let config = ExportConfig::default().normalize(true);

    export_to_mp3_with_config(&mut player, "output.mp3", info, 320, config)?; // 320 kbps

    Ok(())
}
```

### Low-Level Chip Emulation

```rust
use ym2149::ym2149::Ym2149;

fn main() {
    let mut chip = Ym2149::new();

    // Set up channel A: 440Hz tone (A4)
    let period = 2_000_000 / (16 * 440); // ~284
    chip.write_register(0, (period & 0xFF) as u8);       // Period low
    chip.write_register(1, ((period >> 8) & 0x0F) as u8); // Period high
    chip.write_register(7, 0b11111110);                   // Enable tone A
    chip.write_register(8, 0x0F);                         // Max volume

    // Generate samples
    for _ in 0..44100 {
        chip.clock();
        let sample = chip.get_sample(); // -1.0 to 1.0
    }
}
```

## CLI Tools

### Audio Player

Real-time YM file playback with terminal visualization:

```bash
# Basic playback
cargo run --features streaming -- song.ym

# With softsynth
cargo run --features "streaming softsynth" -- --chip softsynth song.ym
```

### Audio Export Tool

Convert YM files to WAV or MP3:

```bash
# Export to WAV (mono)
cargo run --example export --features export-wav -- song.ym output.wav

# Export to WAV (stereo with 2s fade out)
cargo run --example export --features export-wav -- song.ym output.wav --stereo --fade 2.0

# Export to MP3 (320 kbps)
cargo run --example export --features export-mp3 -- song.ym output.mp3 --bitrate 320

# Batch export all YM files in a directory
cargo run --example export --features export-wav -- --batch input_dir/ output_dir/
```

**Export Options:**
- `--stereo` - Export as stereo (duplicates mono channel)
- `--no-normalize` - Disable audio normalization
- `--fade <seconds>` - Add fade out (e.g., `--fade 2.0`)
- `--bitrate <kbps>` - MP3 bitrate: 128, 192, 256, 320 (default: 192)
- `--batch` - Batch convert directory

## Feature Flags

Enable only what you need to minimize dependencies:

```toml
[dependencies.ym2149]
version = "0.6"
default-features = false
features = [
    "emulator",      # Core YM2149 chip emulation
    "ym-format",     # YM file parsing (YM2-YM6)
    "replayer",      # Playback engine with effects
    "streaming",     # Real-time audio output (rodio)
    "export-wav",    # WAV file export
    "export-mp3",    # MP3 file export (LAME)
    "softsynth",     # Experimental softsynth
    "visualization", # Terminal UI helpers
]
```

**Feature Dependencies:**
- `replayer` requires `ym-format`
- `export-wav` and `export-mp3` work independently
- `export = ["export-wav", "export-mp3"]` enables both

## YM Format Support

| Format | Support | Features |
|--------|---------|----------|
| YM2 | ✅ Full | Mad Max digi-drums, special effects |
| YM3 | ✅ Full | Basic register dump |
| YM3b | ✅ Full | YM3 with loop support |
| YM4 | ✅ Full | Metadata, digi-drums |
| YM5 | ✅ Full | SID voices, Sync Buzzer, DigiDrums |
| YM6 | ✅ Full | Extended effects, improved compression |
| YMT1/YMT2 | ✅ Full | Tracker formats |

**All formats support:**
- LHA/LZH compressed files (automatic decompression)
- Interleaved and non-interleaved register data
- Metadata extraction (title, author, comment)
- Loop points

## Export Configuration

### ExportConfig Options

```rust
use ym2149::export::ExportConfig;

let config = ExportConfig {
    sample_rate: 44_100,      // Sample rate (Hz)
    channels: 2,              // 1 = mono, 2 = stereo
    normalize: true,          // Prevent clipping
    fade_out_duration: 2.0,   // Fade out (seconds)
};

// Builder pattern
let config = ExportConfig::stereo()
    .normalize(true)
    .fade_out(2.0);
```

### WAV Export

- **Format:** RIFF WAVE, Microsoft PCM
- **Bit depth:** 16-bit signed
- **Channels:** Mono or stereo
- **Sample rate:** Configurable (default 44.1kHz)

### MP3 Export

- **Encoder:** LAME (best quality settings)
- **Bitrate:** 128-320 kbps (configurable)
- **Channels:** Mono or stereo
- **Sample rate:** Matches source (usually 44.1kHz)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                    │
├─────────────────────────────────────────────────────────┤
│  Replayer API    │  Export API  │  Streaming API        │
├──────────────────┼──────────────┼───────────────────────┤
│  Effects Manager │ YM Parser    │  Audio Device         │
├──────────────────┴──────────────┴───────────────────────┤
│              YM2149 Core Emulator (cycle-accurate)      │
├─────────────────────────────────────────────────────────┤
│  Envelope  │  Noise Gen  │  Mixer  │  DC Filter         │
└─────────────────────────────────────────────────────────┘
```

## Performance

- **Sample generation:** ~0.5-1ms per 50Hz frame (44.1kHz output)
- **Memory usage:** ~1-2MB per player instance
- **Export speed:** ~4-5s to render 8 minute song (WAV/MP3)
- **Real-time streaming:** < 5ms latency with low-latency config

## Examples

The crate includes several examples:

```bash
# List all examples
cargo run --example

# Audio export tool
cargo run --example export --features export-wav -- song.ym output.wav

# See crates/ym2149-core/examples/ for more
```

## API Documentation

Full API documentation is available at [docs.rs/ym2149](https://docs.rs/ym2149).

Key modules:
- `ym2149` - Core PSG emulator
- `replayer` - Playback engine and effects
- `ym_parser` - YM format parsing
- `ym_loader` - High-level file loading
- `export` - Audio export (WAV/MP3)
- `streaming` - Real-time audio output
- `softsynth` - Experimental synthesizer

## Accuracy

This emulator aims for **cycle-accurate** emulation of the YM2149:

✅ **Accurate:**
- Tone generator periods and frequencies
- Noise generator LFSR (17-bit polynomial)
- Envelope generator shapes and timing
- Mixer logic (tone/noise enable)
- Volume table (non-linear amplitude curve)
- DC offset compensation

⚠️ **Approximations:**
- Output filter characteristics
- Analog component tolerances
- Temperature-dependent behavior

## Contributing

Contributions are welcome! Areas for improvement:

- Additional YM format variants
- Performance optimizations (SIMD, etc.)
- More export formats (FLAC, OGG, etc.)
- Enhanced softsynth features
- Better filter accuracy

## License

MIT License - see [LICENSE](../../LICENSE) for details.


Part of the [ym2149-rs monorepo](../../README.md).
