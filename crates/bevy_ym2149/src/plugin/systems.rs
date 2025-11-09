use crate::audio_bridge::{AudioBridgeBuffers, AudioBridgeTargets};
use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::events::{ChannelSnapshot, TrackFinished, TrackStarted};
use crate::oscilloscope::OscilloscopeBuffer;
use crate::playback::{
    ActiveCrossfade, PlaybackMetrics, PlaybackState, TrackSource, YM2149_SAMPLE_RATE_F32,
    Ym2149Playback, Ym2149Settings,
};
use crate::plugin::Ym2149PluginConfig;
use bevy::audio::{AudioPlayer, AudioSink, PlaybackSettings};
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};
use parking_lot::Mutex;
use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;
use ym_replayer::PlaybackController;

const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;

#[derive(Clone, Copy)]
pub(super) struct PlaybackRuntimeState {
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

pub(super) struct PendingFileRead {
    path: String,
    task: Task<Result<Vec<u8>, String>>,
}

impl PendingFileRead {
    fn new(path: String) -> Self {
        let task_path = path.clone();
        let task = IoTaskPool::get().spawn(async move {
            std::fs::read(&task_path)
                .map_err(|err| format!("Failed to read YM file '{task_path}': {err}"))
        });
        Self { path, task }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PendingSlot {
    Primary,
    Crossfade,
}

struct LoadedBytes {
    data: Vec<u8>,
    metadata: Option<Ym2149Metadata>,
}

enum SourceLoadResult {
    Pending,
    Ready(LoadedBytes),
    Failed(String),
}

#[derive(Default)]
pub(super) struct PlaybackScratch {
    stereo: Vec<f32>,
}

fn load_track_source(
    entity: Entity,
    slot: PendingSlot,
    source: &TrackSource,
    pending_reads: &mut HashMap<(Entity, PendingSlot), PendingFileRead>,
    assets: &Assets<Ym2149AudioSource>,
) -> SourceLoadResult {
    match source {
        TrackSource::Bytes(bytes) => SourceLoadResult::Ready(LoadedBytes {
            data: bytes.as_ref().clone(),
            metadata: None,
        }),
        TrackSource::File(path) => match pending_reads.entry((entity, slot)) {
            Entry::Occupied(mut entry) => {
                if entry.get().path != *path {
                    entry.insert(PendingFileRead::new(path.clone()));
                    return SourceLoadResult::Pending;
                }

                match block_on(poll_once(&mut entry.get_mut().task)) {
                    Some(Ok(bytes)) => {
                        pending_reads.remove(&(entity, slot));
                        SourceLoadResult::Ready(LoadedBytes {
                            data: bytes,
                            metadata: None,
                        })
                    }
                    Some(Err(err)) => {
                        pending_reads.remove(&(entity, slot));
                        SourceLoadResult::Failed(err)
                    }
                    None => SourceLoadResult::Pending,
                }
            }
            Entry::Vacant(vacant) => {
                vacant.insert(PendingFileRead::new(path.clone()));
                SourceLoadResult::Pending
            }
        },
        TrackSource::Asset(handle) => match assets.get(handle) {
            Some(asset) => SourceLoadResult::Ready(LoadedBytes {
                data: asset.data.clone(),
                metadata: Some(asset.metadata.clone()),
            }),
            None => SourceLoadResult::Pending,
        },
    }
}

fn current_track_source(playback: &Ym2149Playback) -> Option<TrackSource> {
    playback
        .source_bytes()
        .map(TrackSource::Bytes)
        .or_else(|| {
            playback
                .source_path()
                .map(|path| TrackSource::File(path.to_owned()))
        })
        .or_else(|| playback.source_asset().cloned().map(TrackSource::Asset))
}

fn process_pending_crossfade(
    commands: &mut Commands,
    audio_assets: &mut Assets<Ym2149AudioSource>,
    entity: Entity,
    playback: &mut Ym2149Playback,
    pending_reads: &mut HashMap<(Entity, PendingSlot), PendingFileRead>,
) {
    if playback.crossfade.is_some() {
        return;
    }

    let Some(request) = playback.pending_crossfade.clone() else {
        pending_reads.remove(&(entity, PendingSlot::Crossfade));
        return;
    };

    let loaded = match load_track_source(
        entity,
        PendingSlot::Crossfade,
        &request.source,
        pending_reads,
        audio_assets,
    ) {
        SourceLoadResult::Pending => return,
        SourceLoadResult::Failed(err) => {
            error!("Failed to load crossfade track: {}", err);
            playback.clear_crossfade_request();
            return;
        }
        SourceLoadResult::Ready(bytes) => bytes,
    };

    // Clone data for player creation and for later AudioPlayer recreation
    let data_for_audio = Arc::new(loaded.data.clone());
    let data_for_crossfade_source = loaded.data.clone();

    let mut load = match load_player_from_bytes(loaded.data, loaded.metadata.as_ref()) {
        Ok(load) => load,
        Err(err) => {
            error!("Failed to prepare crossfade deck: {}", err);
            playback.clear_crossfade_request();
            return;
        }
    };

    if let Err(err) = load.player.play() {
        error!("Failed to start crossfade playback: {}", err);
        playback.clear_crossfade_request();
        return;
    }

    let duration = request.duration.max(0.001);
    let player_arc = Arc::new(Mutex::new(load.player));

    // Create a separate AudioPlayer entity for the crossfade track
    // This allows both tracks to play simultaneously with independent volume control
    let crossfade_metadata = loaded.metadata.unwrap_or(Ym2149Metadata {
        title: load.title.clone(),
        author: load.author.clone(),
        comment: "".to_string(),
        frame_count: load.metrics.frame_count,
        duration_seconds: load.metrics.duration_seconds(),
    });

    let crossfade_audio_source = Ym2149AudioSource::from_player(
        player_arc.clone(),
        data_for_crossfade_source,
        crossfade_metadata,
        load.metrics.frame_count,
        load.metrics.samples_per_frame as usize,
    );
    let crossfade_handle = audio_assets.add(crossfade_audio_source);

    // Spawn the crossfade entity with volume starting at 0.0 (silent)
    // Volume will fade in during the crossfade period
    let crossfade_entity = commands
        .spawn((
            AudioPlayer(crossfade_handle),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
        ))
        .id();

    playback.crossfade = Some(ActiveCrossfade {
        player: player_arc,
        metrics: load.metrics,
        song_title: load.title,
        song_author: load.author,
        elapsed: 0.0,
        duration,
        target_index: request.target_index,
        data: data_for_audio,
        crossfade_entity: Some(crossfade_entity),
    });
    playback.clear_crossfade_request();
}

#[allow(clippy::too_many_arguments)]
fn finalize_crossfade(
    commands: &mut Commands,
    entity: Entity,
    playback: &mut Ym2149Playback,
    runtime: &mut PlaybackRuntimeState,
    config: &Ym2149PluginConfig,
    started_events: &mut MessageWriter<TrackStarted>,
    finished_events: &mut MessageWriter<TrackFinished>,
) {
    let Some(crossfade) = playback.crossfade.take() else {
        return;
    };

    // Despawn the crossfade entity now that the transition is complete
    if let Some(cf_entity) = crossfade.crossfade_entity {
        commands.entity(cf_entity).despawn();
    }

    if let Some(old_player) = playback.player.take() {
        match old_player.lock().stop() {
            Ok(()) => {}
            Err(err) => error!("Failed to stop outgoing deck: {}", err),
        }
    }

    let new_player = crossfade.player.clone();
    playback.player = Some(new_player.clone());
    playback.song_title = crossfade.song_title;
    playback.song_author = crossfade.song_author;
    playback.metrics = Some(crossfade.metrics);
    playback.pending_playlist_index = Some(crossfade.target_index);
    playback.frame_position = new_player.lock().get_current_frame() as u32;

    // CRITICAL: Set source_bytes and needs_reload to recreate the AudioPlayer
    // The AudioPlayer's Decoder still references the old player, so we must
    // create a new AudioPlayer+Decoder chain that references the new player
    playback.source_bytes = Some(crossfade.data);
    playback.source_path = None;
    playback.source_asset = None;
    playback.needs_reload = true;

    // Start new track at full volume (crossfade fade-in is complete)
    playback.volume = 1.0;

    runtime.time_since_last_frame = 0.0;
    runtime.frames_rendered = 0;
    runtime.emitted_finished = false;

    if config.channel_events {
        finished_events.write(TrackFinished { entity });
        started_events.write(TrackStarted { entity });
    }
}

pub(super) fn initialize_playback(
    mut commands: Commands,
    mut playbacks: Query<(Entity, &mut Ym2149Playback)>,
    mut audio_assets: ResMut<Assets<Ym2149AudioSource>>,
    mut pending_reads: Local<HashMap<(Entity, PendingSlot), PendingFileRead>>,
) {
    let mut alive = Vec::new();

    for (entity, mut playback) in playbacks.iter_mut() {
        alive.push(entity);

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
                &audio_assets,
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

            let mut load = match load_player_from_bytes(loaded.data, loaded.metadata.as_ref()) {
                Ok(load) => load,
                Err(err) => {
                    error!("Failed to initialize YM2149 player: {}", err);
                    continue;
                }
            };

            playback.song_title = load.title;
            playback.song_author = load.author;
            playback.metrics = Some(load.metrics);

            if playback.state == PlaybackState::Playing
                && let Err(e) = load.player.play()
            {
                error!("Failed to start player: {}", e);
            }

            playback.player = Some(Arc::new(Mutex::new(load.player)));
            playback.needs_reload = false;

            // Create a Ym2149AudioSource asset from the loaded data
            let audio_source = match Ym2149AudioSource::new(data_for_audio) {
                Ok(source) => source,
                Err(err) => {
                    error!("Failed to create audio source: {}", err);
                    continue;
                }
            };

            // Add the asset and get a handle
            let audio_handle = audio_assets.add(audio_source);

            // Remove old AudioPlayer and AudioSink components if they exist
            // This is critical for crossfade: we must stop the old audio before starting new
            commands
                .entity(entity)
                .remove::<AudioPlayer>()
                .remove::<bevy::audio::AudioSink>();

            // Spawn AudioPlayer on the same entity to play the audio
            // Use AudioPlayer(handle) syntax for custom Decodable types
            // Start paused or playing based on current PlaybackState
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
            &mut audio_assets,
            entity,
            &mut playback,
            &mut pending_reads,
        );
    }

    pending_reads.retain(|(entity, _), _| alive.contains(entity));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_playback(
    mut commands: Commands,
    mut playbacks: Query<(Entity, &mut Ym2149Playback)>,
    settings: Res<Ym2149Settings>,
    config: Res<Ym2149PluginConfig>,
    time: Res<Time>,
    mut oscilloscope_buffer: Option<ResMut<OscilloscopeBuffer>>,
    mut snapshot_events: MessageWriter<ChannelSnapshot>,
    mut started_events: MessageWriter<TrackStarted>,
    mut finished_events: MessageWriter<TrackFinished>,
    bridge_targets: Option<Res<AudioBridgeTargets>>,
    mut bridge_buffers: Option<ResMut<AudioBridgeBuffers>>,
    mut runtime_state: Local<HashMap<Entity, PlaybackRuntimeState>>,
    mut scratch_buffers: Local<HashMap<Entity, PlaybackScratch>>,
    mut audio_sinks: Query<&mut AudioSink>,
) {
    let delta = time.delta_secs();
    let master_volume = settings.master_volume.clamp(0.0, 1.0);
    let mut alive = Vec::new();

    for (entity, mut playback) in playbacks.iter_mut() {
        alive.push(entity);

        let Some(player_arc) = playback.player.clone() else {
            continue;
        };

        let entry = runtime_state.entry(entity).or_default();
        let mut player = player_arc.lock();
        let crossfade_arc = playback
            .crossfade
            .as_ref()
            .map(|state| state.player.clone());
        let mut crossfade_player = crossfade_arc.as_ref().map(|arc| arc.lock());

        let bridging_active = config.bevy_audio_bridge
            && bridge_targets
                .as_ref()
                .map(|targets| targets.0.contains(&entity))
                .unwrap_or(false);

        let state_changed = entry.last_state != playback.state;
        let volume_changed = (entry.last_volume - playback.volume).abs() > 0.001;

        if state_changed {
            match playback.state {
                PlaybackState::Playing => {
                    entry.time_since_last_frame = 0.0;
                    entry.emitted_finished = false;
                    if let Err(err) = player.play() {
                        error!("Failed to resume YM playback: {}", err);
                    }
                    if let Some(cf) = crossfade_player.as_mut()
                        && let Err(err) = cf.play()
                    {
                        error!("Failed to resume crossfade deck: {}", err);
                    }
                    // Resume audio via Bevy AudioSink
                    if let Ok(sink) = audio_sinks.get(entity) {
                        info!("Resuming audio for entity {:?}", entity);
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
                    // Pause audio via Bevy AudioSink
                    if let Ok(sink) = audio_sinks.get(entity) {
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
                    // Stop audio via Bevy AudioSink
                    if let Ok(sink) = audio_sinks.get(entity) {
                        sink.pause();
                    }
                    entry.time_since_last_frame = 0.0;
                    entry.emitted_finished = false;
                }
                PlaybackState::Finished => {
                    // Stop audio via Bevy AudioSink
                    if let Ok(sink) = audio_sinks.get(entity) {
                        sink.pause();
                    }
                    if let Some(cf) = crossfade_player.as_mut()
                        && let Err(err) = cf.pause()
                    {
                        error!("Failed to pause crossfade deck: {}", err);
                    }
                    if config.channel_events && !entry.emitted_finished {
                        finished_events.write(TrackFinished { entity });
                        entry.emitted_finished = true;
                    }
                }
            }
            entry.last_state = playback.state;
        }

        // Update volume via AudioSink when it changes
        if volume_changed {
            if let Ok(mut sink) = audio_sinks.get_mut(entity) {
                sink.set_volume(bevy::audio::Volume::Linear(playback.volume));
            }
            entry.last_volume = playback.volume;
        }

        if playback.state != PlaybackState::Playing {
            playback.seek(player.get_current_frame() as u32);
            continue;
        }

        entry.time_since_last_frame += delta;

        let samples_per_frame = player.samples_per_frame_value() as usize;
        if samples_per_frame == 0 {
            continue;
        }

        let frame_duration = samples_per_frame as f32 / YM2149_SAMPLE_RATE_F32;

        while entry.time_since_last_frame >= frame_duration {
            entry.time_since_last_frame -= frame_duration;
            entry.frames_rendered += 1;

            // Update playback frame position to match the player's current frame
            playback.frame_position = player.get_current_frame() as u32;

            let scratch_entry = scratch_buffers.entry(entity).or_default();
            let mut stereo_samples = std::mem::take(&mut scratch_entry.stereo);
            stereo_samples.clear();
            stereo_samples.reserve(samples_per_frame * 2);

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
                let (mut ch_a, mut ch_b, mut ch_c) = player.get_chip().get_channel_outputs();

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

                if let Some(buffer) = oscilloscope_buffer.as_mut() {
                    buffer.push_sample([ch_a, ch_b, ch_c]);
                }

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

            if bridging_active && let Some(buffers) = bridge_buffers.as_mut() {
                buffers.0.insert(entity, stereo_samples.clone());
            }

            // TODO: With Bevy audio, samples are generated on-demand by the Decoder
            // We don't manually push samples anymore

            if config.channel_events {
                let registers = player.get_chip().dump_registers();
                let frequencies = channel_frequencies(&registers);
                let inv_len = 1.0 / samples_per_frame.max(1) as f32;
                for channel in 0..3 {
                    snapshot_events.write(ChannelSnapshot {
                        entity,
                        channel,
                        amplitude: (channel_energy[channel] * inv_len).clamp(0.0, 1.0),
                        frequency: frequencies[channel],
                    });
                }
            }

            scratch_entry.stereo = stereo_samples;

            if let Some(state) = playback.crossfade.as_mut() {
                state.elapsed = (state.elapsed + frame_duration).min(state.duration);

                // Apply crossfade volume transitions:
                // - Old track (primary entity): fade out from 1.0 → 0.0
                // - New track (crossfade entity): fade in from 0.0 → 1.0
                if state.duration > f32::EPSILON {
                    let fade_ratio = (state.elapsed / state.duration).clamp(0.0, 1.0);
                    let fade_out_volume = 1.0 - fade_ratio;
                    let fade_in_volume = fade_ratio;
                    let cf_entity_opt = state.crossfade_entity;

                    // Fade out the old track (this entity)
                    if let Ok(mut sink) = audio_sinks.get_mut(entity) {
                        sink.set_volume(bevy::audio::Volume::Linear(fade_out_volume));
                    }
                    playback.volume = fade_out_volume;
                    entry.last_volume = fade_out_volume;

                    // Fade in the new track (crossfade entity)
                    if let Some(cf_entity) = cf_entity_opt {
                        if let Ok(mut cf_sink) = audio_sinks.get_mut(cf_entity) {
                            cf_sink.set_volume(bevy::audio::Volume::Linear(fade_in_volume));
                        }
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
                entry,
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
            entry.time_since_last_frame = 0.0;

            if settings.loop_enabled {
                match player.stop().and_then(|_| player.play()) {
                    Ok(()) => {
                        entry.frames_rendered = 0;
                        playback.seek(0);
                        entry.emitted_finished = false;
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
                if config.channel_events && !entry.emitted_finished {
                    finished_events.write(TrackFinished { entity });
                    entry.emitted_finished = true;
                }
            }
        }
    }

    runtime_state.retain(|entity, _| alive.contains(entity));
    scratch_buffers.retain(|entity, _| alive.contains(entity));
}

fn channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | lo as u16;
    if period == 0 { None } else { Some(period) }
}

fn period_to_frequency(period: u16) -> f32 {
    PSG_MASTER_CLOCK_HZ / (16.0 * period as f32)
}

fn channel_frequencies(registers: &[u8; 16]) -> [Option<f32>; 3] {
    [
        channel_period(registers[0], registers[1]).map(period_to_frequency),
        channel_period(registers[2], registers[3]).map(period_to_frequency),
        channel_period(registers[4], registers[5]).map(period_to_frequency),
    ]
}

struct LoadResult {
    player: ym_replayer::Ym6Player,
    metrics: PlaybackMetrics,
    title: String,
    author: String,
}

fn load_player_from_bytes(
    data: Vec<u8>,
    metadata: Option<&Ym2149Metadata>,
) -> Result<LoadResult, String> {
    let (player, summary) =
        ym_replayer::load_song(&data).map_err(|e| format!("Failed to load song: {}", e))?;
    let metrics = PlaybackMetrics::from(&summary);

    let (title, author) = if let Some(meta) = metadata {
        (meta.title.clone(), meta.author.clone())
    } else if let Some(info) = player.info() {
        (info.song_name.clone(), info.author.clone())
    } else {
        (String::new(), String::new())
    };

    Ok(LoadResult {
        player,
        metrics,
        title,
        author,
    })
}
