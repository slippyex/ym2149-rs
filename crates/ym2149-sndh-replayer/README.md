# ym2149-sndh-replayer

[![Crates.io](https://img.shields.io/crates/v/ym2149-sndh-replayer.svg)](https://crates.io/crates/ym2149-sndh-replayer)
[![Docs.rs](https://docs.rs/ym2149-sndh-replayer/badge.svg)](https://docs.rs/ym2149-sndh-replayer)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

SNDH file parser and Atari ST machine emulation for YM2149 chiptune playback.

## Overview

This crate provides playback support for SNDH files, a popular format for Atari ST chiptune music. SNDH files contain native Motorola 68000 machine code that must be executed on an emulated Atari ST to produce audio.

### Features

- **ICE! 2.4 Decompression**: Many SNDH files are compressed with ICE! packer
- **SNDH Header Parsing**: Extract metadata (title, author, year, subsong info)
- **68000 CPU Emulation**: Via the `m68000` crate
- **MFP 68901 Timer Emulation**: For SID voice and timer-based effects
- **STE DAC Emulation**: DMA audio support for STe-specific SNDH files (50kHz mode with averaging)
- **YM2149 Sound Chip**: Using `ym2149` crate for cycle-accurate emulation
- **ChiptunePlayer Trait**: Unified interface compatible with other replayers

## Install

```toml
[dependencies]
ym2149-sndh-replayer = "0.7"
```

## Usage

```rust
use ym2149_sndh_replayer::{SndhPlayer, load_sndh, PlaybackMetadata, ChiptunePlayer};

// Load SNDH file
let data = std::fs::read("music.sndh")?;
let mut player = load_sndh(&data, 44100)?;

println!("Title: {}", player.metadata().title());
println!("Author: {}", player.metadata().author());
println!("Subsongs: {}", player.subsong_count());

// Initialize first subsong
player.init_subsong(1)?;
player.play();

// Generate audio samples
let mut buffer = vec![0.0f32; 882]; // ~20ms at 44100Hz
player.generate_samples_into(&mut buffer);
```

### Rendering to i16

For direct audio output, use `render_i16`:

```rust
let mut buffer = vec![0i16; 882];
let loop_count = player.render_i16(&mut buffer);
```

## SNDH Format

SNDH is a standard format for Atari ST music that embeds original 68000 replay code:

| Offset | Description |
|--------|-------------|
| +0 | BRA instruction (jump over header) |
| +12 | "SNDH" magic |
| +16 | Tag-based metadata (TITL, COMM, YEAR, ##, etc.) |
| Entry+0 | Init routine (D0 = subsong number) |
| Entry+4 | Exit/cleanup routine |
| Entry+8 | Play routine (called at player rate) |

### Supported Tags

- `TITL` - Song title
- `COMM` - Composer/author
- `YEAR` - Year of creation
- `##nn` - Number of subsongs
- `!#nn` - Default subsong
- `TA/TB/TC/TD` - Timer and replay rate
- `TIME` - Duration per subsong (in seconds)
- `HDNS` - End of header marker

## Architecture

```
┌─────────────────────────────────────────┐
│           SndhPlayer                    │
├─────────────────────────────────────────┤
│ ┌─────────────────────────────────────┐ │
│ │        AtariMachine                 │ │
│ │ ┌───────────┐ ┌─────────────────┐   │ │
│ │ │  M68000   │ │  AtariMemory    │   │ │
│ │ │   CPU     │ │ ┌─────────────┐ │   │ │
│ │ │ (m68000)  │ │ │   YM2149    │ │   │ │
│ │ └───────────┘ │ │  (ym2149)   │ │   │ │
│ │               │ ├─────────────┤ │   │ │
│ │               │ │  MFP68901   │ │   │ │
│ │               │ │  (timers)   │ │   │ │
│ │               │ ├─────────────┤ │   │ │
│ │               │ │  4MB RAM    │ │   │ │
│ │               │ └─────────────┘ │   │ │
│ │               └─────────────────┘   │ │
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

## Emulation Details

The Atari ST machine emulation provides:

- **4MB RAM** at 0x000000 - 0x3FFFFF
- **YM2149 PSG** at 0xFF8800 - 0xFF88FF
- **MFP 68901** at 0xFFFA00 - 0xFFFA25
- **STE DAC** at 0xFF8900 - 0xFF893F (DMA audio with microwire volume control)
- **Basic GEMDOS/XBIOS traps** for malloc and timer setup

Timer interrupts (used by SID voice effects) are handled by executing the interrupt handler code at audio sample rate.

## Related Crates

- **[ym2149](../ym2149-core)** - Core YM2149 chip emulation
- **[ym2149-common](../ym2149-common)** - Common traits
- **[ym2149-ym-replayer](../ym2149-ym-replayer)** - YM file playback
- **[ym2149-arkos-replayer](../ym2149-arkos-replayer)** - Arkos Tracker playback
- **[ym2149-ay-replayer](../ym2149-ay-replayer)** - AY file playback

## Credits

Based on the [sndh-player](https://github.com/arnaud-carre/sndh-player) C++ implementation by Arnaud Carré (Leonard/Oxygene).

ICE! 2.4 depacker based on the public domain C implementation by Hans Wessels.

68000 emulation via the [m68000](https://crates.io/crates/m68000) crate by Stovent.

## License

MIT License - see [LICENSE](../../LICENSE).
