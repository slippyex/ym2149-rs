//! WebAssembly bindings for YM2149 PSG emulator.
//!
//! This crate provides WebAssembly bindings for playing YM2149 chiptune files
//! directly in web browsers using the Web Audio API.
//!
//! # Features
//!
//! - Load and play YM2-YM6 format files
//! - Load and play Arkos Tracker (.aks) files
//! - Load and play AY format files
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
//!
//! # Module Organization
//!
//! Internal modules handle:
//!
//! - Metadata types and conversion functions
//! - Player wrappers for different file formats

#![warn(missing_docs)]

mod metadata;
mod players;

use wasm_bindgen::prelude::*;
use ym2149_arkos_replayer::{ArkosPlayer, load_aks};
use ym2149_ay_replayer::{AyPlayer, CPC_UNSUPPORTED_MSG};
use ym2149_sndh_replayer::is_sndh_data;
use ym2149_ym_replayer::{PlaybackState, load_song};

use metadata::{YmMetadata, metadata_from_summary};
use players::{BrowserSongPlayer, arkos::ArkosWasmPlayer, ay::AyWasmPlayer, sndh::SndhWasmPlayer};

/// Sample rate used for audio generation.
pub const YM_SAMPLE_RATE_F32: f32 = 44_100.0;

/// Set panic hook for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Log to browser console.
macro_rules! console_log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into());
    }
}

/// Main YM2149 player for WebAssembly.
///
/// This player handles YM/AKS/AY file playback in the browser, generating audio samples
/// that can be fed into the Web Audio API.
#[wasm_bindgen]
pub struct Ym2149Player {
    player: BrowserSongPlayer,
    metadata: YmMetadata,
    volume: f32,
}

#[wasm_bindgen]
impl Ym2149Player {
    /// Create a new player from file data.
    ///
    /// Automatically detects the file format (YM, AKS, AY, or SNDH).
    ///
    /// # Arguments
    ///
    /// * `data` - File data as Uint8Array
    ///
    /// # Returns
    ///
    /// Result containing the player or an error message.
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

    /// Get metadata about the loaded file.
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> YmMetadata {
        self.metadata.clone()
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.player.play();
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.player.pause();
    }

    /// Stop playback and reset to beginning.
    pub fn stop(&mut self) {
        self.player.stop();
    }

    /// Restart playback from the beginning.
    pub fn restart(&mut self) {
        self.player.stop();
        self.player.play();
    }

    /// Get current playback state.
    pub fn is_playing(&self) -> bool {
        self.player.state() == PlaybackState::Playing
    }

    /// Get current playback state as string.
    pub fn state(&self) -> String {
        format!("{:?}", self.player.state())
    }

    /// Set volume (0.0 to 1.0). Applied to generated samples.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Get current volume (0.0 to 1.0).
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Get current frame position.
    pub fn frame_position(&self) -> u32 {
        self.player.frame_position() as u32
    }

    /// Get total frame count.
    pub fn frame_count(&self) -> u32 {
        self.player.frame_count() as u32
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn position_percentage(&self) -> f32 {
        self.player.playback_position()
    }

    /// Seek to a specific frame (silently ignored for Arkos/AY backends).
    pub fn seek_to_frame(&mut self, frame: u32) {
        let _ = self.player.seek_frame(frame as usize);
    }

    /// Seek to a percentage of the song (0.0 to 1.0, silently ignored for Arkos/AY backends).
    pub fn seek_to_percentage(&mut self, percentage: f32) {
        let total_frames = self.player.frame_count().max(1);
        let clamped = percentage.clamp(0.0, 1.0);
        let target = ((total_frames as f32 - 1.0) * clamped).round() as usize;
        let _ = self.player.seek_frame(target);
    }

    /// Mute or unmute a channel (0-2).
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted.
    pub fn is_channel_muted(&self, channel: usize) -> bool {
        self.player.is_channel_muted(channel)
    }

    /// Generate audio samples.
    ///
    /// Returns a Float32Array containing mono samples.
    /// The number of samples generated depends on the sample rate and frame rate.
    ///
    /// For 44.1kHz at 50Hz frame rate: 882 samples per frame.
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

    /// Generate samples into a pre-allocated buffer (zero-allocation).
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

    /// Get the current register values (for visualization).
    pub fn get_registers(&self) -> Vec<u8> {
        self.player.dump_registers().to_vec()
    }

    /// Get channel states for visualization (frequency, amplitude, note, effects).
    ///
    /// Returns a JsValue containing an object with channel data:
    /// ```json
    /// {
    ///   "channels": [
    ///     { "frequency": 440.0, "note": "A4", "amplitude": 0.8, "toneEnabled": true, "noiseEnabled": false, "envelopeEnabled": false },
    ///     ...
    ///   ],
    ///   "envelope": { "period": 256, "shape": 14, "shapeName": "/\\/\\" }
    /// }
    /// ```
    #[wasm_bindgen(js_name = getChannelStates)]
    pub fn get_channel_states(&self) -> JsValue {
        use ym2149::ChannelStates;

        let regs = self.player.dump_registers();
        let states = ChannelStates::from_registers(&regs);

        // Build JavaScript-friendly object
        let obj = js_sys::Object::new();

        // Channels array
        let channels = js_sys::Array::new();
        for ch in &states.channels {
            let ch_obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &ch_obj,
                &"frequency".into(),
                &ch.frequency_hz.unwrap_or(0.0).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &ch_obj,
                &"note".into(),
                &ch.note_name.unwrap_or("--").into(),
            )
            .ok();
            js_sys::Reflect::set(
                &ch_obj,
                &"amplitude".into(),
                &ch.amplitude_normalized.into(),
            )
            .ok();
            js_sys::Reflect::set(&ch_obj, &"toneEnabled".into(), &ch.tone_enabled.into()).ok();
            js_sys::Reflect::set(&ch_obj, &"noiseEnabled".into(), &ch.noise_enabled.into()).ok();
            js_sys::Reflect::set(
                &ch_obj,
                &"envelopeEnabled".into(),
                &ch.envelope_enabled.into(),
            )
            .ok();
            channels.push(&ch_obj);
        }
        js_sys::Reflect::set(&obj, &"channels".into(), &channels).ok();

        // Envelope info
        let env_obj = js_sys::Object::new();
        js_sys::Reflect::set(&env_obj, &"period".into(), &states.envelope.period.into()).ok();
        js_sys::Reflect::set(&env_obj, &"shape".into(), &states.envelope.shape.into()).ok();
        js_sys::Reflect::set(
            &env_obj,
            &"shapeName".into(),
            &states.envelope.shape_name.into(),
        )
        .ok();
        js_sys::Reflect::set(&obj, &"envelope".into(), &env_obj).ok();

        obj.into()
    }

    /// Enable or disable the ST color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }
}

/// Load a file and create the appropriate player.
fn load_browser_player(data: &[u8]) -> Result<(BrowserSongPlayer, YmMetadata), String> {
    // SNDH needs to be detected first to avoid falling back to AY/other formats
    // when the header already looks like a packed SNDH.
    if is_sndh_data(data) {
        let (wrapper, metadata) = SndhWasmPlayer::new(data)?;
        return Ok((BrowserSongPlayer::Sndh(Box::new(wrapper)), metadata));
    }

    // Try YM format first
    if let Ok((player, summary)) = load_song(data) {
        let metadata = metadata_from_summary(&player, &summary);
        return Ok((BrowserSongPlayer::Ym(Box::new(player)), metadata));
    }

    // Try Arkos format
    if let Ok(song) = load_aks(data) {
        let arkos_player =
            ArkosPlayer::new(song, 0).map_err(|e| format!("Failed to init Arkos player: {e}"))?;
        let (wrapper, metadata) = ArkosWasmPlayer::new(arkos_player);
        return Ok((BrowserSongPlayer::Arkos(Box::new(wrapper)), metadata));
    }

    // Try SNDH format (Atari ST) even if the heuristic didn't match
    if let Ok((wrapper, metadata)) = SndhWasmPlayer::new(data) {
        return Ok((BrowserSongPlayer::Sndh(Box::new(wrapper)), metadata));
    }

    // Try AY format
    let (player, meta) =
        AyPlayer::load_from_bytes(data, 0).map_err(|e| format!("Failed to load AY file: {e}"))?;
    if player.requires_cpc_firmware() {
        return Err(CPC_UNSUPPORTED_MSG.to_string());
    }
    let (wrapper, metadata) = AyWasmPlayer::new(player, &meta);
    Ok((BrowserSongPlayer::Ay(Box::new(wrapper)), metadata))
}

// Re-export for wasm-pack
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}
