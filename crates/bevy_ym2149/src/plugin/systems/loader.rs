//! Track loading systems and helpers

use crate::audio_source::{Ym2149AudioSource, Ym2149Metadata};
use crate::playback::TrackSource;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};
use std::collections::hash_map::Entry;

pub(in crate::plugin) struct PendingFileRead {
    pub path: String,
    pub task: Task<Result<Vec<u8>, String>>,
}

impl PendingFileRead {
    pub fn new(path: String) -> Self {
        let task_path = path.clone();
        let task = IoTaskPool::get().spawn(async move {
            std::fs::read(&task_path)
                .map_err(|err| format!("Failed to read YM file '{task_path}': {err}"))
        });
        Self { path, task }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::plugin) enum PendingSlot {
    Primary,
    Crossfade,
}

pub(super) struct LoadedBytes {
    pub data: Vec<u8>,
    pub metadata: Option<Ym2149Metadata>,
}

pub(super) enum SourceLoadResult {
    Pending,
    Ready(LoadedBytes),
    Failed(String),
}

/// Load track source (file or bytes)
pub(super) fn load_track_source(
    entity: Entity,
    slot: PendingSlot,
    source: &TrackSource,
    pending_reads: &mut std::collections::HashMap<(Entity, PendingSlot), PendingFileRead>,
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
