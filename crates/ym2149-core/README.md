# ym2149 – Cycle-Accurate YM2149 PSG Emulator

[![Crates.io](https://img.shields.io/crates/v/ym2149.svg)](https://crates.io/crates/ym2149)
[![Docs.rs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Hardware-accurate emulation of the Yamaha YM2149 Programmable Sound Generator (PSG) chip, as used in the Atari ST, Amstrad CPC, and ZX Spectrum 128.

## Overview

This crate provides the core YM2149 chip emulation with cycle-accurate behavior. For YM file parsing and playback, see the companion crates:

- **ym2149** (this crate): Pure chip emulation
- **[ym2149-ym-replayer](../ym2149-ym-replayer)**: YM file parsing and music playback
- **[ym2149-softsynth](../ym2149-softsynth)**: Experimental synthesizer backend
- **[bevy_ym2149](../bevy_ym2149)**: Bevy game engine integration

## Feature Highlights

| Area | Details |
|------|---------|
| **Emulation** | Integer-accurate tone/noise/envelope pipeline, cycle-exact timing |
| **Effects** | SID voice, Sync Buzzer, Mad Max digi-drums, DC filter |
| **Control** | Per-channel mute, color filter, register dump/load |
| **Backend Trait** | `Ym2149Backend` for interchangeable implementations |
| **Audio Output** | Optional real-time streaming via rodio/cpal (feature: `streaming`) |
| **Visualization** | Optional terminal UI helpers (feature: `visualization`) |

## Install

```toml
[dependencies]
ym2149 = { version = "0.6", features = ["emulator"] }
```

For YM file playback, add:

```toml
ym2149-ym-replayer = "0.6"
```

## Quick Start

### Core Emulation Only

```rust
use ym2149::Ym2149;

let mut chip = Ym2149::new();
chip.write_register(0x00, 0xF0); // Channel A period low
chip.write_register(0x01, 0x01); // Channel A period high
chip.write_register(0x08, 0x0F); // Channel A volume
chip.write_register(0x07, 0x3E); // Mixer: enable tone A

chip.clock();
let sample = chip.get_sample();
```

### YM File Playback

For playing YM music files, use the `ym2149-ym-replayer` crate:

```rust
use ym2149_ym_replayer::{load_song, PlaybackController};

let data = std::fs::read("song.ym")?;
let (mut player, summary) = load_song(&data)?;

player.play()?;
let samples = player.generate_samples(summary.samples_per_frame as usize);
```

### Interactive Chip Demo

Try the interactive chip demo with real-time audio output:

```bash
cargo run --example chip_demo -p ym2149 --features streaming
```

The demo showcases 7 different sound demonstrations:
- Simple tone (440 Hz A4)
- Musical scale (C4-C5)
- Three-channel chord (C Major)
- Envelope generators (Attack-Decay, Sawtooth)
- Noise generator
- Tone + Noise (snare-like sound)

Press **SPACE** to advance between demos, **Q** to quit.

### Backend Trait

The `Ym2149Backend` trait allows alternative implementations:

```rust
use ym2149::Ym2149Backend;

fn play_note<B: Ym2149Backend>(chip: &mut B) {
    chip.write_register(0x00, 0xF0);
    chip.write_register(0x08, 0x0F);
    chip.clock();
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `emulator` (default) | Core chip implementation |
| `streaming` | Real-time audio output (rodio) |
| `visualization` | Terminal UI helpers |

## Migration from < 0.6.0

Version 0.6.0 reorganized the crate structure for better separation of concerns.
All YM file parsing and playback functionality has been moved to the `ym2149-ym-replayer` crate:

```rust
// Old (< 0.6)
use ym2149::replayer::Ym6Player;
use ym2149::ym_loader;

// New (>= 0.6)
use ym2149_ym_replayer::Ym6Player;
use ym2149_ym_replayer::loader;
```

The `ym2149` crate now focuses exclusively on chip emulation, streaming, and visualization.

## Architecture

The YM2149 emulator implements:

- **3 tone generators**: 12-bit period counters
- **1 noise generator**: 17-bit LFSR
- **1 envelope generator**: Hardware-accurate ADSR
- **Mixer**: Configurable tone/noise routing
- **Volume control**: 16-level logarithmic + envelope
- **Effects support**: Special registers for Mad Max effects

See [ARCHITECTURE.md](../../ARCHITECTURE.md) for implementation details.

## Performance

- Sample generation: ~0.5–1 ms per 50 Hz frame (44.1 kHz output)
- Memory: ~1 KB per chip instance
- Zero allocations in sample generation hot path

## Documentation

- API reference: [docs.rs/ym2149](https://docs.rs/ym2149)
- Emulator internals: [`ARCHITECTURE.md`](../../ARCHITECTURE.md)
- Streaming guide: [`STREAMING_GUIDE.md`](STREAMING_GUIDE.md)

## Related Crates

- **[ym2149-ym-replayer](../ym2149-ym-replayer)**: YM file parsing and playback
- **[ym2149-softsynth](../ym2149-softsynth)**: Experimental synthesizer backend
- **[bevy_ym2149](../bevy_ym2149)**: Bevy game engine plugin

## Contributing

Run `cargo fmt`, `cargo clippy`, and `cargo test -p ym2149 --all-features` before submitting changes.

## License

MIT License – see [LICENSE](../../LICENSE).
