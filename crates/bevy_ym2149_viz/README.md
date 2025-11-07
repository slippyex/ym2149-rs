# bevy_ym2149_viz

Visualization helpers for the [`bevy_ym2149`](../bevy_ym2149) audio plugin.

This crate contains the UI components, systems, and convenience builders that draw
oscilloscope traces, channel diagnostics, progress bars, and playlist information for
YM2149 playback. Keeping the visualization layer separate from the audio crate keeps the
core plugin lean and lets applications opt-in to UI only when needed.

## Features

- `Ym2149VizPlugin` wires the visualization systems/resources into a Bevy app.
- Builder helpers (`create_status_display`, `create_oscilloscope`, etc.) to quickly
  spawn ready-made UI layouts.
- Component types (`SpectrumBar`, `PlaybackStatusDisplay`, etc.) exposed so you can build
  your own layouts.
- Systems (`update_song_info`, `update_oscilloscope`, `update_song_progress`, …) that
  react to `Ym2149Playback` state and update the visualization entities.

## Usage

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};
use bevy_ym2149_viz::{
    create_status_display,
    create_detailed_channel_display,
    create_channel_visualization,
    Ym2149VizPlugin,
};

fn main() {
    App::new()
        .add_plugins((Ym2149Plugin::default(), Ym2149VizPlugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Ym2149Playback::new("music/demo.ym"));

    create_status_display(&mut commands);
    create_detailed_channel_display(&mut commands);
    create_channel_visualization(&mut commands, 3);
}
```

Add the crate to your `Cargo.toml` alongside `bevy_ym2149`:

```toml
bevy_ym2149 = { path = "../bevy_ym2149" }
bevy_ym2149_viz = { path = "../bevy_ym2149_viz" }
```

## License

MIT, same as the rest of the workspace.
