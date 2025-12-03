//! YM-file format Special Effects Manager
//!
//! Implements Atari ST software effects by manipulating YM2149 registers.
//! These are NOT hardware features but playback techniques encoded in YM6 files.
//!
//! Effects are managed separately from the core PSG emulation to maintain clean separation
//! of concerns: the chip is pure hardware emulation, effects are format-specific playback tricks.

use std::sync::Arc;
use ym2149::Ym2149Backend;

const DRUM_PREC: u32 = 15;

/// Waveform modes for SID-style amplitude gating
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidMode {
    /// Square wave gating (amplitude on/off based on bit 31)
    Square,
    /// Sinusoidal gating (smooth amplitude modulation)
    Sinus,
}

/// Per-voice SID state for amplitude gating
#[derive(Debug, Clone)]
struct SidState {
    /// Whether this SID voice is currently active
    active: bool,
    /// Phase accumulator for gating (32-bit fixed-point)
    pos: u32,
    /// Phase increment per sample
    step: u32,
    /// Maximum volume for this voice (0-15)
    vol: u8,
    /// Gating waveform mode
    mode: SidMode,
}

impl Default for SidState {
    fn default() -> Self {
        Self {
            active: false,
            pos: 0,
            step: 0,
            vol: 0,
            mode: SidMode::Square,
        }
    }
}

/// Per-voice DigiDrum state for sample playback
#[derive(Debug, Clone)]
struct DrumState {
    /// Whether this DigiDrum is currently playing
    active: bool,
    /// Sample data (8-bit unsigned, shared to avoid cloning)
    data: Arc<[u8]>,
    /// Playback position (fixed-point, DRUM_PREC bits of precision)
    pos: u32,
    /// Playback speed (position increment per sample)
    step: u32,
}

impl Default for DrumState {
    fn default() -> Self {
        Self {
            active: false,
            data: Arc::from([]),
            pos: 0,
            step: 0,
        }
    }
}

impl DrumState {
    /// Get current sample value scaled for better bass presence (8-bit × 255 / 3)
    /// Note: STSound reference uses /6, but /3 provides better punch and bass
    fn current_sample(&self) -> Option<i32> {
        if !self.active {
            return None;
        }
        let idx = (self.pos >> DRUM_PREC) as usize;
        if idx >= self.data.len() {
            return None;
        }
        let sample = self.data[idx] as i32;
        // Changed from /6 to /3 to give samples more amplitude and bass presence
        Some((sample * 255) / 3)
    }

    /// Advance position and return whether playback is still active
    fn advance(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.pos = self.pos.wrapping_add(self.step);
        ((self.pos >> DRUM_PREC) as usize) < self.data.len()
    }
}

/// Manages YM-file format special effects by controlling chip state
///
/// Effects work by:
/// 1. Maintaining per-effect state (phase accumulators, sample data)
/// 2. Calling `tick()` each sample to update effect logic
/// 3. `tick()` calls chip methods like `write_register()` and `trigger_envelope()`
/// 4. After effects tick, the chip is clocked normally
///
/// This maintains the same timing behavior as the original implementation while
/// cleanly separating effect logic from core PSG emulation.
pub struct EffectsManager {
    /// Audio sample rate (used for step calculations)
    sample_rate: u32,

    // === Sync Buzzer Effect ===
    /// Phase accumulator (32-bit fixed-point)
    /// Overflows at bit 31 to trigger envelope
    sync_buzzer_phase: u32,
    /// Phase increment per sample
    sync_buzzer_step: u32,
    /// Whether Sync Buzzer is currently active
    sync_buzzer_enabled: bool,

    // === SID Gating Effect ===
    /// Per-voice SID state
    sid: [SidState; 3],

    // === DigiDrum Effect ===
    /// Per-voice DigiDrum state
    drum: [DrumState; 3],

    // === Mixer Overrides ===
    /// Force tone output for DigiDrum (bypasses mixer gate)
    force_tone: [bool; 3],
    /// Force noise mute for DigiDrum (bypasses mixer gate)
    force_noise_mute: [bool; 3],
}

impl EffectsManager {
    /// Create a new effects manager
    pub fn new(sample_rate: u32) -> Self {
        EffectsManager {
            sample_rate,
            sync_buzzer_phase: 0,
            sync_buzzer_step: 0,
            sync_buzzer_enabled: false,
            sid: [
                SidState::default(),
                SidState::default(),
                SidState::default(),
            ],
            drum: [
                DrumState::default(),
                DrumState::default(),
                DrumState::default(),
            ],
            force_tone: [false; 3],
            force_noise_mute: [false; 3],
        }
    }

    /// Reset all effects to initial state
    pub fn reset(&mut self) {
        self.sync_buzzer_phase = 0;
        self.sync_buzzer_step = 0;
        self.sync_buzzer_enabled = false;
        for i in 0..3 {
            self.sid[i] = SidState::default();
            self.drum[i] = DrumState::default();
            self.force_tone[i] = false;
            self.force_noise_mute[i] = false;
        }
    }

    // ================================================================================
    // SYNC BUZZER EFFECT
    // ================================================================================

    /// Start Sync Buzzer with specified timer frequency
    pub fn sync_buzzer_start(&mut self, timer_freq: u32) {
        // Hardware-accurate formula: syncBuzzerStep = timerFreq * ((1<<31) / replayFrequency)
        let step = if self.sample_rate > 0 {
            ((timer_freq as u64) << 31) / (self.sample_rate as u64)
        } else {
            0
        };

        self.sync_buzzer_step = step as u32;
        self.sync_buzzer_phase = 0;
        self.sync_buzzer_enabled = true;
    }

    /// Stop Sync Buzzer effect
    pub fn sync_buzzer_stop(&mut self) {
        self.sync_buzzer_enabled = false;
        self.sync_buzzer_phase = 0;
        self.sync_buzzer_step = 0;
    }

    /// Check if Sync Buzzer is currently active
    pub fn sync_buzzer_is_enabled(&self) -> bool {
        self.sync_buzzer_enabled
    }

    // ================================================================================
    // SID EFFECT (AMPLITUDE GATING)
    // ================================================================================

    /// Start SID-style square wave gating on a voice
    pub fn sid_start(&mut self, voice: usize, timer_freq: u32, vol: u8) {
        if voice >= 3 {
            return;
        }
        let step = if self.sample_rate > 0 {
            ((timer_freq as u64) << 31) / (self.sample_rate as u64)
        } else {
            0
        } as u32;
        self.sid[voice].vol = vol & 0x0F;
        self.sid[voice].step = step;
        // Do not reset pos if already active to avoid phase pops
        if !self.sid[voice].active {
            self.sid[voice].pos = 0;
        }
        self.sid[voice].mode = SidMode::Square;
        self.sid[voice].active = true;
    }

    /// Stop SID gating on a voice
    pub fn sid_stop(&mut self, voice: usize) {
        if voice >= 3 {
            return;
        }
        self.sid[voice] = SidState::default();
    }

    /// Start SID-style sinusoidal amplitude modulation on a voice
    pub fn sid_sin_start(&mut self, voice: usize, timer_freq: u32, vol: u8) {
        if voice >= 3 {
            return;
        }
        let step = if self.sample_rate > 0 {
            ((timer_freq as u64) << 31) / (self.sample_rate as u64)
        } else {
            0
        } as u32;
        self.sid[voice].vol = vol & 0x0F;
        self.sid[voice].step = step;
        if !self.sid[voice].active {
            self.sid[voice].pos = 0;
        }
        self.sid[voice].mode = SidMode::Sinus;
        self.sid[voice].active = true;
    }

    // ================================================================================
    // DIGIDRUM EFFECT (SAMPLE PLAYBACK)
    // ================================================================================

    /// Start DigiDrum sample playback on a voice
    ///
    /// Takes an `Arc<[u8]>` to avoid cloning sample data in the hot path.
    pub fn digidrum_start(&mut self, voice: usize, sample: Arc<[u8]>, freq: u32) {
        if voice >= 3 {
            return;
        }
        let st = DrumState {
            active: true,
            data: sample,
            pos: 0,
            step: (((freq as u64) << DRUM_PREC) / (self.sample_rate as u64)) as u32,
        };
        self.drum[voice] = st;
        // Force tone include and noise mute for this voice (hardware-compatible behavior)
        self.force_tone[voice] = true;
        self.force_noise_mute[voice] = true;
    }

    /// Stop DigiDrum on a voice
    pub fn digidrum_stop(&mut self, voice: usize) {
        if voice >= 3 {
            return;
        }
        self.drum[voice] = DrumState::default();
        self.force_tone[voice] = false;
        self.force_noise_mute[voice] = false;
    }

    // ================================================================================
    // EFFECT TICK - MUST BE CALLED BEFORE chip.clock()
    // ================================================================================

    /// Update all active effects and write necessary register changes to chip
    ///
    /// This must be called BEFORE `chip.clock()` so that register changes take effect
    /// in the current sample cycle.
    ///
    /// Note: Hardware-specific features (Sync Buzzer, DigiDrums) will only work with
    /// backends that implement them (like Ym2149). Other backends will ignore these effects.
    pub fn tick<B: Ym2149Backend>(&mut self, chip: &mut B) {
        // Handle Sync Buzzer effect (timer-based envelope retriggering)
        if self.sync_buzzer_enabled {
            self.sync_buzzer_phase = self.sync_buzzer_phase.wrapping_add(self.sync_buzzer_step);
            // When bit 31 overflows, retrigger the envelope
            if self.sync_buzzer_phase & 0x80000000 != 0 {
                chip.trigger_envelope();
                self.sync_buzzer_phase &= 0x7fffffff; // Clear bit 31
            }
        }

        // Apply SID per-voice gating (square or sinus) by writing amplitude register
        for voice in 0..3 {
            if self.sid[voice].active {
                let vol_idx: u8 = match self.sid[voice].mode {
                    SidMode::Square => {
                        let gate_on = (self.sid[voice].pos & 0x8000_0000) != 0;
                        if gate_on {
                            self.sid[voice].vol & 0x0F
                        } else {
                            0
                        }
                    }
                    SidMode::Sinus => {
                        // Compute sinusoidal amplitude in [0..vol]
                        let phase = (self.sid[voice].pos as f32)
                            * (std::f32::consts::TAU / (u32::MAX as f32));
                        let s = 0.5f32 * (1.0 + phase.sin());
                        let a = (s * (self.sid[voice].vol as f32)).round() as i32;
                        a.clamp(0, 15) as u8
                    }
                };
                chip.write_register(0x08 + voice as u8, vol_idx);
                self.sid[voice].pos = self.sid[voice].pos.wrapping_add(self.sid[voice].step);
            }
        }

        // Handle DigiDrum: inject samples and manage mixer overrides
        for voice in 0..3 {
            if self.drum[voice].active {
                // Inject current drum sample into chip
                if let Some(sample) = self.drum[voice].current_sample() {
                    chip.set_drum_sample_override(voice, Some(sample as f32));
                } else {
                    chip.set_drum_sample_override(voice, None);
                }

                // Advance position and stop if end of sample
                if !self.drum[voice].advance() {
                    self.drum[voice].active = false;
                    chip.set_drum_sample_override(voice, None);
                    self.force_tone[voice] = false;
                    self.force_noise_mute[voice] = false;
                }
            } else {
                chip.set_drum_sample_override(voice, None);
            }
        }

        // Apply mixer overrides to chip
        chip.set_mixer_overrides(self.force_tone, self.force_noise_mute);
    }
}
