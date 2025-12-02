use std::sync::Arc;

use bevy::prelude::error;
use parking_lot::RwLock;
use ym2149_arkos_replayer::{AksSong, parser::load_aks, player::ArkosPlayer};
use ym2149_ay_replayer::{AyMetadata as AyFileMetadata, AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase, MetadataFields};
use ym2149_sndh_replayer::{SndhPlayer, is_sndh_data, load_sndh};
use ym2149_ym_replayer::{self, LoadSummary, YmPlayer};

use crate::audio_source::Ym2149Metadata;
use crate::error::BevyYm2149Error;
use crate::playback::{PlaybackMetrics, YM2149_SAMPLE_RATE, YM2149_SAMPLE_RATE_F32};
use crate::synth::{YmSynthController, YmSynthPlayer};

/// Shared song player handle used throughout the plugin.
pub type SharedSongPlayer = Arc<RwLock<YmSongPlayer>>;

/// Unified song player that can handle YM or Arkos Tracker sources.
pub enum YmSongPlayer {
    Ym {
        player: Box<YmPlayer>,
        metrics: PlaybackMetrics,
        metadata: Ym2149Metadata,
    },
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
        Self::Ym {
            metrics: PlaybackMetrics::from(summary),
            player: Box::new(player),
            metadata,
        }
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

    pub(crate) fn play(&mut self) {
        match self {
            Self::Ym { player, .. } => player.play(),
            Self::Arkos(p) => p.play(),
            Self::Ay(p) => p.play(),
            Self::Sndh(p) => p.play(),
            Self::Synth(p) => p.play(),
        }
    }

    pub(crate) fn pause(&mut self) {
        match self {
            Self::Ym { player, .. } => player.pause(),
            Self::Arkos(p) => p.pause(),
            Self::Ay(p) => p.pause(),
            Self::Sndh(p) => p.pause(),
            Self::Synth(p) => p.pause(),
        }
    }

    pub(crate) fn stop(&mut self) {
        match self {
            Self::Ym { player, .. } => player.stop(),
            Self::Arkos(p) => p.stop(),
            Self::Ay(p) => p.stop(),
            Self::Sndh(p) => p.stop(),
            Self::Synth(p) => p.stop(),
        }
    }

    pub(crate) fn state(&self) -> ym2149_common::PlaybackState {
        match self {
            Self::Ym { player, .. } => player.state(),
            Self::Arkos(p) => p.state(),
            Self::Ay(p) => p.state(),
            Self::Sndh(p) => p.state(),
            Self::Synth(p) => p.state(),
        }
    }

    pub(crate) fn get_current_frame(&self) -> usize {
        match self {
            Self::Ym { player, .. } => player.get_current_frame(),
            Self::Arkos(p) => p.current_frame(),
            Self::Ay(p) => p.current_frame(),
            Self::Sndh(p) => p.current_frame(),
            Self::Synth(p) => p.current_frame(),
        }
    }

    pub(crate) fn samples_per_frame_value(&self) -> u32 {
        match self {
            Self::Ym { player, .. } => player.samples_per_frame_value(),
            Self::Arkos(p) => p.samples_per_frame(),
            Self::Ay(p) => p.samples_per_frame(),
            Self::Sndh(p) => p.samples_per_frame(),
            Self::Synth(p) => p.samples_per_frame_value(),
        }
    }

    pub(crate) fn generate_sample(&mut self) -> f32 {
        match self {
            Self::Ym { player, .. } => player.generate_sample(),
            Self::Arkos(p) => p.generate_sample(),
            Self::Ay(p) => p.generate_sample(),
            Self::Sndh(p) => p.generate_sample(),
            Self::Synth(p) => p.generate_sample(),
        }
    }

    pub(crate) fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        match self {
            Self::Ym { player, .. } => player.generate_samples_into(buffer),
            Self::Arkos(p) => p.generate_samples_into(buffer),
            Self::Ay(p) => p.generate_samples_into(buffer),
            Self::Sndh(p) => p.generate_samples_into(buffer),
            Self::Synth(p) => p.generate_samples_into(buffer),
        }
    }

    pub(crate) fn metadata(&self) -> &Ym2149Metadata {
        match self {
            Self::Ym { metadata, .. } => metadata,
            Self::Arkos(p) => p.metadata(),
            Self::Ay(p) => p.metadata(),
            Self::Sndh(p) => p.metadata(),
            Self::Synth(p) => p.metadata(),
        }
    }

    pub(crate) fn metrics(&self) -> Option<PlaybackMetrics> {
        match self {
            Self::Ym { metrics, .. } => Some(*metrics),
            Self::Arkos(p) => p.metrics(),
            Self::Ay(p) => Some(p.metrics()),
            Self::Sndh(p) => Some(p.metrics()),
            Self::Synth(p) => Some(p.metrics()),
        }
    }

    pub(crate) fn chip(&self) -> Option<&ym2149::ym2149::Ym2149> {
        match self {
            Self::Ym { player, .. } => Some(player.get_chip()),
            Self::Arkos(p) => p.primary_chip(),
            Self::Ay(p) => Some(p.chip()),
            Self::Sndh(p) => Some(p.chip()),
            Self::Synth(p) => Some(p.chip()),
        }
    }

    /// Backwards-compat helper for visualization crates (exposes current PSG chip)
    pub fn get_chip(&self) -> &ym2149::ym2149::Ym2149 {
        match self {
            Self::Ym { player, .. } => player.get_chip(),
            Self::Arkos(p) => p
                .primary_chip()
                .expect("Arkos player should always expose at least one PSG"),
            Self::Ay(p) => p.chip(),
            Self::Sndh(p) => p.chip(),
            Self::Synth(p) => p.chip(),
        }
    }

    /// Total frame count if known (falls back to metrics/estimates)
    pub fn frame_count(&self) -> usize {
        match self {
            Self::Ym { metrics, .. } => metrics.frame_count,
            Self::Arkos(p) => p.frame_count(),
            Self::Ay(p) => p.frame_count(),
            Self::Sndh(p) => p.frame_count(),
            Self::Synth(p) => p.metrics().frame_count,
        }
    }

    /// Get the number of subsongs/tracks available.
    /// Returns 1 for formats that don't support multiple subsongs.
    pub fn subsong_count(&self) -> usize {
        match self {
            Self::Ym { .. } => 1,
            Self::Arkos(p) => p.subsong_count(),
            Self::Ay(p) => p.subsong_count(),
            Self::Sndh(p) => p.subsong_count(),
            Self::Synth(_) => 1,
        }
    }

    /// Get the current subsong index (1-based).
    /// Returns 1 for formats that don't support multiple subsongs.
    pub fn current_subsong(&self) -> usize {
        match self {
            Self::Ym { .. } => 1,
            Self::Arkos(p) => p.current_subsong(),
            Self::Ay(p) => p.current_subsong(),
            Self::Sndh(p) => p.current_subsong(),
            Self::Synth(_) => 1,
        }
    }

    /// Switch to a different subsong. Returns true if successful.
    /// The index is 1-based.
    pub fn set_subsong(&mut self, index: usize) -> bool {
        match self {
            Self::Ym { .. } => false,
            Self::Arkos(p) => p.set_subsong(index),
            Self::Ay(p) => p.set_subsong(index),
            Self::Sndh(p) => p.set_subsong(index),
            Self::Synth(_) => false,
        }
    }

    /// Check if this player supports multiple subsongs.
    pub fn has_subsongs(&self) -> bool {
        self.subsong_count() > 1
    }
}

/// Load a song (YM, AKS, AY, or SNDH) from raw bytes.
pub(crate) fn load_song_from_bytes(
    data: &[u8],
) -> std::result::Result<(YmSongPlayer, PlaybackMetrics, Ym2149Metadata), String> {
    // Check if this looks like SNDH data first (to avoid wrong format fallback)
    if is_sndh_data(data) {
        // Try SNDH first for SNDH-like data, and return error directly if it fails
        // (don't fall back to AY for SNDH files)
        return YmSongPlayer::new_sndh(data)
            .map(|player| {
                let metadata = player.metadata().clone();
                let metrics = player.metrics().unwrap_or(PlaybackMetrics {
                    frame_count: 0,
                    samples_per_frame: YM2149_SAMPLE_RATE,
                });
                (player, metrics, metadata)
            })
            .map_err(|e| format!("Failed to load SNDH: {}", e));
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
        // Also try SNDH for non-SNDH-looking data (edge cases)
        let metadata = player.metadata().clone();
        let metrics = player.metrics().unwrap_or(PlaybackMetrics {
            frame_count: 0,
            samples_per_frame: YM2149_SAMPLE_RATE,
        });
        Ok((player, metrics, metadata))
    } else {
        let player =
            YmSongPlayer::new_ay(data).map_err(|e| format!("Failed to load AY song: {}", e))?;
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

/// Adapter that exposes [`ArkosPlayer`] through the same interface used by YM playback.
pub struct ArkosBevyPlayer {
    player: ArkosPlayer,
    song: Arc<AksSong>,
    metadata: Ym2149Metadata,
    samples_per_frame: u32,
    estimated_frames: usize,
    cache: Vec<f32>,
    cache_pos: usize,
    cache_len: usize,
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
            cache: vec![0.0; 1024],
            cache_pos: 0,
            cache_len: 0,
            current_subsong: 0,
        }
    }

    pub(crate) fn play(&mut self) {
        ChiptunePlayerBase::play(&mut self.player);
    }

    pub(crate) fn pause(&mut self) {
        ChiptunePlayerBase::pause(&mut self.player);
    }

    pub(crate) fn stop(&mut self) {
        ChiptunePlayerBase::stop(&mut self.player);
    }

    pub(crate) fn state(&self) -> ym2149_common::PlaybackState {
        ChiptunePlayerBase::state(&self.player)
    }

    pub(crate) fn current_frame(&self) -> usize {
        self.player.current_tick_index()
    }

    pub(crate) fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    pub(crate) fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    pub(crate) fn metrics(&self) -> Option<PlaybackMetrics> {
        Some(PlaybackMetrics {
            frame_count: self.estimated_frames,
            samples_per_frame: self.samples_per_frame,
        })
    }

    pub(crate) fn primary_chip(&self) -> Option<&ym2149::ym2149::Ym2149> {
        self.player.chip(0)
    }

    pub fn frame_count(&self) -> usize {
        self.estimated_frames
    }

    pub(crate) fn generate_sample(&mut self) -> f32 {
        if self.cache_pos >= self.cache_len {
            self.refill_cache();
        }
        if self.cache_len == 0 {
            return 0.0;
        }
        let sample = self.cache[self.cache_pos];
        self.cache_pos += 1;
        sample
    }

    pub(crate) fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    fn refill_cache(&mut self) {
        if self.cache.is_empty() {
            self.cache.resize(1024, 0.0);
        }
        self.player.generate_samples_into(&mut self.cache);
        self.cache_len = self.cache.len();
        self.cache_pos = 0;
    }

    pub fn subsong_count(&self) -> usize {
        self.song.subsongs.len()
    }

    pub fn current_subsong(&self) -> usize {
        // Return 1-based index for consistency
        self.current_subsong + 1
    }

    pub fn set_subsong(&mut self, index: usize) -> bool {
        // Convert 1-based input to 0-based
        let zero_based = index.saturating_sub(1);
        if zero_based < self.song.subsongs.len()
            && let Ok(new_player) = ArkosPlayer::new_from_arc(Arc::clone(&self.song), zero_based)
        {
            self.player = new_player;
            self.current_subsong = zero_based;
            let _ = self.player.play();
            // Reset cache
            self.cache_pos = 0;
            self.cache_len = 0;
            return true;
        }
        false
    }
}

const AY_CACHE_SAMPLES: usize = 512;

pub struct AyBevyPlayer {
    player: AyPlayer,
    metadata: Ym2149Metadata,
    cache: Vec<f32>,
    cache_pos: usize,
    cache_len: usize,
    unsupported: bool,
    warned: bool,
}

impl AyBevyPlayer {
    fn new(player: AyPlayer, metadata: Ym2149Metadata) -> Self {
        Self {
            player,
            metadata,
            cache: vec![0.0; AY_CACHE_SAMPLES],
            cache_pos: 0,
            cache_len: 0,
            unsupported: false,
            warned: false,
        }
    }

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
        if self.cache_pos >= self.cache_len {
            self.fill_cache();
            if self.cache_len == 0 {
                return 0.0;
            }
        }
        if self.unsupported {
            return 0.0;
        }
        let sample = self.cache[self.cache_pos];
        self.cache_pos += 1;
        sample
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

    fn fill_cache(&mut self) {
        ChiptunePlayerBase::generate_samples_into(
            &mut self.player,
            &mut self.cache[..AY_CACHE_SAMPLES],
        );
        if self.check_and_mark_unsupported() {
            self.cache.fill(0.0);
        }
        self.cache_pos = 0;
        self.cache_len = AY_CACHE_SAMPLES;
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> PlaybackMetrics {
        PlaybackMetrics {
            frame_count: self.metadata.frame_count,
            samples_per_frame: self.samples_per_frame(),
        }
    }

    fn chip(&self) -> &ym2149::ym2149::Ym2149 {
        self.player.chip()
    }

    fn frame_count(&self) -> usize {
        self.metadata.frame_count
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

    pub fn subsong_count(&self) -> usize {
        // AY files support multiple songs via metadata, but we don't currently
        // support switching songs without reloading
        self.player.metadata().song_count
    }

    pub fn current_subsong(&self) -> usize {
        // 1-based
        self.player.metadata().song_index + 1
    }

    pub fn set_subsong(&mut self, _index: usize) -> bool {
        // TODO: Support subsong switching for AY files (requires storing raw data)
        false
    }
}

const SNDH_CACHE_SAMPLES: usize = 512;

/// Adapter that exposes [`SndhPlayer`] through the same interface used by YM playback.
pub struct SndhBevyPlayer {
    player: SndhPlayer,
    metadata: Ym2149Metadata,
    samples_per_frame: u32,
    cache: Vec<f32>,
    cache_pos: usize,
    cache_len: usize,
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
            cache: vec![0.0; SNDH_CACHE_SAMPLES],
            cache_pos: 0,
            cache_len: 0,
        }
    }

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
        0 // SNDH doesn't track frames like YM
    }

    fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    fn generate_sample(&mut self) -> f32 {
        if self.cache_pos >= self.cache_len {
            self.fill_cache();
        }
        if self.cache_len == 0 {
            return 0.0;
        }
        let sample = self.cache[self.cache_pos];
        self.cache_pos += 1;
        sample
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        ChiptunePlayerBase::generate_samples_into(&mut self.player, buffer);
    }

    fn fill_cache(&mut self) {
        ChiptunePlayerBase::generate_samples_into(
            &mut self.player,
            &mut self.cache[..SNDH_CACHE_SAMPLES],
        );
        self.cache_pos = 0;
        self.cache_len = SNDH_CACHE_SAMPLES;
    }

    fn metadata(&self) -> &Ym2149Metadata {
        &self.metadata
    }

    fn metrics(&self) -> PlaybackMetrics {
        PlaybackMetrics {
            frame_count: 0,
            samples_per_frame: self.samples_per_frame,
        }
    }

    fn chip(&self) -> &ym2149::ym2149::Ym2149 {
        self.player.ym2149()
    }

    fn frame_count(&self) -> usize {
        0 // SNDH files don't have a known frame count
    }

    pub fn subsong_count(&self) -> usize {
        ChiptunePlayerBase::subsong_count(&self.player)
    }

    pub fn current_subsong(&self) -> usize {
        ChiptunePlayerBase::current_subsong(&self.player)
    }

    pub fn set_subsong(&mut self, index: usize) -> bool {
        if ChiptunePlayerBase::set_subsong(&mut self.player, index) {
            // Reset cache
            self.cache_pos = 0;
            self.cache_len = 0;
            true
        } else {
            false
        }
    }
}
