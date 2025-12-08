# ym2149 – Cycle-Accurate YM2149 PSG Emulator

[![Crates.io](https://img.shields.io/crates/v/ym2149.svg)](https://crates.io/crates/ym2149)
[![Docs.rs](https://docs.rs/ym2149/badge.svg)](https://docs.rs/ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Hardware-accurate emulation of the Yamaha YM2149 Programmable Sound Generator (PSG) chip, as used in the Atari ST, Amstrad CPC, and ZX Spectrum 128. The core runs an internal `clk/8` loop (~250 kHz @ 2 MHz) with hardware envelope/volume tables, DC adjust, and buzzer/digidrum correctness.

## Overview

This crate provides **pure chip emulation only** with cycle-accurate behavior. For file playback and audio output, see the companion crates:

- **ym2149** (this crate): Pure chip emulation
- **[ym2149-replayer-cli](../ym2149-replayer-cli)**: Command-line player with real-time audio
- **[ym2149-ym-replayer](../ym2149-ym-replayer)**: YM file parsing and music playback
- **[ym2149-sndh-replayer](../ym2149-sndh-replayer)**: SNDH (Atari ST) playback with 68000 emulation
- **[ym2149-arkos-replayer](../ym2149-arkos-replayer)**: Arkos Tracker playback
- **[ym2149-ay-replayer](../ym2149-ay-replayer)**: AY file playback with Z80 emulation
- **[ym2149-softsynth](../ym2149-softsynth)**: Experimental synthesizer backend
- **[bevy_ym2149](../bevy_ym2149)**: Bevy game engine integration

## Feature Highlights

| Area | Details |
|------|---------|
| **Emulation** | Integer/lookup pipeline with clk/8 substep, hardware envelope/volume tables |
| **Effects** | SID voice, Sync Buzzer, Mad Max digi-drums, DC filter |
| **Control** | Per-channel mute, color filter, register dump/load |
| **Backend Trait** | `Ym2149Backend` for interchangeable implementations |
| **Utilities** | Register math helpers in `ym2149-common` crate |

## Install

```toml
[dependencies]
ym2149 = "0.7"
```

For YM file playback with real-time audio, add the CLI:

```toml
ym2149-replayer-cli = "0.7"
```

## Quick Start

### Core Emulation Only

```rust
use ym2149::{Ym2149, Ym2149Backend};

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

### Real-Time Audio Playback

For real-time audio output, use the CLI:

```bash
cargo run -p ym2149-replayer-cli -- path/to/song.ym
```

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

## Modules

| Module | Description |
|--------|-------------|
| `ym2149` | Core chip implementation |
| `backend` | `Ym2149Backend` trait for alternative implementations |

> **Note:** Utility types like `ChannelStates` and register math helpers (`channel_period`, `period_to_frequency`) are in the `ym2149-common` crate.

## Migration from < 0.7

Version 0.7 reorganized the crate structure for better separation of concerns:

- **Streaming audio** moved to `ym2149-replayer-cli`
- **Visualization** moved to `ym2149-replayer-cli`
- YM file parsing in `ym2149-ym-replayer` (since 0.6)

The `ym2149` crate now focuses exclusively on pure chip emulation.

```rust
// Old (< 0.7)
use ym2149::streaming::AudioDevice;
use ym2149::visualization::create_volume_bar;

// New (>= 0.7)
// Use ym2149-replayer-cli for audio output and visualization
```

## Architecture

The YM2149 emulator implements:

- **3 tone generators**: 12-bit period counters
- **1 noise generator**: 17-bit LFSR
- **1 envelope generator**: Hardware-accurate shapes (10 patterns)
- **Mixer**: Configurable tone/noise routing
- **Volume control**: 32-step logarithmic + envelope
- **Effects support**: DigiDrum, SID voice, Sync Buzzer

See [ARCHITECTURE.md](ARCHITECTURE.md) for implementation details.

## Performance

- Sample generation: ~0.5–1 ms per 50 Hz frame (44.1 kHz output)
- Memory: ~1 KB per chip instance
- Zero allocations in sample generation hot path

## Documentation

- API reference: [docs.rs/ym2149](https://docs.rs/ym2149)
- Emulator internals: [`ARCHITECTURE.md`](ARCHITECTURE.md)

## Related Crates

- **[ym2149-replayer-cli](../ym2149-replayer-cli)**: Command-line player with streaming audio
- **[ym2149-ym-replayer](../ym2149-ym-replayer)**: YM file parsing and playback
- **[ym2149-sndh-replayer](../ym2149-sndh-replayer)**: SNDH (Atari ST) playback with 68000 emulation
- **[ym2149-arkos-replayer](../ym2149-arkos-replayer)**: Arkos Tracker playback
- **[ym2149-ay-replayer](../ym2149-ay-replayer)**: AY file playback with Z80 emulation
- **[ym2149-softsynth](../ym2149-softsynth)**: Experimental synthesizer backend
- **[bevy_ym2149](../bevy_ym2149)**: Bevy game engine plugin

## Contributing

Run `cargo fmt`, `cargo clippy`, and `cargo test -p ym2149` before submitting changes.

## License

MIT License – see [LICENSE](../../LICENSE).
