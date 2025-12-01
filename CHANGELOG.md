# Changelog

All notable changes to the ym2149-rs project.

## [Unreleased] - v0.7.0

### Added
- **`ym2149-sndh-replayer` crate** - New crate for SNDH file playback with full Atari ST machine emulation:
  - **ICE! 2.4 Decompression** - Decompress ICE-packed SNDH files
  - **SNDH Parser** - Parse SNDH headers and metadata (TITL, COMM, YEAR, TIME, timer tags, etc.)
  - **MFP 68901 Timer Emulation** - Accurate timer support for SID voice and other timer-based effects
  - **STE DAC Emulation** - DMA audio support for STe-specific SNDH files (50kHz mode with averaging)
  - **Atari ST Machine** - Memory-mapped I/O emulation with 4MB RAM, YM2149, MFP timers, and STE DAC
  - **68000 CPU Emulation** - Via the `m68000` crate for executing native SNDH replay code
  - **ChiptunePlayer Implementation** - Unified interface compatible with other replayers
  - Supports multiple subsongs, player rate detection (TA/TB/TC/TD/VBL tags), and duration parsing
- **SNDH support in CLI replayer** - `ym-replayer` now supports `.sndh` files from Atari ST
- **SNDH support in bevy_ym2149** - Bevy plugin automatically detects and plays SNDH files
- **SNDH support in ym2149-wasm** - WASM player supports SNDH files in the browser
- **`ym2149-common` crate** - New shared crate providing unified traits and types across all replayers:
  - `ChiptunePlayer` trait - Common playback interface for all player types (play, pause, stop, state, generate_samples)
  - `PlaybackMetadata` trait - Unified metadata access across YM, AKS, AY, and SNDH formats
  - `PlaybackState` enum - Standard playback states (Stopped, Playing, Paused)
  - `BasicMetadata` struct - Simple metadata container for generic use cases
- `ChiptunePlayer` implementations for all four player types:
  - `YmPlayerGeneric<B>` in `ym2149-ym-replayer`
  - `ArkosPlayer` in `ym2149-arkos-replayer`
  - `AyPlayer` in `ym2149-ay-replayer`
  - `SndhPlayer` in `ym2149-sndh-replayer`

### Changed
- **YM2149 Core Emulation Rewrite** - Ported Leonard/Oxygene's cycle-accurate AtariAudio implementation:
  - New `ym2149/chip.rs` with hardware-accurate 250kHz (2MHz/8) emulation
  - New `ym2149/tables.rs` with accurate envelope shapes and logarithmic DAC levels
  - DC-adjust sliding window for clean audio output
  - Timer IRQ support for square-sync buzzer effects
  - Proper noise LFSR implementation
- **Module restructuring** - Split large modules into organized submodules:
  - `ym2149-ym-replayer`: `parser.rs` → `parser/` module, `player.rs` → `player/` module
  - `ym2149-arkos-replayer`: `parser.rs` → `parser/` module, `player.rs` → `player/` module, `channel_player.rs` → `channel_player/` module
- Migrated all error handling to `thiserror` for consistent, idiomatic error types
- Added `Default` trait implementations to key player configuration types
- Added `PartialEq`, `Eq` derives to format structs (`AyFile`, `AyHeader`, `AySong`, etc.)
- Added `PartialEq` to metadata types (`Ym6Metadata`, `ArkosMetadata`, `AyMetadata`, `BasicMetadata`)
- **`AyPlaybackState` deprecated** - Use `PlaybackState` from `ym2149-common` instead (backwards-compatible alias provided)
- **CLI uses native SNDH replayer** - Removed dependency on external `atari-audio` crate; all SNDH playback now uses `ym2149-sndh-replayer`

### Removed
- **`atari-audio` crate** - Removed from workspace; functionality fully integrated into `ym2149-sndh-replayer` and `ym2149-core`
- **`empiric_dac.rs`** - Removed legacy DAC implementation from `ym2149-core`; replaced by accurate logarithmic tables

### Fixed
- Removed redundant `AyPlaybackState` enum - now uses unified `PlaybackState` from `ym2149-common`
- Metadata moved from `ym2149-core` to `ym2149-common` (core crate now only handles YM2149 chip emulation)
- SNDH metadata now correctly extracted from ICE-packed files (decompression handled internally)
- Fixed Clippy warnings across the entire workspace

### Migration Guide
```rust
// Before (0.6.x)
use ym2149_ay_replayer::AyPlaybackState;

// After (0.7.0)
use ym2149_ay_replayer::PlaybackState;
// or
use ym2149_common::PlaybackState;

// New unified interface
use ym2149_common::ChiptunePlayer;
player.play();
player.pause();
player.stop();
let state = player.state(); // Returns PlaybackState
let meta = player.metadata(); // Returns &impl PlaybackMetadata
```

## [v0.6.1] - 2025-11-20

### Added
- AY/ZX playback path via new `ym2149-ay-replayer` crate; integrated into CLI backend selection and workspace.
- Pattern-triggered events in `bevy_ym2149` via `PatternTriggerSet` + `PatternTriggered`.
- Tone-shaping controls (soft saturation, accent, stereo widen, color filter toggle) exposed through `ToneSettings` and wired into the advanced Bevy example.

### Changed
- Complete rewrite of the YM2149 core emulation layer. Removed unnecessary code. Simplified implementation.
- YM2149 core unified to a single clk/8 implementation (`ym2149/` now only `chip.rs`); removed legacy submodules.
- Docs/architecture updated to reflect the clk/8 backend and simplified module layout.
- CLI backend selection simplified to the single hardware core.
- YM replayer CLI now applies the ST-style color filter as a post-process (default on unless `--no-color-filter`) and shares the same filter pipeline as Bevy replay.

### Fixed
- DigiDrum clipping in the clk/8 backend (integer drum injection, DC adjust).
- Buzzers/envelopes aligned to hardware tables and timing.

## [v0.6.0] - Previous Release

### Added
- Arkos Tracker support end-to-end: `ym2149-arkos-replayer` crate, Bevy/wrapper integration, curated fixtures in `examples/arkos`, wasm auto-detection for `.aks`
- New `scripts/build-wasm-examples.sh` helper to rebuild/copy the wasm bundle for demos & releases
- Message-based Bevy runtime: `FrameAudioData`, split `initialize_playback` / `drive_playback_state` / `process_playback_frames` + diagnostics and bridge consumers

### Changed
- `Ym2149Plugin` now accepts `.aks` sources transparently; docs/README updated accordingly
- wasm `Ym2149Player` automatically falls back to Arkos player when YM parsing fails, exposing the same JavaScript API
- Documentation refresh (`README.md`, `ARCHITECTURE.md`, crate READMEs) to reflect Arkos fixtures, new Bevy systems, wasm workflow

### Fixed
- Removed dependency on the upstream `arkostracker3` repo by copying reference `.aks/.ym` fixtures into `examples/arkos`
- Optional `extended-tests` now run against local fixtures; parser tests no longer panic when the Arkos repo is absent


### New Crates

- **bevy_ym2149_examples** - Comprehensive example suite (basic, advanced, feature_showcase, crossfade, demoscene)
- **bevy_ym2149_viz** - Optional visualization companion crate (extracted from core plugin)

### New Features

- **Playlist System** - `.ymplaylist` assets with automatic track progression and multiple playback modes
- **Playlist Crossfading** - Dual-deck blending that starts at 90% by default, supports fixed-time triggers, and now accepts explicit overlap window durations
- **bevy_ym2149_viz** - Visualization helpers extracted into a dedicated crate with `Ym2149VizPlugin`
- **Music State Graph** - Named state machine for dynamic soundtrack transitions
- **Audio Bridge** - Mirror generated samples into Bevy's audio graph with per-entity gain/pan control
- **Channel Events** - Per-frame channel snapshots and lifecycle events (TrackStarted/TrackFinished)
- **Diagnostics Integration** - Buffer fill and frame position metrics via Bevy's diagnostics system
- **Plugin Configuration** - `Ym2149PluginConfig` for toggling subsystems (playlists, events, diagnostics, etc.)
- **Enhanced Playback** - Added `from_asset()` and `from_bytes()` constructors for flexible loading

### Architecture Changes

- Split monolithic plugin into modular structure (plugin/mod.rs, plugin/config.rs, plugin/systems.rs)
- Extracted visualization to separate `bevy_ym2149_viz` crate for optional UI dependencies
- Added comprehensive error handling with `Ym2149Error` enum
- New modules: audio_bridge, diagnostics, error, events, music_state, playlist

### Documentation

- Expanded README from ~100 to 623 lines with comprehensive API guide
- Added example crate README with asset configuration details
- Added visualization crate README
- Moved core architecture docs to `ym2149-core/ARCHITECTURE.md`

### Examples & Assets

- 5 runnable examples demonstrating all plugin features
- 6 YM music files and custom demoscene bitmap font
- WGSL shaders for raymarched scenes and CRT post-processing

### Testing

- Added integration test suite (550+ lines covering playback and plugin lifecycle)
- Added playlist crossfade scheduler tests (triggering, completion, and TrackFinished suppression)
- Added `bevy_ym2149_viz` crate-level coverage via the advanced example

### Breaking Changes

- Visualization moved to `bevy_ym2149_viz` crate - requires separate dependency and import
- `Ym2149PluginConfig::visualization` removed; add `Ym2149VizPlugin` explicitly when UI is desired
- Advanced features require explicit opt-in via `Ym2149PluginConfig`


---

## [0.5.1] - Previous Release

Initial stable release of bevy_ym2149 plugin.
