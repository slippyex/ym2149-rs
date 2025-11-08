use crate::events::AudioBridgeRequest;
use crate::playback::YM2149_SAMPLE_RATE;
use bevy::prelude::*;
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::f32::consts::FRAC_PI_2;

/// Tracks which playback entities should publish audio frames to the bridge buffers.
#[derive(Resource, Default)]
pub struct AudioBridgeTargets(pub HashSet<Entity>);

/// Stores the most recent stereo frame for each bridged playback entity.
#[derive(Resource, Default)]
pub struct AudioBridgeBuffers(pub HashMap<Entity, Vec<f32>>);

/// Handle bridge requests by marking entities as active bridge publishers.
pub fn handle_bridge_requests(
    mut requests: MessageReader<AudioBridgeRequest>,
    mut targets: ResMut<AudioBridgeTargets>,
) {
    for request in requests.read() {
        targets.0.insert(request.entity);
    }
}

/// Rodio output handle used for bridge playback.
#[derive(Resource)]
pub struct BridgeAudioDevice {
    /// The stream must be kept alive for the OutputStreamHandle to remain valid.
    /// We hold an owned reference to ensure the stream isn't dropped while the handle is in use.
    #[allow(dead_code)]
    _stream: Option<OutputStream>,
    /// The handle to the active output stream for creating sinks.
    stream_handle: Option<OutputStreamHandle>,
}

/// SAFETY: BridgeAudioDevice is Send + Sync
///
/// While rodio::OutputStream is marked as not Send/Sync on some platforms due to platform-specific
/// constraints (NotSendSyncAcrossAllPlatforms marker), we can safely implement these traits because:
///
/// 1. The OutputStream is only accessed through OutputStreamHandle, which is Send + Sync
/// 2. We never directly use _stream after initialization; it exists only to keep the stream alive
/// 3. Bevy Resources require Send + Sync for use in systems
/// 4. In practice, the OutputStream is safe to share across threads as long as only one thread
///    holds the handle and calls methods on it (which is our usage pattern)
///
/// This is consistent with how rodio is used in other multi-threaded contexts where
/// OutputStreamHandle itself is known to be thread-safe.
unsafe impl Send for BridgeAudioDevice {}
unsafe impl Sync for BridgeAudioDevice {}

impl Default for BridgeAudioDevice {
    fn default() -> Self {
        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                stream_handle: Some(handle),
            },
            Err(err) => {
                warn!("Failed to initialize bridge audio output: {err:?}");
                Self {
                    _stream: None,
                    stream_handle: None,
                }
            }
        }
    }
}

/// Active rodio sinks fed by the bridge buffers.
#[derive(Resource, Default)]
pub struct BridgeAudioSinks(pub HashMap<Entity, Sink>);

impl BridgeAudioSinks {
    /// Fetch the sink for an entity, if available.
    pub fn get(&self, entity: Entity) -> Option<&Sink> {
        self.0.get(&entity)
    }

    /// Fetch a mutable reference to a sink for custom control.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut Sink> {
        self.0.get_mut(&entity)
    }
}

/// Mixing parameters applied before bridge audio is submitted to rodio.
///
/// Fields can be modified directly:
/// ```
/// # use bevy_ym2149::AudioBridgeMix;
/// let mut mix = AudioBridgeMix::CENTER;
/// mix.volume = 0.5;
/// mix.pan = -0.5; // Pan left
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioBridgeMix {
    /// Overall gain multiplier (0.0 = silent, 1.0 = unchanged).
    pub volume: f32,
    /// Stereo pan in the range [-1.0, 1.0] where -1.0 = full left, +1.0 = full right.
    pub pan: f32,
}

impl AudioBridgeMix {
    /// Neutral mix (unity gain, centered).
    pub const CENTER: Self = Self {
        volume: 1.0,
        pan: 0.0,
    };
    /// Muted output (silences the bridged audio).
    pub const MUTE: Self = Self {
        volume: 0.0,
        pan: 0.0,
    };
    /// Hard pan left at unity gain.
    pub const LEFT: Self = Self {
        volume: 1.0,
        pan: -1.0,
    };
    /// Hard pan right at unity gain.
    pub const RIGHT: Self = Self {
        volume: 1.0,
        pan: 1.0,
    };

    /// Convert decibels to linear gain.
    pub fn db_to_gain(volume_db: f32) -> f32 {
        10_f32.powf(volume_db / 20.0)
    }

    /// Convert linear gain to decibels.
    pub fn gain_to_db(volume: f32) -> f32 {
        20.0 * volume.max(1e-6).log10()
    }

    /// Get the current volume in decibels.
    pub fn volume_db(self) -> f32 {
        Self::gain_to_db(self.volume)
    }

    pub(crate) fn gains(self) -> (f32, f32) {
        let volume = self.volume.clamp(0.0, 2.0);
        let pan = self.pan.clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * FRAC_PI_2 * 0.5;
        let left = volume * angle.cos();
        let right = volume * angle.sin();
        (left, right)
    }
}

/// Stores per-entity bridge mix preferences (volume / pan).
#[derive(Resource, Default)]
pub struct AudioBridgeMixes(pub HashMap<Entity, AudioBridgeMix>);

impl AudioBridgeMixes {
    /// Overrides the mix for an entity.
    pub fn set(&mut self, entity: Entity, mix: AudioBridgeMix) {
        self.0.insert(entity, mix);
    }

    /// Adjusts only the volume component for an entity (inserts defaults if needed).
    pub fn set_volume(&mut self, entity: Entity, volume: f32) {
        self.0
            .entry(entity)
            .and_modify(|mix| mix.volume = volume)
            .or_insert(AudioBridgeMix {
                volume,
                ..Default::default()
            });
    }

    /// Sets the playback volume using decibels.
    pub fn set_volume_db(&mut self, entity: Entity, volume_db: f32) {
        let volume = AudioBridgeMix::db_to_gain(volume_db);
        self.set_volume(entity, volume);
    }

    /// Adjusts only the pan component for an entity (inserts defaults if needed).
    pub fn set_pan(&mut self, entity: Entity, pan: f32) {
        self.0
            .entry(entity)
            .and_modify(|mix| mix.pan = pan)
            .or_insert(AudioBridgeMix {
                pan,
                ..Default::default()
            });
    }

    /// Clears any explicit mix override for an entity.
    pub fn clear(&mut self, entity: Entity) {
        self.0.remove(&entity);
    }

    /// Fetches the effective mix for an entity.
    pub fn get(&self, entity: Entity) -> AudioBridgeMix {
        self.0.get(&entity).copied().unwrap_or_default()
    }
}

/// Push buffered samples into rodio sinks so they play through the audio bridge.
pub fn drive_bridge_audio_buffers(
    config: Res<crate::plugin::Ym2149PluginConfig>,
    device: Res<BridgeAudioDevice>,
    targets: Res<AudioBridgeTargets>,
    mut buffers: ResMut<AudioBridgeBuffers>,
    mut sinks: ResMut<BridgeAudioSinks>,
    mixes: Res<AudioBridgeMixes>,
) {
    if !config.bevy_audio_bridge {
        for (_, sink) in sinks.0.drain() {
            sink.stop();
        }
        return;
    }

    let Some(handle) = device.stream_handle.as_ref() else {
        return;
    };

    // Remove sinks that are no longer requested.
    sinks.0.retain(|entity, sink| {
        if targets.0.contains(entity) {
            true
        } else {
            sink.stop();
            false
        }
    });

    buffers.0.retain(|entity, _| targets.0.contains(entity));

    for entity in targets.0.iter().copied() {
        let sink = match sinks.0.entry(entity) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(vacant) => match Sink::try_new(handle) {
                Ok(sink) => vacant.insert(sink),
                Err(err) => {
                    warn!(
                        "Failed to create bridge audio sink for entity {:?}: {err:?}",
                        entity
                    );
                    continue;
                }
            },
        };

        if let Some(mut samples) = buffers.0.remove(&entity)
            && !samples.is_empty()
        {
            let mix = mixes.get(entity);
            let (left_gain, right_gain) = mix.gains();
            if (left_gain - 1.0).abs() > f32::EPSILON || (right_gain - 1.0).abs() > f32::EPSILON {
                for chunk in samples.chunks_mut(2) {
                    if let Some(left) = chunk.get_mut(0) {
                        *left *= left_gain;
                    }
                    if let Some(right) = chunk.get_mut(1) {
                        *right *= right_gain;
                    }
                }
            }

            let source = SamplesBuffer::new(2u16, YM2149_SAMPLE_RATE, samples);
            sink.append(source);
        }
    }
}
