//! Pattern-triggered gameplay hooks for YM2149 playback.
//!
//! Attach [`PatternTriggerSet`] to a [`Ym2149Playback`](crate::playback::Ym2149Playback)
//! entity to receive [`PatternTriggered`](crate::events::PatternTriggered) events
//! whenever a channel matches your criteria.

use bevy::prelude::{Component, Entity, Resource};
use std::collections::HashMap;

/// Definition for a single pattern trigger.
///
/// A trigger matches when the configured channel's average amplitude
/// surpasses `min_amplitude` (0.0–1.0) and, optionally, when the reported
/// frequency is within `frequency_tolerance_hz` of `frequency_hz`.
#[derive(Clone, Debug)]
pub struct PatternTrigger {
    /// Application-defined identifier returned via [`PatternTriggered`](crate::events::PatternTriggered).
    pub id: String,
    /// YM2149 channel index (0–2).
    pub channel: usize,
    /// Minimum average amplitude required to fire this trigger (0.0–1.0).
    pub min_amplitude: f32,
    /// Optional target frequency in Hz to match before firing.
    pub frequency_hz: Option<f32>,
    /// Allowed deviation when `frequency_hz` is set.
    pub frequency_tolerance_hz: f32,
    /// Cooldown in frames before the pattern may fire again.
    pub cooldown_frames: u64,
}

impl PatternTrigger {
    /// Create a trigger with sensible defaults.
    ///
    /// Defaults: `min_amplitude=0.25`, no frequency constraint, no cooldown.
    pub fn new(id: impl Into<String>, channel: usize) -> Self {
        Self {
            id: id.into(),
            channel,
            min_amplitude: 0.25,
            frequency_hz: None,
            frequency_tolerance_hz: 12.0,
            cooldown_frames: 0,
        }
    }

    /// Override the minimum amplitude threshold.
    pub fn with_min_amplitude(mut self, threshold: f32) -> Self {
        self.min_amplitude = threshold.max(0.0);
        self
    }

    /// Require a frequency match with the given tolerance.
    pub fn with_frequency(mut self, freq_hz: f32, tolerance_hz: f32) -> Self {
        self.frequency_hz = Some(freq_hz.max(0.0));
        self.frequency_tolerance_hz = tolerance_hz.abs();
        self
    }

    /// Set a cooldown window (in YM frames) between hits.
    pub fn with_cooldown(mut self, frames: u64) -> Self {
        self.cooldown_frames = frames;
        self
    }
}

/// Component that stores multiple pattern triggers for a playback entity.
#[derive(Component, Clone, Debug, Default)]
pub struct PatternTriggerSet {
    /// Configured pattern definitions.
    pub patterns: Vec<PatternTrigger>,
}

impl PatternTriggerSet {
    /// Create an empty trigger set.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Create a trigger set with the provided patterns.
    pub fn from_patterns(patterns: Vec<PatternTrigger>) -> Self {
        Self { patterns }
    }

    /// Push a trigger definition to this set.
    pub fn push(&mut self, trigger: PatternTrigger) {
        self.patterns.push(trigger);
    }

    /// Return a new set with an extra trigger.
    pub fn with_pattern(mut self, trigger: PatternTrigger) -> Self {
        self.patterns.push(trigger);
        self
    }

    /// True if there are no triggers.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// Runtime bookkeeping for per-entity trigger cooldowns.
#[derive(Resource, Default)]
pub struct PatternTriggerRuntime(pub HashMap<Entity, Vec<u64>>);
