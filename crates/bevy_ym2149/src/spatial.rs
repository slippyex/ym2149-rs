use crate::playback::Ym2149Playback;
use bevy::prelude::*;

/// Enables simple 2D spatial panning for a playback entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct Ym2149SpatialAudio {
    /// Maximum distance at which the audio can be heard.
    pub max_distance: f32,
}

impl Default for Ym2149SpatialAudio {
    fn default() -> Self {
        Self { max_distance: 20.0 }
    }
}

/// Marker used to identify the listener used for YM2149 spatial panning.
#[derive(Component, Default)]
pub struct Ym2149Listener;

pub fn update_spatial_audio(
    listener_query: Query<&GlobalTransform, (With<Ym2149Listener>, Without<Camera>)>,
    fallback_listener: Query<&GlobalTransform, With<Camera>>,
    mut query: Query<(&GlobalTransform, &Ym2149SpatialAudio, &mut Ym2149Playback)>,
) {
    let listener_transform = listener_query
        .iter()
        .next()
        .or_else(|| fallback_listener.iter().next());

    let Some(listener) = listener_transform else {
        return;
    };

    for (transform, spatial, mut playback) in &mut query {
        let diff = listener.translation() - transform.translation();
        let distance = diff.length();
        let attenuation = (1.0 - distance / spatial.max_distance).clamp(0.0, 1.0);

        let pan = diff.x / spatial.max_distance;
        let pan = pan.clamp(-1.0, 1.0);
        let left = attenuation * (1.0 - pan).clamp(0.0, 2.0) * 0.5;
        let right = attenuation * (1.0 + pan).clamp(0.0, 2.0) * 0.5;
        playback.set_stereo_gain(left, right);
    }
}
