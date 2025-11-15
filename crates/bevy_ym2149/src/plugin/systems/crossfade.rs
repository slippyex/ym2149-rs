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
    let player_arc = Arc::new(RwLock::new(load.player));

    let crossfade_metadata = loaded.metadata.unwrap_or(load.metadata.clone());

    let crossfade_audio_source = Ym2149AudioSource::from_player(
        player_arc.clone(),
        data_for_crossfade_source,
        crossfade_metadata,
        load.metrics,
    );
    let crossfade_handle = audio_assets.add(crossfade_audio_source);

    let crossfade_entity = commands
        .spawn((
            AudioPlayer(crossfade_handle),
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
        data: data_for_audio,
        crossfade_entity: Some(crossfade_entity),
    });
    playback.clear_crossfade_request();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finalize_crossfade(
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

    if let Some(cf_entity) = crossfade.crossfade_entity {
        commands.entity(cf_entity).despawn();
    }

    if let Some(old_player) = playback.player.take() {
        if let Err(err) = old_player.write().stop() {
            error!("Failed to stop outgoing deck: {}", err);
        }
    }

    let new_player = crossfade.player.clone();
    playback.player = Some(new_player.clone());
    playback.song_title = crossfade.song_title;
    playback.song_author = crossfade.song_author;
    playback.metrics = Some(crossfade.metrics);
    playback.pending_playlist_index = Some(crossfade.target_index);
    playback.frame_position = new_player.read().get_current_frame() as u32;

    playback.source_bytes = Some(crossfade.data);
    playback.source_path = None;
    playback.source_asset = None;
    playback.needs_reload = true;

    playback.volume = 1.0;

    runtime.reset_for_crossfade();

    if config.channel_events {
        finished_events.write(TrackFinished { entity });
        started_events.write(TrackStarted { entity });
    }
}
