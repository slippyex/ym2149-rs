use crate::audio_bridge::{AudioBridgeBuffers, AudioBridgeTargets};
use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::events::{ChannelSnapshot, TrackFinished, TrackStarted};
use crate::oscilloscope::OscilloscopeBuffer;
use crate::playback::{
    PlaybackMetrics, PlaybackState, YM2149_SAMPLE_RATE_F32, Ym2149Playback, Ym2149Settings,
};
use crate::plugin::Ym2149PluginConfig;
use crate::song_player::{YmSongPlayer, load_song_from_bytes};
use bevy::audio::{AudioPlayer, AudioSink, PlaybackSettings};
use bevy::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

// Import from sibling modules
use super::crossfade::{finalize_crossfade, process_pending_crossfade};
use super::loader::{
    PendingFileRead, PendingSlot, SourceLoadResult, current_track_source, load_track_source,
};
use ym2149::util::channel_frequencies;

#[derive(Component, Clone, Copy)]
pub(in crate::plugin) struct PlaybackRuntimeState {
    time_since_last_frame: f32,
    last_state: PlaybackState,
    last_volume: f32,
    frames_rendered: u64,
    emitted_finished: bool,
}

impl Default for PlaybackRuntimeState {
    fn default() -> Self {
        Self {
            time_since_last_frame: 0.0,
            last_volume: 1.0,
            last_state: PlaybackState::Idle,
            frames_rendered: 0,
            emitted_finished: false,
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
                playback.stereo_gain.clone(),
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

            let mut load =
                match load_player_from_bytes(loaded.data.clone(), loaded.metadata.as_ref()) {
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
            let audio_source = match Ym2149AudioSource::new_with_gains(
                data_for_audio,
                playback.stereo_gain.clone(),
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

                let scaled = mixed * gain;
                stereo_samples.push(scaled * left_gain);
                stereo_samples.push(scaled * right_gain);
            }

            let frequencies = player
                .chip()
                .map(|chip| channel_frequencies(&chip.dump_registers()))
                .unwrap_or([None; 3]);

            frame_events.write(FrameAudioData {
                entity,
                stereo: Arc::<[f32]>::from(stereo_samples.into_boxed_slice()),
                channel_samples: Arc::<[[f32; 3]]>::from(channel_samples.into_boxed_slice()),
                channel_energy,
                frequencies,
                samples_per_frame,
            });

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
            );
            continue;
        }

        let player_state = player.state();

        if player_state != ym_replayer::PlaybackState::Playing
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

pub(super) struct LoadResult {
    pub(super) player: YmSongPlayer,
    pub(super) metrics: PlaybackMetrics,
    pub(super) metadata: Ym2149Metadata,
}

pub(super) fn load_player_from_bytes(
    data: Vec<u8>,
    override_metadata: Option<&Ym2149Metadata>,
) -> Result<LoadResult, String> {
    let (player, metrics, mut metadata) = load_song_from_bytes(&data)?;
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
