use crate::audio_source::Ym2149Metadata;
use crate::playback::{PlaybackMetrics, YM2149_SAMPLE_RATE};
use parking_lot::RwLock;
use std::sync::Arc;
use ym2149::Ym2149Backend;
use ym2149::ym2149::Ym2149;
use ym2149_ym_replayer::PlaybackState as YmPlaybackState;

const DEFAULT_SAMPLES_PER_FRAME: u32 = YM2149_SAMPLE_RATE / 50;

#[derive(Default)]
struct SynthState {
    registers: [u8; 16],
    dirty_mask: u16,
}

/// Thread-safe controller that allows direct writes to the PSG registers.
#[derive(Clone, Default)]
pub struct YmSynthController {
    inner: Arc<RwLock<SynthState>>,
}

impl YmSynthController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        let mut state = self.inner.write();
        state.registers = [0; 16];
        state.dirty_mask = u16::MAX;
    }

    pub fn write_register(&self, addr: u8, value: u8) {
        let index = (addr & 0x0F) as usize;
        let mut state = self.inner.write();
        if state.registers[index] != value {
            state.registers[index] = value;
            state.dirty_mask |= 1 << index;
        }
    }

    pub fn set_tone_period(&self, channel: usize, period: u16) {
        if channel > 2 {
            return;
        }
        let lo = (period & 0xFF) as u8;
        let hi = ((period >> 8) & 0x0F) as u8;
        self.write_register((channel * 2) as u8, lo);
        self.write_register((channel * 2 + 1) as u8, hi);
    }

    pub fn set_noise_period(&self, period: u8) {
        self.write_register(0x06, period & 0x1F);
    }

    pub fn set_mixer(&self, mask: u8) {
        self.write_register(0x07, mask & 0x3F);
    }

    pub fn set_volume(&self, channel: usize, volume: u8) {
        if channel > 2 {
            return;
        }
        let reg = 0x08 + channel as u8;
        self.write_register(reg, volume & 0x1F);
    }

    pub fn set_envelope_period(&self, period: u16) {
        self.write_register(0x0B, (period & 0xFF) as u8);
        self.write_register(0x0C, ((period >> 8) & 0xFF) as u8);
    }

    pub fn trigger_envelope(&self, shape: u8) {
        self.write_register(0x0D, shape & 0x0F);
    }

    /// Atomically apply a snapshot of register values guarded by a bitmask.
    ///
    /// This acquires the lock once and marks all changed registers as dirty in
    /// a single pass, which avoids exposing partially updated state to the
    /// audio thread when driving the chip from frame-based SFX code.
    pub fn write_snapshot(&self, registers: &[u8; 16], mask: u16) {
        if mask == 0 {
            return;
        }

        let mut state = self.inner.write();
        let mut changed_mask = 0u16;

        for (idx, value) in registers.iter().enumerate().take(16) {
            if (mask & (1u16 << idx)) == 0 {
                continue;
            }
            if state.registers[idx] != *value {
                state.registers[idx] = *value;
                changed_mask |= 1u16 << idx;
            }
        }

        state.dirty_mask |= changed_mask;
    }
}

struct SynthShared {
    controller: YmSynthController,
}

/// Live PSG driver built on the YM2149 emulator.
pub struct YmSynthPlayer {
    chip: Ym2149,
    shared: SynthShared,
    state: YmPlaybackState,
    metadata: Ym2149Metadata,
    metrics: PlaybackMetrics,
    samples_per_frame: u32,
    samples_into_frame: u32,
    frame_index: usize,
}

impl YmSynthPlayer {
    pub fn new(controller: YmSynthController) -> Self {
        let metadata = Ym2149Metadata {
            title: "Live PSG Synth".into(),
            author: "bevy_ym2149".into(),
            comment: "Procedural YM2149 source".into(),
            frame_count: u32::MAX as usize,
            duration_seconds: 0.0,
        };
        let metrics = PlaybackMetrics {
            frame_count: metadata.frame_count,
            samples_per_frame: DEFAULT_SAMPLES_PER_FRAME,
        };
        Self {
            chip: Ym2149::new(),
            shared: SynthShared { controller },
            state: YmPlaybackState::Stopped,
            metadata,
            metrics,
            samples_per_frame: DEFAULT_SAMPLES_PER_FRAME,
            samples_into_frame: 0,
            frame_index: 0,
        }
    }

    pub fn controller(&self) -> YmSynthController {
        self.shared.controller.clone()
    }

    pub fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    pub fn metrics(&self) -> PlaybackMetrics {
        self.metrics
    }

    pub fn play(&mut self) {
        self.state = YmPlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        self.state = YmPlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        self.state = YmPlaybackState::Stopped;
    }

    pub fn state(&self) -> YmPlaybackState {
        self.state
    }

    pub fn current_frame(&self) -> usize {
        self.frame_index
    }

    pub fn samples_per_frame_value(&self) -> u32 {
        self.samples_per_frame
    }

    pub fn chip(&self) -> &Ym2149 {
        &self.chip
    }

    pub fn chip_mut(&mut self) -> &mut Ym2149 {
        &mut self.chip
    }

    pub fn generate_sample(&mut self) -> f32 {
        if self.state != YmPlaybackState::Playing {
            return 0.0;
        }
        self.sync_registers();
        self.chip.clock();
        self.advance_frame_counter();
        self.chip.get_sample()
    }

    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        if self.state != YmPlaybackState::Playing {
            buffer.fill(0.0);
            return;
        }
        for sample in buffer.iter_mut() {
            self.sync_registers();
            self.chip.clock();
            self.advance_frame_counter();
            *sample = self.chip.get_sample();
        }
    }

    fn sync_registers(&mut self) {
        let mut state = self.shared.controller.inner.write();
        let mask = state.dirty_mask;
        if mask == 0 {
            return;
        }
        for idx in 0..16 {
            if (mask & (1 << idx)) != 0 {
                self.chip.write_register(idx as u8, state.registers[idx]);
            }
        }
        state.dirty_mask = 0;
    }

    fn advance_frame_counter(&mut self) {
        self.samples_into_frame += 1;
        if self.samples_into_frame >= self.samples_per_frame {
            self.samples_into_frame -= self.samples_per_frame;
            self.frame_index = self.frame_index.wrapping_add(1);
        }
    }
}
