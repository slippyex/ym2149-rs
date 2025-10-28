use crate::replayer::{PlaybackController, PlaybackState, Ym6Info};
use crate::ym2149::constants::{VOLUME_SCALE, VOLUME_TABLE};
use crate::ym_parser::effects::{EffectCommand, Ym6EffectDecoder};
use crate::Result;
use std::f32::consts::PI;

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

fn channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | (lo as u16);
    if period == 0 {
        None
    } else {
        Some(period)
    }
}

fn period_to_frequency(period: u16) -> f32 {
    if period == 0 {
        0.0
    } else {
        2_000_000.0 / (16.0 * period as f32)
    }
}

/// Simple player for feeding YM frames into the experimental SoftSynth.
///
/// This mirrors `Ym6Player` API but renders using the non-bit-accurate
/// synth engine. YM6 effects (SID, Sync Buzzer) are decoded and applied.
pub struct SoftPlayer {
    chip: SoftSynth,
    frames: Vec<[u8; 16]>,
    current_frame: usize,
    samples_in_frame: u32,
    samples_per_frame: u32,
    loop_point: Option<usize>,
    state: PlaybackState,
    info: Ym6Info,
    effect_decoder: Ym6EffectDecoder,
    sid_active: [bool; 3],
    sync_active: bool,
}

impl SoftPlayer {
    /// Create a new SoftPlayer
    pub fn new() -> Self {
        SoftPlayer {
            chip: SoftSynth::new(),
            frames: Vec::new(),
            current_frame: 0,
            samples_in_frame: 0,
            samples_per_frame: 882,
            loop_point: None,
            state: PlaybackState::Stopped,
            info: Ym6Info {
                song_name: String::new(),
                author: String::new(),
                comment: String::new(),
                frame_count: 0,
                frame_rate: 50,
                loop_frame: 0,
                master_clock: 2_000_000,
            },
            effect_decoder: Ym6EffectDecoder::new(),
            sid_active: [false; 3],
            sync_active: false,
        }
    }

    /// Load pre-parsed YM frames (16-byte register dumps)
    pub fn load_frames(
        &mut self,
        frames: Vec<[u8; 16]>,
        frame_rate: u16,
        loop_point: Option<usize>,
        info: Ym6Info,
    ) {
        self.frames = frames;
        self.info = info;
        self.info.frame_rate = frame_rate;
        self.loop_point = loop_point;
        self.info.loop_frame = loop_point.unwrap_or(0) as u32;
        self.info.frame_count = self.frames.len() as u32;
        self.samples_per_frame = match frame_rate {
            0 => 882,
            rate => (SAMPLE_RATE as u32 / rate as u32).max(1),
        };
        self.current_frame = 0;
        self.samples_in_frame = 0;
        self.state = PlaybackState::Stopped;
    }

    /// Construct from an existing integer-accurate player
    pub fn from_ym_player(src: &crate::replayer::Ym6Player) -> Result<Self> {
        if src.is_tracker_mode() {
            return Err("SoftSynth backend does not yet support tracker formats".into());
        }

        let frames = src
            .frames_clone()
            .ok_or("Unable to clone frames from player")?;

        let loop_point = src.loop_point_value();
        let samples_per_frame = src.samples_per_frame_value();
        let info = src.info().cloned().unwrap_or_else(|| Ym6Info {
            song_name: String::new(),
            author: String::new(),
            comment: String::new(),
            frame_count: frames.len() as u32,
            frame_rate: 50,
            loop_frame: 0,
            master_clock: 2_000_000,
        });

        let frame_rate = info.frame_rate;

        let mut player = SoftPlayer::new();
        player.load_frames(frames, frame_rate, loop_point, info);
        player.samples_per_frame = samples_per_frame.max(1);
        Ok(player)
    }

    /// Mute or unmute a channel (0=A,1=B,2=C)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.chip.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.chip.is_channel_muted(channel)
    }

    /// Return a snapshot of the current register state
    pub fn visual_registers(&self) -> [u8; 16] {
        self.chip.dump_registers()
    }

    /// Active effect state (sync buzzer, SID per voice, digidrums not supported here)
    pub fn get_active_effects(&self) -> (bool, [bool; 3], [bool; 3]) {
        (self.sync_active, self.sid_active, [false; 3])
    }

    /// Samples per frame in the current playback
    pub fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    /// Optional loop point frame index
    pub fn loop_point(&self) -> Option<usize> {
        self.loop_point
    }

    /// Duration based on frames and frame rate assumption
    pub fn duration_seconds(&self) -> f32 {
        if self.frames.is_empty() {
            0.0
        } else {
            self.frames.len() as f32 * self.samples_per_frame as f32 / SAMPLE_RATE
        }
    }

    /// Generate one output sample
    fn generate_sample(&mut self) -> f32 {
        if self.state != PlaybackState::Playing {
            return 0.0;
        }

        if self.frames.is_empty() {
            return 0.0;
        }

        if self.samples_in_frame == 0 {
            let regs = self.frames[self.current_frame];
            self.chip.load_registers(&regs);
            // Decode and apply effects
            let effects = self.effect_decoder.decode_effects(&regs);
            self.apply_effects(&regs, &effects);
        }

        self.chip.clock();
        let sample = self.chip.get_sample();
        self.samples_in_frame += 1;

        if self.samples_in_frame >= self.samples_per_frame {
            self.samples_in_frame = 0;
            if self.current_frame + 1 >= self.frames.len() {
                if let Some(loop_start) = self.loop_point {
                    self.current_frame = loop_start;
                } else {
                    self.state = PlaybackState::Stopped;
                }
            } else {
                self.current_frame += 1;
            }
        }

        sample
    }

    fn apply_effects(&mut self, regs: &[u8; 16], effects: &[EffectCommand; 2]) {
        let mut sid_intent = [None; 3];
        let mut sync_intent: Option<(u32, u8)> = None;

        for eff in effects.iter() {
            match *eff {
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
                        sid_intent[voice as usize] = Some((freq, volume));
                    }
                }
                EffectCommand::SyncBuzzerStart { freq, env_shape } => {
                    sync_intent = Some((freq, env_shape));
                }
                _ => {}
            }
        }

        // Apply sync buzzer intent
        if let Some((freq, env_shape)) = sync_intent {
            // Honor sentinel: if regs[13]==0xFF, don't change shape
            let shape = if regs[13] == 0xFF {
                regs[13]
            } else {
                env_shape & 0x0F
            };
            if !self.sync_active {
                self.chip.sync_buzzer_start(freq, shape);
                self.sync_active = true;
            }
        } else if self.sync_active {
            self.chip.sync_buzzer_stop();
            self.sync_active = false;
        }

        // Apply per-voice SID
        for (v, intent) in sid_intent.iter().enumerate() {
            if let Some((freq, vol)) = intent {
                if !self.sid_active[v] {
                    self.chip.sid_start(v, *freq, *vol);
                    self.sid_active[v] = true;
                }
            } else if self.sid_active[v] {
                self.chip.sid_stop(v);
                self.sid_active[v] = false;
            }
        }
    }

    /// Generate `count` samples
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.generate_sample());
        }
        samples
    }

    /// Total frames loaded
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Playback position [0.0..1.0]
    pub fn get_playback_position(&self) -> f32 {
        if self.frames.is_empty() {
            0.0
        } else {
            (self.current_frame as f32) / (self.frames.len() as f32)
        }
    }

    /// Access metadata info
    pub fn get_info(&self) -> &Ym6Info {
        &self.info
    }

    /// Human-readable info summary
    pub fn format_info(&self) -> String {
        let duration = self.duration_seconds();
        let frame_count = self.frames.len();
        let info = &self.info;
        format!(
            "  Song: {}\n  Author: {}\n  Comment: {}\n  Duration: {:.2}s ({} frames @ {}Hz)\n  Mode: SoftSynth",
            info.song_name,
            info.author,
            info.comment,
            duration,
            frame_count,
            info.frame_rate
        )
    }
}

impl Default for SoftPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackController for SoftPlayer {
    fn play(&mut self) -> Result<()> {
        if !self.frames.is_empty() {
            self.state = PlaybackState::Playing;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.state = PlaybackState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = PlaybackState::Stopped;
        self.current_frame = 0;
        self.samples_in_frame = 0;
        Ok(())
    }

    fn state(&self) -> PlaybackState {
        self.state
    }
}
