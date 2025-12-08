//! Sample caching utilities for CPU-emulated backends.
//!
//! This module provides [`SampleCache`], a helper for efficient sample caching,
//! and [`CachedPlayer`], a full wrapper around chiptune players.
//!
//! # Why Caching?
//!
//! Players like AY (Z80) and SNDH (68000) generate samples in batches through
//! CPU emulation. The cache allows efficient single-sample access while still
//! generating samples in larger batches.
//!
//! # Channel Output Caching
//!
//! The cache also stores YM2149 channel outputs after each refill,
//! enabling synchronized visualization without sample-accurate overhead.

use crate::{ChiptunePlayerBase, PlaybackState};

/// Default cache size in samples.
pub const DEFAULT_CACHE_SIZE: usize = 512;

// ============================================================================
// SampleCache - Standalone cache helper
// ============================================================================

/// A sample cache for efficient single-sample access.
///
/// This is a lightweight helper that manages sample and channel output caching.
/// Use it when you need to add caching to a player without wrapping it entirely.
///
/// # Example
///
/// ```ignore
/// use ym2149_common::SampleCache;
///
/// struct MyPlayer {
///     inner: SomePlayer,
///     cache: SampleCache,
/// }
///
/// impl MyPlayer {
///     fn generate_sample(&mut self) -> f32 {
///         if self.cache.needs_refill() {
///             // Fill the cache
///             self.inner.generate_samples_into(self.cache.sample_buffer_mut());
///             let outputs = self.inner.get_channel_outputs();
///             self.cache.fill_channel_outputs(outputs);
///             self.cache.mark_filled();
///         }
///         self.cache.next_sample()
///     }
/// }
/// ```
#[derive(Clone)]
pub struct SampleCache {
    samples: Vec<f32>,
    channel_outputs: Vec<[f32; 3]>,
    pos: usize,
    len: usize,
    size: usize,
}

impl SampleCache {
    /// Create a new sample cache with the specified size.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            samples: vec![0.0; size],
            channel_outputs: vec![[0.0; 3]; size],
            pos: 0,
            len: 0,
            size,
        }
    }

    /// Check if the cache needs to be refilled.
    #[inline]
    #[must_use]
    pub fn needs_refill(&self) -> bool {
        self.pos >= self.len
    }

    /// Get a mutable reference to the sample buffer for filling.
    pub fn sample_buffer_mut(&mut self) -> &mut [f32] {
        &mut self.samples[..self.size]
    }

    /// Fill all channel output entries with the same value.
    ///
    /// Call this after filling samples to set the channel outputs
    /// for visualization.
    pub fn fill_channel_outputs(&mut self, outputs: [f32; 3]) {
        self.channel_outputs[..self.size].fill(outputs);
    }

    /// Mark the cache as filled and reset position to start.
    pub fn mark_filled(&mut self) {
        self.pos = 0;
        self.len = self.size;
    }

    /// Get the next sample from the cache.
    ///
    /// Returns 0.0 if the cache is empty.
    pub fn next_sample(&mut self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let sample = self.samples[self.pos];
        self.pos += 1;
        sample
    }

    /// Get the channel outputs for the current/last sample.
    #[must_use]
    pub fn channel_outputs(&self) -> [f32; 3] {
        if self.pos > 0 && self.pos <= self.len {
            self.channel_outputs[self.pos - 1]
        } else if !self.channel_outputs.is_empty() {
            self.channel_outputs[0]
        } else {
            [0.0, 0.0, 0.0]
        }
    }

    /// Reset the cache, forcing a refill on the next access.
    pub fn reset(&mut self) {
        self.pos = 0;
        self.len = 0;
    }

    /// Get the cache size.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Default for SampleCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }
}

// ============================================================================
// CachedPlayer - Full player wrapper
// ============================================================================

/// Trait for players that can be wrapped with caching.
///
/// This trait provides the hooks needed by [`CachedPlayer`] to interact
/// with the underlying player.
pub trait CacheablePlayer: ChiptunePlayerBase {
    /// Get the current channel outputs from the chip.
    ///
    /// Returns `[channel_a, channel_b, channel_c]` as f32 values.
    fn get_channel_outputs(&self) -> [f32; 3];

    /// Called when the cache is about to be refilled.
    ///
    /// Override this to perform any pre-fill setup. Default is no-op.
    fn on_cache_refill(&mut self) {}
}

/// A cached wrapper around a chiptune player.
///
/// This wrapper maintains a sample cache and channel output cache,
/// reducing the overhead of single-sample generation for CPU-emulated players.
///
/// # Example
///
/// ```ignore
/// use ym2149_common::{CachedPlayer, CacheablePlayer, DEFAULT_CACHE_SIZE};
///
/// let player = SomePlayer::new();
/// let mut cached = CachedPlayer::new(player, DEFAULT_CACHE_SIZE);
///
/// // Generate samples one at a time (efficient due to caching)
/// let sample = cached.generate_sample();
/// let channels = cached.cached_channel_outputs();
/// ```
pub struct CachedPlayer<P: CacheablePlayer> {
    player: P,
    cache: SampleCache,
}

impl<P: CacheablePlayer> CachedPlayer<P> {
    /// Create a new cached player with the specified cache size.
    pub fn new(player: P, cache_size: usize) -> Self {
        Self {
            player,
            cache: SampleCache::new(cache_size),
        }
    }

    /// Create a new cached player with the default cache size (512 samples).
    pub fn with_default_cache(player: P) -> Self {
        Self::new(player, DEFAULT_CACHE_SIZE)
    }

    /// Get a reference to the underlying player.
    pub fn inner(&self) -> &P {
        &self.player
    }

    /// Get a mutable reference to the underlying player.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.player
    }

    /// Consume the wrapper and return the underlying player.
    pub fn into_inner(self) -> P {
        self.player
    }

    /// Generate a single sample, using the cache.
    ///
    /// If the cache is exhausted, it will be refilled automatically.
    pub fn generate_sample(&mut self) -> f32 {
        if self.cache.needs_refill() {
            self.refill_cache();
        }
        self.cache.next_sample()
    }

    /// Get the channel outputs corresponding to the last generated sample.
    ///
    /// This returns the channel outputs that were captured when the cache
    /// was last filled. Not sample-accurate, but provides reasonable
    /// visualization data.
    pub fn cached_channel_outputs(&self) -> [f32; 3] {
        self.cache.channel_outputs()
    }

    /// Reset the cache, forcing a refill on the next sample request.
    pub fn reset_cache(&mut self) {
        self.cache.reset();
    }

    /// Refill the cache from the underlying player.
    fn refill_cache(&mut self) {
        self.player.on_cache_refill();
        self.player
            .generate_samples_into(self.cache.sample_buffer_mut());
        self.cache
            .fill_channel_outputs(self.player.get_channel_outputs());
        self.cache.mark_filled();
    }
}

// Forward ChiptunePlayerBase methods to the inner player
impl<P: CacheablePlayer> ChiptunePlayerBase for CachedPlayer<P> {
    fn play(&mut self) {
        self.player.play();
    }

    fn pause(&mut self) {
        self.player.pause();
    }

    fn stop(&mut self) {
        self.player.stop();
        self.reset_cache();
    }

    fn state(&self) -> PlaybackState {
        self.player.state()
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        // For bulk generation, bypass the cache and use the player directly
        self.player.generate_samples_into(buffer);
    }

    fn sample_rate(&self) -> u32 {
        self.player.sample_rate()
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    fn playback_position(&self) -> f32 {
        self.player.playback_position()
    }

    fn subsong_count(&self) -> usize {
        self.player.subsong_count()
    }

    fn current_subsong(&self) -> usize {
        self.player.current_subsong()
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        if self.player.set_subsong(index) {
            self.reset_cache();
            true
        } else {
            false
        }
    }

    fn psg_count(&self) -> usize {
        self.player.psg_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_cache_basic() {
        let mut cache = SampleCache::new(4);
        assert!(cache.needs_refill());

        // Fill the cache
        cache
            .sample_buffer_mut()
            .copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
        cache.fill_channel_outputs([0.1, 0.2, 0.3]);
        cache.mark_filled();

        assert!(!cache.needs_refill());
        assert_eq!(cache.next_sample(), 1.0);
        assert_eq!(cache.next_sample(), 2.0);
        assert_eq!(cache.channel_outputs(), [0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_sample_cache_reset() {
        let mut cache = SampleCache::new(4);
        cache
            .sample_buffer_mut()
            .copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
        cache.mark_filled();

        let _ = cache.next_sample();
        cache.reset();

        assert!(cache.needs_refill());
    }

    // Mock player for CachedPlayer tests
    struct MockPlayer {
        samples_generated: usize,
        state: PlaybackState,
    }

    impl MockPlayer {
        fn new() -> Self {
            Self {
                samples_generated: 0,
                state: PlaybackState::Playing,
            }
        }
    }

    impl ChiptunePlayerBase for MockPlayer {
        fn play(&mut self) {
            self.state = PlaybackState::Playing;
        }

        fn pause(&mut self) {
            self.state = PlaybackState::Paused;
        }

        fn stop(&mut self) {
            self.state = PlaybackState::Stopped;
        }

        fn state(&self) -> PlaybackState {
            self.state
        }

        fn generate_samples_into(&mut self, buffer: &mut [f32]) {
            for (i, sample) in buffer.iter_mut().enumerate() {
                *sample = (self.samples_generated + i) as f32 * 0.001;
            }
            self.samples_generated += buffer.len();
        }
    }

    impl CacheablePlayer for MockPlayer {
        fn get_channel_outputs(&self) -> [f32; 3] {
            [0.1, 0.2, 0.3]
        }
    }

    #[test]
    fn test_cached_player_generates_samples() {
        let player = MockPlayer::new();
        let mut cached = CachedPlayer::new(player, 16);

        let s1 = cached.generate_sample();
        let s2 = cached.generate_sample();

        assert!((s1 - 0.0).abs() < 0.0001);
        assert!((s2 - 0.001).abs() < 0.0001);
    }

    #[test]
    fn test_cached_player_channel_outputs() {
        let player = MockPlayer::new();
        let mut cached = CachedPlayer::new(player, 16);

        let _ = cached.generate_sample();
        let channels = cached.cached_channel_outputs();

        assert_eq!(channels, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_cache_reset_on_stop() {
        let player = MockPlayer::new();
        let mut cached = CachedPlayer::new(player, 16);

        let _ = cached.generate_sample();
        cached.stop();

        // After stop, cache should need refill
        assert!(cached.cache.needs_refill());
    }
}
