# ym2149-gist-replayer

GIST Sound File Parser and Multi-PSG Player for YM2149.

## Overview

This crate provides a parser and player for GIST (Graphics, Images, Sound, Text)
sound effect files, originally used on the Atari ST. GIST was developed by Dave Becker
and distributed by Antic Software in the late 1980s.

This is a Rust port of the GIST sound driver from [th-otto/gist](https://github.com/th-otto/gist).

## Credits

- **Original GIST**: Dave Becker / Antic Software
- **Driver preservation & documentation**: [Thorsten Otto](https://github.com/th-otto)

## Features

- **GistPlayer**: High-level player with automatic sample generation
- **GistSound**: Parse and represent GIST sound effect definitions (112 bytes each)
- **GistDriver**: Low-level multi-voice driver supporting up to 3 simultaneous sounds
- **Cycle-accurate emulation**: Faithful port of the original 68000 assembly driver

## Quick Start

For simple playback, use `GistPlayer`:

```rust
use ym2149_gist_replayer::{GistPlayer, GistSound};

let sound = GistSound::load("effect.snd").unwrap();
let mut player = GistPlayer::new();

player.play_sound(&sound, None, None);

// Generate audio samples
while player.is_playing() {
    let samples = player.generate_samples(882); // ~20ms at 44100 Hz
    // Send samples to audio output...
}
```

## Low-Level API

For more control, use `GistDriver` directly with a `Ym2149` chip:

```rust
use ym2149::Ym2149;
use ym2149_gist_replayer::{GistDriver, GistSound, TICK_RATE};

let sound = GistSound::load("effect.snd").unwrap();
let mut chip = Ym2149::new();
let mut driver = GistDriver::new();

// Start playing on first available voice
driver.snd_on(&mut chip, &sound, None, None, -1, i16::MAX - 1);

// In your audio loop, call tick() at 200 Hz (TICK_RATE)
while driver.is_playing() {
    driver.tick(&mut chip);
    // Generate samples from chip.get_sample() and send to audio output
}
```

## Sound Structure

Each GIST sound contains:

- **Duration**: How long the sound plays in ticks (200 Hz)
- **Tone parameters**: Initial frequency, envelope, and LFO settings
- **Noise parameters**: Initial noise frequency, envelope, and LFO settings
- **Volume envelope**: ADSR-style envelope with LFO modulation

Values use 16.16 fixed-point format for smooth envelope transitions.

## API Reference

### GistPlayer (High-Level)

- `GistPlayer::new()` - Create player with default 44100 Hz sample rate
- `GistPlayer::with_sample_rate(rate)` - Create player with custom sample rate
- `player.play_sound(sound, volume, priority)` - Play sound on auto-selected voice
- `player.play_sound_on_voice(sound, voice, volume, priority)` - Play on specific voice
- `player.play_sound_pitched(sound, pitch, voice, volume, priority)` - Play with specific pitch
- `player.stop_voice(voice)` - Stop a specific voice
- `player.stop_all()` - Stop all voices
- `player.is_playing()` - Check if any voice is active
- `player.generate_samples(count)` - Generate audio samples
- `player.generate_samples_into(buffer)` - Generate samples into existing buffer

### GistDriver (Low-Level)

- `GistDriver::new()` - Create a new driver instance
- `driver.snd_on(chip, sound, voice, volume, pitch, priority)` - Start a sound
- `driver.snd_off(voice_idx)` - Stop a specific voice
- `driver.stop_all(chip)` - Stop all voices
- `driver.is_playing()` - Check if any voice is active
- `driver.tick(chip)` - Process one tick (call at 200 Hz)

## Constants

- `TICK_RATE` (200 Hz) - The driver tick rate, matching the Atari ST Timer C
- `DEFAULT_SAMPLE_RATE` (44100 Hz) - Default audio sample rate

## License

See the main ym2149-rs repository for license information.
