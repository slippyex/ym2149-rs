use crate::audio_source::Ym2149AudioSource;
use crate::events::{TrackFinished, TrackStarted};
use crate::playback::{ActiveCrossfade, Ym2149Playback};
use crate::plugin::Ym2149PluginConfig;
use bevy::audio::{AudioPlayer, PlaybackSettings};
use bevy::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::loader::{PendingFileRead, PendingSlot, SourceLoadResult, load_track_source};
use super::main_systems::{PlaybackRuntimeState, load_player_from_bytes};

pub(super) fn process_pending_crossfade(
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

    let bytes = loaded.data;
    let data_for_state = Arc::new(bytes.clone());

    let mut load = match load_player_from_bytes(&bytes, loaded.metadata.as_ref()) {
        Ok(load) => load,
        Err(err) => {
            error!("Failed to prepare crossfade deck: {}", err);
            playback.clear_crossfade_request();
            return;
        }
    };

    load.player.play();

    let duration = request.duration.max(0.001);
    let player_arc = Arc::new(RwLock::new(load.player));

    let crossfade_audio_source = match Ym2149AudioSource::new_with_shared(
        bytes,
        playback.stereo_gain.clone(),
        playback.tone_settings.clone(),
    ) {
        Ok(source) => source,
        Err(err) => {
            error!("Failed to create crossfade audio source: {}", err);
            playback.clear_crossfade_request();
            return;
        }
    };
    let crossfade_handle = audio_assets.add(crossfade_audio_source);

    let crossfade_entity = commands
        .spawn((
            AudioPlayer(crossfade_handle.clone()),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
        ))
        .id();

    playback.crossfade = Some(ActiveCrossfade {
        player: player_arc,
        metrics: load.metrics,
        song_title: load.metadata.title.clone(),
        song_author: load.metadata.author.clone(),
        elapsed: 0.0,
        duration,
        target_index: request.target_index,
        audio_handle: crossfade_handle,
        data: data_for_state,
        crossfade_entity: Some(crossfade_entity),
    });
    playback.clear_crossfade_request();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finalize_crossfade(
    commands: &mut Commands,
    _entity: Entity,
    playback: &mut Ym2149Playback,
    runtime: &mut PlaybackRuntimeState,
    config: &Ym2149PluginConfig,
    started_events: &mut MessageWriter<TrackStarted>,
    finished_events: &mut MessageWriter<TrackFinished>,
    audio_sinks: &mut Query<&mut bevy::audio::AudioSink>,
) {
    let Some(crossfade) = playback.crossfade.take() else {
        return;
    };

    let _ = (config, started_events, finished_events);

    // Stop and dispose the outgoing deck
    if let Ok(sink) = audio_sinks.get_mut(_entity) {
        sink.stop();
    }

    if let Some(cf_entity) = crossfade.crossfade_entity {
        // Ensure the incoming deck is fully up before moving handles around.
        if let Ok(mut sink) = audio_sinks.get_mut(cf_entity) {
            sink.set_volume(bevy::audio::Volume::Linear(1.0));
        }
        commands.entity(cf_entity).despawn();
    }

    if let Some(old_player) = playback.player.take() {
        old_player.write().stop();
    }

    let new_player = crossfade.player.clone();
    playback.player = Some(new_player);
    playback.song_title = crossfade.song_title;
    playback.song_author = crossfade.song_author;
    playback.metrics = Some(crossfade.metrics);
    playback.pending_playlist_index = Some(crossfade.target_index);
    playback.source_path = None;
    playback.source_asset = None;
    playback.needs_reload = false;
    // Keep position as-is; the player already advanced during crossfade.

    playback.source_bytes = Some(crossfade.data);

    playback.volume = 1.0;
    runtime.reset_for_crossfade();

    // Swap audio output to the crossfade deck handle.
    commands
        .entity(_entity)
        .remove::<bevy::audio::AudioSink>()
        .remove::<bevy::audio::AudioPlayer>()
        .insert((
            bevy::audio::AudioPlayer(crossfade.audio_handle.clone()),
            bevy::audio::PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(1.0)),
        ));
}
