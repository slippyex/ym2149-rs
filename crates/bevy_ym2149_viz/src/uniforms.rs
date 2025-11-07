use bevy::prelude::*;

/// Buffer storing oscilloscope samples ready to upload to GPU uniforms.
#[derive(Resource, Default, Clone)]
pub struct OscilloscopeUniform(pub Vec<[f32; 3]>);

/// Buffer storing spectrum magnitudes ready for GPU uniforms.
#[derive(Resource, Default, Clone)]
pub struct SpectrumUniform(pub Vec<[f32; 16]>);
