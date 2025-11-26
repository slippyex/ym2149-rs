# bevy_ym2149 – YM2149 Audio for Bevy

[![Crates.io](https://img.shields.io/crates/v/bevy_ym2149.svg)](https://crates.io/crates/bevy_ym2149)
[![Docs.rs](https://docs.rs/bevy_ym2149/badge.svg)](https://docs.rs/bevy_ym2149)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

Bevy plugin that embeds the cycle-accurate [`ym2149`](../ym2149-core) emulator, providing real-time YM/AKS/AY playback, playlists, crossfades, diagnostics, audio mirroring, and optional UI widgets via `bevy_ym2149_viz`.

<img src="../../docs/screenshots/advanced_example.png" alt="Advanced Bevy example" width="780">

## Why Use This Plugin?

- 🎵 **Accurate playback**: YM2–YM6/YMT + AKS + AY files rendered with the same cores as the CLI/exporter (`ym2149-ym-replayer`, `ym2149-arkos-replayer`, `ym2149-ay-replayer`)
- 🎚️ **ECS-native control**: `Ym2149Playback` component (play/pause/seek/volume/stereo gain)
- 🧭 **Music systems**: playlists with seamless crossfades, `.ymplaylist` loader, music state graphs
- ✨ **Tone shaping**: single-chip post FX (soft saturation, accent boost, stereo widen, ST color filter) via `ToneSettings`
- 🔊 **Audio bridge**: mirror samples into Bevy's audio graph or your own sinks
- 🎯 **Pattern triggers**: declaratively flag YM channel hits and drive gameplay via `PatternTriggerSet`
- 📈 **Diagnostics & events**: buffer fill metrics + `TrackStarted/TrackFinished`/`ChannelSnapshot`/`PlaybackFrameMarker`
- 🪄 **Gameplay hooks**: audio-reactive state (avg/peak/freq per channel) and PSG one-shot SFX via `YmSfxRequest`
- 🖥️ **Visualization**: drop in `Ym2149VizPlugin` for oscilloscope, spectrum, progress HUD (kept in a separate crate so headless builds stay lean)

## Getting Started

```toml
[dependencies]
bevy = "0.17"
bevy_ym2149 = "0.7"
bevy_ym2149_viz = { version = "0.7", optional = true }  # For visualization features
```

```rust
use bevy::prelude::*;
use bevy_ym2149::{Ym2149Playback, Ym2149Plugin};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Ym2149Plugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Ym2149Playback::new("assets/music/ND-Toxygene.ym"));
}
```

Add `Ym2149VizPlugin` (from `bevy_ym2149_viz`) when you want the UI builders:

```rust
use bevy_ym2149_viz::{create_status_display, Ym2149VizPlugin};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Ym2149Plugin::default(), Ym2149VizPlugin::default()))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
            commands.spawn(Ym2149Playback::new("music/song.ym"));
            create_status_display(&mut commands);
        })
        .run();
}
```

### Quick presets

`bevy_ym2149::presets::add_audio_stack(&mut app)` wires the audio plugin with all subsystems enabled.  
If you want audio + visualization in one go, `bevy_ym2149_viz::add_full_stack(&mut app)` adds both plugins.

## Subsystem Toggles

Configure the plugin at runtime via `Ym2149PluginConfig`:

| Config flag | Default | Purpose |
|-------------|---------|---------|
| `playlists` | ✅ | `.ymplaylist` loader + `Ym2149PlaylistPlayer`, crossfade driver |
| `channel_events` | ✅ | Emits `ChannelSnapshot` + `TrackStarted/Finished` |
| `music_state` | ✅ | `MusicStateGraph` + `MusicStateRequest` routing |
| `diagnostics` | ✅ | Registers `ym2149/frame_position` metric |
| `bevy_audio_bridge` | ✅ | Mirrors samples into `AudioBridgeBuffers` for custom DSP chains |
| `pattern_events` | ✅ | Enables `PatternTriggerSet` + `PatternTriggered` gameplay events |

Disable what you don’t need to keep your app lean.

## Runtime Flow / Systems

1. **Asset Loading** – `.ym`/`.aks` files load via Bevy’s asset system as `Ym2149AudioSource` (implements `Decodable`)
2. **Initialization (PreUpdate)** – `initialize_playback` attaches `AudioPlayer`/`PlaybackRuntimeState` to entities
3. **State Driving (PreUpdate)** – `drive_playback_state` reacts to `Ym2149Playback.state`, controlling `AudioSink`s and emitting `TrackStarted/TrackFinished`
4. **Frame Processing (Update)** – `process_playback_frames` generates audio samples per VBL frame, drives crossfades, and emits lightweight `FrameAudioData` messages
5. **Observability (Update)** – `emit_playback_diagnostics` (when enabled) converts `FrameAudioData` into `ChannelSnapshot`s and oscilloscope buffers; `publish_bridge_audio` mirrors raw stereo data if the audio bridge is on
6. **Visualization (optional)** – `bevy_ym2149_viz` systems consume those diagnostics/resources to render their UI widgets

## Key APIs

### `Ym2149Playback`

```rust
fn handle_input(mut query: Query<&mut Ym2149Playback>, keyboard: Res<ButtonInput<KeyCode>>) {
    if let Ok(mut playback) = query.get_single_mut() {
        if keyboard.just_pressed(KeyCode::Space) {
            if playback.is_playing() {
                playback.pause();
            } else {
                playback.play();
            }
        }
        if keyboard.just_pressed(KeyCode::KeyR) {
            playback.restart();
            playback.play();
        }
        if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::ArrowDown]) {
            let delta = if keyboard.just_pressed(KeyCode::ArrowUp) { 0.1 } else { -0.1 };
            playback.set_volume((playback.volume + delta).clamp(0.0, 1.0));
        }
    }
}
```

Other helpers:
- `Ym2149Playback::from_asset(handle)` and `::from_bytes(bytes)` for asset-server or embedded sources (auto-detects YM vs AKS)
- `set_source_path / asset / bytes` to retarget an entity mid-game (supports `.ym`/`.aks`)

### Tone shaping (soft saturation / accent / stereo widen)

Each `Ym2149Playback` exposes [`tone_settings`](https://docs.rs/bevy_ym2149/latest/bevy_ym2149/struct.Ym2149Playback.html#method.tone_settings) / `set_tone_settings` to dial in subtle post-processing without adding extra chips:

```rust
use bevy_ym2149::{ToneSettings, Ym2149Playback};

fn thicken_tracks(mut query: Query<&mut Ym2149Playback>) {
    for mut playback in &mut query {
        let mut tone = playback.tone_settings();
        tone.saturation = 0.2;       // Soft waveshaper
        tone.accent = 0.25;          // Dynamic boost that follows accents
        tone.widen = 0.15;           // Gentle stereo spread
        tone.color_filter = true;    // Authentic ST-style low-pass
        playback.set_tone_settings(tone);
    }
}
```

Settings are applied inside the decoder at audio-thread speed, so tweaks take effect immediately (see the advanced example for key bindings).

### Pattern-triggered gameplay events

Attach [`PatternTriggerSet`](https://docs.rs/bevy_ym2149/latest/bevy_ym2149/struct.PatternTriggerSet.html) to a playback entity and listen for [`PatternTriggered`](https://docs.rs/bevy_ym2149/latest/bevy_ym2149/struct.PatternTriggered.html):

```rust
use bevy::prelude::*;
use bevy_ym2149::{PatternTrigger, PatternTriggerSet, PatternTriggered, Ym2149Playback};

fn setup(mut commands: Commands) {
    let playback = Ym2149Playback::new("music/song.ym");
    let triggers = PatternTriggerSet::from_patterns(vec![
        PatternTrigger::new("bass_hit", 0).with_min_amplitude(0.35),
        PatternTrigger::new("lead_a4", 1)
            .with_min_amplitude(0.2)
            .with_frequency(440.0, 10.0)
            .with_cooldown(4),
    ]);
    commands.spawn((playback, triggers));
}

fn react_to_hits(mut hits: MessageReader<PatternTriggered>) {
    for hit in hits.read() {
        info!(
            "[{}] Channel {} amp {:.2} freq {:?}",
            hit.pattern_id, hit.channel, hit.amplitude, hit.frequency
        );
    }
}
```

Patterns are checked once per YM frame (≈50 Hz) using average channel amplitude, so they’re perfect for driving gameplay beats, VFX pulses, or scripted events.

Some practical ideas and mini-snippets:

- **Beat-synced lighting** – channel A threshold-only trigger toggles emissive sprites:

```rust
fn pulse_lights(
    mut hits: MessageReader<PatternTriggered>,
    mut materials: Query<&mut Emissive, With<MyLight>>,
) {
    for hit in hits.read().filter(|h| h.pattern_id == "Channel A Accent") {
        if let Ok(mut glow) = materials.get_single_mut() {
            glow.intensity = 3.0 * hit.amplitude;
        }
    }
}
```

- **Solo detection** – add a frequency constraint (e.g., 440 Hz ±10 Hz) to gate prompts:

```rust
commands.spawn((
    Ym2149Playback::new("song.ym"),
    PatternTriggerSet::from_patterns(vec![
        PatternTrigger::new("solo_a4", 1)
            .with_min_amplitude(0.22)
            .with_frequency(440.0, 10.0),
    ]),
));
```

- **Call-and-response** – fire PSG SFX when a hit lands:

```rust
fn answer_with_sfx(
    mut hits: MessageReader<PatternTriggered>,
    mut sfx: MessageWriter<YmSfxRequest>,
) {
    for hit in hits.read().filter(|h| h.pattern_id == "Lead A4") {
        sfx.write(YmSfxRequest {
            target: None,
            channel: 2,
            freq_hz: hit.frequency.unwrap_or(880.0),
            volume: 0.5,
            duration_frames: 8,
        });
    }
}
```
- `set_stereo_gain(left, right)` for manual stereo/pan control
- `PlaybackFrameMarker` event stream for 50Hz markers (frame, elapsed_seconds, looped)
- `AudioReactiveState` resource for smoothed channel avg/peak/frequency per playback entity
- `YmSfxRequest` to trigger short PSG tones mixed into playback (channel/freq/volume/duration)

### Playlists & Crossfades

```ron
// assets/music/demo.ymplaylist
(
    mode: "loop",
    tracks: [
        { type: "asset", path: "music/Ashtray.ym" },
        { type: "asset", path: "music/Credits.ym" },
    ],
)
```

```rust
commands.spawn((
    Ym2149Playback::default(),
    Ym2149PlaylistPlayer::with_crossfade(
        asset_server.load("music/demo.ymplaylist"),
        CrossfadeConfig::start_at_seconds(12.0).with_window_seconds(12.0),
    ),
));
```

`drive_crossfade_playlists` automatically preloads the next deck, and `PlaylistAdvanceRequest` lets you manually jump to indices.

### Music State Graph

```rust
fn configure(mut graph: ResMut<MusicStateGraph>, playlist: Handle<Ym2149Playlist>) {
    graph.set_target(Entity::from_raw(1)); // default playback entity
    graph.insert("title", MusicStateDefinition::Playlist(playlist));
    graph.insert("battle", MusicStateDefinition::SourcePath("music/battle.ym".into()));
}

fn switch(mut requests: MessageWriter<MusicStateRequest>) {
    requests.write(MusicStateRequest { state: "battle".into(), target: None });
}
```

### Audio Bridge

```rust
fn enable_bridge(
    mut requests: MessageWriter<AudioBridgeRequest>,
    mut mixes: ResMut<AudioBridgeMixes>,
    playback: Query<Entity, With<Ym2149Playback>>,
) {
    let entity = playback.single();
    requests.write(AudioBridgeRequest { entity });
    mixes.set(entity, AudioBridgeMix::from_db(-2.0, -0.25));
}

fn consume(buffers: Res<AudioBridgeBuffers>) {
    if let Some(samples) = buffers.0.get(&my_entity) {
        // feed samples into Bevy's audio graph or a custom DSP chain
    }
}
```

### Diagnostics

- `FRAME_POSITION_PATH` tracks the furthest frame processed across playbacks
- Use Bevy's standard `DiagnosticsStore` to access metrics

### Visualization (`bevy_ym2149_viz`)

Builders such as `create_status_display`, `create_detailed_channel_display`, and `create_channel_visualization` spawn flexbox-based UIs. Component types (`bevy_ym2149_viz::SongInfoDisplay`, `SpectrumBar`, `OscilloscopePoint`, etc.) are public so you can author your own layouts. See the `advanced_example` and `demoscene` demos for reference.

## Examples

The `bevy_ym2149_examples` crate contains:

| Example | Focus |
|---------|-------|
| `basic_example` | Minimal playback + keyboard control |
| `crossfade_example` | Playlist crossfades with decks |
| `advanced_example` | Full tracker-style UI + drag-and-drop + audio bridge knobs |
| `feature_showcase` | Multiple playbacks, music state graph, diagnostics |
| `demoscene` | Shader-heavy scene synchronized to YM playback |

Run e.g. `cargo run --example advanced_example -p bevy_ym2149_examples`.

## Troubleshooting Tips

- **No audio**: ensure the YM path is valid, `Ym2149Plugin` is added with `.add_audio_source::<Ym2149AudioSource>()`, and your OS has a working audio device
- **Crackling / underruns**: Bevy's audio system handles buffering automatically; check for heavy work blocking the audio callback thread
- **Playback too fast/slow**: confirm `Time` resource is advancing; visualization uses frame position tracking
- **Bridge audio silent**: make sure the entity was added to `AudioBridgeTargets` (via `AudioBridgeRequest`) and that its mix isn't muted
- **Player not started**: ensure `PlaybackController::play()` is called on the player instance in `Ym2149AudioSource::new()`

## License

MIT License – see [LICENSE](../../LICENSE).
