# YM2149-RS Monorepo

A comprehensive Rust ecosystem for Yamaha YM2149 PSG (Atari ST) emulation, playback, and integration.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Overview

This monorepo contains a complete suite of tools and libraries for working with the YM2149 Programmable Sound Generator chip, as used in the Atari ST, Amstrad CPC, and other classic computers. From cycle-accurate emulation to real-time audio streaming, game engine integration, and audio export, ym2149-rs provides everything needed to bring YM chiptunes to modern platforms.

## 🎯 What is this?

The **YM2149** is a legendary sound chip from the 1980s, responsible for the distinctive audio of Atari ST demo scene classics, game soundtracks, and computer music. This project brings authentic YM2149 sound to modern Rust applications with:

- **Cycle-accurate emulation** - Hardware-faithful PSG core
- **Complete format support** - YM2-YM6 file formats with all effects
- **Modern integrations** - Bevy game engine plugin, CLI tools, audio export
- **Production-ready** - Clean API, modular architecture, comprehensive tests

## 📦 Crates

### Core Libraries

#### [`ym2149-core`](crates/ym2149-core) (Published as `ym2149`)
[![Crates.io](https://img.shields.io/crates/v/ym2149.svg)](https://crates.io/crates/ym2149)
[![Docs.rs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149)

The foundation: cycle-accurate YM2149 emulator with YM file support, audio streaming, and export.

**Features:**
- Integer-accurate PSG core (envelope, LFSR noise, mixer)
- YM2-YM6 format parsing (including LHA decompression)
- Real-time audio streaming (rodio)
- WAV/MP3 export
- Experimental softsynth
- Modular feature flags

**Use cases:**
- Standalone YM playback
- Audio processing pipelines
- Custom music players
- Audio archiving/conversion

[→ Full Documentation](crates/ym2149-core/README.md)

### Bevy Integration

#### [`bevy_ym2149`](crates/bevy_ym2149)

Production-ready Bevy plugin for YM2149 audio playback and visualization.

**Features:**
- ECS-friendly playback API
- Playlist support
- Live channel visualization
- Spatial audio
- Music state machine
- Shader uniforms (oscilloscope/spectrum data)
- Audio bridge for Bevy's audio graph

**Use cases:**
- Retro-style game soundtracks
- Demoscene-inspired games
- Chiptune visualizers
- Interactive music applications

[→ Documentation](crates/bevy_ym2149/README.md)

#### [`bevy_ym2149_viz`](crates/bevy_ym2149_viz)

Visualization components for `bevy_ym2149` (oscilloscope, spectrum analyzer, channel displays).

**Features:**
- Ready-made UI components
- Customizable layouts
- Real-time waveform rendering
- Spectrum analysis

[→ Documentation](crates/bevy_ym2149_viz/README.md)

#### [`bevy_ym2149_examples`](crates/bevy_ym2149_examples)

Comprehensive examples showcasing the Bevy plugin ecosystem.

**Examples:**
- `basic_example` - Minimal playback setup
- `advanced_example` - Full-featured tracker UI
- `feature_showcase` - All plugin capabilities
- `demoscene` - Demo-style visuals and effects

### Legacy/Experimental

#### [`ym2149-bevy`](crates/ym2149-bevy)

Alternative Bevy integration (legacy). Consider using `bevy_ym2149` for new projects.

## 🚀 Quick Start

### As a Library (Standalone)

```toml
[dependencies]
ym2149 = { version = "0.6", features = ["replayer", "streaming"] }
```

```rust
use ym2149::replayer::load_song;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("song.ym")?;
    let (mut player, info) = load_song(&data)?;

    player.play()?;
    let samples = player.generate_samples(44_100);

    println!("Generated {} samples", samples.len());
    Ok(())
}
```

### With Bevy Game Engine

```toml
[dependencies]
bevy = "0.17"
bevy_ym2149 = "0.6"
```

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Plugin, Ym2149Playback};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Ym2149Plugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());
    commands.spawn(Ym2149Playback::new("assets/song.ym"));
}
```

### CLI Tools

```bash
# Play YM file with real-time audio
cd crates/ym2149-core
cargo run --features streaming -- song.ym

# Export to WAV
cargo run --example export --features export-wav -- song.ym output.wav --stereo --fade 2.0

# Export to MP3
cargo run --example export --features export-mp3 -- song.ym output.mp3 --bitrate 320

# Batch convert directory
cargo run --example export --features export-wav -- --batch input_dir/ output_dir/

# Run Bevy examples
cd crates/bevy_ym2149_examples

cargo run --example basic_example
cargo run --example advanced_example
cargo run --example crossfade_example
cargo run --example feature_showcase
cargo run --example demoscene
```

## 🎮 Use Cases

### Game Development
- Retro game soundtracks (Bevy plugin)
- Demoscene-style games
- Chiptune rhythm games
- Interactive music visualizers

### Audio Production
- YM file archiving/conversion
- Sample extraction for remixes
- Batch processing of collections
- High-quality WAV/MP3 export

### Education & Research
- PSG programming tutorials
- Audio DSP demonstrations
- Retro computing education
- Music technology history

### Emulation & Preservation
- Atari ST software emulation
- Demoscene archive playback
- Historical accuracy research
- Format documentation

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Applications & Games                     │
├─────────────────────────────────────────────────────────────┤
│  bevy_ym2149_examples  │  CLI Tools  │  Custom Apps         │
├────────────────────────┴─────────────┴──────────────────────┤
│        Bevy Plugin Layer (bevy_ym2149 + bevy_ym2149_viz)    │
├─────────────────────────────────────────────────────────────┤
│              ym2149-core (Emulator + I/O + Export)          │
├─────────────────────────────────────────────────────────────┤
│  PSG Core  │  YM Parser  │  Replayer  │  Export  │  Stream  │
└─────────────────────────────────────────────────────────────┘
```

## 🎨 Features

### Emulation Quality
- ✅ Cycle-accurate tone generators
- ✅ Hardware-faithful noise LFSR
- ✅ All 16 envelope shapes
- ✅ Non-linear volume table
- ✅ DC offset compensation
- ✅ VBL/MFP timer sync

### Format Support
| Format | Support | Features |
|--------|---------|----------|
| YM2 | ✅ | Mad Max digi-drums |
| YM3/YM3b | ✅ | Basic + loop support |
| YM4 | ✅ | Metadata + digi-drums |
| YM5 | ✅ | SID, Sync Buzzer, DigiDrums |
| YM6 | ✅ | Extended effects |
| YMT1/YMT2 | ✅ | Tracker formats |

### Export Formats
- **WAV** - Uncompressed PCM (mono/stereo, 16-bit)
- **MP3** - LAME-encoded (128-320 kbps)
- **Configurable** - Sample rate, normalization, fade out

### Bevy Plugin Features
- Real-time playback control
- Playlist management
- Channel visualization
- Spatial audio positioning
- Music state machine
- Diagnostics integration
- Audio bridge for DSP

## 📊 Performance

- **Emulation:** ~0.5-1ms per 50Hz frame
- **Memory:** ~1-2MB per player instance
- **Export:** ~4-5s to render 8-minute song
- **Latency:** < 5ms in streaming mode
- **Bevy:** 60 FPS with visualization at 1080p

## 🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p ym2149
cargo test -p bevy_ym2149

# With features
cargo test -p ym2149 --features export-wav

# Test count: 152 tests (as of v0.6)
```

## 📚 Documentation

- **Core Library:** [docs.rs/ym2149](https://docs.rs/ym2149)
- **Bevy Plugin:** [crates/bevy_ym2149/README.md](crates/bevy_ym2149/README.md)
- **Examples:** Each crate has `examples/` directory with runnable code

### Key Resources
- [YM Format Specification](http://leonard.oxg.free.fr/ymformat.html)
- [YM2149 Datasheet](http://www.ym2149.com/)

## 🛠️ Development

### Prerequisites
- Rust 2024 edition (1.83+)
- For MP3 export: LAME library
- For examples: SDL2 (via rodio)

### Building

```bash
# Clone repository
git clone https://github.com/slippyex/ym2149-rs.git
cd ym2149-rs

# Build all crates
cargo build --workspace

# Build with all features
cargo build --workspace --all-features

# Run tests
cargo test --workspace

# Generate documentation
cargo doc --workspace --open
```

### Project Structure

```
ym2149-rs/
├── crates/
│   ├── ym2149-core/          # Core emulator (published as 'ym2149')
│   ├── bevy_ym2149/          # Bevy audio plugin
│   ├── bevy_ym2149_viz/      # Bevy visualization components
│   ├── bevy_ym2149_examples/ # Comprehensive examples
│   └── ym2149-bevy/          # Legacy Bevy integration
├── examples/                  # Sample YM files
├── Cargo.toml                 # Workspace configuration
└── README.md                  # This file
```

## 🤝 Contributing

Contributions are welcome! Areas of interest:

Please ensure:
- Tests pass (`cargo test --workspace`)
- Code is formatted (`cargo fmt`)
- Documentation is updated
- Examples demonstrate new features

## 📜 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Credits

- **Leonard/Oxygene** - YM format specification and ST-Sound
- **Atari ST Community** - Hardware documentation and YM archives
- **Rust Audio Community** - rodio, cpal, and audio infrastructure
- **Bevy Community** - Game engine framework and ECS patterns

## 📬 Links

- **Crate:** [crates.io/crates/ym2149](https://crates.io/crates/ym2149)
- **Documentation:** [docs.rs/ym2149](https://docs.rs/ym2149)
- **Repository:** [github.com/slippyex/ym2149-rs](https://github.com/slippyex/ym2149-rs)
- **Issues:** [github.com/slippyex/ym2149-rs/issues](https://github.com/slippyex/ym2149-rs/issues)
