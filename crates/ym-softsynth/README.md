# ym-softsynth

Experimental software synthesizer backend for YM2149 emulation.

This crate provides a lightweight, non-cycle-accurate alternative to the hardware-accurate YM2149 emulator. It implements the same `Ym2149Backend` trait, allowing it to be used as a drop-in replacement where accuracy is less critical than performance or simplicity.

## Status

**Experimental**: This backend is a work in progress and not suitable for production use. It provides basic tone generation but lacks many features of the hardware-accurate emulator.

## Features

- Implements `Ym2149Backend` trait
- Basic 3-channel tone generation
- Volume control
- Compatible with ym-replayer (for simple YM files)

## Not Implemented

- Envelope generator
- Noise generator
- Hardware effects (digi-drums, SID voice, sync buzzer)
- Cycle-accurate timing
- Many PSG registers

## Usage

```rust
use ym2149::Ym2149Backend;
use ym_softsynth::SoftSynth;

// Use as any Ym2149Backend
let mut synth = SoftSynth::new();
synth.write_register(0x00, 0xF0); // Channel A period low
synth.write_register(0x08, 0x0F); // Channel A volume

let sample = synth.get_sample();
```

## When to Use

- **Educational purposes**: Understanding PSG basics
- **Prototyping**: Quick experiments with PSG sounds
- **Low resource environments**: Where full emulation is too heavy

## When NOT to Use

- Accurate YM file playback (use ym2149-core instead)
- YM6 effects processing (requires hardware features)
- Production applications requiring accuracy

## Architecture

This crate was extracted from `ym2149-core` v0.6.0 as part of the backend trait abstraction:

- **ym2149-core**: Hardware-accurate emulation (production-ready)
- **ym-softsynth**: Experimental synthesizer (this crate)
- Both implement `Ym2149Backend` for interoperability

## License

See the main ym2149-rs repository for license information.
