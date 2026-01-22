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
use ym2149_common::DEFAULT_SAMPLE_RATE;

/// Sample rate used for audio generation.
pub const YM_SAMPLE_RATE_F32: f32 = DEFAULT_SAMPLE_RATE as f32;

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

        let (player, metadata) = load_browser_player(data).map_err(|e| {
            JsValue::from_str(&format!(
                "Failed to load chiptune file ({} bytes): {}",
                data.len(),
                e
            ))
        })?;

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

    /// Get the number of times the song has looped.
    pub fn loop_count(&self) -> u32 {
        self.player.loop_count()
    }

    /// Get playback position as percentage (0.0 to 1.0).
    pub fn position_percentage(&self) -> f32 {
        self.player.playback_position()
    }

    /// Seek to a specific frame (silently ignored for Arkos/AY backends).
    pub fn seek_to_frame(&mut self, frame: u32) {
        let _ = self.player.seek_frame(frame as usize);
    }

    /// Seek to a percentage of the song (0.0 to 1.0).
    ///
    /// Returns true if seek succeeded. Works for all SNDH files (uses fallback duration for older files).
    pub fn seek_to_percentage(&mut self, percentage: f32) -> bool {
        self.player.seek_percentage(percentage)
    }

    /// Get duration in seconds.
    ///
    /// For SNDH < 2.2 without FRMS/TIME, returns 300 (5 minute fallback).
    pub fn duration_seconds(&self) -> f32 {
        self.player.duration_seconds()
    }

    /// Check if the duration is from actual metadata or estimated.
    ///
    /// Returns false for older SNDH files using the 5-minute fallback.
    #[wasm_bindgen(js_name = hasDurationInfo)]
    pub fn has_duration_info(&self) -> bool {
        self.player.has_duration_info()
    }

    /// Mute or unmute a channel (0-2 for YM2149, 3-4 for STE DAC L/R).
    #[wasm_bindgen(js_name = setChannelMute)]
    pub fn set_channel_mute(&mut self, channel: usize, mute: bool) {
        self.player.set_channel_mute(channel, mute);
    }

    /// Check if a channel is muted.
    #[wasm_bindgen(js_name = isChannelMuted)]
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

    /// Generate stereo audio samples (interleaved L/R).
    ///
    /// Returns frame_count * 2 samples. SNDH uses native stereo output,
    /// other formats duplicate mono to stereo.
    #[wasm_bindgen(js_name = generateSamplesStereo)]
    pub fn generate_samples_stereo(&mut self, frame_count: usize) -> Vec<f32> {
        let mut samples = self.player.generate_samples_stereo(frame_count);
        if self.volume != 1.0 {
            for sample in &mut samples {
                *sample *= self.volume;
            }
        }
        samples
    }

    /// Generate stereo samples into a pre-allocated buffer (zero-allocation).
    ///
    /// Buffer length must be even (frame_count * 2). Interleaved L/R format.
    /// SNDH uses native stereo output, other formats duplicate mono to stereo.
    #[wasm_bindgen(js_name = generateSamplesIntoStereo)]
    pub fn generate_samples_into_stereo(&mut self, buffer: &mut [f32]) {
        self.player.generate_samples_into_stereo(buffer);
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
    /// Returns a JsValue containing an object with channel data for all PSG chips:
    /// ```json
    /// {
    ///   "channels": [
    ///     { "frequency": 440.0, "note": "A4", "amplitude": 0.8, "toneEnabled": true, "noiseEnabled": false, "envelopeEnabled": false },
    ///     ...
    ///   ],
    ///   "envelopes": [
    ///     { "period": 256, "shape": 14, "shapeName": "/\\/\\" },
    ///     ...
    ///   ]
    /// }
    /// ```
    #[wasm_bindgen(js_name = getChannelStates)]
    pub fn get_channel_states(&self) -> JsValue {
        use ym2149_common::ChannelStates;

        let all_regs = self.player.dump_all_registers();

        // Build JavaScript-friendly object
        let obj = js_sys::Object::new();

        // Channels array (all channels from all PSGs)
        let channels = js_sys::Array::new();
        // Envelopes array (one per PSG)
        let envelopes = js_sys::Array::new();

        for regs in &all_regs {
            let states = ChannelStates::from_registers(regs);

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

            // Envelope info for this PSG
            let env_obj = js_sys::Object::new();
            js_sys::Reflect::set(&env_obj, &"period".into(), &states.envelope.period.into()).ok();
            js_sys::Reflect::set(&env_obj, &"shape".into(), &states.envelope.shape.into()).ok();
            js_sys::Reflect::set(
                &env_obj,
                &"shapeName".into(),
                &states.envelope.shape_name.into(),
            )
            .ok();
            envelopes.push(&env_obj);
        }

        // For SNDH with STE features, add DAC channels (L/R)
        if let BrowserSongPlayer::Sndh(sndh_player) = &self.player {
            if sndh_player.uses_ste_features() {
                let (dac_left, dac_right) = sndh_player.get_dac_levels();

                // DAC Left channel
                let dac_l_obj = js_sys::Object::new();
                js_sys::Reflect::set(&dac_l_obj, &"frequency".into(), &0.0f64.into()).ok();
                js_sys::Reflect::set(&dac_l_obj, &"note".into(), &"DAC".into()).ok();
                js_sys::Reflect::set(&dac_l_obj, &"amplitude".into(), &(dac_left as f64).into())
                    .ok();
                js_sys::Reflect::set(&dac_l_obj, &"toneEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_l_obj, &"noiseEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_l_obj, &"envelopeEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_l_obj, &"isDac".into(), &true.into()).ok();
                channels.push(&dac_l_obj);

                // DAC Right channel
                let dac_r_obj = js_sys::Object::new();
                js_sys::Reflect::set(&dac_r_obj, &"frequency".into(), &0.0f64.into()).ok();
                js_sys::Reflect::set(&dac_r_obj, &"note".into(), &"DAC".into()).ok();
                js_sys::Reflect::set(&dac_r_obj, &"amplitude".into(), &(dac_right as f64).into())
                    .ok();
                js_sys::Reflect::set(&dac_r_obj, &"toneEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_r_obj, &"noiseEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_r_obj, &"envelopeEnabled".into(), &false.into()).ok();
                js_sys::Reflect::set(&dac_r_obj, &"isDac".into(), &true.into()).ok();
                channels.push(&dac_r_obj);
            }
        }

        js_sys::Reflect::set(&obj, &"channels".into(), &channels).ok();
        js_sys::Reflect::set(&obj, &"envelopes".into(), &envelopes).ok();

        // For backwards compatibility, also include first envelope as "envelope"
        if let Some(first_env) = all_regs.first() {
            let states = ChannelStates::from_registers(first_env);
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
        }

        obj.into()
    }

    /// Get LMC1992 state for visualization (SNDH only).
    ///
    /// Returns a JsValue containing an object with LMC1992 state:
    /// ```json
    /// {
    ///   "masterVolume": 0,      // dB (-80 to 0)
    ///   "leftVolume": 0,        // dB (-40 to 0)
    ///   "rightVolume": 0,       // dB (-40 to 0)
    ///   "bass": 0,              // dB (-12 to +12)
    ///   "treble": 0             // dB (-12 to +12)
    /// }
    /// ```
    ///
    /// Returns null for non-SNDH formats.
    #[wasm_bindgen(js_name = getLmc1992State)]
    pub fn get_lmc1992_state(&self) -> JsValue {
        if let BrowserSongPlayer::Sndh(sndh_player) = &self.player {
            let obj = js_sys::Object::new();
            // dB values
            js_sys::Reflect::set(
                &obj,
                &"masterVolume".into(),
                &(sndh_player.lmc1992_master_volume_db() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"leftVolume".into(),
                &(sndh_player.lmc1992_left_volume_db() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"rightVolume".into(),
                &(sndh_player.lmc1992_right_volume_db() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"bass".into(),
                &(sndh_player.lmc1992_bass_db() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"treble".into(),
                &(sndh_player.lmc1992_treble_db() as i32).into(),
            )
            .ok();
            // Raw register values
            js_sys::Reflect::set(
                &obj,
                &"masterVolumeRaw".into(),
                &(sndh_player.lmc1992_master_volume_raw() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"leftVolumeRaw".into(),
                &(sndh_player.lmc1992_left_volume_raw() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"rightVolumeRaw".into(),
                &(sndh_player.lmc1992_right_volume_raw() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"bassRaw".into(),
                &(sndh_player.lmc1992_bass_raw() as i32).into(),
            )
            .ok();
            js_sys::Reflect::set(
                &obj,
                &"trebleRaw".into(),
                &(sndh_player.lmc1992_treble_raw() as i32).into(),
            )
            .ok();
            obj.into()
        } else {
            JsValue::NULL
        }
    }

    /// Get current per-channel audio outputs for oscilloscope visualization.
    ///
    /// Returns a flat array of channel outputs: [A0, B0, C0, A1, B1, C1, ...]
    /// where each group of 3 represents one PSG chip.
    ///
    /// These are the actual audio output values (updated at sample rate),
    /// not register values. Perfect for real-time oscilloscope display.
    #[wasm_bindgen(js_name = getChannelOutputs)]
    pub fn get_channel_outputs(&self) -> Vec<f32> {
        let outputs = self.player.get_channel_outputs();
        outputs.into_iter().flat_map(|[a, b, c]| [a, b, c]).collect()
    }

    /// Generate audio samples with per-sample channel outputs for visualization.
    ///
    /// Returns a JavaScript object containing:
    /// - `mono`: Float32Array of mono samples
    /// - `channels`: Float32Array of per-sample channel outputs
    ///
    /// The channels array is organized as [A0, B0, C0, A0, B0, C0, ...] for each sample,
    /// where each group of 3 (or more for multi-chip) represents one sample's channel outputs.
    ///
    /// This enables accurate per-sample oscilloscope visualization at the full audio sample rate.
    #[wasm_bindgen(js_name = generateSamplesWithChannels)]
    pub fn generate_samples_with_channels(&mut self, count: usize) -> JsValue {
        let (mut mono, channels) = self.player.generate_samples_with_channels(count);

        // Apply volume
        if self.volume != 1.0 {
            for sample in &mut mono {
                *sample *= self.volume;
            }
        }

        // Create JS object with both arrays
        let obj = js_sys::Object::new();
        let mono_arr = js_sys::Float32Array::from(&mono[..]);
        let channels_arr = js_sys::Float32Array::from(&channels[..]);

        js_sys::Reflect::set(&obj, &"mono".into(), &mono_arr).ok();
        js_sys::Reflect::set(&obj, &"channels".into(), &channels_arr).ok();
        js_sys::Reflect::set(&obj, &"channelCount".into(), &(self.player.channel_count() as u32).into()).ok();

        obj.into()
    }

    /// Enable or disable the ST color filter.
    pub fn set_color_filter(&mut self, enabled: bool) {
        self.player.set_color_filter(enabled);
    }

    /// Get the number of subsongs (1 for most formats, >1 for multi-song SNDH files).
    #[wasm_bindgen(js_name = subsongCount)]
    pub fn subsong_count(&self) -> usize {
        self.player.subsong_count()
    }

    /// Get the number of audio channels.
    ///
    /// Returns 3 for standard single-chip songs, 6 for dual-chip (some Arkos songs), etc.
    #[wasm_bindgen(js_name = channelCount)]
    pub fn channel_count(&self) -> usize {
        self.player.channel_count()
    }

    /// Get the current subsong index (1-based).
    #[wasm_bindgen(js_name = currentSubsong)]
    pub fn current_subsong(&self) -> usize {
        self.player.current_subsong()
    }

    /// Set the current subsong (1-based index). Returns true on success.
    #[wasm_bindgen(js_name = setSubsong)]
    pub fn set_subsong(&mut self, index: usize) -> bool {
        self.player.set_subsong(index)
    }
}

/// Load a file and create the appropriate player.
fn load_browser_player(data: &[u8]) -> Result<(BrowserSongPlayer, YmMetadata), String> {
    if data.is_empty() {
        return Err("empty file data".to_string());
    }

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
        let psg_count = song.subsongs.first().map(|s| s.psgs.len()).unwrap_or(0);
        console_log!("Arkos: loaded song with {} PSGs ({} channels)", psg_count, psg_count * 3);
        let arkos_player =
            ArkosPlayer::new(song, 0).map_err(|e| format!("Arkos player init failed: {e}"))?;
        let (wrapper, metadata) = ArkosWasmPlayer::new(arkos_player);
        return Ok((BrowserSongPlayer::Arkos(Box::new(wrapper)), metadata));
    }

    // Try SNDH format (Atari ST) even if the heuristic didn't match
    if let Ok((wrapper, metadata)) = SndhWasmPlayer::new(data) {
        return Ok((BrowserSongPlayer::Sndh(Box::new(wrapper)), metadata));
    }

    // Try AY format as last resort
    let (player, meta) = AyPlayer::load_from_bytes(data, 0)
        .map_err(|e| format!("unrecognized format (AY parse error: {e})"))?;
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
