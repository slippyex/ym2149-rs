use bevy::prelude::*;
use std::collections::HashMap;

/// Smoothed per-entity audio metrics for visualization and gameplay hooks.
#[derive(Clone, Debug)]
pub struct ReactiveMetrics {
    pub average: [f32; 3],
    pub peak: [f32; 3],
    pub frequencies: [Option<f32>; 3],
}

impl ReactiveMetrics {
    pub fn new() -> Self {
        Self {
            average: [0.0; 3],
            peak: [0.0; 3],
            frequencies: [None; 3],
        }
    }
}

impl Default for ReactiveMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource mapping playback entities to their most recent reactive metrics.
#[derive(Resource, Default)]
pub struct AudioReactiveState {
    pub metrics: HashMap<Entity, ReactiveMetrics>,
}
