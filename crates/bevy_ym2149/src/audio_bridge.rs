// LEGACY: This module is being replaced with Bevy-native audio system
// Temporarily stubbed out during migration to bevy::audio
//
// TODO: Rewrite using Bevy's AudioPlayer and spatial audio features

#![allow(dead_code, unused_imports)]

use crate::events::AudioBridgeRequest;
use crate::playback::YM2149_SAMPLE_RATE;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
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

/// Placeholder for BridgeAudioDevice during migration to Bevy audio
#[derive(Resource, Default)]
pub struct BridgeAudioDevice;

/// Placeholder for BridgeAudioSinks during migration
#[derive(Resource, Default)]
pub struct BridgeAudioSinks;

impl BridgeAudioSinks {
    /// Placeholder get method
    pub fn get(&self, _entity: Entity) -> Option<&()> {
        None
    }

    /// Placeholder get_mut method
    pub fn get_mut(&mut self, _entity: Entity) -> Option<&mut ()> {
        None
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

/// Stubbed out during migration to Bevy audio
pub fn drive_bridge_audio_buffers(
    _config: Res<crate::plugin::Ym2149PluginConfig>,
    _device: Res<BridgeAudioDevice>,
    _targets: Res<AudioBridgeTargets>,
    _buffers: ResMut<AudioBridgeBuffers>,
    _sinks: ResMut<BridgeAudioSinks>,
    _mixes: Res<AudioBridgeMixes>,
) {
    // TODO: Reimplement using Bevy's audio system
}
