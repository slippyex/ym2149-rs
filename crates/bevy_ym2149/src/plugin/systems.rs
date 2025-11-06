use crate::audio_bridge::{AudioBridgeBuffers, AudioBridgeTargets};
use crate::audio_sink;
use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::events::{ChannelSnapshot, TrackFinished, TrackStarted};
use crate::playback::{PlaybackState, Ym2149Playback, Ym2149Settings};
use crate::plugin::Ym2149PluginConfig;
#[cfg(feature = "visualization")]
use crate::viz_components::OscilloscopeBuffer;
use bevy::prelude::*;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use ym2149::replayer::PlaybackController;

pub(crate) const OUTPUT_SAMPLE_RATE: u32 = 44_100;
const OUTPUT_SAMPLE_RATE_F32: f32 = OUTPUT_SAMPLE_RATE as f32;
const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;

#[derive(Clone, Copy)]
pub(super) struct PlaybackRuntimeState {
    time_since_last_frame: f32,
    last_state: PlaybackState,
    frames_rendered: u64,
    emitted_finished: bool,
}

impl Default for PlaybackRuntimeState {
    fn default() -> Self {
        Self {
            time_since_last_frame: 0.0,
            last_state: PlaybackState::Idle,
            frames_rendered: 0,
            emitted_finished: false,
        }
    }
}

pub(super) fn initialize_playback(
    mut playbacks: Query<&mut Ym2149Playback>,
    assets: Res<Assets<Ym2149AudioSource>>,
) {
    for mut playback in playbacks.iter_mut() {
        if playback.player.is_some() && !playback.needs_reload {
            continue;
        }

        let load_result = if let Some(bytes) = playback.source_bytes() {
            load_player_from_bytes(bytes.as_ref().clone(), None)
        } else if let Some(path) = playback.source_path() {
            match std::fs::read(path) {
                Ok(bytes) => load_player_from_bytes(bytes, None),
                Err(e) => {
                    error!("Failed to read YM file '{}': {}", path, e);
                    continue;
                }
            }
        } else if let Some(handle) = playback.source_asset().cloned() {
            match assets.get(&handle) {
                Some(asset) => load_player_from_bytes(asset.data.clone(), Some(&asset.metadata)),
                None => continue,
            }
        } else {
            continue;
        };

        let mut load = match load_result {
            Ok(load) => load,
            Err(err) => {
                error!("Failed to initialize YM2149 player: {}", err);
                continue;
            }
        };

        playback.song_title = load.title;
        playback.song_author = load.author;

        if playback.state == PlaybackState::Playing {
            if let Err(e) = load.player.play() {
                error!("Failed to start player: {}", e);
            }
        }

        playback.player = Some(Arc::new(Mutex::new(load.player)));
        playback.needs_reload = false;

        if playback.audio_device.is_none() {
            match audio_sink::rodio::RodioAudioSink::new(OUTPUT_SAMPLE_RATE, 2) {
                Ok(sink) => {
                    if let Err(e) = sink.start() {
                        error!("Failed to start audio device: {}", e);
                    } else {
                        info!("Audio device started successfully!");
                    }
                    playback.audio_device = Some(Arc::new(sink));
                }
                Err(e) => {
                    error!("Failed to create audio device: {}", e);
                }
            }
        }

        info!(
            "Loaded YM song: {} frames, {} samples/frame",
            load.summary.frame_count, load.summary.samples_per_frame
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_playback(
    mut playbacks: Query<(Entity, &mut Ym2149Playback)>,
    settings: Res<Ym2149Settings>,
    config: Res<Ym2149PluginConfig>,
    time: Res<Time>,
    #[cfg(feature = "visualization")] mut oscilloscope_buffer: Option<ResMut<OscilloscopeBuffer>>,
    mut snapshot_events: MessageWriter<ChannelSnapshot>,
    mut started_events: MessageWriter<TrackStarted>,
    mut finished_events: MessageWriter<TrackFinished>,
    bridge_targets: Option<Res<AudioBridgeTargets>>,
    mut bridge_buffers: Option<ResMut<AudioBridgeBuffers>>,
    mut runtime_state: Local<HashMap<Entity, PlaybackRuntimeState>>,
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

        let bridging_active = config.bevy_audio_bridge
            && bridge_targets
                .as_ref()
                .map(|targets| targets.0.contains(&entity))
                .unwrap_or(false);

        let state_changed = entry.last_state != playback.state;
        if state_changed {
            match playback.state {
                PlaybackState::Playing => {
                    entry.time_since_last_frame = 0.0;
                    entry.emitted_finished = false;
                    if let Err(err) = player.play() {
                        error!("Failed to resume YM playback: {}", err);
                    }
                    if let Some(device) = &playback.audio_device {
                        device.resume();
                    }
                    if config.channel_events {
                        started_events.write(TrackStarted { entity });
                    }
                }
                PlaybackState::Paused => {
                    if let Err(err) = player.pause() {
                        error!("Failed to pause YM playback: {}", err);
                    }
                    if let Some(device) = &playback.audio_device {
                        device.pause();
                    }
                }
                PlaybackState::Idle => {
                    if let Err(err) = player.pause() {
                        error!("Failed to stop YM playback: {}", err);
                    }
                    if let Some(device) = &playback.audio_device {
                        device.pause();
                    }
                    entry.time_since_last_frame = 0.0;
                    entry.emitted_finished = false;
                }
                PlaybackState::Finished => {
                    if let Some(device) = &playback.audio_device {
                        device.pause();
                    }
                    if config.channel_events && !entry.emitted_finished {
                        finished_events.write(TrackFinished { entity });
                        entry.emitted_finished = true;
                    }
                }
            }
            entry.last_state = playback.state;
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

        let frame_duration = samples_per_frame as f32 / OUTPUT_SAMPLE_RATE_F32;

        while entry.time_since_last_frame >= frame_duration {
            entry.time_since_last_frame -= frame_duration;
            entry.frames_rendered += 1;

            let mut mono_samples = Vec::with_capacity(samples_per_frame);
            let mut channel_energy = [0.0f32; 3];

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

                #[cfg(feature = "visualization")]
                if let Some(buffer) = oscilloscope_buffer.as_mut() {
                    buffer.push_sample([ch_a, ch_b, ch_c]);
                }

                mono_samples.push(sample);
            }

            let stereo_samples = to_stereo_samples(&mono_samples, &playback, master_volume);

            if bridging_active {
                if let Some(buffers) = bridge_buffers.as_mut() {
                    buffers.0.insert(entity, stereo_samples.clone());
                }
            }

            if let Some(device) = &playback.audio_device {
                if let Err(err) = device.push_samples(stereo_samples) {
                    warn!("Failed to push samples to audio device: {}", err);
                } else if entry.frames_rendered.is_multiple_of(60) {
                    let fill = device.buffer_fill_level();
                    debug!(
                        "Playback buffer fill for {:?}: {:.1}%",
                        entity,
                        fill * 100.0
                    );
                }
            }

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
        }

        playback.seek(player.get_current_frame() as u32);
        let player_state = player.state();

        if player_state != ym2149::replayer::PlaybackState::Playing
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
}

fn channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | lo as u16;
    if period == 0 {
        None
    } else {
        Some(period)
    }
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
    player: ym2149::replayer::Ym6Player,
    summary: ym2149::LoadSummary,
    title: String,
    author: String,
}

fn load_player_from_bytes(
    data: Vec<u8>,
    metadata: Option<&Ym2149Metadata>,
) -> Result<LoadResult, String> {
    let (player, summary) =
        ym2149::load_song(&data).map_err(|e| format!("Failed to load song: {}", e))?;

    let (title, author) = if let Some(meta) = metadata {
        (meta.title.clone(), meta.author.clone())
    } else if let Some(info) = player.info() {
        (info.song_name.clone(), info.author.clone())
    } else {
        (String::new(), String::new())
    };

    Ok(LoadResult {
        player,
        summary,
        title,
        author,
    })
}

fn to_stereo_samples(
    mono_samples: &[f32],
    playback: &Ym2149Playback,
    master_volume: f32,
) -> Vec<f32> {
    let gain = (playback.volume * master_volume).clamp(0.0, 1.0);
    let left = playback.left_gain.clamp(0.0, 1.0);
    let right = playback.right_gain.clamp(0.0, 1.0);

    let mut stereo = Vec::with_capacity(mono_samples.len() * 2);
    for &sample in mono_samples {
        let scaled = sample * gain;
        stereo.push(scaled * left);
        stereo.push(scaled * right);
    }
    stereo
}
