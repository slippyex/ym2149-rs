// Allow clippy lints that conflict with the 68000 assembly port structure.
// This code intentionally mirrors the original gistdrvr.s control flow.
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_clamp)]

//! GIST Sound Driver - Cycle-accurate port from gistdrvr.s
//!
//! # Overview
//!
//! The GIST (Graphics, Images, Sound, Text) sound driver is a multi-voice sound
//! effect driver for the YM2149 PSG chip. It was originally developed by Dave Becker
//! for Antic Software on the Atari ST.
//!
//! This driver manages 3 voices (corresponding to the 3 channels of the YM2149) and
//! handles automatic priority-based voice allocation, volume/frequency/noise envelopes,
//! and LFO modulation.
//!
//! # API Functions
//!
//! ## `snd_on` - Start playing a sound
//!
//! Starts a sound effect on an available voice.
//!
//! **Parameters:**
//! - `chip`: Reference to the YM2149 chip
//! - `sound`: Reference to the [`GistSound`] to play
//! - `requested_voice`: Optional voice index (0, 1, or 2). If `None`, the driver
//!   automatically selects a voice based on availability and priority.
//! - `volume`: Optional volume override (0-15). If `None`, uses the sound's default volume.
//! - `pitch`: Pitch value using MIDI note numbers:
//!   - 60 = Middle C (C4)
//!   - 24-108 = Valid range (2 octaves below to 4 octaves above middle C)
//!   - Values outside this range are octave-wrapped
//!   - Use -1 to disable pitch override (use sound's default frequency)
//! - `priority`: Priority level (0-32767). Higher priority sounds can interrupt
//!   lower priority sounds when all voices are busy.
//!
//! **Returns:** The voice index (0-2) that the sound was started on, or `None` if
//! no voice was available (all voices busy with higher priority sounds).
//!
//! ## `snd_off` - Release a sound
//!
//! Moves a sound into its release phase. The sound will continue to play through
//! its release envelope before stopping naturally. The voice's priority is set to
//! zero, allowing other sounds to use the voice.
//!
//! This is the "graceful" way to stop a sound - it allows fade-outs and release
//! envelopes to complete.
//!
//! **Parameters:**
//! - `voice_idx`: The voice index (0, 1, or 2) to release
//!
//! ## `stop_all` - Immediately stop all sounds
//!
//! Immediately stops all sounds on all voices. Unlike `snd_off`, this does not
//! allow release envelopes to complete - sounds are cut off immediately and
//! all voice volumes are set to zero.
//!
//! ## `tick` - Process one driver tick
//!
//! Must be called 200 times per second (every 5ms) to match the original
//! Atari ST Timer C interrupt rate. This function:
//! - Updates all envelope phases (attack, decay, sustain, release)
//! - Processes LFO modulation for volume, frequency, and noise
//! - Writes updated values to the YM2149 chip registers
//! - Handles sound duration countdown and automatic voice release
//!
//! ## `is_playing` - Check if any sound is active
//!
//! Returns `true` if any voice is currently playing a sound.
//!
//! # Technical Details
//!
//! Critical 68000 semantics that must be preserved:
//! - MULS.W uses only the low 16 bits of both operands
//! - SWAP exchanges high and low words of a 32-bit register
//! - ASR.L is arithmetic (sign-extending) shift right
//! - CMP.L uses signed comparison (BGT, BLT, BGE, BLE)
//! - OR.W at offset 26 reads the HIGH word of the long at that offset

use ym2149::Ym2149;

use super::gist_sound::GistSound;

const NUM_VOICES: usize = 3;

const YM_FREQS: [u16; 85] = [
    3822, 3608, 3405, 3214, 3034, 2863, 2703, 2551, 2408, 2273, 2145, 2025, 1911, 1804, 1703, 1607,
    1517, 1432, 1351, 1276, 1204, 1136, 1073, 1012, 956, 902, 851, 804, 758, 716, 676, 638, 602,
    568, 536, 506, 478, 451, 426, 402, 379, 358, 338, 319, 301, 284, 268, 253, 239, 225, 213, 201,
    190, 179, 169, 159, 150, 142, 134, 127, 119, 113, 106, 100, 95, 89, 84, 80, 75, 71, 67, 63, 60,
    56, 53, 50, 47, 45, 42, 40, 38, 36, 34, 32, 30,
];

const DIV_15: [i16; 16] = [
    0, 18, 35, 52, 69, 86, 103, 120, 137, 154, 171, 188, 205, 222, 239, 256,
];

const MIXER_MASK: [u8; 3] = [0xf6, 0xed, 0xdb];

pub struct GistDriver {
    voices: [super::voice::Voice; NUM_VOICES],
    mixer: u8,
    tick_count: u32,
    debug: bool,
}

impl Default for GistDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl GistDriver {
    /// Creates a new GIST sound driver instance.
    ///
    /// The driver is initialized with:
    /// - All 3 voices inactive
    /// - Mixer set to disable all tone and noise channels (0x3F)
    /// - Debug mode disabled
    ///
    /// # Example
    ///
    /// ```
    /// use ym2149_gist_replayer::GistDriver;
    ///
    /// let driver = GistDriver::new();
    /// ```
    pub fn new() -> Self {
        Self {
            voices: Default::default(),
            mixer: 0x3f,
            debug: false,
            tick_count: 0,
        }
    }

    /// Enables or disables debug output for the driver.
    ///
    /// When enabled, the driver will print diagnostic information about
    /// envelope phases and voice state transitions to stdout.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` to enable debug output, `false` to disable
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug = enabled;
    }

    /// Returns `true` if any voice is currently playing a sound.
    ///
    /// A voice is considered "playing" if its `inuse` counter is non-zero,
    /// which includes sounds in their attack, decay, sustain, or release phases.
    ///
    /// # Example
    ///
    /// ```
    /// use ym2149_gist_replayer::GistDriver;
    ///
    /// let driver = GistDriver::new();
    /// assert!(!driver.is_playing()); // No sounds playing initially
    /// ```
    pub fn is_playing(&self) -> bool {
        self.voices.iter().any(|v| v.inuse != 0)
    }

    /// Immediately stops all sounds on all voices.
    ///
    /// Unlike [`snd_off`](Self::snd_off), this does not allow release envelopes
    /// to complete - sounds are cut off immediately. This function:
    ///
    /// - Sets all voice `inuse` counters to 0
    /// - Clears all voice priorities
    /// - Sets all voice volumes to 0 on the YM2149
    /// - Disables all tone and noise channels in the mixer
    ///
    /// # Arguments
    ///
    /// * `chip` - Mutable reference to the YM2149 chip instance
    ///
    /// # Example
    ///
    /// ```
    /// use ym2149::Ym2149;
    /// use ym2149_gist_replayer::GistDriver;
    ///
    /// let mut chip = Ym2149::default();
    /// let mut driver = GistDriver::new();
    /// // ... play some sounds ...
    /// driver.stop_all(&mut chip); // Silence everything immediately
    /// ```
    pub fn stop_all(&mut self, chip: &mut Ym2149) {
        for (i, v) in self.voices.iter_mut().enumerate() {
            v.inuse = 0;
            v.priority = 0;
            chip.write_register(8 + i as u8, 0);
        }
        self.mixer = 0x3f;
        chip.write_register(7, self.mixer);
    }

    /// Releases a sound, moving it into its release phase.
    ///
    /// This is the "graceful" way to stop a sound. The sound will continue
    /// to play through its volume release envelope before stopping naturally.
    /// The voice's priority is set to zero, allowing other sounds to use
    /// the voice if needed.
    ///
    /// If the voice is not currently playing (`inuse == 0`), this function
    /// has no effect.
    ///
    /// # Arguments
    ///
    /// * `voice_idx` - The voice index (0, 1, or 2) to release. Values >= 3
    ///   are ignored.
    ///
    /// # Note
    ///
    /// The sound will only fade out if it has a volume envelope defined.
    /// Sounds without envelopes will stop on the next duration tick.
    pub fn snd_off(&mut self, voice_idx: usize) {
        if voice_idx < NUM_VOICES && self.voices[voice_idx].inuse != 0 {
            self.voices[voice_idx].inuse = 1;
            self.voices[voice_idx].pitch = -1;
        }
    }

    /// Starts playing a sound effect on an available voice.
    ///
    /// This function allocates a voice (either the requested one or the best
    /// available based on priority), configures all envelope and LFO parameters
    /// from the sound definition, and begins playback.
    ///
    /// # Arguments
    ///
    /// * `chip` - Mutable reference to the YM2149 chip instance
    /// * `sound` - Reference to the [`GistSound`] to play
    /// * `requested_voice` - Optional specific voice (0, 1, or 2) to use.
    ///   If `None`, the driver automatically selects based on availability
    ///   and priority.
    /// * `volume` - Optional volume override (0-15). If `None`, uses the
    ///   sound's default volume.
    /// * `pitch` - Pitch value using MIDI note numbers:
    ///   - 60 = Middle C (C4)
    ///   - 24-108 = Valid range (values outside are octave-wrapped)
    ///   - -1 = Use sound's default frequency (no pitch override)
    /// * `priority` - Priority level (0-32767). Higher priority sounds can
    ///   interrupt lower priority sounds when all voices are busy.
    ///
    /// # Returns
    ///
    /// * `Some(voice_idx)` - The voice index (0-2) where the sound started
    /// * `None` - No voice available (all busy with higher priority sounds)
    ///
    /// # Voice Allocation Logic
    ///
    /// 1. If `requested_voice` is specified and its priority <= new priority,
    ///    use that voice
    /// 2. Otherwise, find a free voice (`inuse == 0`)
    /// 3. If all voices are busy, steal the lowest priority voice
    ///    (if its priority <= new priority)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Play a sound with default settings
    /// driver.snd_on(&mut chip, &sound, None, None, -1, 100);
    ///
    /// // Play at middle C with high priority on voice 0
    /// driver.snd_on(&mut chip, &sound, Some(0), Some(12), 60, 1000);
    /// ```
    pub fn snd_on(
        &mut self,
        chip: &mut Ym2149,
        sound: &GistSound,
        requested_voice: Option<usize>,
        volume: Option<i16>,
        pitch: i16,
        priority: i16,
    ) -> Option<usize> {
        let duration = sound.duration;
        if duration == 0 {
            return requested_voice.or(Some(0));
        }

        let voice_idx = self.pick_voice(requested_voice, priority)?;

        // stop_snd equivalent
        self.voices[voice_idx].inuse = 0;
        self.voices[voice_idx].priority = 0;
        chip.write_register(8 + voice_idx as u8, 0);

        // Load sound
        self.voices[voice_idx].from_sound(sound, pitch, priority, volume);

        let v = &mut self.voices[voice_idx];

        // Configure tone
        let tonemask: u8;
        if v.freq >= 0 {
            tonemask = 0;
            if pitch >= 0 {
                let mut p = pitch;
                while p > 108 {
                    p -= 12;
                }
                while p < 24 {
                    p += 12;
                }
                if let Some(&freq) = YM_FREQS.get((p - 24) as usize) {
                    v.freq = freq as i16;
                }
            }
            chip.write_register((voice_idx * 2) as u8, (v.freq & 0xff) as u8);
            chip.write_register((voice_idx * 2 + 1) as u8, ((v.freq >> 8) & 0x0f) as u8);
        } else {
            tonemask = 1 << voice_idx;
            // When tone disabled, clear freq envelope/LFO
            v.freq_phase = 0;
            v.freq_lfo_limit = 0;
        }

        // Configure noise
        let noisemask: u8;
        if v.noise_freq >= 0 {
            noisemask = 0;
            chip.write_register(6, (v.noise_freq & 0x1f) as u8);
        } else {
            noisemask = 8 << voice_idx;
            // When noise disabled, clear noise envelope/LFO
            v.noise_phase = 0;
            v.noise_lfo_limit = 0;
        }

        // Update mixer
        self.mixer = (self.mixer & MIXER_MASK[voice_idx]) | tonemask | noisemask;
        chip.write_register(7, self.mixer);

        // If no volume envelope, set initial volume and max envelope accumulator
        if v.vol_phase == 0 {
            v.vol_env_acc = 0x000F_0000;
            chip.write_register(8 + voice_idx as u8, (v.volume & 0x0f) as u8);
        }

        // Finally set duration
        v.inuse = duration;

        Some(voice_idx)
    }

    /// Selects the best voice for a new sound based on availability and priority.
    ///
    /// # Voice Selection Algorithm
    ///
    /// 1. If a specific voice is requested and available (priority check passes),
    ///    return that voice
    /// 2. Find any free voice (`inuse == 0`)
    /// 3. If all busy, find the voice with lowest priority that can be stolen
    ///
    /// # Arguments
    ///
    /// * `requested` - Optional specific voice index to prefer
    /// * `priority` - The priority of the new sound
    ///
    /// # Returns
    ///
    /// * `Some(idx)` - Voice index to use
    /// * `None` - No voice available (all have higher priority)
    fn pick_voice(&self, requested: Option<usize>, priority: i16) -> Option<usize> {
        if let Some(idx) = requested {
            if idx < NUM_VOICES && self.voices[idx].priority <= priority {
                return Some(idx);
            }
            if idx >= NUM_VOICES {
                return None;
            }
        }

        // Find free voice
        for i in 0..NUM_VOICES {
            if self.voices[i].inuse == 0 {
                return Some(i);
            }
        }

        // All in use - find lowest priority
        let mut best = if self.voices[0].priority < self.voices[1].priority {
            0
        } else {
            1
        };
        if self.voices[2].priority <= self.voices[best].priority {
            best = 2;
        }
        if self.voices[best].priority > priority {
            None
        } else {
            Some(best)
        }
    }

    /// Main driver tick - must be called 200 times per second.
    ///
    /// This is a cycle-accurate translation of the `timer_irq` routine from
    /// the original 68000 assembly (`gistdrvr.s`). On the Atari ST, this was
    /// called from Timer C at 200 Hz (every 5ms).
    ///
    /// Each tick, this function:
    ///
    /// 1. **Volume Envelope**: Processes attack → decay → sustain → release
    ///    phases for each active voice
    /// 2. **Volume LFO**: Applies volume modulation (tremolo) if configured
    /// 3. **Frequency Envelope**: Processes pitch slides and glides
    /// 4. **Frequency LFO**: Applies pitch modulation (vibrato) if configured
    /// 5. **Noise Envelope/LFO**: Same as frequency, for noise channel
    /// 6. **Duration**: Decrements sound duration and handles voice release
    ///
    /// # Arguments
    ///
    /// * `chip` - Mutable reference to the YM2149 chip instance
    ///
    /// # Timing
    ///
    /// For accurate playback, ensure this is called at exactly 200 Hz.
    /// At 44100 Hz sample rate, call every 220.5 samples:
    ///
    /// ```ignore
    /// const TICK_RATE: u32 = 200;
    /// const SAMPLE_RATE: u32 = 44100;
    /// let samples_per_tick = SAMPLE_RATE / TICK_RATE; // 220
    /// ```
    ///
    /// # Implementation Note
    ///
    /// Voices are processed in reverse order (2, 1, 0) to match the original
    /// assembly. This matters because all voices share the noise register.
    pub fn tick(&mut self, chip: &mut Ym2149) {
        self.tick_count += 1;

        // Original processes voices 2, 1, 0 (dbf d2,vcloop)
        // This matters because all voices share the noise register (6)
        for voice_idx in (0..NUM_VOICES).rev() {
            let v = &mut self.voices[voice_idx];

            // vcloop: tst.w (a0) / beq endloop
            if v.inuse == 0 {
                continue;
            }

            // ===== VOLUME ENVELOPE (offset 8, 10-22, 116) =====
            // move.w 8(a0),d0  ; vol_phase
            // move.l 116(a0),d1 ; vol_env_acc
            let mut d1 = v.vol_env_acc;

            match v.vol_phase {
                1 => {
                    // Attack: add.l 10(a0),d1
                    d1 = d1.wrapping_add(v.vol_attack);
                    // cmp.l #0x000F0000,d1 / blt.s endve
                    if d1 >= 0x000F_0000 {
                        d1 = 0x000F_0000;
                        v.vol_phase += 1;
                    }
                }
                2 => {
                    // Decay: add.l 14(a0),d1
                    d1 = d1.wrapping_add(v.vol_decay);
                    // cmp.l 18(a0),d1 / bgt.s endve
                    // Note: BGT is signed, and decay step is typically negative
                    if d1 <= v.vol_sustain {
                        d1 = v.vol_sustain;
                        v.vol_phase += 1;
                    }
                }
                4 => {
                    // Release: add.l 22(a0),d1
                    let old_d1 = d1;
                    d1 = d1.wrapping_add(v.vol_release);
                    // tst.l d1 / bgt.s endve
                    if d1 <= 0 {
                        if self.debug && v.inuse < 0 {
                            println!(
                                "Release done: vol_env went from {:08X} to {:08X}, vol_release={:08X}, took {} ticks",
                                old_d1 as u32, d1 as u32, v.vol_release as u32, -v.inuse
                            );
                        }
                        d1 = 0;
                        v.vol_phase = 0;
                        v.inuse = 1;
                    }
                }
                _ => {}
            }
            v.vol_env_acc = d1;

            // ===== VOLUME LFO (offset 26-34, 120) =====
            // lva: move.l 26(a0),d0 / beq.s do_vol
            if v.vol_lfo_limit != 0 {
                // tst.w 34(a0) / beq.s do_lv / subq.w #1,34(a0) / bra.s do_vol
                if v.vol_lfo_delay > 0 {
                    v.vol_lfo_delay -= 1;
                } else {
                    // do_lv: move.l 120(a0),d1 / add.l 30(a0),d1
                    let mut lfo = v.vol_lfo_acc.wrapping_add(v.vol_lfo_step);
                    let limit = v.vol_lfo_limit;

                    // cmp.l d0,d1 / bge.s do_lv1
                    if lfo >= limit {
                        // do_lv1: move.l d0,d1 / neg.l 30(a0)
                        lfo = limit;
                        v.vol_lfo_step = v.vol_lfo_step.wrapping_neg();
                    } else {
                        // neg.l d0 / cmp.l d0,d1 / bgt.s enddo_lv
                        let neg_limit = limit.wrapping_neg();
                        if lfo <= neg_limit {
                            lfo = neg_limit;
                            v.vol_lfo_step = v.vol_lfo_step.wrapping_neg();
                        }
                    }
                    v.vol_lfo_acc = lfo;
                }
            }

            // ===== WRITE VOLUME TO CHIP =====
            // do_vol: move.w 8(a0),d0 / or.w 26(a0),d0 / beq.s fe
            // Note: or.w 26(a0) reads the HIGH word of vol_lfo_limit
            let vol_lfo_limit_hi = (v.vol_lfo_limit >> 16) as i16;
            if v.vol_phase != 0 || vol_lfo_limit_hi != 0 {
                // move.w 6(a0),d0 / add.w d0,d0 / move.w 0(a2,d0.w),d0
                let vol_idx = (v.volume.clamp(0, 15)) as usize;
                let mut d0: i32 = DIV_15[vol_idx] as i32;

                // move.l 116(a0),d1 / add.l 120(a0),d1
                let d1 = v.vol_env_acc.wrapping_add(v.vol_lfo_acc);

                // bpl.s do_vol1
                let level: u8 = if d1 < 0 {
                    // moveq.l #0,d0
                    0
                } else {
                    // do_vol1: asr.l #8,d1
                    let shifted = d1 >> 8;
                    // muls.w d1,d0 - multiply low 16 bits
                    let d1_lo = shifted as i16;
                    let d0_lo = d0 as i16;
                    d0 = (d0_lo as i32) * (d1_lo as i32);
                    // swap d0 - get high word
                    d0 = (d0 >> 16) & 0xffff;
                    // Handle sign extension from swap
                    if d0 > 0x7fff {
                        d0 = (d0 as i16) as i32;
                    }
                    // cmp.w #15,d0 / ble.s do_vol2
                    if d0 > 15 {
                        15
                    } else if d0 < 0 {
                        0
                    } else {
                        d0 as u8
                    }
                };

                chip.write_register(8 + voice_idx as u8, level);
            }

            // ===== FREQUENCY ENVELOPE (offset 36-54, 124) =====
            // fe: move.w 36(a0),d0 / move.l 124(a0),d1
            let mut d1 = v.freq_env_acc;

            match v.freq_phase {
                1 => {
                    // add.l 38(a0),d1
                    d1 = d1.wrapping_add(v.freq_attack);
                    // tst.w 38(a0) / bmi.s fea1
                    let step_hi = (v.freq_attack >> 16) as i16;
                    if step_hi >= 0 {
                        // cmp.l 42(a0),d1 / blt.s endfe
                        if d1 >= v.freq_attack_target {
                            d1 = v.freq_attack_target;
                            v.freq_phase += 1;
                        }
                    } else {
                        // fea1: cmp.l 42(a0),d1 / bgt.s endfe
                        if d1 <= v.freq_attack_target {
                            d1 = v.freq_attack_target;
                            v.freq_phase += 1;
                        }
                    }
                }
                2 => {
                    d1 = d1.wrapping_add(v.freq_decay);
                    let step_hi = (v.freq_decay >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.freq_decay_target {
                            d1 = v.freq_decay_target;
                            v.freq_phase += 1;
                        }
                    } else {
                        if d1 <= v.freq_decay_target {
                            d1 = v.freq_decay_target;
                            v.freq_phase += 1;
                        }
                    }
                }
                4 => {
                    d1 = d1.wrapping_add(v.freq_release);
                    let step_hi = (v.freq_release >> 16) as i16;
                    // tst.w 54(a0) / bmi.s fer1
                    if step_hi >= 0 {
                        // tst.l d1 / bmi.s endfe
                        if d1 >= 0 {
                            d1 = 0;
                        }
                    } else {
                        // fer1: tst.l d1 / bgt.s endfe
                        if d1 <= 0 {
                            d1 = 0;
                        }
                    }
                }
                _ => {}
            }
            v.freq_env_acc = d1;

            // ===== FREQUENCY LFO (offset 58-78, 128) =====
            // lfa: move.l 58(a0),d0 / beq.s do_fr
            if v.freq_lfo_limit != 0 {
                // tst.w 78(a0) / beq.s do_lf / subq.w #1,78(a0) / bra.s do_fr
                if v.freq_lfo_delay > 0 {
                    v.freq_lfo_delay -= 1;
                } else {
                    // do_lf: move.l 62(a0),d1 / bmi.s do_lf2
                    let step = v.freq_lfo_step;

                    if step >= 0 {
                        // Positive direction
                        // add.l 128(a0),d1 - add acc to step
                        let (lfo, carry) = (step as u32).overflowing_add(v.freq_lfo_acc as u32);
                        let lfo = lfo as i32;

                        // bcc.s do_lf1 / move.l 66(a0),62(a0)
                        if carry {
                            v.freq_lfo_step = v.freq_lfo_reset_pos;
                        }

                        // cmp.l d0,d1 / blt.s enddo_lf
                        let limit = v.freq_lfo_limit;
                        if lfo >= limit {
                            // do_lf4: move.l d0,d1 / neg.l 62(a0)
                            v.freq_lfo_acc = limit;
                            v.freq_lfo_step = v.freq_lfo_step.wrapping_neg();
                        } else {
                            v.freq_lfo_acc = lfo;
                        }
                    } else {
                        // Negative direction
                        // move.l 70(a0),d0 - get negative limit
                        let neg_limit = v.freq_lfo_limit_neg;

                        // add.l 128(a0),d1 - add acc to step
                        let (lfo, carry) = (step as u32).overflowing_add(v.freq_lfo_acc as u32);
                        let lfo = lfo as i32;

                        // bcs.s do_lf3 / move.l 74(a0),62(a0)
                        // Note: BCS means "branch if carry SET" - opposite of BCC
                        if !carry {
                            v.freq_lfo_step = v.freq_lfo_reset_neg;
                        }

                        // cmp.l d0,d1 / bgt.s enddo_lf
                        if lfo <= neg_limit {
                            // do_lf4: move.l d0,d1 / neg.l 62(a0)
                            v.freq_lfo_acc = neg_limit;
                            v.freq_lfo_step = v.freq_lfo_step.wrapping_neg();
                        } else {
                            v.freq_lfo_acc = lfo;
                        }
                    }
                }
            }

            // ===== WRITE FREQUENCY TO CHIP =====
            // do_fr: move.w 36(a0),d0 / or.w 58(a0),d0 / beq.s nfe
            let freq_lfo_limit_hi = (v.freq_lfo_limit >> 16) as i16;
            if v.freq_phase != 0 || freq_lfo_limit_hi != 0 {
                if v.freq >= 0 {
                    // move.l 128(a0),d0 / add.l 124(a0),d0
                    let combined = v.freq_lfo_acc.wrapping_add(v.freq_env_acc);

                    // swap d0
                    let hi = ((combined as u32) >> 16) as i16;

                    // muls.w 2(a0),d0
                    let mut d0: i32 = (hi as i32) * (v.freq as i32);

                    // asl.l #4,d0
                    d0 = d0.wrapping_shl(4);

                    // swap d0
                    d0 = ((d0 as u32) >> 16) as i16 as i32;

                    // bpl.s do_fr0 / addq.w #1,d0
                    if d0 < 0 {
                        d0 = d0.wrapping_add(1);
                    }

                    // add.w 2(a0),d0
                    d0 = ((d0 as i16).wrapping_add(v.freq)) as i32;

                    // bpl.s do_fr1
                    if d0 < 0 {
                        d0 = 0;
                    }

                    // cmp.w #0x0FFF,d0 / ble.s do_fr2
                    if d0 > 0x0fff {
                        d0 = 0x0fff;
                    }

                    chip.write_register((voice_idx * 2) as u8, (d0 & 0xff) as u8);
                    chip.write_register((voice_idx * 2 + 1) as u8, ((d0 >> 8) & 0x0f) as u8);
                }
            }

            // ===== NOISE ENVELOPE (offset 80-98, 132) =====
            let mut d1 = v.noise_env_acc;

            match v.noise_phase {
                1 => {
                    d1 = d1.wrapping_add(v.noise_attack);
                    let step_hi = (v.noise_attack >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.noise_attack_target {
                            d1 = v.noise_attack_target;
                            v.noise_phase += 1;
                        }
                    } else {
                        if d1 <= v.noise_attack_target {
                            d1 = v.noise_attack_target;
                            v.noise_phase += 1;
                        }
                    }
                }
                2 => {
                    d1 = d1.wrapping_add(v.noise_decay);
                    let step_hi = (v.noise_decay >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.noise_decay_target {
                            d1 = v.noise_decay_target;
                            v.noise_phase += 1;
                        }
                    } else {
                        if d1 <= v.noise_decay_target {
                            d1 = v.noise_decay_target;
                            v.noise_phase += 1;
                        }
                    }
                }
                4 => {
                    d1 = d1.wrapping_add(v.noise_release);
                    let step_hi = (v.noise_release >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= 0 {
                            d1 = 0;
                        }
                    } else {
                        if d1 <= 0 {
                            d1 = 0;
                        }
                    }
                }
                _ => {}
            }
            v.noise_env_acc = d1;

            // ===== NOISE LFO (offset 102-110, 136) =====
            if v.noise_lfo_limit != 0 {
                if v.noise_lfo_delay > 0 {
                    v.noise_lfo_delay -= 1;
                } else {
                    let mut lfo = v.noise_lfo_acc.wrapping_add(v.noise_lfo_step);
                    let limit = v.noise_lfo_limit;

                    if lfo >= limit {
                        lfo = limit;
                        v.noise_lfo_step = v.noise_lfo_step.wrapping_neg();
                    } else {
                        let neg_limit = limit.wrapping_neg();
                        if lfo <= neg_limit {
                            lfo = neg_limit;
                            v.noise_lfo_step = v.noise_lfo_step.wrapping_neg();
                        }
                    }
                    v.noise_lfo_acc = lfo;
                }
            }

            // ===== WRITE NOISE TO CHIP =====
            let noise_lfo_limit_hi = (v.noise_lfo_limit >> 16) as i16;
            if v.noise_phase != 0 || noise_lfo_limit_hi != 0 {
                if v.noise_freq >= 0 {
                    // move.l 136(a0),d0 / add.l 132(a0),d0
                    let combined = v.noise_lfo_acc.wrapping_add(v.noise_env_acc);

                    // swap d0
                    let hi = ((combined as u32) >> 16) as i16;

                    // add.w 4(a0),d0
                    let mut d0 = (hi as i32) + (v.noise_freq as i32);

                    // bpl.s do_nfr1
                    if d0 < 0 {
                        d0 = 0;
                    }

                    // cmp.b #31,d0 / ble.s do_nfr2
                    if d0 > 31 {
                        d0 = 31;
                    }

                    chip.write_register(6, d0 as u8);
                }
            }

            // ===== DURATION HANDLING =====
            // dec_dur: tst.w 112(a0) / bpl.s endloop
            if v.pitch >= 0 {
                continue;
            }

            // subq.w #1,(a0)
            v.inuse = v.inuse.wrapping_sub(1);

            // bne.s endloop
            if v.inuse != 0 {
                continue;
            }

            // clr.w 114(a0)
            v.priority = 0;

            // tst.w 8(a0) / bne.s dec_dur1
            if v.vol_phase == 0 {
                chip.write_register(8 + voice_idx as u8, 0);
                continue;
            }

            // dec_dur1: subq.w #1,(a0)
            v.inuse = -1;

            // moveq.l #4,d0 / move.w d0,8(a0)
            v.vol_phase = 4;

            // tst.w 36(a0) / beq.s dec_dur2
            if v.freq_phase != 0 {
                v.freq_phase = 4;
                // move.w 54(a0),d1 / move.w 124(a0),d3 / eor.w d1,d3 / bmi.s dec_dur2
                let release_hi = (v.freq_release >> 16) as i16;
                let acc_hi = (v.freq_env_acc >> 16) as i16;
                // If signs are the same (XOR result is positive), negate
                if (release_hi ^ acc_hi) >= 0 {
                    v.freq_release = v.freq_release.wrapping_neg();
                }
            }

            // tst.w 80(a0) / beq.s endloop
            if v.noise_phase != 0 {
                v.noise_phase = 4;
                let release_hi = (v.noise_release >> 16) as i16;
                let acc_hi = (v.noise_env_acc >> 16) as i16;
                if (release_hi ^ acc_hi) >= 0 {
                    v.noise_release = v.noise_release.wrapping_neg();
                }
            }
        }
    }
}
