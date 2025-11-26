use std::sync::Arc;

use bevy::prelude::error;
use parking_lot::RwLock;
use ym2149_arkos_replayer::{parser::load_aks, player::ArkosPlayer};
use ym2149_ay_replayer::{
    AyMetadata as AyFileMetadata, AyPlayer, CPC_UNSUPPORTED_MSG, PlaybackState as AyState,
};
use ym2149_ym_replayer::{self, LoadSummary, PlaybackController, Ym6Player};

use crate::audio_source::Ym2149Metadata;
use crate::error::BevyYm2149Error;
use crate::playback::{PlaybackMetrics, YM2149_SAMPLE_RATE, YM2149_SAMPLE_RATE_F32};
use crate::synth::{YmSynthController, YmSynthPlayer};

/// Shared song player handle used throughout the plugin.
pub type SharedSongPlayer = Arc<RwLock<YmSongPlayer>>;

/// Unified song player that can handle YM or Arkos Tracker sources.
pub enum YmSongPlayer {
    Ym {
        player: Box<Ym6Player>,
        metrics: PlaybackMetrics,
        metadata: Ym2149Metadata,
    },
    Arkos(Box<ArkosBevyPlayer>),
    Ay(Box<AyBevyPlayer>),
    Synth(Box<YmSynthPlayer>),
}

impl YmSongPlayer {
    pub(crate) fn new_ym(
        player: Ym6Player,
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
        let player = ArkosPlayer::new(song, 0)
            .map_err(|e| BevyYm2149Error::Other(format!("AKS player init failed: {e}")))?;
        Ok(Self::Arkos(Box::new(ArkosBevyPlayer::new(
            player, metadata,
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

    pub(crate) fn new_synth(controller: YmSynthController) -> Self {
        Self::Synth(Box::new(YmSynthPlayer::new(controller)))
    }

    pub(crate) fn play(&mut self) -> Result<(), BevyYm2149Error> {
        match self {
            Self::Ym { player, .. } => player
                .play()
                .map_err(|e| BevyYm2149Error::Other(format!("YM play failed: {e}"))),
            Self::Arkos(p) => p.play(),
            Self::Ay(p) => p.play(),
            Self::Synth(p) => {
                p.play();
                Ok(())
            }
        }
    }

    pub(crate) fn pause(&mut self) -> Result<(), BevyYm2149Error> {
        match self {
            Self::Ym { player, .. } => player
                .pause()
                .map_err(|e| BevyYm2149Error::Other(format!("YM pause failed: {e}"))),
            Self::Arkos(p) => p.pause(),
            Self::Ay(p) => p.pause(),
            Self::Synth(p) => {
                p.pause();
                Ok(())
            }
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), BevyYm2149Error> {
        match self {
            Self::Ym { player, .. } => player
                .stop()
                .map_err(|e| BevyYm2149Error::Other(format!("YM stop failed: {e}"))),
            Self::Arkos(p) => p.stop(),
            Self::Ay(p) => p.stop(),
            Self::Synth(p) => {
                p.stop();
                Ok(())
            }
        }
    }

    pub(crate) fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        match self {
            Self::Ym { player, .. } => player.state(),
            Self::Arkos(p) => p.state(),
            Self::Ay(p) => p.state(),
            Self::Synth(p) => p.state(),
        }
    }

    pub(crate) fn get_current_frame(&self) -> usize {
        match self {
            Self::Ym { player, .. } => player.get_current_frame(),
            Self::Arkos(p) => p.current_frame(),
            Self::Ay(p) => p.current_frame(),
            Self::Synth(p) => p.current_frame(),
        }
    }

    pub(crate) fn samples_per_frame_value(&self) -> u32 {
        match self {
            Self::Ym { player, .. } => player.samples_per_frame_value(),
            Self::Arkos(p) => p.samples_per_frame(),
            Self::Ay(p) => p.samples_per_frame(),
            Self::Synth(p) => p.samples_per_frame_value(),
        }
    }

    pub(crate) fn generate_sample(&mut self) -> f32 {
        match self {
            Self::Ym { player, .. } => player.generate_sample(),
            Self::Arkos(p) => p.generate_sample(),
            Self::Ay(p) => p.generate_sample(),
            Self::Synth(p) => p.generate_sample(),
        }
    }

    pub(crate) fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        match self {
            Self::Ym { player, .. } => player.generate_samples_into(buffer),
            Self::Arkos(p) => p.generate_samples_into(buffer),
            Self::Ay(p) => p.generate_samples_into(buffer),
            Self::Synth(p) => p.generate_samples_into(buffer),
        }
    }

    pub(crate) fn metadata(&self) -> &Ym2149Metadata {
        match self {
            Self::Ym { metadata, .. } => metadata,
            Self::Arkos(p) => p.metadata(),
            Self::Ay(p) => p.metadata(),
            Self::Synth(p) => p.metadata(),
        }
    }

    pub(crate) fn metrics(&self) -> Option<PlaybackMetrics> {
        match self {
            Self::Ym { metrics, .. } => Some(*metrics),
            Self::Arkos(p) => p.metrics(),
            Self::Ay(p) => Some(p.metrics()),
            Self::Synth(p) => Some(p.metrics()),
        }
    }

    pub(crate) fn chip(&self) -> Option<&ym2149::ym2149::Ym2149> {
        match self {
            Self::Ym { player, .. } => Some(player.get_chip()),
            Self::Arkos(p) => p.primary_chip(),
            Self::Ay(p) => Some(p.chip()),
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
            Self::Synth(p) => p.chip(),
        }
    }

    /// Total frame count if known (falls back to metrics/estimates)
    pub fn frame_count(&self) -> usize {
        match self {
            Self::Ym { metrics, .. } => metrics.frame_count,
            Self::Arkos(p) => p.frame_count(),
            Self::Ay(p) => p.frame_count(),
            Self::Synth(p) => p.metrics().frame_count,
        }
    }
}

/// Load a song (YM or AKS) from raw bytes.
pub(crate) fn load_song_from_bytes(
    data: &[u8],
) -> std::result::Result<(YmSongPlayer, PlaybackMetrics, Ym2149Metadata), String> {
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

fn metadata_from_player(player: &Ym6Player, summary: &LoadSummary) -> Ym2149Metadata {
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
    metadata: Ym2149Metadata,
    samples_per_frame: u32,
    estimated_frames: usize,
    cache: Vec<f32>,
    cache_pos: usize,
    cache_len: usize,
}

impl ArkosBevyPlayer {
    fn new(player: ArkosPlayer, mut metadata: Ym2149Metadata) -> Self {
        let samples_per_frame = (YM2149_SAMPLE_RATE_F32 / player.replay_frequency_hz())
            .round()
            .max(1.0) as u32;
        let estimated_frames = player.estimated_total_ticks().max(1);
        metadata.frame_count = estimated_frames;
        metadata.duration_seconds =
            (estimated_frames as f32 * samples_per_frame as f32) / YM2149_SAMPLE_RATE_F32;

        Self {
            player,
            metadata,
            samples_per_frame,
            estimated_frames,
            cache: vec![0.0; 1024],
            cache_pos: 0,
            cache_len: 0,
        }
    }

    pub(crate) fn play(&mut self) -> Result<(), BevyYm2149Error> {
        self.player
            .play()
            .map_err(|e| BevyYm2149Error::Other(format!("AKS play failed: {e}")))
    }

    pub(crate) fn pause(&mut self) -> Result<(), BevyYm2149Error> {
        self.player
            .pause()
            .map_err(|e| BevyYm2149Error::Other(format!("AKS pause failed: {e}")))
    }

    pub(crate) fn stop(&mut self) -> Result<(), BevyYm2149Error> {
        self.player
            .stop()
            .map_err(|e| BevyYm2149Error::Other(format!("AKS stop failed: {e}")))
    }

    pub(crate) fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        if self.player.is_playing() {
            ym2149_ym_replayer::PlaybackState::Playing
        } else {
            ym2149_ym_replayer::PlaybackState::Stopped
        }
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
        self.player.generate_samples_into(buffer);
    }

    fn refill_cache(&mut self) {
        if self.cache.is_empty() {
            self.cache.resize(1024, 0.0);
        }
        self.player.generate_samples_into(&mut self.cache);
        self.cache_len = self.cache.len();
        self.cache_pos = 0;
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

    fn play(&mut self) -> Result<(), BevyYm2149Error> {
        if self.unsupported {
            return Err(BevyYm2149Error::Other(CPC_UNSUPPORTED_MSG.to_string()));
        }
        self.player
            .play()
            .map_err(|e| BevyYm2149Error::Other(format!("AY play failed: {e}")))?;
        self.ensure_supported()
    }

    fn pause(&mut self) -> Result<(), BevyYm2149Error> {
        self.player.pause();
        Ok(())
    }

    fn stop(&mut self) -> Result<(), BevyYm2149Error> {
        self.player
            .stop()
            .map_err(|e| BevyYm2149Error::Other(format!("AY stop failed: {e}")))
    }

    fn state(&self) -> ym2149_ym_replayer::PlaybackState {
        match self.player.playback_state() {
            AyState::Playing => ym2149_ym_replayer::PlaybackState::Playing,
            AyState::Paused => ym2149_ym_replayer::PlaybackState::Paused,
            AyState::Stopped => ym2149_ym_replayer::PlaybackState::Stopped,
        }
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
        self.player.generate_samples_into(buffer);
        if self.check_and_mark_unsupported() {
            buffer.fill(0.0);
        }
    }

    fn fill_cache(&mut self) {
        self.player
            .generate_samples_into(&mut self.cache[..AY_CACHE_SAMPLES]);
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

    fn ensure_supported(&mut self) -> Result<(), BevyYm2149Error> {
        if self.check_and_mark_unsupported() {
            Err(BevyYm2149Error::Other(CPC_UNSUPPORTED_MSG.to_string()))
        } else {
            Ok(())
        }
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
