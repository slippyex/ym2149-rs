use std::f32::consts::PI;
use ym2149::util::{channel_period, period_to_frequency};
use ym2149::ym2149::constants::{VOLUME_SCALE, VOLUME_TABLE};

const SAMPLE_RATE: f32 = 44_100.0;

#[derive(Clone, Copy)]
struct BiquadLP {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl BiquadLP {
    fn new() -> Self {
        Self {
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn set_lowpass(&mut self, cutoff: f32, q: f32) {
        let sr = SAMPLE_RATE;
        let w0 = 2.0 * PI * (cutoff / sr);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.1));
        let b0 = (1.0 - cos_w0) * 0.5;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) * 0.5;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.z1 + self.b2 * self.z2
            - self.a1 * self.z1
            - self.a2 * self.z2;
        self.z2 = self.z1;
        self.z1 = y;
        y
    }
}

#[derive(Clone, Copy)]
struct SoftVoice {
    freq: f32,
    phase: f32,
    phase_inc: f32,
    target_amp: f32,
    amp: f32,
    env_enabled: bool,
    env_phase: f32,
    env_speed: f32,
    env_shape: u8,
    pwm_width: f32,
    filt_cut: f32,
    filt_q: f32,
    biq: BiquadLP,
}

impl SoftVoice {
    fn new() -> Self {
        SoftVoice {
            freq: 0.0,
            phase: 0.0,
            phase_inc: 0.0,
            target_amp: 0.0,
            amp: 0.0,
            env_enabled: false,
            env_phase: 0.0,
            env_speed: 0.0,
            env_shape: 0,
            pwm_width: 0.5,
            filt_cut: 1200.0,
            filt_q: 0.8,
            biq: BiquadLP::new(),
        }
    }

    fn update(
        &mut self,
        freq: f32,
        target_amp: f32,
        env_enabled: bool,
        env_speed: f32,
        env_shape: u8,
    ) {
        self.freq = freq.max(0.0);
        self.phase_inc = if self.freq > 0.0 {
            2.0 * PI * self.freq / SAMPLE_RATE
        } else {
            0.0
        };
        // Clamp to 1.0 since VOLUME_TABLE already provides normalized [0.0, 1.0] range
        self.target_amp = target_amp.clamp(0.0, 1.0);
        self.env_enabled = env_enabled;
        self.env_speed = env_speed;
        self.env_shape = env_shape & 0x0F;
        // Default PWM and filter
        self.pwm_width = 0.5;
        self.filt_cut = 1200.0;
        self.filt_q = 0.8;
        self.biq.set_lowpass(self.filt_cut, self.filt_q);
    }

    fn advance(&mut self) -> f32 {
        if self.phase_inc == 0.0 {
            self.amp *= 0.995;
            return 0.0;
        }

        self.phase += self.phase_inc;
        if self.phase > 2.0 * PI {
            self.phase -= 2.0 * PI;
        }

        // No smoothing: respond immediately for punch
        self.amp = self.target_amp;

        let env = if self.env_enabled {
            self.env_phase += self.env_speed;
            if self.env_phase >= 1.0 {
                self.env_phase -= 1.0;
            }
            match self.env_shape {
                0x08 => 1.0 - self.env_phase, // saw down
                0x0C => self.env_phase,       // saw up
                0x0D | 0x0F => 1.0,           // hold
                0x0E => 0.0,                  // off
                _ => {
                    if self.env_phase < 0.5 {
                        self.env_phase * 2.0
                    } else {
                        (1.0 - self.env_phase) * 2.0
                    }
                }
            }
        } else {
            1.0
        };

        // Modulate PWM and filter cutoff with env for synthy movement
        self.pwm_width = (0.5 + 0.3 * (env - 0.5)).clamp(0.1, 0.9);
        self.filt_cut = (300.0 + env * 7000.0).clamp(100.0, 10_000.0);
        self.biq.set_lowpass(self.filt_cut, self.filt_q);

        // Oscillator: saw + pulse mixture
        // Saw
        let mut saw = (self.phase / PI) - 1.0; // -1..1 over 0..2PI
        // Tanh soft edge for less aliasing
        saw = (saw * 1.5).tanh();
        // Pulse
        let pulse = if (self.phase / (2.0 * PI)) % 1.0 < self.pwm_width {
            1.0
        } else {
            -1.0
        };
        let mut osc = 0.7 * saw + 0.3 * pulse;

        // Filter
        osc = self.biq.process(osc);
        // Mild saturation
        let drive = 1.6;
        let sat = (osc * drive).tanh() / (drive.tanh());
        // Blend some pre-filter to retain presence
        let blended = 0.7 * sat + 0.3 * ((0.7 * saw + 0.3 * pulse) * 0.8);
        // Apply amplitude and a floor so tones remain audible even at low env
        let env_amp = 0.35 + 0.65 * env;
        blended * self.amp * env_amp
    }
}

/// Experimental software synthesizer that reinterprets YM frames as a synth
///
/// This engine is intentionally not bit-accurate. It provides a musical,
/// synth-like sound using PWM/saw oscillators, a resonant low-pass filter,
/// envelope-to-filter/PWM modulation, noise shaping for snares/hats, and
/// mild saturation. SID and Sync Buzzer effects from YM6 are supported.
pub struct SoftSynth {
    voices: [SoftVoice; 3],
    registers: [u8; 16],
    last_sample: f32,
    filter_memory: f32,
    color_filter: bool,
    // User mutes per channel (A/B/C)
    user_mute: [bool; 3],
    // Noise state
    noise_phase: f32,
    noise_step: f32,
    lfsr: u32,
    noise_bit: bool,
    // Color filter state
    lp_mem0: f32,
    lp_mem1: f32,
    // Effects
    sid_active: [bool; 3],
    sid_pos: [u32; 3],
    sid_step: [u32; 3],
    sid_vol: [u8; 3],
    sync_buzzer_enabled: bool,
    sync_pos: u32,
    sync_step: u32,
    // Noise shaping for drums
    noise_val: f32,
    noise_smooth: f32,
    noise_burst: [f32; 3],
    noise_gate_prev: [bool; 3],
}

impl SoftSynth {
    /// Create a new experimental softsynth
    pub fn new() -> Self {
        SoftSynth {
            voices: [SoftVoice::new(), SoftVoice::new(), SoftVoice::new()],
            registers: [0; 16],
            last_sample: 0.0,
            filter_memory: 0.0,
            color_filter: true,
            user_mute: [false; 3],
            noise_phase: 0.0,
            noise_step: 0.0,
            lfsr: 1,
            noise_bit: true,
            lp_mem0: 0.0,
            lp_mem1: 0.0,
            sid_active: [false; 3],
            sid_pos: [0; 3],
            sid_step: [0; 3],
            sid_vol: [0; 3],
            sync_buzzer_enabled: false,
            sync_pos: 0,
            sync_step: 0,
            noise_val: 1.0,
            noise_smooth: 0.0,
            noise_burst: [0.0; 3],
            noise_gate_prev: [false; 3],
        }
    }

    /// Load all 16 YM register values into the softsynth
    pub fn load_registers(&mut self, regs: &[u8; 16]) {
        self.registers = *regs;
        self.update_from_registers();
    }

    /// Write a single YM register value into the softsynth
    pub fn write_register(&mut self, addr: u8, value: u8) {
        let idx = (addr as usize) & 0x0F;
        self.registers[idx] = value;
        self.update_from_registers();
    }

    fn update_from_registers(&mut self) {
        let env_period_raw = ((self.registers[12] as u16) << 8) | self.registers[11] as u16;
        let env_speed = if env_period_raw <= 2 {
            0.0
        } else {
            (2_000_000.0 / (512.0 * env_period_raw as f32)) / SAMPLE_RATE
        };
        let env_shape = self.registers[13] & 0x0F;
        // Noise step
        let per = (self.registers[6] & 0x1F) as u32;
        self.noise_step = if per < 3 {
            0.0
        } else {
            (2_000_000.0 / (16.0 * per as f32)) / SAMPLE_RATE
        };
        if self.noise_step == 0.0 {
            self.noise_bit = true;
        }

        for (i, voice) in self.voices.iter_mut().enumerate() {
            let lo = self.registers[i * 2];
            let hi = self.registers[i * 2 + 1];
            let period = channel_period(lo, hi);
            let freq = period.map(period_to_frequency).unwrap_or(0.0);
            let amp_reg = self.registers[8 + i] & 0x0F;
            let amp = VOLUME_TABLE[amp_reg as usize] as f32 * VOLUME_SCALE;
            let env_enabled = (self.registers[8 + i] & 0x10) != 0;
            voice.update(freq, amp, env_enabled, env_speed, env_shape);
        }
    }

    /// Advance the softsynth by one sample and update internal state
    pub fn clock(&mut self) {
        // Sync buzzer envelope retrigger
        if self.sync_buzzer_enabled {
            self.sync_pos = self.sync_pos.wrapping_add(self.sync_step);
            if (self.sync_pos & 0x8000_0000) != 0 {
                for v in &mut self.voices {
                    v.env_phase = 0.0;
                }
                self.sync_pos &= 0x7fff_ffff;
            }
        }

        // Update SID gating timers
        for i in 0..3 {
            if self.sid_active[i] {
                self.sid_pos[i] = self.sid_pos[i].wrapping_add(self.sid_step[i]);
            }
        }

        // Update noise bit
        if self.noise_step > 0.0 {
            self.noise_phase += self.noise_step;
            if self.noise_phase >= 1.0 {
                self.noise_phase -= 1.0;
                let r_bit = ((self.lfsr & 1) ^ ((self.lfsr >> 2) & 1)) != 0;
                self.lfsr = (self.lfsr >> 1) | ((r_bit as u32) << 16);
                self.noise_bit = !r_bit;
            }
        }
        self.noise_val = if self.noise_bit { 1.0 } else { -1.0 };
        // Brighten noise (simple high-pass)
        self.noise_smooth += 0.05 * (self.noise_val - self.noise_smooth);
        let noise_hp = self.noise_val - self.noise_smooth;

        let mixer = self.registers[7];
        let mut acc = 0.0;
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if self.user_mute[i] {
                // If muted, decay a bit and add nothing
                let _ = voice.advance();
                self.noise_gate_prev[i] = false;
                if self.noise_burst[i] > 0.0 {
                    self.noise_burst[i] -= 1.0;
                }
                continue;
            }
            let mut v = voice.advance();
            // Experimental soft gating factors for a more musical feel
            let tone_enabled = (mixer & (1 << i)) == 0;
            let noise_enabled = (mixer & (1 << (i + 3))) == 0;
            let tone_factor = if tone_enabled { 1.0 } else { 0.85 };
            v *= tone_factor; // leave tone independent of instantaneous noise_bit

            // Apply SID gating if active (overrides tone)
            if self.sid_active[i] {
                let gate_on = (self.sid_pos[i] & 0x8000_0000) != 0;
                let sid_amp = VOLUME_TABLE[(self.sid_vol[i] & 0x0F) as usize] as f32 * VOLUME_SCALE;
                v = if gate_on { sid_amp } else { 0.0 };
            }

            // Noise layer for snares/hats — more present and punchy
            if noise_enabled {
                // Edge detect: gate opened => burst
                if !self.noise_gate_prev[i] {
                    self.noise_burst[i] = 300.0; // ~7ms burst
                }
                self.noise_gate_prev[i] = true;
                let burst_env = (self.noise_burst[i] / 300.0).clamp(0.0, 1.0);
                if self.noise_burst[i] > 0.0 {
                    self.noise_burst[i] -= 1.0;
                }

                let amp_idx = (self.registers[8 + i] & 0x0F) as usize;
                let fixed_amp = VOLUME_TABLE[amp_idx] as f32 * VOLUME_SCALE;
                // Base noise gain + burst accent, shaped by envelope
                let env_amt = if voice.env_enabled {
                    voice.env_phase.fract()
                } else {
                    1.0
                };
                let noise_gain = (0.5 + 0.5 * env_amt) + 0.6 * burst_env;
                v += (noise_hp * noise_gain * fixed_amp * 0.8).clamp(-1.2, 1.2);
            } else {
                self.noise_gate_prev[i] = false;
                if self.noise_burst[i] > 0.0 {
                    self.noise_burst[i] -= 1.0;
                }
            }
            acc += v;
        }

        // Normalize by voice count to prevent clipping from summing 3 channels
        // This brings the range from [-4.5, 4.5] down to [-1.5, 1.5]
        let combined = acc / self.voices.len() as f32;
        // DC removal
        self.filter_memory += 0.002 * (combined - self.filter_memory);
        let mut out = combined - self.filter_memory;
        // Optional color filter
        if self.color_filter {
            let filtered = (self.lp_mem0 * 0.25) + (self.lp_mem1 * 0.5) + (out * 0.25);
            self.lp_mem0 = self.lp_mem1;
            self.lp_mem1 = out;
            out = filtered;
        } else {
            self.lp_mem0 = out;
            self.lp_mem1 = out;
        }
        self.last_sample = out.clamp(-1.0, 1.0);
    }

    /// Start SID-style amplitude gating on a voice
    pub fn sid_start(&mut self, voice: usize, timer_freq: u32, vol: u8) {
        if voice < 3 {
            let step = ((timer_freq as u64) << 31) / (SAMPLE_RATE as u64);
            self.sid_step[voice] = step as u32;
            self.sid_pos[voice] = 0;
            self.sid_vol[voice] = vol & 0x0F;
            self.sid_active[voice] = true;
        }
    }

    /// Stop SID gating on a voice
    pub fn sid_stop(&mut self, voice: usize) {
        if voice < 3 {
            self.sid_active[voice] = false;
        }
    }

    /// Start Sync Buzzer envelope retriggering
    pub fn sync_buzzer_start(&mut self, timer_freq: u32, env_shape: u8) {
        let step = ((timer_freq as u64) << 31) / (SAMPLE_RATE as u64);
        self.sync_step = step as u32;
        self.sync_pos = 0;
        self.sync_buzzer_enabled = true;
        // Update env shape without touching registers (caller handles sentinel logic)
        for v in &mut self.voices {
            v.env_shape = env_shape & 0x0F;
        }
    }

    /// Stop Sync Buzzer
    pub fn sync_buzzer_stop(&mut self) {
        self.sync_buzzer_enabled = false;
        self.sync_pos = 0;
        self.sync_step = 0;
    }

    /// Get the last generated audio sample (-1.0..1.0)
    pub fn get_sample(&self) -> f32 {
        self.last_sample
    }

    /// Dump the current register state snapshot
    pub fn dump_registers(&self) -> [u8; 16] {
        self.registers
    }

    /// Enable/disable the post-mix color filter (gentle low-pass)
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.color_filter = enabled;
    }

    /// Mute or unmute a channel (0=A,1=B,2=C)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        if channel < 3 {
            self.user_mute[channel] = mute;
        }
    }

    /// Get current mute state of a channel
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        channel < 3 && self.user_mute[channel]
    }
}

impl Default for SoftSynth {
    fn default() -> Self {
        Self::new()
    }
}

// NOTE: SoftPlayer has been intentionally removed.
//
// For playback, use `Ym6PlayerGeneric<SoftSynth>` from ym-replayer:
//
// ```rust
// use ym_replayer::Ym6PlayerGeneric;
// use ym_softsynth::SoftSynth;
//
// let player: Ym6PlayerGeneric<SoftSynth> = Ym6PlayerGeneric::new();
// ```
//
// This avoids circular dependencies while maintaining trait-based abstraction.
