use crate::gist::gist_sound::GistSound;

/// Voice state - maps directly to the 140-byte structure in the original driver
/// All offsets are in bytes from the structure start
#[derive(Clone, Debug, Default)]
pub struct Voice {
    pub inuse: i16,

    // basically all fields from the original structure
    // NOTE: They change during playback.
    pub freq: i16,
    pub noise_freq: i16,
    pub volume: i16,

    pub vol_phase: i16,
    pub vol_attack: i32,
    pub vol_decay: i32,
    pub vol_sustain: i32,
    pub vol_release: i32,
    pub vol_lfo_limit: i32,
    pub vol_lfo_step: i32,
    pub vol_lfo_delay: i16,

    pub freq_phase: i16,
    pub freq_attack: i32,
    pub freq_attack_target: i32,
    pub freq_decay: i32,
    pub freq_decay_target: i32,
    pub freq_release: i32,
    pub freq_lfo_limit: i32,
    pub freq_lfo_step: i32,
    pub freq_lfo_reset_pos: i32,
    pub freq_lfo_limit_neg: i32,
    pub freq_lfo_reset_neg: i32,
    pub freq_lfo_delay: i16,

    pub noise_phase: i16,
    pub noise_attack: i32,
    pub noise_attack_target: i32,
    pub noise_decay: i32,
    pub noise_decay_target: i32,
    pub noise_release: i32,
    pub noise_lfo_limit: i32,
    pub noise_lfo_step: i32,
    pub noise_lfo_delay: i16,

    pub pitch: i16,
    pub priority: i16,
    pub vol_env_acc: i32,
    pub vol_lfo_acc: i32,
    pub freq_env_acc: i32,
    pub freq_lfo_acc: i32,
    pub noise_env_acc: i32,
    pub noise_lfo_acc: i32,
}

impl Voice {
    pub fn from_sound(&mut self, tpl: &GistSound, pitch: i16, priority: i16, volume: Option<i16>) {
        // Copy sound parameters
        self.freq = tpl.initial_freq;
        self.noise_freq = tpl.initial_noise_freq;
        self.volume = volume.unwrap_or(tpl.initial_volume);
        self.vol_phase = tpl.vol_phase;
        self.vol_attack = tpl.vol_attack;
        self.vol_decay = tpl.vol_decay;
        self.vol_sustain = tpl.vol_sustain;
        self.vol_release = tpl.vol_release;
        self.vol_lfo_limit = tpl.vol_lfo_limit;
        self.vol_lfo_step = tpl.vol_lfo_step;
        self.vol_lfo_delay = tpl.vol_lfo_delay;
        self.freq_phase = tpl.freq_env_phase;
        self.freq_attack = tpl.freq_attack;
        self.freq_attack_target = tpl.freq_attack_target;
        self.freq_decay = tpl.freq_decay;
        self.freq_decay_target = tpl.freq_decay_target;
        self.freq_release = tpl.freq_release;
        self.freq_lfo_limit = tpl.freq_lfo_limit;
        self.freq_lfo_step = tpl.freq_lfo_step;
        self.freq_lfo_reset_pos = tpl.freq_lfo_reset_positive;
        self.freq_lfo_limit_neg = tpl.freq_lfo_negative_limit;
        self.freq_lfo_reset_neg = tpl.freq_lfo_reset_negative;
        self.freq_lfo_delay = tpl.freq_lfo_delay;
        self.noise_phase = tpl.noise_env_phase;
        self.noise_attack = tpl.noise_attack;
        self.noise_attack_target = tpl.noise_attack_target;
        self.noise_decay = tpl.noise_decay;
        self.noise_decay_target = tpl.noise_decay_target;
        self.noise_release = tpl.noise_release;
        self.noise_lfo_limit = tpl.noise_lfo_limit;
        self.noise_lfo_step = tpl.noise_lfo_step;
        self.noise_lfo_delay = tpl.noise_lfo_delay;

        // Set runtime fields
        self.pitch = pitch;
        self.priority = priority;

        // Clear all accumulators (from snd_on: sndptr->o116 = sndptr->o120 = ... = 0)
        self.vol_env_acc = 0;
        self.vol_lfo_acc = 0;
        self.freq_env_acc = 0;
        self.freq_lfo_acc = 0;
        self.noise_env_acc = 0;
        self.noise_lfo_acc = 0;
    }
}
