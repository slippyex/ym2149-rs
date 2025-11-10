//! Audio Sample Generation Hot Path
//!
//! This module contains the performance-critical audio rendering logic,
//! including frame register loading, effect application, and sample generation.

use super::PlaybackState;
use super::madmax_digidrums::MADMAX_SAMPLE_RATE_BASE;
use super::ym_player::Ym6PlayerGeneric;
use crate::parser::effects::{EffectCommand, decode_effects_ym5};
use ym2149::Ym2149Backend;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Generate the next sample and advance playback
    pub fn generate_sample(&mut self) -> f32 {
        if self.state != PlaybackState::Playing {
            return 0.0;
        }

        if self.is_tracker_mode {
            return self.generate_tracker_sample();
        }

        if self.frames.is_empty() {
            return 0.0;
        }

        // Load registers for current frame (once per frame)
        if self.samples_in_frame == 0 {
            self.load_frame_registers();
        }

        // Update effects before clocking chip
        self.effects.tick(&mut self.chip);

        // Generate sample
        self.chip.clock();
        let sample = self.chip.get_sample();

        // Advance frame counter
        self.advance_frame();

        sample
    }

    /// Load and apply register values for the current frame
    pub(in crate::player) fn load_frame_registers(&mut self) {
        let frame_to_load = self.current_frame;
        // Clone the frame data to avoid borrow checker issues
        let regs = self.frames[frame_to_load];

        if self.is_ym2_mode {
            self.load_ym2_frame(&regs);
        } else {
            self.load_ymx_frame(&regs);
        }
    }

    /// Load YM2 (Mad Max) frame with special drum handling
    pub(in crate::player) fn load_ym2_frame(&mut self, regs: &[u8; 16]) {
        // Reset effect state that is not used in YM2 playback
        self.effects.sync_buzzer_stop();
        for voice in 0..3 {
            if self.sid_active[voice] {
                self.effects.sid_stop(voice);
                self.sid_active[voice] = false;
            }
            if voice != 2 && self.drum_active[voice] {
                self.effects.digidrum_stop(voice);
                self.drum_active[voice] = false;
            }
        }

        // Write registers 0-10
        for (reg_idx, &val) in regs.iter().enumerate().take(11) {
            self.chip.write_register(reg_idx as u8, val);
        }

        // YM2 (Mad Max): if R13 != 0xFF, force envelope (R11), set R12=0 and R13=0x0A
        if regs[13] != 0xFF {
            self.chip.write_register(11, regs[11]);
            self.chip.write_register(12, 0);
            self.chip.write_register(13, 0x0A);
        }

        // Handle Mad Max DigiDrum on channel C
        if (regs[10] & 0x80) != 0 {
            let mixer = self.chip.read_register(0x07) | 0x24;
            self.chip.write_register(0x07, mixer);

            let sample_idx = (regs[10] & 0x7F) as usize;
            if let Some(sample) = self.digidrums.get(sample_idx).cloned() {
                let timer = regs[12] as u32;
                if timer > 0 {
                    let freq = (MADMAX_SAMPLE_RATE_BASE / 4) / timer;
                    if freq > 0 {
                        self.effects.digidrum_start(2, sample, freq);
                        self.drum_active[2] = true;
                        self.active_drum_index[2] = Some(sample_idx as u8);
                        self.active_drum_freq[2] = freq;
                    }
                }
            }
        } else if self.drum_active[2] {
            self.effects.digidrum_stop(2);
            self.drum_active[2] = false;
            self.active_drum_index[2] = None;
            self.active_drum_freq[2] = 0;
        }
    }

    /// Load YM5/YM6 frame with advanced effect support
    pub(in crate::player) fn load_ymx_frame(&mut self, regs: &[u8; 16]) {
        // Write all registers; only gate R13 by sentinel 0xFF
        for r in 0u8..=15u8 {
            if r == 13 {
                let shape = regs[13];
                if shape != 0xFF {
                    self.chip.write_register(13, shape);
                }
            } else {
                self.chip.write_register(r, regs[r as usize]);
            }
        }

        // Decode effects based on format
        let cmds = self.decode_frame_effects(regs);

        // Apply effect commands
        self.apply_effect_intents(&cmds, regs);
    }

    /// Decode effect commands from frame registers
    pub(in crate::player) fn decode_frame_effects(&self, regs: &[u8; 16]) -> Vec<EffectCommand> {
        if self.is_ym5_mode {
            decode_effects_ym5(regs)
        } else if self.is_ym6_mode {
            self.fx_decoder.decode_effects(regs).to_vec()
        } else {
            Vec::new()
        }
    }

    /// Apply decoded effect commands to the effects manager
    pub(in crate::player) fn apply_effect_intents(
        &mut self,
        cmds: &[EffectCommand],
        regs: &[u8; 16],
    ) {
        // Aggregate per-voice intents
        let mut sid_intent: [Option<(u32, u8)>; 3] = [None, None, None];
        let mut sid_sin_intent: [Option<(u32, u8)>; 3] = [None, None, None];
        let mut drum_intent: [Option<(u8, u32)>; 3] = [None, None, None];
        let mut sync_intent: Option<(u32, u8)> = None;

        for cmd in cmds.iter() {
            match *cmd {
                EffectCommand::None => {}
                EffectCommand::SidStart {
                    voice,
                    freq,
                    volume,
                } => {
                    if (voice as usize) < 3 {
                        sid_intent[voice as usize] = Some((freq, volume));
                    }
                }
                EffectCommand::SinusSidStart {
                    voice,
                    freq,
                    volume,
                } => {
                    if (voice as usize) < 3 {
                        sid_sin_intent[voice as usize] = Some((freq, volume));
                    }
                }
                EffectCommand::DigiDrumStart {
                    voice,
                    drum_num,
                    freq,
                } => {
                    if (voice as usize) < 3 {
                        drum_intent[voice as usize] = Some((drum_num, freq));
                    }
                }
                EffectCommand::SyncBuzzerStart { freq, env_shape } => {
                    sync_intent = Some((freq, env_shape));
                }
            }
        }

        // Apply Sync Buzzer
        self.apply_sync_buzzer_intent(sync_intent, regs);

        // Apply per-voice effects
        self.apply_voice_effects(sid_intent, sid_sin_intent, drum_intent);
    }

    /// Apply sync buzzer effect intent
    pub(in crate::player) fn apply_sync_buzzer_intent(
        &mut self,
        sync_intent: Option<(u32, u8)>,
        regs: &[u8; 16],
    ) {
        if let Some((freq, env_shape)) = sync_intent {
            if !self.effects.sync_buzzer_is_enabled() {
                // Respect YM6 sentinel: if R13==0xFF, do not change the shape
                if regs[13] != 0xFF {
                    self.chip.write_register(0x0D, env_shape & 0x0F);
                }
                self.effects.sync_buzzer_start(freq);
            }
        } else if self.effects.sync_buzzer_is_enabled() {
            self.effects.sync_buzzer_stop();
        }
    }

    /// Apply per-voice SID and DigiDrum effects
    pub(in crate::player) fn apply_voice_effects(
        &mut self,
        sid_intent: [Option<(u32, u8)>; 3],
        sid_sin_intent: [Option<(u32, u8)>; 3],
        drum_intent: [Option<(u8, u32)>; 3],
    ) {
        for voice in 0..3 {
            // Handle DigiDrum
            if let Some((drum_idx, freq)) = drum_intent[voice] {
                if let Some(sample) = self.digidrums.get(drum_idx as usize) {
                    let should_restart = !self.drum_active[voice]
                        || self.active_drum_index[voice] != Some(drum_idx)
                        || self.active_drum_freq[voice] != freq;
                    if should_restart {
                        self.effects.digidrum_start(voice, sample.clone(), freq);
                        self.drum_active[voice] = true;
                        self.active_drum_index[voice] = Some(drum_idx);
                        self.active_drum_freq[voice] = freq;
                    }
                }
            } else if self.drum_active[voice] {
                self.effects.digidrum_stop(voice);
                self.drum_active[voice] = false;
                self.active_drum_index[voice] = None;
                self.active_drum_freq[voice] = 0;
            }

            // Handle SID
            if let Some((freq, volume)) = sid_sin_intent[voice] {
                self.effects.sid_sin_start(voice, freq, volume);
                self.sid_active[voice] = true;
            } else if let Some((freq, volume)) = sid_intent[voice] {
                self.effects.sid_start(voice, freq, volume);
                self.sid_active[voice] = true;
            } else if self.sid_active[voice] {
                self.effects.sid_stop(voice);
                self.sid_active[voice] = false;
            }
        }
    }

    /// Generate a block of samples (allocates new Vec)
    ///
    /// For performance-critical code, prefer [`Self::generate_samples_into`] to avoid allocations.
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.generate_sample());
        }
        samples
    }

    /// Generate samples into a pre-allocated buffer (zero-allocation hot path)
    ///
    /// This method fills the provided buffer without allocating, making it suitable
    /// for audio threads that need predictable performance.
    ///
    /// # Example
    /// ```no_run
    /// # use ym_replayer::Ym6Player;
    /// # let mut player = Ym6Player::new();
    /// let mut buffer = vec![0.0f32; 882]; // Reusable buffer
    /// player.generate_samples_into(&mut buffer);
    /// // Use buffer...
    /// player.generate_samples_into(&mut buffer); // Reuse same buffer
    /// ```
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.generate_sample();
        }
    }

    pub(in crate::player) fn generate_tracker_sample(&mut self) -> f32 {
        let tracker = match self.tracker.as_mut() {
            Some(state) => state,
            None => return 0.0,
        };

        if tracker.samples_per_step <= 0.0 {
            return 0.0;
        }

        while tracker.samples_until_update <= 0.0 {
            if !tracker.advance_frame() {
                self.state = PlaybackState::Stopped;
                return 0.0;
            }
            tracker.samples_until_update += tracker.samples_per_step;
        }

        let sample = tracker.mix_sample();
        tracker.samples_until_update -= 1.0;
        sample
    }
}
