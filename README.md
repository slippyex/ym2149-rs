# YM2149-RS Monorepo

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> Cycle-accurate Yamaha YM2149 tooling for Rust: standalone emulator, CLI player/exporter, and Bevy integration with ready-made visualizers.

## At a Glance

- 🧠 **Core emulator**: integer-accurate PSG with YM2–YM6 + YMT tracker support
- 🪕 **Audio workflows**: real-time streaming, WAV/MP3 export, playlist & music-state automation
- 🕹️ **Game-ready**: Bevy plugins with spatial audio, diagnostics, visual components, and full example scenes
- 📦 **Monorepo cohesion**: shared workspace versioning, consistent docs, cross-crate testing (`cargo test --workspace`)

## Workspace Packages

| Crate | Purpose | Crates.io | Docs |
|-------|---------|-----------|------|
| [`ym2149`](crates/ym2149-core) | Core YM2149 chip emulator (cycle-accurate) | [crates.io/crates/ym2149](https://crates.io/crates/ym2149) | [docs.rs/ym2149](https://docs.rs/ym2149) |
| [`ym-replayer`](crates/ym-replayer) | YM file parsing and music playback (YM2–YM6, tracker support) | [crates.io/crates/ym-replayer](https://crates.io/crates/ym-replayer) | [docs.rs/ym-replayer](https://docs.rs/ym-replayer) |
| [`ym-softsynth`](crates/ym-softsynth) | Experimental software synthesizer backend | [crates.io/crates/ym-softsynth](https://crates.io/crates/ym-softsynth) | [docs.rs/ym-softsynth](https://docs.rs/ym-softsynth) |
| [`bevy_ym2149`](crates/bevy_ym2149) | Bevy audio plugin (playback, playlists, diagnostics, audio bridge) | [crates.io/crates/bevy_ym2149](https://crates.io/crates/bevy_ym2149) | [docs.rs/bevy_ym2149](https://docs.rs/bevy_ym2149) |
| [`bevy_ym2149_viz`](crates/bevy_ym2149_viz) | Optional visualization systems & UI builders | [crates.io/crates/bevy_ym2149_viz](https://crates.io/crates/bevy_ym2149_viz) | [docs.rs/bevy_ym2149_viz](https://docs.rs/bevy_ym2149_viz) |
| [`bevy_ym2149_examples`](crates/bevy_ym2149_examples) | Runnable Bevy demos (basic, advanced, crossfade, feature showcase, demoscene) | Workspace-only | [crates/bevy_ym2149_examples/README.md](crates/bevy_ym2149_examples/README.md) |
| [`ym2149-bevy`](crates/ym2149-bevy) | Legacy re-export (shim to `bevy_ym2149`) | [crates.io/crates/ym2149-bevy](https://crates.io/crates/ym2149-bevy) | – |

<img src="docs/screenshots/advanced_example.png" alt="Advanced Bevy example" width="780">

## Highlights

- ✅ **Hardware-faithful**: precise envelope, noise, mixer, SID, Sync Buzzer, digi-drum behaviours
- 🧰 **CLI ready**: stream YM files in the terminal with real-time visualization
- 🎵 **Native Bevy audio**: seamless integration via `Decodable` trait with pull-based sample generation
- 🛰️ **Configurable Bevy subsystems**: playlists, crossfade decks, music state graphs, channel events, diagnostics, audio bridge
- 🖼️ **Visualization stack**: drop-in oscilloscope, spectrum bars, progress HUD, and demoscene showcase based on the viz crate
- 🧪 **Well-tested**: `cargo test --workspace` (165+ tests) plus example scenes to validate runtime flows

## Quick Start

### Use the Core Library

```toml
[dependencies]
ym2149 = { version = "0.6", features = ["emulator", "streaming"] }
ym-replayer = "0.6"
```

```rust
use ym_replayer::{load_song, PlaybackController};

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("song.ym")?;
    let (mut player, summary) = load_song(&data)?;

    player.play()?;
    let samples = player.generate_samples(summary.samples_per_frame as usize);
    println!("{} frames • {} samples", summary.frame_count, samples.len());
    Ok(())
}
```

### Run the CLI Player

```bash
# Real-time playback with scope overlay
cargo run -p ym-replayer --features streaming -- examples/ND-Toxygene.ym

# Interactive chip demo with audio output
cargo run --example chip_demo -p ym2149 --features streaming
```

<img src="docs/screenshots/cli.png" alt="CLI player" width="700">

### Add the Bevy Plugin

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_viz::Ym2149VizPlugin;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Ym2149Plugin::default(), Ym2149VizPlugin::default()))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
            commands.spawn(Ym2149Playback::new("assets/music/song.ym")).insert(Name::new("Tracker"));
        })
        .run();
}
```

Need a reference scene? `cargo run --example advanced_example -p bevy_ym2149_examples`.

## Documentation & Guides

- `crates/ym2149-core/README.md` – emulator architecture, feature flags, CLI/export instructions
- `crates/bevy_ym2149/README.md` – plugin subsystems, playlists, music state graph, audio bridge, diagnostics
- `crates/bevy_ym2149_viz/README.md` – visualization builders and systems
- `crates/bevy_ym2149_examples/README.md` – example matrix + screenshot gallery
- [ARCHITECTURE.md](ARCHITECTURE.md) – deeper dive into the emulator internals
- [STREAMING_GUIDE.md](STREAMING_GUIDE.md) – low-latency streaming details

## Testing

```bash
# Entire workspace
cargo test --workspace

# Focus a crate
cargo test -p ym2149
cargo test -p bevy_ym2149

# Feature-specific tests
cargo test -p ym2149 --features export-wav
```

## Development Prerequisites

- Rust 1.83+ (Rust 2024 edition)
- libmp3lame for `export-mp3`
- A working audio backend (cpal/rodio) for streaming playback

## Project Structure

```
ym2149-rs/
├── crates/
│   ├── ym2149-core/          # Core YM2149 chip emulator (published as `ym2149`)
│   ├── ym-replayer/          # YM file parsing and playback
│   ├── ym-softsynth/         # Experimental synthesizer backend
│   ├── bevy_ym2149/          # Bevy audio plugin
│   ├── bevy_ym2149_viz/      # Visualization helpers
│   ├── bevy_ym2149_examples/ # Runnable Bevy demos
│   └── ym2149-bevy/          # Legacy shim
├── examples/                 # YM sample files
├── docs/                     # Guides + screenshots
├── Cargo.toml                # Workspace configuration
└── README.md                 # You are here
```

## Contributing

Contributions are welcome! Please ensure:
- `cargo fmt` + `cargo clippy`
- `cargo test --workspace`
- Documentation and examples updated for new features

## License

MIT License – see [LICENSE](LICENSE).

## Credits

- **Leonard/Oxygene** – YM format specification & ST-Sound reference material
- **Atari ST + demoscene community** – for the original tunes and docs
- **Rust audio and Bevy ecosystems** – rodio/cpal, Bevy ECS, and community inspiration
