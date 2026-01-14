# Changelog

All notable changes to the ym2149-rs project.

## 2026/01/14 - v0.9.0

### Added
- **LMC1992 STE Audio Mixer Emulation** - Full implementation of the Atari STE's digitally controlled audio processor:
  - Microwire serial interface at $FF8922-$FF8925 (11-bit commands)
  - Master volume control (0-40, -80dB to 0dB in 2dB steps)
  - Independent left/right channel volume (0-20, -40dB to 0dB)
  - Bass EQ with biquad low-shelf filter at ~100Hz (-12dB to +12dB)
  - Treble EQ with biquad high-shelf filter at ~10kHz (-12dB to +12dB)
  - Mix control to enable/disable YM2149 in output path
  - Proper transmission state machine for software polling
- **Stereo audio output for SNDH** - SNDH replayer now outputs true stereo:
  - YM2149 mixed with STE DAC stereo samples
  - LMC1992 processing applied to final stereo mix
- **Complete MFP68901 interrupt register emulation** - Full MC68901 interrupt handling:
  - IPRA/IPRB (Interrupt Pending Registers) - Track pending timer interrupts
  - ISRA/ISRB (Interrupt In-Service Registers) - Track active interrupt handlers
  - AER (Active Edge Register) - Configure rising/falling edge triggers for TAI/TBI/GPI7
  - DDR (Data Direction Register) - GPIO pin direction control
  - VR (Vector Register) with S-bit for software/automatic EOI mode
  - Proper interrupt acknowledge and end-of-interrupt sequencing
  - Timer A/B/GPI7 input pin edge detection respecting AER settings (STF/STFM compatible)
- **`MASTER_GAIN` constant** - Global 2x amplification factor in `ym2149-common` for louder output across all backends (core, softsynth, SNDH)
- **New SNDH example songs** - Added tracks from 505, Jess, Modmate, and Tao
- **SNDH v2.2 format support** - Full support for the January 2026 SNDH specification:
  - `FRMS` tag parsing - Frame-based duration (32-bit per subtune, 0 = endless loop)
  - `FLAG` tag parsing - Feature flags for hardware requirements (Timer A-D, STE, DSP, LMC1992, blitter, etc.)
  - `#!SN` tag parsing - Subtune names with word-offset table
  - `SndhFlags` struct - Typed access to all SNDH v2.2 feature flags
  - `DmaSampleRate` enum - STE (6-50kHz) and Falcon (12-49kHz) sample rates
  - Backward compatible: TIME tag still supported, FRMS takes priority when present
  - `SubsongInfo::subtune_name` - Access subtune names from #!SN tag
  - Duration calculation prefers FRMS (exact frames) over TIME (seconds × rate)
- **SNDH seeking support** - Frame-based position tracking and seeking:
  - `current_frame()` - Get current playback frame position
  - `total_frames()` - Get total frame count from FRMS/TIME metadata
  - `progress()` - Get playback progress as 0.0-1.0 fraction
  - `seek_to_frame(frame)` - Seek to specific frame by fast-forwarding
  - `seek_to_time(seconds)` - Seek to time position (converted to frames)
  - `playback_position()` now returns actual progress based on frame metadata
- **Unified seeking in ChiptunePlayerBase trait** - New trait methods for position tracking and seeking:
  - `seek(position: f32)` - Seek to position (0.0-1.0 fraction)
  - `duration_seconds()` - Get total duration in seconds
  - `elapsed_seconds()` - Get elapsed time based on playback position
- **CLI seeking and volume control** - Arrow key controls for TUI mode:
  - **←/→**: Seek ±5 seconds (works with FRMS/TIME metadata)
  - **↑/↓**: Volume up/down (±5%)
  - **+/-**: Subsong navigation (for multi-song SNDH files)
  - Time display now shows actual playback position (not wallclock time)
  - Seek throttling (250ms cooldown) prevents stuttering when holding arrow keys
- **WASM player seeking** - Full seeking support for all formats in browser:
  - `seek_to_frame()` and `seek_to_percentage()` now work for SNDH
  - `duration_seconds()` - Get total song duration
  - `hasDurationInfo()` - Check if duration is from metadata (vs 5-minute fallback)
  - Progress bar reflects actual playback position from FRMS/TIME metadata
- **Bevy seeking** - Full seeking support in bevy_ym2149:
  - `Ym2149Playback::seek_percentage(position)` - Seek to position (0.0-1.0)
  - `Ym2149Playback::duration_seconds()` - Get total song duration
  - `Ym2149Playback::has_duration_info()` - Check if duration is from metadata
  - `Ym2149Playback::playback_position()` - Get current position (0.0-1.0)
- **Interactive progress bar** in Bevy advanced_example:
  - Click anywhere on the progress bar to seek to that position
  - `ProgressBarContainer` component for custom seek UI integration

### Changed
- **68000 CPU backend replaced** - Switched from `m68000` crate to `r68k` (based on Musashi):
  - New `cpu_backend` module with abstraction layer for CPU emulation
  - `CpuMemory` trait for memory access abstraction
  - `Cpu68k` trait for CPU execution interface
  - Better instruction set compatibility for SNDH drivers
  - Atari ST bus timing with 4-cycle boundary alignment (GLUE/MMU wait states)
- **SNDH machine architecture refactored** - Cleaner separation of CPU, memory, and I/O emulation:
  - `DefaultCpu` type alias for easy backend switching
  - Improved memory-mapped I/O handling for LMC1992 registers
- **SNDH seeking ~3500x faster** - Optimized `seek_to_frame()` for near-instant seeking:
  - Reduced hardware simulation from 882 samples to 1 per tick during seek
  - Reduced CPU cycle budget from 400k to 100k during seek phase
  - Seeking now responds instantly even for long songs

### Fixed
- **Audio volume too quiet** - Applied 2x gain boost to all audio output paths
- **SNDH FLAG tag parsing** - Fixed parser incorrectly interpreting next tag as flags when FLAG contained null-terminated entries (e.g., `FLAG~abdy\0\0FRMS` was parsing `FRMS` as flag characters)
- **SNDH < 2.2 seeking** - Older SNDH files without FRMS/TIME metadata now support seeking:
  - Fallback duration of 5 minutes (15000 frames at 50Hz) when no duration info available
  - `has_duration_info()` returns false for these files to indicate estimated duration
  - Frame count set early in `init_subsong()` before potential early returns
- **Bevy SndhBevyPlayer frame tracking** - Fixed `current_frame()` and `frame_count()` returning 0:
  - Now correctly delegates to underlying player methods

## 2025/12/08 - v0.8.0

### Added
- **Directory playback with playlist selection** - Play all songs from a directory
  - Recursive scanning of directories for supported music files (YM, AKS, AY, SNDH)
  - Playlist overlay with song title, author, and duration from metadata
  - Press `[p]` to toggle playlist, `[↑↓]` to navigate, `[Enter]` to select song
  - Seamless song switching without restarting the player or leaving TUI mode
  - Auto-advance to next song when current song ends
  - Falls back to filename display when metadata is unavailable
  - Directory mode starts with playlist overlay open for immediate song selection
  - **Type-ahead search** in playlist: just start typing to filter songs
    - Searches title, author, and filename
    - `[↑↓]` jumps to next/previous match when searching
    - `[Backspace]` deletes characters, `[Esc]` clears search
    - Matching text highlighted in yellow, non-matching entries dimmed
- **Ratatui-based TUI for CLI** - New terminal UI with oscilloscope and spectrum analyzer
  - Oscilloscope with per-channel waveform display (A=Red, B=Green, C=Blue)
  - Spectrum analyzer with 16 note-aligned frequency bins
  - Auto-scaling with DC offset correction
  - Auto-detection: TUI mode for terminals ≥80×24, classic mode for smaller
  - Controls: 1-9 mute channels, Space pause, ↑↓ subsong, p playlist, Q quit
  - **Volume control** with `[←→]` arrow keys (5% steps, displayed in footer)
  - **Quick song navigation** with `[,]` previous / `[.]` next (playlist mode)
- **`ym2149_common::visualization` module** - Shared visualization utilities for all frontends:
  - `WaveformSynthesizer` - Register-based waveform synthesis with per-channel phase accumulators
  - `SpectrumAnalyzer` - Note-aligned spectrum with 16 bins (C1-B8, half-octave resolution)
  - `freq_to_bin()` - Musical frequency to spectrum bin mapping
  - Unified implementation used by both Bevy and CLI

### Changed
- **Register-based visualization** - Complete rewrite of oscilloscope and spectrum in both Bevy and CLI
  - Oscilloscope now synthesizes waveforms from YM2149 register state, not audio samples
  - Works with digidrums and STE-DAC samples that bypass the PSG
  - Square waves for tone, pseudo-random noise, realistic envelope shapes for buzz
  - Per-channel phase accumulators with proper overflow handling (`.fract()`)
- **Realistic envelope waveform synthesis** - All 16 YM2149 envelope shapes now visualize correctly:
  - Decay shapes (0x00-0x03, 0x09): High to low
  - Attack shapes (0x04-0x07, 0x0F): Low to high
  - Sawtooth down (0x08): Continuous decay
  - Triangle (0x0A): /\/\/\
  - Decay + hold high (0x0B): Decay then sustain max
  - Sawtooth up (0x0C): Continuous attack
  - Attack + hold high (0x0D): Attack then sustain max
  - Triangle inverted (0x0E): \/\/\/
- **Note-aligned spectrum bins** - Spectrum analyzer now shows musical notes
  - 16 bins covering 8 octaves (C1 to B8), 2 bins per octave
  - Base frequency C1 = 32.703 Hz (MIDI note 24)
  - Logarithmic mapping: `bin = log2(freq / C1) * 2`
- **Improved envelope/sync-buzzer detection** - Spectrum shows correct frequency for:
  - Pure envelope buzz (uses envelope frequency)
  - Sync-buzzer (uses tone period even when tone disabled in mixer)
  - Noise (spread across high frequency bins based on noise period)
- **Smooth spectrum decay** - Decay factor 0.85 for responsive yet smooth visualization
- **`ym2149-core` restructured** - Now contains only pure chip emulation:
  - `Ym2149Backend` trait moved to `ym2149-common`
  - Directory structure flattened: `src/ym2149/*` → `src/*`
  - Shared utilities (`ChannelStates`, `channel_period`, `period_to_frequency`) live in `ym2149-common`
  - Import paths simplified: `ym2149::Ym2149`, `ym2149::PsgBank`, `ym2149::constants::*`

### Fixed
- **Bevy spectrum bars** - Now show actually played notes instead of FFT analysis
- **Bevy oscilloscope** - Shows distinct per-channel waveforms (was showing same signal 3x)
- **CLI oscilloscope** - Per-channel waveform synthesis with auto-scaling
- **Envelope amplitude detection** - Fixed check: `amplitude > 0 || envelope_enabled`
- **Phase accumulator overflow** - Now uses `.fract()` to handle high frequency edge cases
- **TUI visual glitches** - Suppressed `println!` output when TUI mode is active
  - File loading messages no longer bleed into TUI borders
  - Format detection output hidden in TUI mode
- **Playlist mode auto-start** - Player now waits for user song selection instead of auto-playing
  - Directory mode starts paused with playlist overlay open
  - Auto-advance only triggers after user has selected a song

### Refactored
- **Bevy visualization code cleanup** - Removed ~100 lines of redundant spectrum calculation from `systems.rs`
  - Now uses shared `ym2149_common::visualization` module instead of local FFT-based computation
  - Eliminated duplicate constants (`SPECTRUM_DECAY`, `SPECTRUM_BINS`, `SPECTRUM_BASE_FREQ`, `BINS_PER_OCTAVE`)
  - Removed duplicate `freq_to_bin()` function - now imported from shared library
  - Removed unused `OscilloscopeBuffer` parameter (register-based waveforms don't need audio samples)
- **CLI streaming cleanup** - Removed unused capture parameter from producer loop
  - Waveform visualization now generated from register state in TUI, not from audio thread

## 2025/12/04 - v0.7.1

### Fixed
- **bevy_ym2149 diagnostics module** - Made `diagnostics` module public and exported `FRAME_POSITION_PATH`, `BUFFER_FILL_PATH`, `update_diagnostics`, and `register_diagnostics` to fix compilation of dependent crates
- **WASM player iOS audio** - Fixed audio playback on iOS Safari using MediaStream routing through HTML Audio element
- **WASM player UI** - Redesigned index.html and simple-player.html for better mobile experience

### Added
- Added 4 new SNDH example songs from Tao (Intensity_200hz) and !Cube (Bullet_Sequence, Elusive_Groove, Outpost)

## 2025/12/03 - v0.7.0

### Added
- **`ChannelStates` module** - New `ym2149::channel_state` module for unified register-based visualization:
  - `ChannelState` - Per-channel state (tone period, frequency, note name, MIDI note, amplitude, mixer flags)
  - `EnvelopeState` - Envelope generator state (period, shape, shape name)
  - `NoiseState` - Noise generator state (period, frequency)
  - `ChannelStates::from_registers()` - Extract visualization-ready data from any YM2149 register dump
  - Works consistently across all formats (YM, AKS, AY, SNDH) since all use the same YM2149 registers
- **`ChipStateSnapshot` resource** - New Bevy resource in `bevy_ym2149` for visualization access:
  - Provides latest YM2149 register dump without locking the player
  - Includes derived `ChannelStates` for immediate use by visualization systems
- **`generate_samples_with_channels()` method** - New `Ym2149Backend` trait method for synchronized visualization:
  - Generates audio samples and captures per-sample channel outputs simultaneously
  - Ensures visualization data is perfectly synchronized with audio playback
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
  - `ChiptunePlayerBase` trait - Object-safe base trait for dynamic dispatch (play, pause, stop, state, generate_samples, subsong support, multi-PSG)
  - `ChiptunePlayer` trait - Extends `ChiptunePlayerBase` with metadata access via associated type
  - `PlaybackMetadata` trait - Unified metadata access across YM, AKS, AY, and SNDH formats
  - `PlaybackState` enum - Standard playback states (Stopped, Playing, Paused)
  - `BasicMetadata` struct - Simple metadata container for generic use cases
- `ChiptunePlayerBase` and `ChiptunePlayer` implementations for all four player types:
  - `YmPlayerGeneric<B>` in `ym2149-ym-replayer`
  - `ArkosPlayer` in `ym2149-arkos-replayer`
  - `AyPlayer` in `ym2149-ay-replayer`
  - `SndhPlayer` in `ym2149-sndh-replayer`
- **Multi-PSG CLI visualization** - CLI replayer now displays all PSG chips for Arkos songs with 6+ channels:
  - Dynamic channel display (A-L for up to 12 channels / 4 PSGs)
  - Extended mute controls (keys 1-9, 0 for channels 1-10)
  - PSG count indicator in status line
- **WASM Mobile Audio** - Fixed audio playback on iOS/Android browsers:
  - AudioContext created on user interaction (tap/click)
  - Automatic resume from suspended state
  - Visibility change handler for background/foreground transitions

### Changed
- **YM2149 Core Emulation Rewrite** - Ported Leonard/Oxygene's cycle-accurate [AtariAudio](https://github.com/arnaud-carre/sndh-player/tree/main/AtariAudio) implementation:
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


### Fixed
- Removed redundant `AyPlaybackState` enum - now uses unified `PlaybackState` from `ym2149-common`
- Metadata moved from `ym2149-core` to `ym2149-common` (core crate now only handles YM2149 chip emulation)
- SNDH metadata now correctly extracted from ICE-packed files (decompression handled internally)
- Fixed Clippy warnings across the entire workspace

### Documentation
- Added comprehensive documentation to `bevy_ym2149::playlist` module (all public types and fields)
- Added documentation to `bevy_ym2149_viz` crate (components, builders, systems, uniforms)
- WASM example page updated with all four formats (YM, AKS, SNDH, AY) grouped by format type

### Migration Guide
```rust
// Before (0.6.x)
use ym2149_ay_replayer::AyPlaybackState;

// After (0.7.0)
use ym2149_ay_replayer::PlaybackState;
// or
use ym2149_common::PlaybackState;

// New unified interface - use ChiptunePlayerBase for playback methods
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase};
player.play();         // from ChiptunePlayerBase
player.pause();        // from ChiptunePlayerBase
player.stop();         // from ChiptunePlayerBase
let state = player.state();    // from ChiptunePlayerBase
let meta = player.metadata();  // from ChiptunePlayer (requires concrete type)

// For trait objects, use ChiptunePlayerBase
fn play_any(player: &mut dyn ChiptunePlayerBase) {
    player.play();
    player.generate_samples_into(&mut buffer);
}
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
