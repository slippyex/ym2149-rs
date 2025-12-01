//! GPU uniform buffer resources for visualization shaders.

use bevy::prelude::*;

/// Buffer storing oscilloscope samples ready to upload to GPU uniforms.
///
/// Each entry is `[amplitude_a, amplitude_b, amplitude_c]` for one time sample.
#[derive(Resource, Default, Clone)]
pub struct OscilloscopeUniform(
    /// Per-sample amplitude values for channels A, B, C.
    pub Vec<[f32; 3]>,
);

/// Buffer storing spectrum magnitudes ready for GPU uniforms.
///
/// Each entry contains 16 frequency bin magnitudes for one channel.
#[derive(Resource, Default, Clone)]
pub struct SpectrumUniform(
    /// Per-channel array of 16 frequency bin magnitudes.
    pub Vec<[f32; 16]>,
);
