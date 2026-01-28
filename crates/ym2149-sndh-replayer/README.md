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
- **68000 CPU Emulation**: Via the `r68k` crate with cycle-accurate timing
- **MFP 68901 Timer Emulation**: For SID voice and timer-based effects
- **STE DAC Emulation**: DMA audio support for STe-specific SNDH files (50kHz mode with averaging)
- **YM2149 Sound Chip**: Using `ym2149` crate for cycle-accurate emulation
- **ChiptunePlayer Trait**: Unified interface compatible with other replayers

## Install

```toml
[dependencies]
ym2149-sndh-replayer = "0.9"
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
│ │ │  (r68k)   │ │ │   YM2149    │ │   │ │
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

## Hardware Accuracy

This emulator aims for high accuracy to correctly replay even the most demanding SNDH files. The following optimizations bring the emulation to approximately **98-99% hardware accuracy** for SNDH audio output.

### CPU Emulation (r68k)

The 68000 CPU is emulated via a customized [r68k](https://github.com/marhel/r68k) backend with the following enhancements:

| Feature | Description |
|---------|-------------|
| **Cycle Granularity** | 4-cycle boundary alignment matching Atari ST GLUE/MMU wait states |
| **Exception Cycles** | Correct cycle counts for interrupts (44 cycles) and TRAPs (34 cycles) |
| **YM2149 Access Latency** | Additional cycles for PSG register access timing |
| **Cycle Counter API** | `add_cycles()` method for modeling external delays |

### MFP 68901 Timer Emulation

The MFP timer system provides cycle-accurate interrupt generation:

| Feature | Description |
|---------|-------------|
| **FP16 Clock Precision** | High-precision lookup table using 16-bit fixed-point math eliminates cumulative rounding errors. Uses exact ratio 3125/960 for CPU-to-MFP clock conversion |
| **Dual-Mode Architecture** | Separate legacy (sample-based) and cycle-accurate timer states for seek compatibility |
| **Relative Cycle Tracking** | `cycles_until_fire` uses delta-based tracking instead of absolute cycles |
| **Phase Preservation** | Virtual cycle accumulation during seek preserves timer phase relationships. Multi-timer effects (SID voice, digidrum) maintain correct phase after seeking |
| **Prescale Switch Delay** | Per MC68901 manual: changing prescaler while running causes indeterminate 1-200 timer clock delay. Modeled as ~100 clocks |
| **Cycle-Accurate Counter Read** | TxDR reads return the actual countdown value based on CPU cycle, not just the last sampled value |
| **State Consistency** | Clean reset of all timer states after seek (counters, pending flags, in-service flags) |
| **Interrupt Latency** | Models MFP-internal propagation delay (~10 cycles). CPU-side latency is implicit through instruction-boundary checking |

### Nested Interrupt Support

Full MFP interrupt priority handling enables complex multi-timer drivers:

| Feature | Description |
|---------|-------------|
| **Priority Levels** | GPI7=15, Timer A=13, Timer B=8, Timer C=5, Timer D=4 |
| **Nesting** | Higher-priority interrupts can preempt lower-priority handlers |
| **Stack Protection** | Maximum nesting depth of 4 prevents stack overflow |
| **In-Service Tracking** | Proper acknowledge/end-of-interrupt handling per MFP specification |

### STE DMA Audio

Complete STE sound DMA emulation with bus contention modeling:

| Feature | Description |
|---------|-------------|
| **Sample Rates** | 6.25 kHz, 12.5 kHz, 25 kHz, 50 kHz |
| **Mono/Stereo** | Both modes supported |
| **50kHz Averaging** | Special handling for Tao MS3/Quartet-style 4-voice interleaved output |
| **Bus Contention** | DMA transfers steal ~8 CPU cycles per sample, affecting timer-relative timing |
| **Microwire Interface** | LMC1992 volume/bass/treble control |

### Timing Model

```
┌─────────────────────────────────────────────────────────────┐
│                    CPU Execution Loop                       │
├─────────────────────────────────────────────────────────────┤
│  1. Execute instruction (r68k with Musashi cycle tables)    │
│  2. Add DMA bus contention cycles (if STE DAC active)       │
│  3. Check MFP timer fire + latency threshold                │
│  4. Dispatch interrupt if:                                  │
│     - Timer fired (cycle-accurate check)                    │
│     - Priority > current handler (nested interrupt support) │
│     - Nesting depth < 4                                     │
│  5. Add exception cycles (44) on interrupt entry            │
│  6. Execute handler, RTE adds 20 cycles (r68k native)       │
└─────────────────────────────────────────────────────────────┘
```

### Accuracy Comparison

| Component | Basic Emulation | This Implementation |
|-----------|-----------------|---------------------|
| YM2149 Writes | Sample-rate | Cycle-accurate queue |
| MFP Timers | Integer math | FP16 precision LUT |
| MFP Prescale Switch | Instant | ~100 clock delay modeled |
| MFP Counter Read | Last sample | Cycle-accurate value |
| Interrupts | Single-level | Nested with priorities |
| Exception Cycles | Ignored | 44/34/20 cycles modeled |
| Interrupt Latency | Instant | 10+ cycles (variable) |
| DMA Contention | None | ~8 cycles per transfer |
| Seek Support | State corruption | Phase-preserving sync |

### Remaining Gaps (for 100%)

For reference, these features are **not** emulated but rarely affect SNDH playback:

- CPU prefetch queue (affects only cycle-exact raster effects)
- Cycle-exact bus arbitration (sub-instruction timing)
- Blitter interaction (not used in audio code)
- GLUE/MMU exact wait state patterns

## Related Crates

- **[ym2149](../ym2149-core)** - Core YM2149 chip emulation
- **[ym2149-common](../ym2149-common)** - Common traits
- **[ym2149-ym-replayer](../ym2149-ym-replayer)** - YM file playback
- **[ym2149-arkos-replayer](../ym2149-arkos-replayer)** - Arkos Tracker playback
- **[ym2149-ay-replayer](../ym2149-ay-replayer)** - AY file playback

## Credits

Based on the [sndh-player](https://github.com/arnaud-carre/sndh-player) C++ implementation by Arnaud Carré (Leonard/Oxygene).

ICE! 2.4 depacker based on the public domain C implementation by Hans Wessels.

68000 emulation via the [r68k](https://github.com/marhel/r68k) crate by Martin Helgesson, with custom cycle-accuracy enhancements.

## License

MIT License - see [LICENSE](../../LICENSE).
