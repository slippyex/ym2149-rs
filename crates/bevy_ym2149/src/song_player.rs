use std::sync::Arc;

use bevy::prelude::error;
use parking_lot::RwLock;
use ym2149::Ym2149Backend;
use ym2149_arkos_replayer::{AksSong, parser::load_aks, player::ArkosPlayer};
use ym2149_ay_replayer::{AyMetadata as AyFileMetadata, AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, MetadataFields, SampleCache};
use ym2149_sndh_replayer::{SndhPlayer, is_sndh_data, load_sndh};
use ym2149_ym_replayer::{self, LoadSummary, YmPlayer};

use crate::audio_source::Ym2149Metadata;
use crate::error::BevyYm2149Error;
use crate::playback::{PlaybackMetrics, YM2149_SAMPLE_RATE, YM2149_SAMPLE_RATE_F32};
use crate::synth::{YmSynthController, YmSynthPlayer};

/// Shared song player handle used throughout the plugin.
pub type SharedSongPlayer = Arc<RwLock<YmSongPlayer>>;

// ============================================================================
// BevyPlayerTrait - Common interface for all player wrappers
// ============================================================================

/// Common interface for all Bevy player wrappers.
///
/// This trait defines the methods that `YmSongPlayer` delegates to its variants.
pub(crate) trait BevyPlayerTrait {
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn state(&self) -> ym2149_common::PlaybackState;
    fn current_frame(&self) -> usize;
    fn samples_per_frame(&self) -> u32;
    fn generate_sample(&mut self) -> f32;
    fn generate_samples_into(&mut self, buffer: &mut [f32]);
    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]);
    fn metadata(&self) -> &Ym2149Metadata;
    fn metrics(&self) -> Option<PlaybackMetrics>;
    fn chip(&self) -> Option<&ym2149::Ym2149>;
    fn frame_count(&self) -> usize;
    fn subsong_count(&self) -> usize;
    fn current_subsong(&self) -> usize;
    fn set_subsong(&mut self, index: usize) -> bool;
}

/// Macro for delegating `YmSongPlayer` methods (with &self) to the inner player via `BevyPlayerTrait`.
macro_rules! delegate_to_inner {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Ym(p) => BevyPlayerTrait::$method(p.as_ref() $(, $arg)*),
            Self::Arkos(p) => BevyPlayerTrait::$method(p.as_ref() $(, $arg)*),
            Self::Ay(p) => BevyPlayerTrait::$method(p.as_ref() $(, $arg)*),
            Self::Sndh(p) => BevyPlayerTrait::$method(p.as_ref() $(, $arg)*),
            Self::Synth(p) => BevyPlayerTrait::$method(p.as_ref() $(, $arg)*),
        }
    };
}

/// Macro for delegating `YmSongPlayer` methods (with &mut self) to the inner player via `BevyPlayerTrait`.
macro_rules! delegate_to_inner_mut {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Ym(p) => BevyPlayerTrait::$method(p.as_mut() $(, $arg)*),
            Self::Arkos(p) => BevyPlayerTrait::$method(p.as_mut() $(, $arg)*),
            Self::Ay(p) => BevyPlayerTrait::$method(p.as_mut() $(, $arg)*),
            Self::Sndh(p) => BevyPlayerTrait::$method(p.as_mut() $(, $arg)*),
            Self::Synth(p) => BevyPlayerTrait::$method(p.as_mut() $(, $arg)*),
        }
    };
}

// ============================================================================
// YmSongPlayer - Unified player enum
// ============================================================================

/// Unified song player that can handle YM, Arkos, AY, SNDH, or Synth sources.
pub enum YmSongPlayer {
    Ym(Box<YmBevyPlayer>),
    Arkos(Box<ArkosBevyPlayer>),
    Ay(Box<AyBevyPlayer>),
    Sndh(Box<SndhBevyPlayer>),
    Synth(Box<YmSynthPlayer>),
}

impl YmSongPlayer {
    pub(crate) fn new_ym(
        player: YmPlayer,
        summary: &LoadSummary,
        metadata: Ym2149Metadata,
    ) -> Self {
        Self::Ym(Box::new(YmBevyPlayer::new(player, summary, metadata)))
    }

    pub(crate) fn new_arkos(song_data: &[u8]) -> Result<Self, BevyYm2149Error> {
        let song = load_aks(song_data)
            .map_err(|e| BevyYm2149Error::Other(format!("AKS load failed: {e}")))?;
        let metadata = Ym2149Metadata {
            title: song.metadata.title.clone(),
            author: song.metadata.author.clone(),
            comment: song.metadata.comments.clone(),
            frame_count: 0,
            duration_seconds: 0.0,
        };
        let song = Arc::new(song);
        let player = ArkosPlayer::new_from_arc(Arc::clone(&song), 0)
            .map_err(|e| BevyYm2149Error::Other(format!("AKS player init failed: {e}")))?;
        Ok(Self::Arkos(Box::new(ArkosBevyPlayer::new(
            player, song, metadata,
        ))))
    }

    pub(crate) fn new_ay(song_data: &[u8]) -> Result<Self, BevyYm2149Error> {
        let (player, metadata) = AyPlayer::load_from_bytes(song_data, 0)
            .map_err(|e| BevyYm2149Error::Other(format!("AY load failed: {e}")))?;
        if player.requires_cpc_firmware() {
            return Err(BevyYm2149Error::Other(CPC_UNSUPPORTED_MSG.to_string()));
        }
        let ym_meta = metadata_from_ay(&metadata);
        Ok(Self::Ay(Box::new(AyBevyPlayer::new(player, ym_meta))))
    }

    pub(crate) fn new_sndh(song_data: &[u8]) -> Result<Self, BevyYm2149Error> {
        let mut player = load_sndh(song_data, YM2149_SAMPLE_RATE)
            .map_err(|e| BevyYm2149Error::Other(format!("SNDH load failed: {e}")))?;

        // Initialize default subsong
        let default_subsong = player.default_subsong();
        player
            .init_subsong(default_subsong)
            .map_err(|e| BevyYm2149Error::Other(format!("SNDH init failed: {e}")))?;

        let meta = ChiptunePlayer::metadata(&player);
        let metadata = Ym2149Metadata {
            title: meta.title().to_string(),
            author: meta.author().to_string(),
            comment: meta.comments().to_string(),
            frame_count: 0, // SNDH doesn't track frames like YM
            duration_seconds: 0.0,
        };

        Ok(Self::Sndh(Box::new(SndhBevyPlayer::new(player, metadata))))
    }

    pub(crate) fn new_synth(controller: YmSynthController) -> Self {
        Self::Synth(Box::new(YmSynthPlayer::new(controller)))
    }

    // Delegated methods using the macro
    pub(crate) fn play(&mut self) {
        delegate_to_inner_mut!(self, play);
    }

    pub(crate) fn pause(&mut self) {
        delegate_to_inner_mut!(self, pause);
    }

    pub(crate) fn stop(&mut self) {
        delegate_to_inner_mut!(self, stop);
    }

    pub(crate) fn state(&self) -> ym2149_common::PlaybackState {
        delegate_to_inner!(self, state)
    }

    pub(crate) fn current_frame(&self) -> usize {
        delegate_to_inner!(self, current_frame)
    }

    pub(crate) fn samples_per_frame(&self) -> u32 {
        delegate_to_inner!(self, samples_per_frame)
    }

    pub(crate) fn generate_sample(&mut self) -> f32 {
        delegate_to_inner_mut!(self, generate_sample)
    }

    pub(crate) fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        delegate_to_inner_mut!(self, generate_samples_into, buffer);
    }

    /// Generate samples and capture per-sample channel outputs for visualization.
    ///
    /// This method generates mono samples into `buffer` and simultaneously captures
    /// the individual channel outputs (A, B, C) into `channel_outputs` for oscilloscope
    /// and spectrum visualization.
    pub(crate) fn generate_samples_with_channels(
        &mut self,
        buffer: &mut [f32],
        channel_outputs: &mut [[f32; 3]],
    ) {
        debug_assert_eq!(buffer.len(), channel_outputs.len());
        for (sample, channels) in buffer.iter_mut().zip(channel_outputs.iter_mut()) {
            let (s, c) = delegate_to_inner_mut!(self, generate_sample_with_channels);
            *sample = s;
            *channels = c;
        }
    }

    pub(crate) fn metadata(&self) -> &Ym2149Metadata {
        delegate_to_inner!(self, metadata)
    }

    pub(crate) fn metrics(&self) -> Option<PlaybackMetrics> {
        delegate_to_inner!(self, metrics)
    }

    /// Returns a reference to the underlying YM2149 chip, if available.
    pub fn chip(&self) -> Option<&ym2149::Ym2149> {
        delegate_to_inner!(self, chip)
    }

    /// Returns a reference to the underlying YM2149 chip.
    ///
    /// # Panics
    ///
    /// Panics if the player doesn't have an associated chip (should not happen
    /// for normal playback).
    #[deprecated(since = "0.8.0", note = "Use chip() which returns Option instead")]
    pub fn get_chip(&self) -> &ym2149::Ym2149 {
        self.chip()
            .expect("Player should always expose at least one PSG")
    }

    /// Total frame count if known (falls back to metrics/estimates)
    pub fn frame_count(&self) -> usize {
        delegate_to_inner!(self, frame_count)
    }

    /// Get the number of subsongs/tracks available.
    pub fn subsong_count(&self) -> usize {
        delegate_to_inner!(self, subsong_count)
    }

    /// Get the current subsong index (1-based).
    pub fn current_subsong(&self) -> usize {
        delegate_to_inner!(self, current_subsong)
    }

    /// Switch to a different subsong. Returns true if successful.
    pub fn set_subsong(&mut self, index: usize) -> bool {
        delegate_to_inner_mut!(self, set_subsong, index)
    }

    /// Check if this player supports multiple subsongs.
    pub fn has_subsongs(&self) -> bool {
        self.subsong_count() > 1
    }

    /// Seek to a percentage position (0.0 to 1.0).
    ///
    /// Returns true if seeking succeeded. Currently only supported for SNDH.
    pub fn seek_percentage(&mut self, position: f32) -> bool {
        match self {
            Self::Sndh(p) => p.seek_percentage(position),
            _ => false, // Other formats don't support percentage seeking yet
        }
    }

    /// Get duration in seconds.
    ///
    /// For SNDH < 2.2 without FRMS/TIME, returns 300 (5 minute fallback).
    pub fn duration_seconds(&self) -> f32 {
        match self {
            Self::Ym(p) => p.metrics.duration_seconds(),
            Self::Arkos(p) => p.metadata.duration_seconds,
            Self::Ay(p) => p.metadata.duration_seconds,
            Self::Sndh(p) => p.duration_seconds(),
            Self::Synth(p) => p.metrics().duration_seconds(),
        }
    }

    /// Check if duration is from actual metadata or estimated.
    ///
    /// Returns false for older SNDH files using the 5-minute fallback.
    pub fn has_duration_info(&self) -> bool {
        match self {
            Self::Sndh(p) => p.has_duration_info(),
            _ => true, // Other formats always have duration info
        }
    }
}

// ============================================================================
// YmBevyPlayer - Wrapper for YM format
// ============================================================================

/// Wrapper for YM player that implements `BevyPlayerTrait`.
pub struct YmBevyPlayer {
    player: YmPlayer,
    metrics: PlaybackMetrics,
    metadata: Ym2149Metadata,
}

impl YmBevyPlayer {
    fn new(player: YmPlayer, summary: &LoadSummary, metadata: Ym2149Metadata) -> Self {
        Self {
            player,
            metrics: PlaybackMetrics::from(summary),
            metadata,
        }
    }
}

impl BevyPlayerTrait for YmBevyPlayer {
    fn play(&mut self) {
        self.player.play();
    }

    fn pause(&mut self) {
        self.player.pause();
    }

    fn stop(&mut self) {
        self.player.stop();
    }

    fn state(&self) -> ym2149_common::PlaybackState {
        self.player.state()
    }

    fn current_frame(&self) -> usize {
        self.player.get_current_frame()
    }

    fn samples_per_frame(&self) -> u32 {
        self.player.samples_per_frame_value()
    }

    fn generate_sample(&mut self) -> f32 {
        self.player.generate_sample()
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]) {
        let sample = self.player.generate_sample();
        let (a, b, c) = self.player.get_chip().get_channel_outputs();
        (sample, [a, b, c])
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(self.metrics)
    }

    fn chip(&self) -> Option<&ym2149::Ym2149> {
        Some(self.player.get_chip())
    }

    fn frame_count(&self) -> usize {
        self.metrics.frame_count
    }

    fn subsong_count(&self) -> usize {
        1
    }

    fn current_subsong(&self) -> usize {
        1
    }

    fn set_subsong(&mut self, _index: usize) -> bool {
        false
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Load a song (YM, AKS, AY, or SNDH) from raw bytes.
pub(crate) fn load_song_from_bytes(
    data: &[u8],
) -> std::result::Result<(YmSongPlayer, PlaybackMetrics, Ym2149Metadata), String> {
    // Check if this looks like SNDH data first (to avoid wrong format fallback)
    if is_sndh_data(data) {
        return YmSongPlayer::new_sndh(data)
            .map(|player| {
                let metadata = player.metadata().clone();
                let metrics = player.metrics().unwrap_or(PlaybackMetrics {
                    frame_count: 0,
                    samples_per_frame: YM2149_SAMPLE_RATE,
                });
                (player, metrics, metadata)
            })
            .map_err(|e| format!("Failed to load SNDH: {e}"));
    }

    // Try other formats in order
    if let Ok((player, summary)) = ym2149_ym_replayer::load_song(data) {
        let metadata = metadata_from_player(&player, &summary);
        let metrics = PlaybackMetrics::from(&summary);
        Ok((
            YmSongPlayer::new_ym(player, &summary, metadata.clone()),
            metrics,
            metadata,
        ))
    } else if let Ok(player) = YmSongPlayer::new_arkos(data) {
        let metadata = player.metadata().clone();
        let metrics = player.metrics().unwrap_or(PlaybackMetrics {
            frame_count: metadata.frame_count,
            samples_per_frame: YM2149_SAMPLE_RATE,
        });
        Ok((player, metrics, metadata))
    } else if let Ok(player) = YmSongPlayer::new_sndh(data) {
        let metadata = player.metadata().clone();
        let metrics = player.metrics().unwrap_or(PlaybackMetrics {
            frame_count: 0,
            samples_per_frame: YM2149_SAMPLE_RATE,
        });
        Ok((player, metrics, metadata))
    } else {
        let player =
            YmSongPlayer::new_ay(data).map_err(|e| format!("Failed to load AY song: {e}"))?;
        let metadata = player.metadata().clone();
        let metrics = PlaybackMetrics {
            frame_count: metadata.frame_count,
            samples_per_frame: YM2149_SAMPLE_RATE,
        };
        Ok((player, metrics, metadata))
    }
}

fn metadata_from_player(player: &YmPlayer, summary: &LoadSummary) -> Ym2149Metadata {
    let frame_count = summary.frame_count;
    let (title, author, comment) = if let Some(info) = player.info() {
        (
            info.song_name.clone(),
            info.author.clone(),
            info.comment.clone(),
        )
    } else {
        (String::new(), String::new(), String::new())
    };

    let samples_per_frame = summary.samples_per_frame as f32;
    let total_samples = frame_count as f32 * samples_per_frame;
    let duration_seconds = total_samples / YM2149_SAMPLE_RATE_F32;

    Ym2149Metadata {
        title,
        author,
        comment,
        frame_count,
        duration_seconds,
    }
}

fn metadata_from_ay(meta: &AyFileMetadata) -> Ym2149Metadata {
    let frame_count = meta.frame_count.unwrap_or(0);
    let duration_seconds = meta
        .duration_seconds
        .unwrap_or_else(|| frame_count as f32 / 50.0);
    Ym2149Metadata {
        title: meta.song_name.clone(),
        author: meta.author.clone(),
        comment: meta.misc.clone(),
        frame_count,
        duration_seconds,
    }
}

// ============================================================================
// ArkosBevyPlayer
// ============================================================================

const ARKOS_CACHE_SIZE: usize = 1024;

/// Adapter that exposes [`ArkosPlayer`] through the `BevyPlayerTrait` interface.
pub struct ArkosBevyPlayer {
    player: ArkosPlayer,
    song: Arc<AksSong>,
    metadata: Ym2149Metadata,
    samples_per_frame: u32,
    estimated_frames: usize,
    cache: SampleCache,
    current_subsong: usize,
}

impl ArkosBevyPlayer {
    fn new(player: ArkosPlayer, song: Arc<AksSong>, mut metadata: Ym2149Metadata) -> Self {
        let samples_per_frame = (YM2149_SAMPLE_RATE_F32 / player.replay_frequency_hz())
            .round()
            .max(1.0) as u32;
        let estimated_frames = player.estimated_total_ticks().max(1);
        metadata.frame_count = estimated_frames;
        metadata.duration_seconds =
            (estimated_frames as f32 * samples_per_frame as f32) / YM2149_SAMPLE_RATE_F32;

        Self {
            player,
            song,
            metadata,
            samples_per_frame,
            estimated_frames,
            cache: SampleCache::new(ARKOS_CACHE_SIZE),
            current_subsong: 0,
        }
    }

    fn refill_cache(&mut self) {
        self.player
            .generate_samples_into(self.cache.sample_buffer_mut());
        let outputs = self
            .player
            .chip(0)
            .map(|chip| {
                let (a, b, c) = chip.get_channel_outputs();
                [a, b, c]
            })
            .unwrap_or([0.0; 3]);
        self.cache.fill_channel_outputs(outputs);
        self.cache.mark_filled();
    }
}

impl BevyPlayerTrait for ArkosBevyPlayer {
    fn play(&mut self) {
        ChiptunePlayerBase::play(&mut self.player);
    }

    fn pause(&mut self) {
        ChiptunePlayerBase::pause(&mut self.player);
    }

    fn stop(&mut self) {
        ChiptunePlayerBase::stop(&mut self.player);
    }

    fn state(&self) -> ym2149_common::PlaybackState {
        ChiptunePlayerBase::state(&self.player)
    }

    fn current_frame(&self) -> usize {
        self.player.current_tick_index()
    }

    fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    fn generate_sample(&mut self) -> f32 {
        if self.cache.needs_refill() {
            self.refill_cache();
        }
        self.cache.next_sample()
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]) {
        let sample = self.generate_sample();
        (sample, self.cache.channel_outputs())
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(PlaybackMetrics {
            frame_count: self.estimated_frames,
            samples_per_frame: self.samples_per_frame,
        })
    }

    fn chip(&self) -> Option<&ym2149::Ym2149> {
        self.player.chip(0)
    }

    fn frame_count(&self) -> usize {
        self.estimated_frames
    }

    fn subsong_count(&self) -> usize {
        self.song.subsongs.len()
    }

    fn current_subsong(&self) -> usize {
        self.current_subsong + 1
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        let zero_based = index.saturating_sub(1);
        if zero_based < self.song.subsongs.len()
            && let Ok(new_player) = ArkosPlayer::new_from_arc(Arc::clone(&self.song), zero_based)
        {
            self.player = new_player;
            self.current_subsong = zero_based;
            let _ = self.player.play();
            self.cache.reset();
            return true;
        }
        false
    }
}

// ============================================================================
// AyBevyPlayer
// ============================================================================

const AY_CACHE_SIZE: usize = 512;

pub struct AyBevyPlayer {
    player: AyPlayer,
    metadata: Ym2149Metadata,
    cache: SampleCache,
    unsupported: bool,
    warned: bool,
}

impl AyBevyPlayer {
    fn new(player: AyPlayer, metadata: Ym2149Metadata) -> Self {
        Self {
            player,
            metadata,
            cache: SampleCache::new(AY_CACHE_SIZE),
            unsupported: false,
            warned: false,
        }
    }

    fn fill_cache(&mut self) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, self.cache.sample_buffer_mut());
        if self.check_and_mark_unsupported() {
            self.cache.sample_buffer_mut().fill(0.0);
            self.cache.fill_channel_outputs([0.0; 3]);
        } else {
            let (a, b, c) = self.player.chip().get_channel_outputs();
            self.cache.fill_channel_outputs([a, b, c]);
        }
        self.cache.mark_filled();
    }

    fn check_and_mark_unsupported(&mut self) -> bool {
        if self.unsupported || self.player.requires_cpc_firmware() {
            if !self.warned {
                error!("{CPC_UNSUPPORTED_MSG}");
                self.warned = true;
            }
            self.unsupported = true;
            true
        } else {
            false
        }
    }
}

impl BevyPlayerTrait for AyBevyPlayer {
    fn play(&mut self) {
        if self.unsupported {
            return;
        }
        ChiptunePlayerBase::play(&mut self.player);
        self.check_and_mark_unsupported();
    }

    fn pause(&mut self) {
        ChiptunePlayerBase::pause(&mut self.player);
    }

    fn stop(&mut self) {
        ChiptunePlayerBase::stop(&mut self.player);
    }

    fn state(&self) -> ym2149_common::PlaybackState {
        ChiptunePlayerBase::state(&self.player)
    }

    fn current_frame(&self) -> usize {
        self.player.current_frame()
    }

    fn samples_per_frame(&self) -> u32 {
        YM2149_SAMPLE_RATE
    }

    fn generate_sample(&mut self) -> f32 {
        if self.unsupported {
            return 0.0;
        }
        if self.cache.needs_refill() {
            self.fill_cache();
        }
        self.cache.next_sample()
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        if self.unsupported {
            buffer.fill(0.0);
            return;
        }
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
        if self.check_and_mark_unsupported() {
            buffer.fill(0.0);
        }
    }

    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]) {
        let sample = self.generate_sample();
        (sample, self.cache.channel_outputs())
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(PlaybackMetrics {
            frame_count: self.metadata.frame_count,
            samples_per_frame: YM2149_SAMPLE_RATE,
        })
    }

    fn chip(&self) -> Option<&ym2149::Ym2149> {
        Some(self.player.chip())
    }

    fn frame_count(&self) -> usize {
        self.metadata.frame_count
    }

    fn subsong_count(&self) -> usize {
        self.player.metadata().song_count
    }

    fn current_subsong(&self) -> usize {
        self.player.metadata().song_index + 1
    }

    fn set_subsong(&mut self, _index: usize) -> bool {
        false
    }
}

// ============================================================================
// SndhBevyPlayer
// ============================================================================

const SNDH_CACHE_SIZE: usize = 512;

/// Adapter that exposes [`SndhPlayer`] through the `BevyPlayerTrait` interface.
pub struct SndhBevyPlayer {
    player: SndhPlayer,
    metadata: Ym2149Metadata,
    samples_per_frame: u32,
    cache: SampleCache,
}

impl SndhBevyPlayer {
    fn new(player: SndhPlayer, metadata: Ym2149Metadata) -> Self {
        let samples_per_frame = (YM2149_SAMPLE_RATE_F32 / player.player_rate() as f32)
            .round()
            .max(1.0) as u32;

        Self {
            player,
            metadata,
            samples_per_frame,
            cache: SampleCache::new(SNDH_CACHE_SIZE),
        }
    }

    fn fill_cache(&mut self) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, self.cache.sample_buffer_mut());
        let (a, b, c) = self.player.ym2149().get_channel_outputs();
        self.cache.fill_channel_outputs([a, b, c]);
        self.cache.mark_filled();
    }
}

impl SndhBevyPlayer {
    /// Seek to a percentage position (0.0 to 1.0).
    pub fn seek_percentage(&mut self, position: f32) -> bool {
        let result = ChiptunePlayerBase::seek(&mut self.player, position);
        if result {
            self.cache.reset();
        }
        result
    }

    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        ChiptunePlayerBase::duration_seconds(&self.player)
    }

    /// Check if duration is from metadata or estimated.
    pub fn has_duration_info(&self) -> bool {
        self.player.has_duration_info()
    }
}

impl BevyPlayerTrait for SndhBevyPlayer {
    fn play(&mut self) {
        ChiptunePlayerBase::play(&mut self.player);
    }

    fn pause(&mut self) {
        ChiptunePlayerBase::pause(&mut self.player);
    }

    fn stop(&mut self) {
        ChiptunePlayerBase::stop(&mut self.player);
    }

    fn state(&self) -> ym2149_common::PlaybackState {
        ChiptunePlayerBase::state(&self.player)
    }

    fn current_frame(&self) -> usize {
        self.player.current_frame() as usize
    }

    fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    fn generate_sample(&mut self) -> f32 {
        if self.cache.needs_refill() {
            self.fill_cache();
        }
        self.cache.next_sample()
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]) {
        let sample = self.generate_sample();
        (sample, self.cache.channel_outputs())
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(PlaybackMetrics {
            frame_count: self.player.total_frames() as usize,
            samples_per_frame: self.samples_per_frame,
        })
    }

    fn chip(&self) -> Option<&ym2149::Ym2149> {
        Some(self.player.ym2149())
    }

    fn frame_count(&self) -> usize {
        self.player.total_frames() as usize
    }

    fn subsong_count(&self) -> usize {
        ChiptunePlayerBase::subsong_count(&self.player)
    }

    fn current_subsong(&self) -> usize {
        ChiptunePlayerBase::current_subsong(&self.player)
    }

    fn set_subsong(&mut self, index: usize) -> bool {
        if ChiptunePlayerBase::set_subsong(&mut self.player, index) {
            self.cache.reset();
            true
        } else {
            false
        }
    }
}

// ============================================================================
// YmSynthPlayer trait impl
// ============================================================================

impl BevyPlayerTrait for YmSynthPlayer {
    fn play(&mut self) {
        YmSynthPlayer::play(self);
    }

    fn pause(&mut self) {
        YmSynthPlayer::pause(self);
    }

    fn stop(&mut self) {
        YmSynthPlayer::stop(self);
    }

    fn state(&self) -> ym2149_common::PlaybackState {
        YmSynthPlayer::state(self)
    }

    fn current_frame(&self) -> usize {
        YmSynthPlayer::current_frame(self)
    }

    fn samples_per_frame(&self) -> u32 {
        YmSynthPlayer::samples_per_frame(self)
    }

    fn generate_sample(&mut self) -> f32 {
        YmSynthPlayer::generate_sample(self)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        YmSynthPlayer::generate_samples_into(self, buffer);
    }

    fn generate_sample_with_channels(&mut self) -> (f32, [f32; 3]) {
        let sample = YmSynthPlayer::generate_sample(self);
        let (a, b, c) = self.chip().get_channel_outputs();
        (sample, [a, b, c])
    }

    fn metadata(&self) -> &Ym2149Metadata {
        YmSynthPlayer::metadata(self)
    }

    fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(YmSynthPlayer::metrics(self))
    }

    fn chip(&self) -> Option<&ym2149::Ym2149> {
        Some(YmSynthPlayer::chip(self))
    }

    fn frame_count(&self) -> usize {
        YmSynthPlayer::metrics(self).frame_count
    }

    fn subsong_count(&self) -> usize {
        1
    }

    fn current_subsong(&self) -> usize {
        1
    }

    fn set_subsong(&mut self, _index: usize) -> bool {
        false
    }
}
