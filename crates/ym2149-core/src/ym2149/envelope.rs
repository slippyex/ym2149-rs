//! YM2149 Envelope Generator
//!
//! Implements the YM2149 envelope generator using a lookup table approach for hardware-accurate envelope generation.
//! Each of the 16 envelope shapes is defined by 4 phases, each interpolating between
//! two amplitude values over 16 steps, creating 64 total amplitude values per shape.
//!
//! The envelope uses a 32-bit position accumulator that advances at a frequency
//! controlled by register R11-R12 (envelope frequency). This creates the correct
//! timing and enables repeating patterns like sawtooth waves for buzzer effects.

use std::fmt;

use super::constants::get_volume;

/// Envelope Shape Control - Register R13
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeShape {
    /// 0000: Attack-Decay
    /// Pattern: Attack 0→1, Decay 1→0 once, then silence
    AttackDecay = 0x00,
    /// 0001: Attack-Decay (variant 1 - same pattern as 0x00)
    AttackDecay1 = 0x01,
    /// 0010: Attack-Decay (variant 2 - same pattern as 0x00)
    AttackDecay2 = 0x02,
    /// 0011: Attack-Decay (variant 3 - same pattern as 0x00)
    AttackDecay3 = 0x03,
    /// 0100: Attack-Decay-Release
    /// Pattern: Attack, Decay to middle, Release to silence
    AttackDecayRelease = 0x04,
    /// 0101: Attack-Sustain-Release
    /// Pattern: Attack, Hold high, Release to silence
    AttackSustainRelease = 0x05,
    /// 0110: Triangle (Attack-Decay symmetric)
    /// Pattern: Attack 0→1, Decay 1→0, Hold 0, Hold 0
    Triangle = 0x06,
    /// 0111: Triangle-Sustain
    /// Pattern: Attack 0→1, Hold 1, Decay 1→0, Hold 0
    TriangleSustain = 0x07,
    /// 1000: Sawtooth-Down (repeating) - BUZZER SOUND
    /// Pattern: Decay 1→0 repeated 4 times = continuous sawtooth
    /// Creates buzzing when combined with tone frequency
    SawtoothDown = 0x08,
    /// 1001: Attack then Sawtooth-Down (repeating)
    /// Pattern: Attack 0→1, then Sawtooth-Down 1→0 repeated 3x
    AttackSawtoothDown = 0x09,
    /// 1010: Sustain then Sawtooth-Down (repeating)
    /// Pattern: Hold 1, then Sawtooth-Down 1→0 repeated 3x
    SustainSawtoothDown = 0x0A,
    /// 1011: Attack-Sustain-Sawtooth (repeating)
    /// Pattern: Attack 0→1, Hold 1, then Sawtooth 1→0 repeated twice
    AttackSustainSawtooth = 0x0B,
    /// 1100: Sawtooth-Up (repeating) - BUZZER SOUND
    /// Pattern: Attack 0→1 repeated 4 times = continuous sawtooth
    /// Creates buzzing when combined with tone frequency
    SawtoothUp = 0x0C,
    /// 1101: Attack then Hold
    /// Pattern: Attack 0→1, then Hold 1 forever
    AttackHold = 0x0D,
    /// 1110: Sawtooth-Down once then silence
    /// Pattern: Decay 1→0 once, then silence
    SawtoothDownOnce = 0x0E,
    /// 1111: Attack then Hold
    /// Pattern: Attack 0→1, then Hold 1 forever
    AttackHoldLong = 0x0F,
}

impl EnvelopeShape {
    /// Create from raw register value
    pub fn from_value(val: u8) -> Self {
        match val & 0x0F {
            0x00 => EnvelopeShape::AttackDecay,
            0x01 => EnvelopeShape::AttackDecay1,
            0x02 => EnvelopeShape::AttackDecay2,
            0x03 => EnvelopeShape::AttackDecay3,
            0x04 => EnvelopeShape::AttackDecayRelease,
            0x05 => EnvelopeShape::AttackSustainRelease,
            0x06 => EnvelopeShape::Triangle,
            0x07 => EnvelopeShape::TriangleSustain,
            0x08 => EnvelopeShape::SawtoothDown,
            0x09 => EnvelopeShape::AttackSawtoothDown,
            0x0A => EnvelopeShape::SustainSawtoothDown,
            0x0B => EnvelopeShape::AttackSustainSawtooth,
            0x0C => EnvelopeShape::SawtoothUp,
            0x0D => EnvelopeShape::AttackHold,
            0x0E => EnvelopeShape::SawtoothDownOnce,
            0x0F => EnvelopeShape::AttackHoldLong,
            _ => EnvelopeShape::AttackDecay,
        }
    }
}

impl fmt::Display for EnvelopeShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeShape::AttackDecay => write!(f, "Attack-Decay"),
            EnvelopeShape::AttackDecay1 => write!(f, "Attack-Decay (variant 1)"),
            EnvelopeShape::AttackDecay2 => write!(f, "Attack-Decay (variant 2)"),
            EnvelopeShape::AttackDecay3 => write!(f, "Attack-Decay (variant 3)"),
            EnvelopeShape::AttackDecayRelease => write!(f, "Attack-Decay-Release"),
            EnvelopeShape::AttackSustainRelease => write!(f, "Attack-Sustain-Release"),
            EnvelopeShape::Triangle => write!(f, "Triangle"),
            EnvelopeShape::TriangleSustain => write!(f, "Triangle-Sustain"),
            EnvelopeShape::SawtoothDown => write!(f, "Sawtooth-Down (Buzzer)"),
            EnvelopeShape::AttackSawtoothDown => write!(f, "Attack-Sawtooth-Down"),
            EnvelopeShape::SustainSawtoothDown => write!(f, "Sustain-Sawtooth-Down"),
            EnvelopeShape::AttackSustainSawtooth => write!(f, "Attack-Sustain-Sawtooth"),
            EnvelopeShape::SawtoothUp => write!(f, "Sawtooth-Up (Buzzer)"),
            EnvelopeShape::AttackHold => write!(f, "Attack-Hold"),
            EnvelopeShape::SawtoothDownOnce => write!(f, "Sawtooth-Down-Once"),
            EnvelopeShape::AttackHoldLong => write!(f, "Attack-Hold-Long"),
        }
    }
}

/// Hardware-accurate envelope patterns
/// Each pattern is 8 amplitude values representing the 4 phases of the envelope
/// Each value is a start→end amplitude pair that gets interpolated over 32 steps
const ENVELOPE_PATTERNS: [[u8; 8]; 11] = [
    // Pattern 1: Attack-Decay (0x00-0x03, 0x01, 0x09)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→0 (decay)
    //   Phase 2: 0→0 (silent)
    //   Phase 3: 0→0 (silent)
    [0, 1, 1, 0, 0, 0, 0, 0],
    // Pattern 2: Attack-Sustain-Release (0x05, 0x0A)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→1 (sustain)
    //   Phase 2: 1→0 (release)
    //   Phase 3: 0→0 (silent)
    [0, 1, 1, 1, 1, 0, 0, 0],
    // Pattern 3: Triangle (0x06)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→0 (decay)
    //   Phase 2: 0→0 (silent)
    //   Phase 3: 0→0 (silent)
    [0, 1, 1, 0, 0, 0, 0, 0],
    // Pattern 4: Triangle-Sustain (0x07)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→1 (sustain)
    //   Phase 2: 1→0 (decay)
    //   Phase 3: 0→0 (silent)
    [0, 1, 1, 1, 1, 0, 0, 0],
    // Pattern 5: Sawtooth-Down repeating (0x08) - BUZZER
    //   Phase 0: 1→0 (sawtooth down)
    //   Phase 1: 1→0 (sawtooth down)
    //   Phase 2: 1→0 (sawtooth down)
    //   Phase 3: 1→0 (sawtooth down)
    // Loops continuously: creates amplitude modulation 4 times per period
    [1, 0, 1, 0, 1, 0, 1, 0],
    // Pattern 6: Attack-Sawtooth-Down (0x09)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→0 (sawtooth down)
    //   Phase 2: 1→0 (sawtooth down)
    //   Phase 3: 1→0 (sawtooth down)
    [0, 1, 1, 0, 1, 0, 1, 0],
    // Pattern 7: Sustain-Sawtooth-Down (0x0A)
    //   Phase 0: 1→1 (sustain)
    //   Phase 1: 1→0 (sawtooth down)
    //   Phase 2: 1→0 (sawtooth down)
    //   Phase 3: 1→0 (sawtooth down)
    [1, 1, 1, 0, 1, 0, 1, 0],
    // Pattern 8: Attack-Sustain-Sawtooth (0x0B)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→1 (sustain)
    //   Phase 2: 1→0 (sawtooth down)
    //   Phase 3: 1→0 (sawtooth down)
    [0, 1, 1, 1, 1, 0, 1, 0],
    // Pattern 9: Sawtooth-Up repeating (0x0C) - BUZZER
    //   Phase 0: 0→1 (sawtooth up)
    //   Phase 1: 0→1 (sawtooth up)
    //   Phase 2: 0→1 (sawtooth up)
    //   Phase 3: 0→1 (sawtooth up)
    // Loops continuously: creates amplitude modulation 4 times per period
    [0, 1, 0, 1, 0, 1, 0, 1],
    // Pattern 9: Attack-Hold (0x0D, 0x0F)
    //   Phase 0: 0→1 (attack)
    //   Phase 1: 1→1 (hold)
    //   Phase 2: 1→1 (hold)
    //   Phase 3: 1→1 (hold)
    [0, 1, 1, 1, 1, 1, 1, 1],
    // Pattern 10: Sawtooth-Down Once (0x0E)
    //   Phase 0: 1→0 (single decay/sawtooth)
    //   Phase 1: 0→0 (silent)
    //   Phase 2: 0→0 (silent)
    //   Phase 3: 0→0 (silent)
    // Unlike pattern 4 (continuous sawtooth), this produces only one decay then holds at silence
    [1, 0, 0, 0, 0, 0, 0, 0],
];

/// Map envelope shapes to their pattern index
const SHAPE_TO_PATTERN: [usize; 16] = [
    0, 0, 0, 0, // 0x00-0x03: Attack-Decay
    1, 1, // 0x04-0x05: Attack-Sustain-Release (different names but same pattern)
    2, 3, // 0x06-0x07: Triangle / Triangle-Sustain
    4, 5, 6, 7,  // 0x08-0x0B: Various sawtooth patterns
    8,  // 0x0C: Sawtooth-Up (continuous buzzer)
    9,  // 0x0D: Attack-Hold
    10, // 0x0E: Sawtooth-Down Once (single pulse then silent)
    9,  // 0x0F: Attack-Hold-Long (same as 0x0D)
];

/// Pre-computed lookup table: [pattern][phase][step]
/// 11 patterns × 4 phases × 32 amplitude steps = 1408 values
///
/// # Structure
/// - 11 patterns: Maps to 16 envelope shapes via SHAPE_TO_PATTERN
/// - 4 phases: Phase data per pattern
///   * Phase 0: Initial pattern (executes once on trigger)
///   * Phase 1: Continuation pattern (loops forever via position overflow)
///   * Phases 2-3: Reserved (not used in normal operation, included for lookup table structure)
/// - 32 steps: Determined by using top 5 bits of 32-bit position accumulator (2^5 = 32)
struct EnvelopeLookup {
    /// Table[pattern_idx][phase][step] = normalized amplitude 0.0-1.0
    data: [[[f32; 32]; 4]; 11],
}

impl EnvelopeLookup {
    fn new() -> Self {
        let mut data = [[[0.0; 32]; 4]; 11];

        // Generate lookup table for each pattern
        for (pattern_idx, pattern) in ENVELOPE_PATTERNS.iter().enumerate() {
            // Each pattern has 4 phases (pairs of start→end values)
            // Pattern values are 0-1 normalized amplitude
            for phase in 0..4 {
                let start_val = pattern[phase * 2] as f32;
                let end_val = pattern[phase * 2 + 1] as f32;

                // Interpolate 32 values using top 5 bits of position accumulator
                // Step 0 = start_val, Step 31 = end_val
                let phase_slice = &mut data[pattern_idx][phase];
                for (step, slot) in phase_slice.iter_mut().enumerate() {
                    // Linear interpolation from start to end across 32 steps
                    let progress = (step as f32) / 31.0;
                    let amplitude = start_val + (end_val - start_val) * progress;
                    *slot = amplitude.clamp(0.0, 1.0);
                }
            }
        }

        EnvelopeLookup { data }
    }

    fn get(&self, pattern: usize, phase: usize, step: usize) -> f32 {
        if pattern < 11 && phase < 4 && step < 32 {
            self.data[pattern][phase][step]
        } else {
            0.0
        }
    }
}

/// Static lookup table - initialized once
static ENVELOPE_LOOKUP: std::sync::OnceLock<EnvelopeLookup> = std::sync::OnceLock::new();

fn get_envelope_lookup() -> &'static EnvelopeLookup {
    ENVELOPE_LOOKUP.get_or_init(EnvelopeLookup::new)
}

/// Envelope Generator
///
/// Uses a 32-bit position accumulator and phase advancement system for
/// cycle-accurate envelope generation. Each envelope shape cycles through 4 phases,
/// with amplitude interpolated at each position.
#[derive(Debug, Clone)]
pub struct EnvelopeGen {
    shape: EnvelopeShape,
    freq_period: u16, // Envelope frequency in internal clock units

    // 32-bit position accumulator (hardware-accurate timing model)
    position: u32, // Accumulator: advances by step each sample
    step: u32,     // Step value: calculated from freq_period
    phase: u8,     // Current phase: 0-3

    // Cached amplitude
    amplitude: f32,
}

impl EnvelopeGen {
    /// Create a new envelope generator
    pub fn new() -> Self {
        EnvelopeGen {
            shape: EnvelopeShape::AttackDecay,
            freq_period: 1,
            position: 0,
            step: 0xFFFFFFFF, // Fast by default
            phase: 0,
            amplitude: 0.0,
        }
    }

    /// Set the envelope shape (retriggers envelope)
    pub fn set_shape(&mut self, shape: EnvelopeShape) {
        self.shape = shape;
        self.trigger();
    }

    /// Set the envelope frequency register values (R11-R12)
    ///
    /// This sets the period but does NOT calculate the step.
    /// Call `compute_step(master_clock, sample_rate)` after this to update timing.
    pub fn set_frequency(&mut self, freq_lo: u8, freq_hi: u8) {
        self.freq_period = ((freq_hi as u16) << 8) | (freq_lo as u16);
        if self.freq_period == 0 {
            self.freq_period = 1;
        }
        // Note: Step is NOT computed here. It must be computed via compute_step()
        // which requires knowledge of the master clock frequency.
    }

    /// Compute the step value based on hardware clock timing
    ///
    /// Must be called after `set_frequency()` or when clock parameters change.
    ///
    /// # Hardware Formula
    /// YM2149 envelope frequency = Master_Clock / (256 × freq_period)
    ///
    /// # 32-bit Accumulator Formula
    /// For 32-bit position accumulator (32 steps per phase):
    /// step = 2^32 × (envelope_clock / sample_rate)
    /// step = 2^32 × Master_Clock / (256 × freq_period × sample_rate)
    ///
    /// # Example
    /// With freq_period=256, master_clock=2MHz, sample_rate=44.1kHz:
    /// - Envelope freq = 2000000 / (256 × 256) = 30.5 Hz
    /// - Full 4-phase cycle = 44100 / 30.5 ≈ 1446 samples
    /// - Phase 0 duration ≈ 362 samples (1 of 4 phases)
    /// - Phase 1 repeats every position overflow (~362 samples thereafter)
    /// - Step = 2^32 / 1446 ≈ 2,965,820
    ///
    /// This produces envelope timing that exactly matches hardware behavior.
    pub fn compute_step(&mut self, master_clock: u32, sample_rate: u32) {
        if self.freq_period == 0 {
            self.freq_period = 1;
        }

        // Use floating-point for precision (same as Channel::compute_step)
        let step_f64 =
            (master_clock as f64) / ((self.freq_period as f64) * 256.0 * (sample_rate as f64));
        let step_f64 = step_f64 * (65536.0 * 65536.0); // 2^32
        self.step = step_f64 as u32;
    }

    /// Trigger the envelope (start from phase 0)
    pub fn trigger(&mut self) {
        self.position = 0;
        self.phase = 0;
        // Update amplitude immediately at trigger
        self.update_amplitude();
    }

    /// Clock the envelope (advance one sample)
    pub fn clock(&mut self) {
        // Advance position accumulator
        let old_position = self.position;
        self.position = self.position.wrapping_add(self.step);

        // Check for phase overflow: position wraps around 0 (u32 overflow)
        // When position < old_position, it means we wrapped from near-MAX back to a small number
        //
        // Phase advancement (hardware-accurate behavior):
        //   - Phase 0: Initial pattern (executes once after trigger)
        //   - Phase 1: Continuation pattern (loops forever via position overflow)
        //
        // Hardware behavior: When phase 0 completes, advance to phase 1 and stay there
        if self.position < old_position && self.phase == 0 {
            // Position wrapped and we're in phase 0, advance to phase 1 (and stay there)
            self.phase = 1;
        }

        // Update amplitude from lookup table
        self.update_amplitude();
    }

    /// Update amplitude from lookup table based on current shape, phase, and position
    fn update_amplitude(&mut self) {
        let pattern_idx = SHAPE_TO_PATTERN[self.shape as usize];
        let step_idx = (self.position >> 27) as usize; // Top 5 bits select step 0-31

        let lookup = get_envelope_lookup();
        let normalized = lookup.get(pattern_idx, self.phase as usize, step_idx);
        let level = (normalized * 15.0).round() as u8;
        let level = level.min(15);
        self.amplitude = get_volume(level);
    }

    /// Get the current envelope amplitude (0.0 to 1.0)
    pub fn get_amplitude(&self) -> f32 {
        self.amplitude.clamp(0.0, 1.0)
    }
}

impl Default for EnvelopeGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ym2149::constants::get_volume;

    #[test]
    fn test_envelope_shape_creation() {
        assert_eq!(EnvelopeShape::from_value(0x00), EnvelopeShape::AttackDecay);
        assert_eq!(EnvelopeShape::from_value(0x0C), EnvelopeShape::SawtoothUp);
    }

    #[test]
    fn test_envelope_trigger() {
        let mut env = EnvelopeGen::new();
        env.set_frequency(0xFF, 0x00);
        env.compute_step(2_000_000, 44_100);
        env.set_shape(EnvelopeShape::AttackDecay);

        assert_eq!(env.phase, 0);
        let expected = get_volume(0);
        assert!(
            (env.amplitude - expected).abs() < 1e-6,
            "expected amplitude {}, got {}",
            expected,
            env.amplitude
        );
    }

    #[test]
    fn test_envelope_lookup_generation() {
        let lookup = EnvelopeLookup::new();
        // Pattern 8 is SawtoothUp: [0,1,0,1,0,1,0,1]
        // Phase 0: 0→1 interpolation
        let phase0_start = lookup.get(8, 0, 0); // Should be near 0
        let phase0_end = lookup.get(8, 0, 31); // Should be near 1

        eprintln!("Phase 0: start={}, end={}", phase0_start, phase0_end);

        assert!(
            phase0_start < 0.1,
            "phase0_start should be near 0, got {}",
            phase0_start
        );
        assert!(
            phase0_end > 0.9,
            "phase0_end should be near 1, got {}",
            phase0_end
        );
    }

    #[test]
    fn test_envelope_sawtooth_buzzer() {
        let mut env = EnvelopeGen::new();
        // freq_period = 0x0100 = 256
        // With 2MHz clock: envelope_freq = 2000000 / (256 * 256) = 30.5 Hz
        env.set_frequency(0x00, 0x01); // freq_period = 0x0100
        env.compute_step(2_000_000, 44_100);
        env.set_shape(EnvelopeShape::SawtoothUp);

        eprintln!(
            "Initial: phase={}, position={}, amplitude={}, step={}",
            env.phase, env.position, env.amplitude, env.step
        );

        // Sawtooth should cycle: 0→1, 0→1, 0→1, 0→1
        let mut min_amp = 1.0f32;
        let mut max_amp = 0.0f32;

        for i in 0..10000 {
            env.clock();
            min_amp = min_amp.min(env.amplitude);
            max_amp = max_amp.max(env.amplitude);

            if i < 10 || i % 2000 == 0 {
                eprintln!(
                    "Clock {}: phase={}, position={}, amplitude={}, min={}, max={}",
                    i, env.phase, env.position, env.amplitude, min_amp, max_amp
                );
            }
        }

        eprintln!("Final: min_amp={}, max_amp={}", min_amp, max_amp);

        // Should have seen both low and high amplitudes (sawtooth pattern)
        // Sawtooth goes 0→1, so min should be near 0 and max near 1
        let min_expected = get_volume(0);
        assert!(
            min_amp <= min_expected + 1e-4,
            "min_amp={}, expected <= {}",
            min_amp,
            min_expected
        );

        let max_expected = get_volume(15);
        assert!(
            max_amp >= max_expected * 0.9,
            "max_amp={}, expected >= {}",
            max_amp,
            max_expected * 0.9
        );
        assert!(
            max_amp <= max_expected + 5e-3,
            "max_amp={}, expected close to {}",
            max_amp,
            max_expected
        );
    }

    #[test]
    fn test_envelope_sawtooth_down_once() {
        let mut env = EnvelopeGen::new();
        // freq_period = 0x0100 for reasonable speed
        env.set_frequency(0x00, 0x01);
        env.compute_step(2_000_000, 44_100);
        env.set_shape(EnvelopeShape::SawtoothDownOnce);

        eprintln!(
            "Initial: phase={}, position={}, amplitude={}",
            env.phase, env.position, env.amplitude
        );

        // SawtoothDownOnce should: 1→0 once, then silence forever
        // Pattern: [1→0, 0→0, 0→0, 0→0]
        let mut observed_phases = std::collections::HashSet::new();

        for i in 0..10000 {
            env.clock();
            observed_phases.insert(env.phase);

            if i < 10 || i % 2000 == 0 {
                eprintln!(
                    "Clock {}: phase={}, position={}, amplitude={}",
                    i, env.phase, env.position, env.amplitude
                );
            }
        }

        eprintln!("Observed phases: {:?}", observed_phases);

        // Should only see phases 0 and 1 (correct behavior: 0→1 transition only)
        // Phase 0: 1→0 (single decay/sawtooth)
        // Phase 1: 0→0 (silence, repeats forever after phase 0)
        // Phases 2-3: never accessed (phase stays at 0→1 transition)
        assert!(
            observed_phases.contains(&0) && observed_phases.contains(&1),
            "Should observe phases 0 (attack) and 1 (silence), got {:?}",
            observed_phases
        );
        assert_eq!(
            observed_phases.len(),
            2,
            "Should only see 2 phases (0 and 1), not all 4, got {:?}",
            observed_phases
        );
    }

    #[test]
    fn test_envelope_timing_accuracy() {
        let mut env = EnvelopeGen::new();
        // freq_period = 256, master_clock = 2MHz
        // Envelope frequency = 2000000 / (256 * 256) = 30.5 Hz
        env.set_frequency(0x00, 0x01); // 256
        env.compute_step(2_000_000, 44_100);
        env.set_shape(EnvelopeShape::SawtoothUp);

        // Expected: one full cycle (4 phases) should take approximately:
        // 1 / 30.5 Hz = 32.8 ms = 1447 samples at 44.1 kHz
        // Each phase: ~362 samples

        let mut phase_changes = 0;
        let mut samples_since_last_phase_change = 0;

        for i in 0..10000 {
            let old_phase = env.phase;
            env.clock();
            samples_since_last_phase_change += 1;

            if env.phase != old_phase {
                eprintln!(
                    "Phase change at sample {}: {} → {}, {} samples per phase",
                    i, old_phase, env.phase, samples_since_last_phase_change
                );
                phase_changes += 1;
                samples_since_last_phase_change = 0;
            }
        }

        eprintln!("Total phase changes in 10000 samples: {}", phase_changes);

        // With correct phase behavior (0→1 transition only):
        // We should see exactly 1 phase change (0 → 1) at ~1445 samples
        // (30.5 Hz means one complete 4-phase cycle takes ~1445 samples)
        // Phase 0 executes once during this cycle, then phase 1 repeats forever
        assert_eq!(
            phase_changes, 1,
            "With correct phase behavior (0→1 only), should see exactly one phase change, got {}",
            phase_changes
        );
    }

    #[test]
    fn test_envelope_master_clock_sensitivity() {
        let master_clock_fast = 2_000_000u32;
        let master_clock_slow = 1_000_000u32;
        let sample_rate = 44_100u32;

        let mut env_fast = EnvelopeGen::new();
        env_fast.set_frequency(0x00, 0x01); // freq_period = 256
        env_fast.compute_step(master_clock_fast, sample_rate);

        let mut env_slow = EnvelopeGen::new();
        env_slow.set_frequency(0x00, 0x01); // freq_period = 256
        env_slow.compute_step(master_clock_slow, sample_rate);

        eprintln!("Fast (2MHz) step: {}", env_fast.step);
        eprintln!("Slow (1MHz) step: {}", env_slow.step);

        // Half master clock should produce half the step value
        let ratio = env_fast.step as f64 / env_slow.step as f64;
        eprintln!("Ratio: {}", ratio);

        assert!(
            (ratio - 2.0).abs() < 0.01,
            "Step should double with double clock speed, got ratio {}",
            ratio
        );
    }
}
