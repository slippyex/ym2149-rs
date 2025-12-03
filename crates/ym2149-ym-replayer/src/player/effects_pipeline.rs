use std::sync::Arc;

use super::effects_manager::EffectsManager;
use ym2149::Ym2149Backend;

/// High-level wrapper around `EffectsManager` that also tracks per-voice metadata
/// (SID/DigiDrum active flags, last drum index/frequency) for consumers such as
/// metadata queries and replay heuristics.
pub struct EffectsPipeline {
    manager: EffectsManager,
    sid_active: [bool; 3],
    drum_active: [bool; 3],
    last_drum_index: [Option<u8>; 3],
    last_drum_freq: [u32; 3],
}

impl EffectsPipeline {
    /// Create a pipeline with the desired sample rate.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            manager: EffectsManager::new(sample_rate),
            sid_active: [false; 3],
            drum_active: [false; 3],
            last_drum_index: [None; 3],
            last_drum_freq: [0; 3],
        }
    }

    /// Reset internal state while preserving sample rate.
    pub fn reset(&mut self) {
        self.manager.reset();
        self.sid_active = [false; 3];
        self.drum_active = [false; 3];
        self.last_drum_index = [None; 3];
        self.last_drum_freq = [0; 3];
    }

    /// Recreate the manager with a new sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.manager = EffectsManager::new(sample_rate);
        self.sid_active = [false; 3];
        self.drum_active = [false; 3];
        self.last_drum_index = [None; 3];
        self.last_drum_freq = [0; 3];
    }

    /// Tick all active effects (call before `chip.clock()`).
    pub fn tick<B: Ym2149Backend>(&mut self, chip: &mut B) {
        self.manager.tick(chip);
    }

    /// Start sync buzzer at the specified timer frequency.
    pub fn sync_buzzer_start(&mut self, freq: u32) {
        self.manager.sync_buzzer_start(freq);
    }

    /// Stop sync buzzer.
    pub fn sync_buzzer_stop(&mut self) {
        self.manager.sync_buzzer_stop();
    }

    /// Whether sync buzzer currently enabled.
    pub fn sync_buzzer_is_enabled(&self) -> bool {
        self.manager.sync_buzzer_is_enabled()
    }

    /// Start SID square gating on a voice.
    pub fn sid_start(&mut self, voice: usize, timer_freq: u32, vol: u8) {
        self.manager.sid_start(voice, timer_freq, vol);
        if voice < self.sid_active.len() {
            self.sid_active[voice] = true;
        }
    }

    /// Start SID sinus gating on a voice.
    pub fn sid_sin_start(&mut self, voice: usize, timer_freq: u32, vol: u8) {
        self.manager.sid_sin_start(voice, timer_freq, vol);
        if voice < self.sid_active.len() {
            self.sid_active[voice] = true;
        }
    }

    /// Stop SID gating on a voice.
    pub fn sid_stop(&mut self, voice: usize) {
        self.manager.sid_stop(voice);
        if voice < self.sid_active.len() {
            self.sid_active[voice] = false;
        }
    }

    /// Whether SID gating active on a voice.
    pub fn is_sid_active(&self, voice: usize) -> bool {
        self.sid_active.get(voice).copied().unwrap_or(false)
    }

    /// Start a DigiDrum on a voice, tracking the drum index/frequency.
    ///
    /// Takes an `Arc<[u8]>` to avoid cloning sample data in the hot path.
    pub fn digidrum_start(
        &mut self,
        voice: usize,
        drum_index: Option<u8>,
        freq: u32,
        sample: Arc<[u8]>,
    ) {
        self.manager.digidrum_start(voice, sample, freq);
        if voice < self.drum_active.len() {
            self.drum_active[voice] = true;
            self.last_drum_index[voice] = drum_index;
            self.last_drum_freq[voice] = freq;
        }
    }

    /// Stop DigiDrum on a voice.
    pub fn digidrum_stop(&mut self, voice: usize) {
        self.manager.digidrum_stop(voice);
        if voice < self.drum_active.len() {
            self.drum_active[voice] = false;
            self.last_drum_index[voice] = None;
            self.last_drum_freq[voice] = 0;
        }
    }

    /// Whether DigiDrum active on a voice.
    pub fn is_drum_active(&self, voice: usize) -> bool {
        self.drum_active.get(voice).copied().unwrap_or(false)
    }

    /// Get last played drum signature (index, freq) if available.
    pub fn drum_signature(&self, voice: usize) -> Option<(u8, u32)> {
        self.last_drum_index
            .get(voice)
            .and_then(|idx| idx.map(|i| (i, self.last_drum_freq[voice])))
    }

    /// Surface effect status for visualization.
    pub fn effect_flags(&self) -> (bool, [bool; 3], [bool; 3]) {
        (
            self.manager.sync_buzzer_is_enabled(),
            self.sid_active,
            self.drum_active,
        )
    }
}
