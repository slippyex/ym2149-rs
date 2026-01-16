# ym2149-common

Shared traits, types, and utilities for YM2149 chiptune replayers.

## Overview

This crate provides the unified interface that all replayer crates (`ym2149-ym-replayer`, `ym2149-arkos-replayer`, `ym2149-ay-replayer`, `ym2149-sndh-replayer`) implement. It also provides shared utilities for register parsing, frequency calculations, and visualization.

**Key exports:**
- Player traits: `ChiptunePlayer`, `ChiptunePlayerBase`
- State types: `PlaybackState`, `ChannelStates`, `BasicMetadata`
- Register utilities: `channel_period`, `period_to_frequency`, `channel_frequencies`
- Constants: `PSG_MASTER_CLOCK_HZ`, `NOTE_NAMES`

## Key Types

### Trait Hierarchy

```
ChiptunePlayerBase (object-safe)
    │
    └── ChiptunePlayer (adds metadata access)
```

### `ChiptunePlayerBase` trait

Object-safe base trait for playback operations. Use this when you need trait objects (`Box<dyn ChiptunePlayerBase>`):

```rust
use ym2149_common::{ChiptunePlayerBase, PlaybackState};

fn play_any(player: &mut dyn ChiptunePlayerBase) {
    player.play();

    // Generate audio samples
    let mut buffer = vec![0.0f32; 882]; // ~20ms at 44.1kHz
    player.generate_samples_into(&mut buffer);

    // Check playback state
    if player.state() == PlaybackState::Playing {
        println!("Playing audio...");
    }
}
```

### `ChiptunePlayer` trait

Extends `ChiptunePlayerBase` with metadata access. Use this when you need the specific metadata type:

```rust
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, PlaybackState};

fn play_with_info<P: ChiptunePlayer>(player: &mut P) {
    player.play();

    // Generate audio samples (from ChiptunePlayerBase)
    let mut buffer = vec![0.0f32; 882];
    player.generate_samples_into(&mut buffer);

    // Access metadata (from ChiptunePlayer)
    if player.state() == PlaybackState::Playing {
        println!("Currently playing: {}", player.metadata().title());
    }
}
```

### `PlaybackMetadata` trait

Unified metadata access across all formats:

```rust
use ym2149_common::PlaybackMetadata;

fn show_info<M: PlaybackMetadata>(meta: &M) {
    println!("Title:  {}", meta.title());
    println!("Author: {}", meta.author());
    println!("Format: {}", meta.format());

    if let Some(frames) = meta.frame_count() {
        let duration = frames as f32 / meta.frame_rate() as f32;
        println!("Duration: {:.1}s", duration);
    }
}
```

### `PlaybackState` enum

Standard playback states used by all players:

```rust
use ym2149_common::PlaybackState;

match player.state() {
    PlaybackState::Stopped => println!("Stopped"),
    PlaybackState::Playing => println!("Playing"),
    PlaybackState::Paused  => println!("Paused"),
}
```

### `BasicMetadata` struct

A concrete metadata type for simple use cases:

```rust
use ym2149_common::BasicMetadata;

let meta = BasicMetadata {
    title: "My Song".into(),
    author: "Composer".into(),
    format: "YM".into(),
    frame_count: Some(5000),
    frame_rate: 50,
    ..Default::default()
};
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
ym2149-common = "0.9"
```

All replayer crates re-export these types, so you typically don't need to depend on `ym2149-common` directly:

```rust
// These are equivalent:
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase};
use ym2149_ym_replayer::{ChiptunePlayer, ChiptunePlayerBase};
use ym2149_arkos_replayer::{ChiptunePlayer, ChiptunePlayerBase};
use ym2149_ay_replayer::{ChiptunePlayer, ChiptunePlayerBase};
use ym2149_sndh_replayer::{ChiptunePlayer, ChiptunePlayerBase};
```

## License

MIT
