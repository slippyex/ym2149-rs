//! Core playback systems for YM2149 audio.
//!
//! This module contains the main ECS systems that drive YM2149 playback:
//!
//! # Playback Lifecycle
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │ Ym2149Playback  │────▶│ initialize_      │────▶│ PlaybackRuntime │
//! │ (Component)     │     │ playback         │     │ State           │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//!                                │
//!                                ▼
//!                         ┌──────────────────┐
//!                         │ drive_playback_  │
//!                         │ state            │
//!                         └──────────────────┘
//!                                │
//!                                ▼
//!                         ┌──────────────────┐
//!                         │ process_playback │────▶ FrameAudioData
//!                         │ _frames          │      (message event)
//!                         └──────────────────┘
//!                                │
//!                 ┌──────────────┼──────────────┐
//!                 ▼              ▼              ▼
//!          ┌───────────┐  ┌───────────┐  ┌───────────┐
//!          │Diagnostics│  │ Pattern   │  │ Audio     │
//!          │ Events    │  │ Triggers  │  │ Bridge    │
//!          └───────────┘  └───────────┘  └───────────┘
//! ```
//!
//! # Key Types
//!
//! - [`PlaybackRuntimeState`]: Internal per-entity state (frame timing, SFX layer)
//! - [`FrameAudioData`]: Per-frame audio samples and channel metrics
//! - [`SfxLayer`]: Overlay synth for one-shot sound effects

use crate::audio_bridge::{AudioBridgeBuffers, AudioBridgeTargets};
use crate::audio_reactive::AudioReactiveState;
use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::events::{
    BeatHit, ChannelSnapshot, PatternTriggered, PlaybackFrameMarker, TrackFinished, TrackStarted,
    YmSfxRequest,
};
use crate::oscilloscope::OscilloscopeBuffer;
use crate::patterns::{PatternTriggerRuntime, PatternTriggerSet};
use crate::playback::{
    PlaybackMetrics, PlaybackState, YM2149_SAMPLE_RATE_F32, Ym2149Playback, Ym2149Settings,
};
use crate::plugin::Ym2149PluginConfig;
use crate::song_player::{YmSongPlayer, load_song_from_bytes};
use crate::synth::{YmSynthController, YmSynthPlayer};
use bevy::audio::{AudioPlayer, AudioSink, PlaybackSettings};
use bevy::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use ym2149::Ym2149Backend;
use ym2149::util::PSG_MASTER_CLOCK_HZ;
use ym2149::util::channel_frequencies;

// Import from sibling modules
use super::crossfade::{finalize_crossfade, process_pending_crossfade};
use super::loader::{
    PendingFileRead, PendingSlot, SourceLoadResult, current_track_source, load_track_source,
};

// ============================================================================
// Runtime State
// ============================================================================

/// Internal runtime state for a playback entity.
///
/// Tracks frame timing, volume changes, and manages the optional SFX overlay.
/// This component is automatically added by [`initialize_playback`].
#[derive(Component)]
pub(in crate::plugin) struct PlaybackRuntimeState {
    time_since_last_frame: f32,
    last_state: PlaybackState,
    last_volume: f32,
    frames_rendered: u64,
    emitted_finished: bool,
    sfx: Option<SfxLayer>,
}

impl Default for PlaybackRuntimeState {
    fn default() -> Self {
        Self {
            time_since_last_frame: 0.0,
            last_volume: 1.0,
            last_state: PlaybackState::Idle,
            frames_rendered: 0,
            emitted_finished: false,
            sfx: None,
        }
    }
}

struct SfxLayer {
    player: YmSynthPlayer,
    controller: YmSynthController,
    remaining_frames: [u32; 3],
}

impl SfxLayer {
    fn new() -> Self {
        let controller = YmSynthController::new();
        let player = YmSynthPlayer::new(controller.clone());
        Self {
            player,
            controller,
            remaining_frames: [0; 3],
        }
    }

    fn ensure_playing(&mut self) {
        self.player.play();
    }

    fn silence_channel(&self, channel: usize) {
        self.controller.set_volume(channel, 0);
    }

    fn tick_frame(&mut self) {
        for idx in 0..self.remaining_frames.len() {
            let remaining = &mut self.remaining_frames[idx];
            if *remaining == 0 {
                self.silence_channel(idx);
                continue;
            }
            *remaining = remaining.saturating_sub(1);
            if *remaining == 0 {
                self.silence_channel(idx);
            }
        }
    }
}

pub(in crate::plugin) fn emit_playback_diagnostics(
    config: Res<Ym2149PluginConfig>,
    mut frames: MessageReader<FrameAudioData>,
    mut snapshot_events: MessageWriter<ChannelSnapshot>,
    mut oscilloscope_buffer: Option<ResMut<OscilloscopeBuffer>>,
) {
    let emit_snapshots = config.channel_events;
    let mut buffer = oscilloscope_buffer.as_deref_mut();
    if !emit_snapshots && buffer.is_none() {
        return;
    }

    for frame in frames.read() {
        if emit_snapshots && frame.samples_per_frame > 0 {
            let inv_len = 1.0 / frame.samples_per_frame.max(1) as f32;
            for (channel, amplitude) in frame.channel_energy.iter().enumerate() {
                snapshot_events.write(ChannelSnapshot {
                    entity: frame.entity,
                    channel,
                    amplitude: (*amplitude * inv_len).clamp(0.0, 1.0),
                    frequency: frame.frequencies[channel],
                });
            }
        }

        if let Some(buffer) = buffer.as_mut() {
            for sample in frame.channel_samples.iter() {
                buffer.push_sample(*sample);
            }
        }
    }
}

pub(in crate::plugin) fn publish_bridge_audio(
    config: Res<Ym2149PluginConfig>,
    mut frames: MessageReader<FrameAudioData>,
    targets: Option<Res<AudioBridgeTargets>>,
    buffers: Option<ResMut<AudioBridgeBuffers>>,
) {
    if !config.bevy_audio_bridge {
        return;
    }
    let Some(targets) = targets else {
        return;
    };
    let Some(mut buffers) = buffers else {
        return;
    };

    for frame in frames.read() {
        if !targets.0.contains(&frame.entity) {
            continue;
        }
        buffers
            .0
            .insert(frame.entity, frame.stereo.as_ref().to_vec());
    }
}

pub(in crate::plugin) fn emit_frame_markers(
    mut frames: MessageReader<FrameAudioData>,
    mut markers: MessageWriter<PlaybackFrameMarker>,
) {
    for frame in frames.read() {
        markers.write(PlaybackFrameMarker {
            entity: frame.entity,
            frame: frame.frame_index,
            elapsed_seconds: frame.elapsed_seconds,
            looped: frame.looped,
        });
    }
}

pub(in crate::plugin) fn update_audio_reactive_state(
    mut frames: MessageReader<FrameAudioData>,
    mut state: ResMut<AudioReactiveState>,
) {
    const SMOOTHING: f32 = 0.25;
    for frame in frames.read() {
        let entry = state.metrics.entry(frame.entity).or_default();

        let inv_len = 1.0 / frame.samples_per_frame.max(1) as f32;
        for channel in 0..3 {
            let avg = (frame.channel_energy[channel] * inv_len).clamp(0.0, 1.0);
            entry.average[channel] = entry.average[channel] * (1.0 - SMOOTHING) + avg * SMOOTHING;

            let mut peak: f32 = 0.0;
            for sample in frame.channel_samples.iter() {
                peak = peak.max(sample[channel].abs());
            }
            entry.peak[channel] = entry.peak[channel] * (1.0 - SMOOTHING) + peak * SMOOTHING;
        }
        entry.frequencies = frame.frequencies;
    }
}

pub(in crate::plugin) fn detect_pattern_triggers(
    config: Res<Ym2149PluginConfig>,
    mut frames: MessageReader<FrameAudioData>,
    pattern_sets: Query<&PatternTriggerSet>,
    mut runtime: ResMut<PatternTriggerRuntime>,
    mut pattern_events: MessageWriter<PatternTriggered>,
) {
    if !config.pattern_events {
        return;
    }

    for frame in frames.read() {
        let Ok(set) = pattern_sets.get(frame.entity) else {
            runtime.0.remove(&frame.entity);
            continue;
        };

        if set.patterns.is_empty() {
            runtime.0.remove(&frame.entity);
            continue;
        }

        let samples = frame.samples_per_frame.max(1) as f32;
        let entry = runtime.0.entry(frame.entity).or_default();
        if entry.len() < set.patterns.len() {
            entry.resize(set.patterns.len(), u64::MAX);
        }

        for (idx, trigger) in set.patterns.iter().enumerate() {
            let channel = trigger.channel.min(2);
            let avg_amp = (frame.channel_energy[channel] / samples).clamp(0.0, 1.0);
            if avg_amp < trigger.min_amplitude {
                continue;
            }

            if let Some(target) = trigger.frequency_hz {
                let Some(actual) = frame.frequencies[channel] else {
                    continue;
                };
                let tolerance = trigger.frequency_tolerance_hz.max(0.0);
                if (actual - target).abs() > tolerance {
                    continue;
                }
            }

            let last_frame = entry[idx];
            let on_cooldown = last_frame != u64::MAX
                && frame.frame_index < last_frame.saturating_add(trigger.cooldown_frames);
            if on_cooldown {
                continue;
            }

            entry[idx] = frame.frame_index;
            pattern_events.write(PatternTriggered {
                entity: frame.entity,
                pattern_id: trigger.id.clone(),
                channel,
                amplitude: avg_amp,
                frequency: frame.frequencies[channel],
                frame: frame.frame_index,
                elapsed_seconds: frame.elapsed_seconds,
            });
        }
    }
}

pub(in crate::plugin) fn emit_beat_hits(
    mut frames: MessageReader<PlaybackFrameMarker>,
    mut beats: MessageWriter<BeatHit>,
    config: Res<Ym2149PluginConfig>,
) {
    // Derive beats from frame markers; defaults to 50Hz frame rate => set bpm in config later
    let frames_per_beat = (config.frames_per_beat.unwrap_or(50)).max(1);
    for marker in frames.read() {
        if marker.frame % frames_per_beat == 0 {
            beats.write(BeatHit {
                entity: marker.entity,
                beat_index: marker.frame / frames_per_beat,
                elapsed_seconds: marker.elapsed_seconds,
            });
        }
    }
}

fn tone_period_from_hz(freq_hz: f32) -> u16 {
    if freq_hz <= 0.0 {
        return 0;
    }
    let period = (PSG_MASTER_CLOCK_HZ / (16.0 * freq_hz)).round();
    period.clamp(1.0, 0x0FFF as f32).abs() as u16
}

pub(in crate::plugin) fn process_sfx_requests(
    mut requests: MessageReader<YmSfxRequest>,
    mut playbacks: Query<(Entity, &Ym2149Playback, &mut PlaybackRuntimeState)>,
) {
    for request in requests.read() {
        for (entity, _pb, mut runtime) in playbacks.iter_mut() {
            if let Some(target) = request.target
                && target != entity
            {
                continue;
            }
            let sfx = runtime.sfx.get_or_insert_with(SfxLayer::new);
            let channel = request.channel.min(2);
            let volume = request.volume.clamp(0.0, 1.0);
            let period = tone_period_from_hz(request.freq_hz);

            sfx.controller.set_mixer(0x38); // enable all tones, mute all noise
            sfx.controller.set_tone_period(channel, period);
            let vol_reg = (volume * 15.0).round().clamp(0.0, 15.0) as u8;
            sfx.controller.set_volume(channel, vol_reg);
            sfx.remaining_frames[channel] = request.duration_frames.max(1);
            sfx.ensure_playing();
        }
    }
}

impl PlaybackRuntimeState {
    pub(super) fn reset(&mut self) {
        self.time_since_last_frame = 0.0;
        self.frames_rendered = 0;
        self.emitted_finished = false;
        self.last_state = PlaybackState::Idle;
    }

    pub(super) fn reset_for_crossfade(&mut self) {
        self.time_since_last_frame = 0.0;
        self.frames_rendered = 0;
        self.emitted_finished = false;
    }
}

#[derive(Clone, Message)]
pub(crate) struct FrameAudioData {
    pub entity: Entity,
    pub frame_index: u64,
    pub elapsed_seconds: f32,
    pub looped: bool,
    pub stereo: Arc<[f32]>,
    pub channel_samples: Arc<[[f32; 3]]>,
    pub channel_energy: [f32; 3],
    pub frequencies: [Option<f32>; 3],
    pub samples_per_frame: usize,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::plugin) fn initialize_playback(
    mut commands: Commands,
    mut playbacks: Query<(
        Entity,
        &mut Ym2149Playback,
        Option<&mut PlaybackRuntimeState>,
    )>,
    mut audio_assets: ResMut<Assets<Ym2149AudioSource>>,
    mut pending_reads: Local<HashMap<(Entity, PendingSlot), PendingFileRead>>,
) {
    let mut alive = Vec::new();

    for (entity, mut playback, runtime_state) in playbacks.iter_mut() {
        alive.push(entity);

        if runtime_state.is_none() {
            commands
                .entity(entity)
                .insert(PlaybackRuntimeState::default());
        } else if (playback.player.is_none() || playback.needs_reload)
            && let Some(mut rt) = runtime_state
        {
            rt.reset();
        }

        if playback.inline_player
            && !playback.inline_audio_ready
            && let Some(player_arc) = playback.player.clone()
        {
            let metadata = playback
                .inline_metadata
                .clone()
                .unwrap_or_else(|| Ym2149Metadata {
                    title: playback.song_title.clone(),
                    author: playback.song_author.clone(),
                    comment: String::new(),
                    frame_count: playback
                        .metrics
                        .as_ref()
                        .map(|m| m.frame_count)
                        .unwrap_or_default(),
                    duration_seconds: 0.0,
                });
            let total_samples = playback
                .metrics
                .as_ref()
                .map(|m| m.total_samples())
                .unwrap_or(usize::MAX);
            let audio_source = Ym2149AudioSource::from_shared_player(
                player_arc,
                metadata,
                total_samples,
            );
            let audio_handle = audio_assets.add(audio_source);
            let settings = if playback.state == PlaybackState::Playing {
                PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(playback.volume))
            } else {
                PlaybackSettings::LOOP
                    .paused()
                    .with_volume(bevy::audio::Volume::Linear(playback.volume))
            };
            commands
                .entity(entity)
                .insert((AudioPlayer(audio_handle), settings));
            playback.inline_audio_ready = true;
        }

        // If a new player is already prepared (e.g., crossfade finalized), refresh only the audio source.
        if playback.needs_reload
            && playback.player.is_some()
            && playback.metrics.is_some()
            && !playback.inline_player
        {
            let player_arc = playback.player.clone().unwrap();
            let metrics = playback.metrics.unwrap();
            let metadata = Ym2149Metadata {
                title: playback.song_title.clone(),
                author: playback.song_author.clone(),
                comment: String::new(),
                frame_count: metrics.frame_count,
                duration_seconds: metrics.duration_seconds(),
            };
            let total_samples = metrics.total_samples();

            let audio_source = Ym2149AudioSource::from_shared_player(
                player_arc,
                metadata,
                total_samples,
            );
            let audio_handle = audio_assets.add(audio_source);

            // Remove old AudioPlayer and AudioSink components if they exist
            commands
                .entity(entity)
                .remove::<AudioPlayer>()
                .remove::<bevy::audio::AudioSink>();

            let settings = if playback.state == PlaybackState::Playing {
                PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(playback.volume))
            } else {
                PlaybackSettings::LOOP
                    .paused()
                    .with_volume(bevy::audio::Volume::Linear(playback.volume))
            };

            commands
                .entity(entity)
                .insert((AudioPlayer(audio_handle), settings));

            playback.needs_reload = false;
            continue;
        }

        if playback.player.is_none() || playback.needs_reload {
            if playback.source_path().is_none() {
                pending_reads.remove(&(entity, PendingSlot::Primary));
            }

            let Some(source) = current_track_source(&playback) else {
                continue;
            };

            let loaded = match load_track_source(
                entity,
                PendingSlot::Primary,
                &source,
                &mut pending_reads,
                audio_assets.as_ref(),
            ) {
                SourceLoadResult::Pending => continue,
                SourceLoadResult::Failed(err) => {
                    error!("{err}");
                    continue;
                }
                SourceLoadResult::Ready(bytes) => bytes,
            };

            // Clone data for both player and audio source creation
            let data_for_audio = loaded.data.clone();

            let mut load = match load_player_from_bytes(&loaded.data, loaded.metadata.as_ref()) {
                Ok(load) => load,
                Err(err) => {
                    error!("Failed to initialize YM2149 player: {}", err);
                    continue;
                }
            };

            playback.song_title = load.metadata.title.clone();
            playback.song_author = load.metadata.author.clone();
            playback.metrics = Some(load.metrics);

            if playback.state == PlaybackState::Playing
                && let Err(e) = load.player.play()
            {
                error!("Failed to start player: {}", e);
            }

            let player_arc = Arc::new(RwLock::new(load.player));
            // Diagnostics/crossfade use this player; audio playback uses the audio source below
            playback.player = Some(player_arc);
            playback.needs_reload = false;

            // Create a Ym2149AudioSource asset from the loaded data
            let audio_source = match Ym2149AudioSource::new_with_shared(
                data_for_audio,
                playback.stereo_gain.clone(),
                playback.tone_settings.clone(),
            ) {
                Ok(source) => source,
                Err(err) => {
                    error!("Failed to create audio source: {}", err);
                    continue;
                }
            };

            // Add the asset and get a handle
            let audio_handle = audio_assets.add(audio_source);

            // Remove old AudioPlayer and AudioSink components if they exist
            commands
                .entity(entity)
                .remove::<AudioPlayer>()
                .remove::<bevy::audio::AudioSink>();

            let settings = if playback.state == PlaybackState::Playing {
                PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(playback.volume))
            } else {
                PlaybackSettings::LOOP
                    .paused()
                    .with_volume(bevy::audio::Volume::Linear(playback.volume))
            };

            commands
                .entity(entity)
                .insert((AudioPlayer(audio_handle), settings));

            info!(
                "Loaded YM song: {} frames, {} samples/frame",
                load.metrics.frame_count, load.metrics.samples_per_frame
            );
        }

        process_pending_crossfade(
            &mut commands,
            audio_assets.as_mut(),
            entity,
            &mut playback,
            &mut pending_reads,
        );
    }

    pending_reads.retain(|(entity, _), _| alive.contains(entity));
}

pub(in crate::plugin) fn drive_playback_state(
    mut playbacks: Query<(Entity, &Ym2149Playback, &mut PlaybackRuntimeState)>,
    config: Res<Ym2149PluginConfig>,
    mut started_events: MessageWriter<TrackStarted>,
    mut finished_events: MessageWriter<TrackFinished>,
    mut audio_sinks: Query<&mut AudioSink>,
) {
    for (entity, playback, mut runtime) in playbacks.iter_mut() {
        let Some(player_arc) = playback.player.clone() else {
            continue;
        };

        let mut player = player_arc.write();
        let crossfade_arc = playback
            .crossfade
            .as_ref()
            .map(|state| state.player.clone());
        let mut crossfade_player = crossfade_arc.as_ref().map(|arc| arc.write());

        if runtime.last_state == playback.state {
            continue;
        }

        match playback.state {
            PlaybackState::Playing => {
                runtime.time_since_last_frame = 0.0;
                runtime.emitted_finished = false;
                if let Err(err) = player.play() {
                    error!("Failed to resume YM playback: {}", err);
                }
                if let Some(cf) = crossfade_player.as_mut()
                    && let Err(err) = cf.play()
                {
                    error!("Failed to resume crossfade deck: {}", err);
                }
                if let Ok(sink) = audio_sinks.get_mut(entity) {
                    sink.play();
                } else {
                    warn!(
                        "No AudioSink found for entity {:?} - audio may not play",
                        entity
                    );
                }
                if config.channel_events {
                    started_events.write(TrackStarted { entity });
                }
            }
            PlaybackState::Paused => {
                if let Err(err) = player.pause() {
                    error!("Failed to pause YM playback: {}", err);
                }
                if let Some(cf) = crossfade_player.as_mut()
                    && let Err(err) = cf.pause()
                {
                    error!("Failed to pause crossfade deck: {}", err);
                }
                if let Ok(sink) = audio_sinks.get_mut(entity) {
                    sink.pause();
                }
            }
            PlaybackState::Idle => {
                if let Err(err) = player.pause() {
                    error!("Failed to stop YM playback: {}", err);
                }
                if let Some(cf) = crossfade_player.as_mut()
                    && let Err(err) = cf.pause()
                {
                    error!("Failed to stop crossfade deck: {}", err);
                }
                if let Ok(sink) = audio_sinks.get_mut(entity) {
                    sink.pause();
                }
                runtime.time_since_last_frame = 0.0;
                runtime.emitted_finished = false;
            }
            PlaybackState::Finished => {
                if let Ok(sink) = audio_sinks.get_mut(entity) {
                    sink.pause();
                }
                if let Some(cf) = crossfade_player.as_mut()
                    && let Err(err) = cf.pause()
                {
                    error!("Failed to pause crossfade deck: {}", err);
                }
                if config.channel_events && !runtime.emitted_finished {
                    finished_events.write(TrackFinished { entity });
                    runtime.emitted_finished = true;
                }
            }
        }

        runtime.last_state = playback.state;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::plugin) fn process_playback_frames(
    mut commands: Commands,
    mut playbacks: Query<(Entity, &mut Ym2149Playback, &mut PlaybackRuntimeState)>,
    settings: Res<Ym2149Settings>,
    config: Res<Ym2149PluginConfig>,
    time: Res<Time>,
    mut started_events: MessageWriter<TrackStarted>,
    mut finished_events: MessageWriter<TrackFinished>,
    mut audio_sinks: Query<&mut AudioSink>,
    mut frame_events: MessageWriter<FrameAudioData>,
) {
    let delta = time.delta_secs();
    let master_volume = settings.master_volume.clamp(0.0, 1.0);

    for (entity, mut playback, mut runtime) in playbacks.iter_mut() {
        let Some(player_arc) = playback.player.clone() else {
            continue;
        };

        let mut player = player_arc.write();
        let crossfade_arc = playback
            .crossfade
            .as_ref()
            .map(|state| state.player.clone());
        let mut crossfade_player = crossfade_arc.as_ref().map(|arc| arc.write());

        if (runtime.last_volume - playback.volume).abs() > 0.001 {
            if let Ok(mut sink) = audio_sinks.get_mut(entity) {
                sink.set_volume(bevy::audio::Volume::Linear(playback.volume));
            }
            runtime.last_volume = playback.volume;
        }

        if playback.state != PlaybackState::Playing {
            playback.seek(player.get_current_frame() as u32);
            continue;
        }

        runtime.time_since_last_frame += delta;

        let samples_per_frame = player.samples_per_frame_value() as usize;
        if samples_per_frame == 0 {
            continue;
        }

        let frame_duration = samples_per_frame as f32 / YM2149_SAMPLE_RATE_F32;

        while runtime.time_since_last_frame >= frame_duration {
            runtime.time_since_last_frame -= frame_duration;
            runtime.frames_rendered += 1;

            let prev_frame = playback.frame_position;
            playback.frame_position = player.get_current_frame() as u32;

            let mut stereo_samples = Vec::with_capacity(samples_per_frame * 2);
            let mut channel_samples = Vec::with_capacity(samples_per_frame);
            let mut channel_energy = [0.0f32; 3];
            let gain = (playback.volume * master_volume).clamp(0.0, 1.0);
            let left_gain = playback.left_gain.clamp(0.0, 1.0);
            let right_gain = playback.right_gain.clamp(0.0, 1.0);
            let (primary_mix, secondary_mix) = playback
                .crossfade
                .as_ref()
                .map(|cf| {
                    if cf.duration <= f32::EPSILON {
                        (0.0, 1.0)
                    } else {
                        let ratio = (cf.elapsed / cf.duration).clamp(0.0, 1.0);
                        (1.0 - ratio, ratio)
                    }
                })
                .unwrap_or((1.0, 0.0));

            for _ in 0..samples_per_frame {
                let sample = player.generate_sample();
                let (mut ch_a, mut ch_b, mut ch_c) = player
                    .chip()
                    .map(|chip| chip.get_channel_outputs())
                    .unwrap_or((0.0, 0.0, 0.0));

                if playback.volume != 1.0 {
                    ch_a *= playback.volume;
                    ch_b *= playback.volume;
                    ch_c *= playback.volume;
                }
                if master_volume != 1.0 {
                    ch_a *= master_volume;
                    ch_b *= master_volume;
                    ch_c *= master_volume;
                }

                channel_energy[0] += ch_a.abs();
                channel_energy[1] += ch_b.abs();
                channel_energy[2] += ch_c.abs();
                channel_samples.push([ch_a, ch_b, ch_c]);

                let mut mixed = sample * primary_mix;
                if secondary_mix > 0.0
                    && let Some(secondary) = crossfade_player.as_mut()
                {
                    mixed += secondary.generate_sample() * secondary_mix;
                }
                if let Some(sfx) = runtime.sfx.as_mut() {
                    mixed += sfx.player.generate_sample();
                }

                let scaled = mixed * gain;
                stereo_samples.push(scaled * left_gain);
                stereo_samples.push(scaled * right_gain);
            }

            let frequencies = player
                .chip()
                .map(|chip| channel_frequencies(&chip.dump_registers()))
                .unwrap_or([None; 3]);

            let elapsed_seconds = runtime.frames_rendered as f32 * frame_duration;
            let looped = playback.frame_position < prev_frame;
            frame_events.write(FrameAudioData {
                entity,
                frame_index: runtime.frames_rendered,
                elapsed_seconds,
                looped,
                stereo: Arc::<[f32]>::from(stereo_samples.into_boxed_slice()),
                channel_samples: Arc::<[[f32; 3]]>::from(channel_samples.into_boxed_slice()),
                channel_energy,
                frequencies,
                samples_per_frame,
            });
            if let Some(sfx) = runtime.sfx.as_mut() {
                sfx.tick_frame();
            }

            if let Some(state) = playback.crossfade.as_mut() {
                state.elapsed = (state.elapsed + frame_duration).min(state.duration);

                if state.duration > f32::EPSILON {
                    let fade_ratio = (state.elapsed / state.duration).clamp(0.0, 1.0);
                    let fade_out_volume = 1.0 - fade_ratio;
                    let fade_in_volume = fade_ratio;
                    let cf_entity_opt = state.crossfade_entity;

                    if let Ok(mut sink) = audio_sinks.get_mut(entity) {
                        sink.set_volume(bevy::audio::Volume::Linear(fade_out_volume));
                    }
                    playback.volume = fade_out_volume;
                    runtime.last_volume = fade_out_volume;

                    if let Some(cf_entity) = cf_entity_opt
                        && let Ok(mut cf_sink) = audio_sinks.get_mut(cf_entity)
                    {
                        cf_sink.set_volume(bevy::audio::Volume::Linear(fade_in_volume));
                    }
                }
            }
        }

        playback.seek(player.get_current_frame() as u32);
        let crossfade_complete = playback
            .crossfade
            .as_ref()
            .map(|cf| cf.elapsed >= cf.duration)
            .unwrap_or(false);

        if crossfade_complete {
            drop(player);
            drop(crossfade_player);
            finalize_crossfade(
                &mut commands,
                entity,
                &mut playback,
                &mut runtime,
                &config,
                &mut started_events,
                &mut finished_events,
                &mut audio_sinks,
            );
            continue;
        }

        let player_state = player.state();

        if player_state != ym2149_ym_replayer::PlaybackState::Playing
            && playback.state == PlaybackState::Playing
        {
            runtime.time_since_last_frame = 0.0;

            if settings.loop_enabled {
                match player.stop().and_then(|_| player.play()) {
                    Ok(()) => {
                        runtime.frames_rendered = 0;
                        playback.seek(0);
                        runtime.emitted_finished = false;
                        if config.channel_events {
                            started_events.write(TrackStarted { entity });
                        }
                    }
                    Err(err) => {
                        error!("Failed to loop YM playback: {}", err);
                        playback.state = PlaybackState::Finished;
                    }
                }
            } else {
                if let Err(err) = player.stop() {
                    error!("Failed to stop YM playback: {}", err);
                }
                playback.seek(0);
                playback.state = PlaybackState::Finished;
                if config.channel_events && !runtime.emitted_finished {
                    finished_events.write(TrackFinished { entity });
                    runtime.emitted_finished = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::PatternTrigger;
    use bevy::prelude::Messages;
    use std::sync::Arc;

    fn drain_hits(app: &mut App) -> Vec<PatternTriggered> {
        let mut events = app.world_mut().resource_mut::<Messages<PatternTriggered>>();
        events.drain().collect()
    }

    fn send_frame(
        app: &mut App,
        entity: Entity,
        frame_index: u64,
        amplitude: f32,
        freq: Option<f32>,
    ) {
        let mut events = app.world_mut().resource_mut::<Messages<FrameAudioData>>();
        events.write(FrameAudioData {
            entity,
            frame_index,
            elapsed_seconds: frame_index as f32 * 0.02,
            looped: false,
            stereo: Arc::from(vec![0.0; 2].into_boxed_slice()),
            channel_samples: Arc::from(vec![[0.0; 3]; 1].into_boxed_slice()),
            channel_energy: [amplitude, 0.0, 0.0],
            frequencies: [freq, None, None],
            samples_per_frame: 1,
        });
    }

    #[test]
    fn pattern_trigger_emits_and_respects_cooldown() {
        let mut app = App::new();
        app.insert_resource(Ym2149PluginConfig {
            pattern_events: true,
            ..Default::default()
        });
        app.add_message::<FrameAudioData>();
        app.add_message::<PatternTriggered>();
        app.insert_resource(PatternTriggerRuntime::default());

        let entity = app
            .world_mut()
            .spawn(PatternTriggerSet::from_patterns(vec![
                PatternTrigger::new("lead", 0)
                    .with_min_amplitude(0.2)
                    .with_frequency(440.0, 5.0)
                    .with_cooldown(2),
            ]))
            .id();

        app.add_systems(Update, detect_pattern_triggers);

        send_frame(&mut app, entity, 1, 0.4, Some(441.0));
        app.update();
        assert_eq!(drain_hits(&mut app).len(), 1);

        // Within cooldown -> suppressed
        send_frame(&mut app, entity, 2, 0.4, Some(441.0));
        app.update();
        assert!(drain_hits(&mut app).is_empty());

        // After cooldown -> fires again
        send_frame(&mut app, entity, 4, 0.4, Some(441.0));
        app.update();
        assert_eq!(drain_hits(&mut app).len(), 1);
    }
}

pub(super) struct LoadResult {
    pub(super) player: YmSongPlayer,
    pub(super) metrics: PlaybackMetrics,
    pub(super) metadata: Ym2149Metadata,
}

pub(super) fn load_player_from_bytes(
    data: &[u8],
    override_metadata: Option<&Ym2149Metadata>,
) -> Result<LoadResult, String> {
    let (player, metrics, mut metadata) = load_song_from_bytes(data)?;
    if let Some(meta) = override_metadata {
        metadata.title = meta.title.clone();
        metadata.author = meta.author.clone();
        metadata.comment = meta.comment.clone();
    }
    Ok(LoadResult {
        player,
        metrics,
        metadata,
    })
}
