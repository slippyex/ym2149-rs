//! WebAssembly bindings for YM2149 PSG emulator
//!
//! This crate provides WebAssembly bindings for playing YM2149 chiptune files
//! directly in web browsers using the Web Audio API.
//!
//! # Features
//!
//! - Load and play YM2-YM6 format files
//! - Playback control (play, pause, stop, seek)
//! - Volume control
//! - Metadata extraction (title, author, comments)
//! - Channel muting/solo
//! - Real-time waveform data for visualization
//!
//! # Example Usage (JavaScript)
//!
//! ```javascript
//! import init, { Ym2149Player } from './ym2149_wasm.js';
//!
//! async function playYmFile(fileData) {
//!     await init();
//!
//!     const player = Ym2149Player.new(fileData);
//!     const metadata = player.get_metadata();
//!     console.log(`Playing: ${metadata.title} by ${metadata.author}`);
//!
//!     player.play();
//! }
//! ```

#![warn(missing_docs)]

use wasm_bindgen::prelude::*;
use ym2149_arkos_replayer::{ArkosPlayer, load_aks};
use ym2149_ay_replayer::{AyMetadata as AyFileMetadata, AyPlaybackState as AyState, AyPlayer};
use ym2149_ym_replayer::{LoadSummary, PlaybackController, PlaybackState, load_song};

const YM_SAMPLE_RATE_F32: f32 = 44_100.0;

/// Set panic hook for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Log to browser console
macro_rules! console_log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into());
    }
}

/// YM file metadata exposed to JavaScript
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct YmMetadata {
    title: String,
    author: String,
    comments: String,
    format: String,
    frame_count: u32,
    frame_rate: u32,
    duration_seconds: f32,
}

#[wasm_bindgen]
impl YmMetadata {
    /// Get the song title
    #[wasm_bindgen(getter)]
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// Get the song author
    #[wasm_bindgen(getter)]
    pub fn author(&self) -> String {
        self.author.clone()
    }

    /// Get the song comments
    #[wasm_bindgen(getter)]
    pub fn comments(&self) -> String {
        self.comments.clone()
    }

    /// Get the YM format version
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> String {
        self.format.clone()
    }

    /// Get frame count
    #[wasm_bindgen(getter)]
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Get frame rate in Hz
    #[wasm_bindgen(getter)]
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Get duration in seconds
    #[wasm_bindgen(getter)]
    pub fn duration_seconds(&self) -> f32 {
        self.duration_seconds
    }
}

enum BrowserSongPlayer {
    Ym(Box<ym2149_ym_replayer::Ym6Player>),
    Arkos(Box<ArkosWasmPlayer>),
    Ay(Box<AyWasmPlayer>),
}

impl BrowserSongPlayer {
    fn seek_frame(&mut self, frame: usize) -> bool {
        match self {
            BrowserSongPlayer::Ym(player) => {
                player.seek_frame(frame);
                true
            }
            BrowserSongPlayer::Arkos(_) => false,
            BrowserSongPlayer::Ay(_) => false,
        }
    }
}

struct ArkosWasmPlayer {
    player: ArkosPlayer,
    estimated_frames: usize,
}

impl ArkosWasmPlayer {
    fn new(player: ArkosPlayer) -> (Self, YmMetadata) {
        let samples_per_frame = (YM_SAMPLE_RATE_F32 / player.replay_frequency_hz())
            .round()
            .max(1.0) as u32;
        let estimated_frames = player.estimated_total_ticks().max(1);
        let duration_seconds =
            (estimated_frames as f32 * samples_per_frame as f32) / YM_SAMPLE_RATE_F32;
        let info = player.metadata().clone();
        let frame_rate = player.replay_frequency_hz().round().max(1.0) as u32;

        let metadata = YmMetadata {
            title: info.title,
            author: if info.author.is_empty() {
                info.composer
            } else {
                info.author
            },
            comments: info.comments,
            format: "AKS".to_string(),
            frame_count: estimated_frames as u32,
            frame_rate,
            duration_seconds,
        };

        (
            Self {
                player,
                estimated_frames,
            },
            metadata,
        )
    }

    fn play(&mut self) -> Result<(), String> {
        self.player
            .play()
            .map_err(|e| format!("AKS play failed: {e}"))
    }

    fn pause(&mut self) -> Result<(), String> {
        self.player
            .pause()
            .map_err(|e| format!("AKS pause failed: {e}"))
    }

    fn stop(&mut self) -> Result<(), String> {
        self.player
            .stop()
            .map_err(|e| format!("AKS stop failed: {e}"))
    }

    fn state(&self) -> PlaybackState {
        if self.player.is_playing() {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        }
    }

    fn frame_position(&self) -> usize {
        self.player.current_tick_index()
    }

    fn frame_count(&self) -> usize {
        self.estimated_frames
    }

    fn playback_position(&self) -> f32 {
        if self.estimated_frames == 0 {
            0.0
        } else {
            self.player.current_tick_index() as f32 / self.estimated_frames as f32
        }
    }

    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        self.player.generate_samples(count)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    fn dump_registers(&self) -> [u8; 16] {
        self.player
            .chip(0)
            .map(|chip| chip.dump_registers())
            .unwrap_or([0; 16])
    }

    fn set_color_filter(&mut self, enabled: bool) {
        if let Some(chip) = self.player.chip_mut(0) {
            chip.set_color_filter(enabled);
        }
    }
}

struct AyWasmPlayer {
    player: AyPlayer,
    frame_count: usize,
}

impl AyWasmPlayer {
    fn new(player: AyPlayer, meta: &AyFileMetadata) -> (Self, YmMetadata) {
        let metadata = metadata_from_ay(meta);
        let frame_count = metadata.frame_count as usize;
        (
            Self {
                player,
                frame_count,
            },
            metadata,
        )
    }

    fn play(&mut self) -> Result<(), String> {
        self.player.play().map_err(|e| e.to_string())
    }

    fn pause(&mut self) -> Result<(), String> {
        self.player.pause();
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.player.stop().map_err(|e| e.to_string())
    }

    fn state(&self) -> PlaybackState {
        match self.player.state() {
            AyState::Playing => PlaybackState::Playing,
            AyState::Paused => PlaybackState::Paused,
            AyState::Stopped => PlaybackState::Stopped,
        }
    }

    fn frame_position(&self) -> usize {
        self.player.current_frame()
    }

    fn frame_count(&self) -> usize {
        self.frame_count
    }

    fn playback_position(&self) -> f32 {
        self.player.playback_position()
    }

    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        self.player.generate_samples(count)
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    fn dump_registers(&self) -> [u8; 16] {
        self.player.chip().dump_registers()
    }

    fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }
}

impl BrowserSongPlayer {
    fn play(&mut self) -> Result<(), String> {
        match self {
            BrowserSongPlayer::Ym(player) => player.play().map_err(|e| e.to_string()),
            BrowserSongPlayer::Arkos(player) => player.play(),
            BrowserSongPlayer::Ay(player) => player.play(),
        }
    }

    fn pause(&mut self) -> Result<(), String> {
        match self {
            BrowserSongPlayer::Ym(player) => player.pause().map_err(|e| e.to_string()),
            BrowserSongPlayer::Arkos(player) => player.pause(),
            BrowserSongPlayer::Ay(player) => player.pause(),
        }
    }

    fn stop(&mut self) -> Result<(), String> {
        match self {
            BrowserSongPlayer::Ym(player) => player.stop().map_err(|e| e.to_string()),
            BrowserSongPlayer::Arkos(player) => player.stop(),
            BrowserSongPlayer::Ay(player) => player.stop(),
        }
    }

    fn state(&self) -> PlaybackState {
        match self {
            BrowserSongPlayer::Ym(player) => player.state(),
            BrowserSongPlayer::Arkos(player) => player.state(),
            BrowserSongPlayer::Ay(player) => player.state(),
        }
    }

    fn frame_position(&self) -> usize {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_current_frame(),
            BrowserSongPlayer::Arkos(player) => player.frame_position(),
            BrowserSongPlayer::Ay(player) => player.frame_position(),
        }
    }

    fn frame_count(&self) -> usize {
        match self {
            BrowserSongPlayer::Ym(player) => player.frame_count(),
            BrowserSongPlayer::Arkos(player) => player.frame_count(),
            BrowserSongPlayer::Ay(player) => player.frame_count(),
        }
    }

    fn playback_position(&self) -> f32 {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_playback_position(),
            BrowserSongPlayer::Arkos(player) => player.playback_position(),
            BrowserSongPlayer::Ay(player) => player.playback_position(),
        }
    }

    fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        match self {
            BrowserSongPlayer::Ym(player) => player.generate_samples(count),
            BrowserSongPlayer::Arkos(player) => player.generate_samples(count),
            BrowserSongPlayer::Ay(player) => player.generate_samples(count),
        }
    }

    fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        match self {
            BrowserSongPlayer::Ym(player) => player.generate_samples_into(buffer),
            BrowserSongPlayer::Arkos(player) => player.generate_samples_into(buffer),
            BrowserSongPlayer::Ay(player) => player.generate_samples_into(buffer),
        }
    }

    fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        match self {
            BrowserSongPlayer::Ym(player) => player.set_channel_mute(channel, mute),
            BrowserSongPlayer::Arkos(player) => player.set_channel_mute(channel, mute),
            BrowserSongPlayer::Ay(player) => player.set_channel_mute(channel, mute),
        }
    }

    fn is_channel_muted(&self, channel: usize) -> bool {
        match self {
            BrowserSongPlayer::Ym(player) => player.is_channel_muted(channel),
            BrowserSongPlayer::Arkos(player) => player.is_channel_muted(channel),
            BrowserSongPlayer::Ay(player) => player.is_channel_muted(channel),
        }
    }

    fn dump_registers(&self) -> [u8; 16] {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_chip().dump_registers(),
            BrowserSongPlayer::Arkos(player) => player.dump_registers(),
            BrowserSongPlayer::Ay(player) => player.dump_registers(),
        }
    }

    fn set_color_filter(&mut self, enabled: bool) {
        match self {
            BrowserSongPlayer::Ym(player) => player.get_chip_mut().set_color_filter(enabled),
            BrowserSongPlayer::Arkos(player) => player.set_color_filter(enabled),
            BrowserSongPlayer::Ay(player) => player.set_color_filter(enabled),
        }
    }
}

/// Main YM2149 player for WebAssembly
///
/// This player handles YM file playback in the browser, generating audio samples
/// that can be fed into the Web Audio API.
#[wasm_bindgen]
pub struct Ym2149Player {
    player: BrowserSongPlayer,
    metadata: YmMetadata,
    volume: f32,
}

#[wasm_bindgen]
impl Ym2149Player {
    /// Create a new player from YM file data
    ///
    /// # Arguments
    ///
    /// * `data` - YM file data as Uint8Array
    ///
    /// # Returns
    ///
    /// Result containing the player or an error message
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<Ym2149Player, JsValue> {
        console_log!("Loading file ({} bytes)...", data.len());

        let (player, metadata) = load_browser_player(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load YM/AKS/AY file: {}", e)))?;

        console_log!("Song loaded successfully");
        console_log!("  Title: {}", metadata.title);
        console_log!("  Format: {}", metadata.format);

        Ok(Ym2149Player {
            player,
            metadata,
            volume: 1.0,
        })
    }

    /// Get metadata about the loaded YM file
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> YmMetadata {
        self.metadata.clone()
    }

    /// Start playback
    pub fn play(&mut self) -> Result<(), JsValue> {
        self.player
            .play()
            .map_err(|e| JsValue::from_str(&format!("Failed to play: {}", e)))
    }

    /// Pause playback
    pub fn pause(&mut self) {
        let _ = self.player.pause();
    }

    /// Stop playback and reset to beginning
    pub fn stop(&mut self) -> Result<(), JsValue> {
        self.player
            .stop()
            .map_err(|e| JsValue::from_str(&format!("Failed to stop: {}", e)))
    }

    /// Restart playback from the beginning
    pub fn restart(&mut self) -> Result<(), JsValue> {
        self.player
            .stop()
            .and_then(|_| self.player.play())
            .map_err(|e| JsValue::from_str(&format!("Failed to restart: {}", e)))
    }

    /// Get current playback state
    pub fn is_playing(&self) -> bool {
        self.player.state() == PlaybackState::Playing
    }

    /// Get current playback state as string
    pub fn state(&self) -> String {
        format!("{:?}", self.player.state())
    }

    /// Set volume (0.0 to 1.0). Applied to generated samples.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Get current volume (0.0 to 1.0)
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Get current frame position
    pub fn frame_position(&self) -> u32 {
        self.player.frame_position() as u32
    }

    /// Get total frame count
    pub fn frame_count(&self) -> u32 {
        self.player.frame_count() as u32
    }

    /// Get playback position as percentage (0.0 to 1.0)
    pub fn position_percentage(&self) -> f32 {
        self.player.playback_position()
    }

    /// Seek to a specific frame (silently ignored for Arkos backend).
    pub fn seek_to_frame(&mut self, frame: u32) {
        let _ = self.player.seek_frame(frame as usize);
    }

    /// Seek to a percentage of the song (0.0 to 1.0, silently ignored for Arkos backend).
    pub fn seek_to_percentage(&mut self, percentage: f32) {
        let total_frames = self.player.frame_count().max(1);
        let clamped = percentage.clamp(0.0, 1.0);
        let target = ((total_frames as f32 - 1.0) * clamped).round() as usize;
        let _ = self.player.seek_frame(target);
    }

    /// Mute or unmute a channel (0-2)
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    /// Generate audio samples
    ///
    /// Returns a Float32Array containing interleaved stereo samples.
    /// The number of samples generated depends on the sample rate and frame rate.
    ///
    /// For 44.1kHz at 50Hz frame rate: 882 samples per frame
    #[wasm_bindgen(js_name = generateSamples)]
    pub fn generate_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = self.player.generate_samples(count);
        if self.volume != 1.0 {
            for sample in &mut samples {
                *sample *= self.volume;
            }
        }
        samples
    }

    /// Generate samples into a pre-allocated buffer (zero-allocation)
    ///
    /// This is more efficient than `generate_samples` as it reuses the same buffer.
    #[wasm_bindgen(js_name = generateSamplesInto)]
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
        if self.volume != 1.0 {
            for sample in buffer.iter_mut() {
                *sample *= self.volume;
            }
        }
    }

    /// Get the current register values (for visualization)
    pub fn get_registers(&self) -> Vec<u8> {
        self.player.dump_registers().to_vec()
    }

    /// Enable or disable the ST color filter
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }
}

fn load_browser_player(data: &[u8]) -> Result<(BrowserSongPlayer, YmMetadata), String> {
    if let Ok((player, summary)) = load_song(data) {
        let metadata = metadata_from_summary(&player, &summary);
        return Ok((BrowserSongPlayer::Ym(Box::new(player)), metadata));
    }

    if let Ok(song) = load_aks(data) {
        let arkos_player =
            ArkosPlayer::new(song, 0).map_err(|e| format!("Failed to init Arkos player: {e}"))?;
        let (wrapper, metadata) = ArkosWasmPlayer::new(arkos_player);
        return Ok((BrowserSongPlayer::Arkos(Box::new(wrapper)), metadata));
    }

    let (player, meta) =
        AyPlayer::load_from_bytes(data, 0).map_err(|e| format!("Failed to load AY file: {e}"))?;
    let (wrapper, metadata) = AyWasmPlayer::new(player, &meta);
    Ok((BrowserSongPlayer::Ay(Box::new(wrapper)), metadata))
}

fn metadata_from_summary(
    player: &ym2149_ym_replayer::Ym6Player,
    summary: &LoadSummary,
) -> YmMetadata {
    let (title, author, comments, frame_rate) = if let Some(info) = player.info() {
        (
            info.song_name.clone(),
            info.author.clone(),
            info.comment.clone(),
            info.frame_rate as u32,
        )
    } else {
        (
            "Unknown".to_string(),
            "Unknown".to_string(),
            String::new(),
            50u32,
        )
    };

    YmMetadata {
        title,
        author,
        comments,
        format: format!("{:?}", summary.format),
        frame_count: summary.frame_count as u32,
        frame_rate,
        duration_seconds: player.get_duration_seconds(),
    }
}

fn metadata_from_ay(meta: &AyFileMetadata) -> YmMetadata {
    let frame_count = meta.frame_count.unwrap_or(0);
    let duration_seconds = meta
        .duration_seconds
        .unwrap_or_else(|| frame_count as f32 / 50.0);

    YmMetadata {
        title: meta.song_name.clone(),
        author: meta.author.clone(),
        comments: meta.misc.clone(),
        format: "AY".to_string(),
        frame_count: frame_count as u32,
        frame_rate: 50,
        duration_seconds,
    }
}

// Re-export for wasm-pack
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}
