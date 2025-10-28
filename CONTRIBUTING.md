# Contributing to YM2149-RS

Thank you for your interest in contributing to the YM2149 PSG emulator! This document provides guidelines for contributions.

## Code Style & Conventions

### Constant Naming Conventions

Constants should be clearly named and well-documented. We organize constants into logical modules:

**Module Organization**:
- **`src/ym2149/constants.rs`** - Hardware-specific constants (frequency tables, volume table, etc.)
- **`src/replayer/mod.rs`** - Application-level timing configuration (sample rates, VBL frequencies)
- **`src/streaming/mod.rs`** - Streaming constants (buffer backoff time, update intervals)

**Constant Documentation Pattern**:

Every constant should follow this documentation structure:

```rust
/// What the constant represents (one line summary)
/// Extended description explaining units and purpose.
///
/// Design rationale: Why this value was chosen, what tradeoffs it represents.
/// Context: How it affects other parts of the system.
pub const CONSTANT_NAME: Type = value;
```

**Example**:

```rust
/// Visualization update interval in milliseconds
/// 50ms = 20 updates/second, providing smooth real-time feedback.
///
/// Design rationale:
/// - Visual smoothness: Human eye perceives <30fps as smooth, 20fps is comfortable
/// - Lock contention: Monitoring thread acquires player lock every 50ms
/// - Terminal I/O: Modern terminals handle 20 lines/second without flicker
/// - Balance: Low enough for responsiveness, high enough to avoid excessive locking
pub const VISUALIZATION_UPDATE_MS: u64 = 50;
```

**Good Practices**:
1. ✅ Name constants in SCREAMING_SNAKE_CASE
2. ✅ Document the unit of measurement (Hz, ms, bytes, etc.)
3. ✅ Explain the design rationale, not just what the constant does
4. ✅ Include human factors considerations when relevant
5. ✅ Group related constants in the same module
6. ✅ Use `pub` for constants that might be needed by dependent code

**Avoid**:
- ❌ Magic numbers without explanation
- ❌ Arbitrary values without rationale
- ❌ Scattering related constants across different files
- ❌ Using private constants when other modules might reuse them

## Testing

Run the test suite before submitting:

```bash
# Run all tests
cargo test

# Run with output for debugging
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run benchmarks (if available)
cargo test --benches
```

## Code Quality

### Clippy

Ensure your code passes clippy with no warnings:

```bash
cargo clippy --lib --bins
```

### Formatting

Format your code using rustfmt:

```bash
cargo fmt
```

## Documentation

- Add doc comments to all public items using `///`
- Include examples in doc comments where helpful
- Update README.md if adding new public APIs
- Document design decisions in comments for complex code

## Commit Messages

- Use clear, descriptive messages
- Reference issues where applicable
- Explain the "why" not just the "what"

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Make your changes
4. Run `cargo fmt` and `cargo clippy` to ensure code quality
5. Run `cargo test` to verify tests pass
6. Commit with clear messages
7. Push to your fork
8. Submit a pull request with a description of your changes

## Architecture

The codebase is organized into domain-based modules with clear separation of concerns:

- **`ym2149/`** - YM2149 PSG emulation (chip implementation with section markers, registers, envelope, mixer, oscillators)
- **`mfp/`** - Atari ST MFP timer infrastructure (timer constants and utilities)
- **`ym_parser/`** - YM file format parsing (YM3/4/5/6 formats, raw frames, and special effects decoding)
- **`ym_loader/`** - YM file I/O and loading with auto-detection
- **`replayer/`** - Playback engine (player, VBL synchronization, cycle counting, timing config)
- **`compression/`** - Data decompression utilities (LHA/LZH support)
- **`streaming/`** - Real-time audio streaming (ring buffer, audio device, playback)
- **`visualization/`** - Terminal UI utilities for real-time displays

### Design Principles

- **Single Concern**: Each domain handles one business concern (not a catch-all utils folder)
- **Format-Specific Logic**: Effects decoding is part of `ym_parser/` because it's YM file format-specific
- **Tight Coupling**: PSG effects (Sync Buzzer, SID, DigiDrum) remain in `ym2149/chip.rs` because they're core modulation behaviors, not separate features

When adding features, place code in the appropriate domain module.

## Performance Considerations

- The library is used for real-time audio generation
- Minimize allocations in hot paths
- Lock contention should be measured and minimized
- Consider CPU cache efficiency for frequently-accessed data

## Licensing

By contributing, you agree that your contributions are licensed under the same license as the project (MIT).

## Questions?

Feel free to open an issue if you have questions or need clarification on anything!

Thank you for contributing! 🎵
