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
use ym_replayer::{load_song, PlaybackController, PlaybackState};

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

/// Main YM2149 player for WebAssembly
///
/// This player handles YM file playback in the browser, generating audio samples
/// that can be fed into the Web Audio API.
#[wasm_bindgen]
pub struct Ym2149Player {
    player: ym_replayer::Ym6Player,
    metadata: YmMetadata,
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
        console_log!("Loading YM file ({} bytes)...", data.len());

        let (player, summary) = load_song(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load YM file: {}", e)))?;

        console_log!("YM file loaded successfully");
        console_log!("  Format: {:?}", summary.format);
        console_log!("  Frames: {}", summary.frame_count);

        // Extract metadata from player info
        let (title, author, comments, frame_rate) = if let Some(info) = player.info() {
            (
                info.song_name.clone(),
                info.author.clone(),
                info.comment.clone(),
                info.frame_rate as u32,
            )
        } else {
            ("Unknown".to_string(), "Unknown".to_string(), String::new(), 50u32)
        };

        console_log!("  Title: {}", title);
        console_log!("  Author: {}", author);

        let metadata = YmMetadata {
            title,
            author,
            comments,
            format: format!("{:?}", summary.format),
            frame_count: summary.frame_count as u32,
            frame_rate,
            duration_seconds: player.get_duration_seconds(),
        };

        Ok(Ym2149Player { player, metadata })
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
            .map_err(|e| JsValue::from_str(&format!("Failed to stop: {}", e)))?;
        self.player
            .play()
            .map_err(|e| JsValue::from_str(&format!("Failed to play: {}", e)))
    }

    /// Get current playback state
    pub fn is_playing(&self) -> bool {
        self.player.state() == PlaybackState::Playing
    }

    /// Get current playback state as string
    pub fn state(&self) -> String {
        format!("{:?}", self.player.state())
    }

    /// Set volume (0.0 to 1.0)
    /// Note: Volume control is done in JavaScript via Web Audio API gain node
    pub fn set_volume(&mut self, _volume: f32) {
        // Volume is typically handled by Web Audio API gain nodes
        // If needed, this could scale the generated samples
    }

    /// Get current volume
    /// Note: Always returns 1.0 as volume is handled in JavaScript
    pub fn volume(&self) -> f32 {
        1.0
    }

    /// Get current frame position
    pub fn frame_position(&self) -> u32 {
        self.player.get_current_frame() as u32
    }

    /// Get total frame count
    pub fn frame_count(&self) -> u32 {
        self.player.frame_count() as u32
    }

    /// Get playback position as percentage (0.0 to 1.0)
    pub fn position_percentage(&self) -> f32 {
        self.player.get_playback_position()
    }

    /// Seek to a specific frame
    /// Note: Seeking is implemented by stopping and restarting playback
    pub fn seek_to_frame(&mut self, _frame: u32) {
        // TODO: Implement proper seeking when player API supports it
        // For now, seeking is not supported in WASM
    }

    /// Seek to a percentage of the song (0.0 to 1.0)
    /// Note: Seeking is implemented by stopping and restarting playback
    pub fn seek_to_percentage(&mut self, _percentage: f32) {
        // TODO: Implement proper seeking when player API supports it
        // For now, seeking is not supported in WASM
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
        self.player.generate_samples(count)
    }

    /// Generate samples into a pre-allocated buffer (zero-allocation)
    ///
    /// This is more efficient than `generate_samples` as it reuses the same buffer.
    #[wasm_bindgen(js_name = generateSamplesInto)]
    pub fn generate_samples_into(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into(buffer);
    }

    /// Get the current register values (for visualization)
    pub fn get_registers(&self) -> Vec<u8> {
        self.player.get_chip().dump_registers().to_vec()
    }

    /// Enable or disable the ST color filter
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.get_chip_mut().set_color_filter(enabled);
    }
}

// Re-export for wasm-pack
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}
