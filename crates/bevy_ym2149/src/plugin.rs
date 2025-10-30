//! Bevy plugin for YM2149 audio playback
use bevy::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::audio_sink;
use crate::audio_source::Ym2149Loader;
use crate::playback::{PlaybackState, Ym2149Playback, Ym2149Settings};
use crate::visualization;
use ym2149::replayer::PlaybackController;

/// Plugin for YM2149 audio playback in Bevy
pub struct Ym2149Plugin;

impl Plugin for Ym2149Plugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(Ym2149Loader)
            .insert_resource(Ym2149Settings::default())
            .insert_resource(visualization::OscilloscopeBuffer::new(256))
            .add_systems(
                Update,
                (
                    handle_file_drop,
                    initialize_playback,
                    update_playback,
                    visualization::update_song_info,
                    visualization::update_status_display,
                    visualization::update_channel_levels,
                    visualization::update_detailed_channel_display,
                    visualization::update_channel_bars,
                    visualization::update_oscilloscope,
                )
                    .chain(),
            );
    }
}

/// Handle file drop events to load and play dropped YM files
fn handle_file_drop(
    mut drop_events: MessageReader<FileDragAndDrop>,
    mut playbacks: Query<&mut Ym2149Playback>,
) {
    for event in drop_events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let path_str = path_buf.to_string_lossy().to_string();

            // Only handle files ending in .ym (case-insensitive)
            if path_str.to_lowercase().ends_with(".ym") {
                // Update the first playback entity with the dropped file
                if let Some(mut playback) = playbacks.iter_mut().next() {
                    playback.source_path = path_str.clone();
                    playback.restart();
                    playback.play();
                    info!("Loaded YM file from drag-and-drop: {}", path_str);
                }
            } else {
                warn!("Dropped file is not a YM file: {}", path_str);
            }
        }
    }
}

/// Initialize playback when a playback component is first created or needs reloading
fn initialize_playback(mut playbacks: Query<&mut Ym2149Playback>) {
    for mut playback in playbacks.iter_mut() {
        // Initialize only if player doesn't exist OR if reload is needed
        let should_load = playback.player.is_none() || playback.needs_reload;

        if should_load {
            playback.needs_reload = false;
            match std::fs::read(&playback.source_path) {
                Ok(data) => {
                    match ym2149::load_song(&data) {
                        Ok((mut player, summary)) => {
                            // Extract song metadata
                            let info_str = player.format_info();
                            // Parse title and author from info string if available
                            // Format is typically "Song Name\nAuthor: Author Name\n..."
                            let lines: Vec<&str> = info_str.lines().collect();
                            playback.song_title = lines.first().unwrap_or(&"").to_string();
                            playback.song_author = if lines.len() > 1 {
                                lines[1].to_string()
                            } else {
                                String::new()
                            };

                            // Start the player if we're in Playing state
                            if playback.state == PlaybackState::Playing {
                                if let Err(e) = player.play() {
                                    error!("Failed to start player: {}", e);
                                }
                                info!("Player started!");
                            }

                            playback.player = Some(Arc::new(Mutex::new(player)));

                            // Create audio device with standard settings (44.1kHz stereo)
                            // Uses RodioAudioSink by default (trait-based, user can provide custom sink)
                            match audio_sink::rodio::RodioAudioSink::new(44_100, 2) {
                                Ok(sink) => {
                                    if let Err(e) = sink.start() {
                                        error!("Failed to start audio device: {}", e);
                                    } else {
                                        info!("Audio device started successfully!");
                                    }
                                    // Wrap the sink in Arc<dyn AudioSink> for trait-based usage
                                    playback.audio_device = Some(Arc::new(sink));
                                }
                                Err(e) => {
                                    error!("Failed to create audio device: {}", e);
                                }
                            }

                            info!(
                                "Loaded YM song: {} frames, {} samples/frame",
                                summary.frame_count, summary.samples_per_frame
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to initialize YM2149 player for '{}': {}",
                                playback.source_path, e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read YM file '{}': {}", playback.source_path, e);
                }
            }
        }
    }
}

/// Update playback state and advance frames based on real elapsed time
fn update_playback(
    mut playbacks: Query<&mut Ym2149Playback>,
    settings: Res<Ym2149Settings>,
    time: Res<Time>,
    mut oscilloscope_buffer: ResMut<visualization::OscilloscopeBuffer>,
    mut frame_count: Local<u32>,
    mut prev_state: Local<Option<PlaybackState>>,
    mut time_since_last_frame: Local<f32>,
) {
    let delta = time.delta_secs();

    for mut playback in playbacks.iter_mut() {
        // Track state transitions
        let state_changed = prev_state.as_ref() != Some(&playback.state);
        *prev_state = Some(playback.state);

        if let Some(player) = &playback.player.clone() {
            let mut player_locked = player.lock();

            // Handle state transitions
            if state_changed {
                match playback.state {
                    PlaybackState::Playing => {
                        info!("Transitioning to Playing state");
                        if let Err(e) = player_locked.play() {
                            error!("Failed to resume player: {}", e);
                        }
                        if let Some(device) = &playback.audio_device {
                            device.resume();
                        }
                        *time_since_last_frame = 0.0;
                    }
                    PlaybackState::Paused => {
                        info!("Transitioning to Paused state");
                        if let Err(e) = player_locked.pause() {
                            error!("Failed to pause player: {}", e);
                        }
                        if let Some(device) = &playback.audio_device {
                            device.pause();
                        }
                    }
                    _ => {}
                }
            }

            match playback.state {
                PlaybackState::Playing => {
                    // Accumulate time since last frame generation
                    *time_since_last_frame += delta;

                    // Calculate frame duration based on YM file's frame rate
                    // samples_per_frame = 44100 / frame_rate, so frame_duration = samples_per_frame / 44100
                    let samples_per_frame = player_locked.samples_per_frame_value() as usize;
                    let frame_duration = samples_per_frame as f32 / 44100.0;

                    // Only generate a frame when enough real time has passed
                    if *time_since_last_frame >= frame_duration {
                        let mut samples = player_locked.generate_samples(samples_per_frame);

                        *frame_count += 1;
                        let should_log = (*frame_count).is_multiple_of(60);

                        if should_log {
                            debug!(
                                "Frame {}: Generated {} samples, time_since_last: {:.4}s",
                                *frame_count,
                                samples.len(),
                                *time_since_last_frame
                            );
                        }

                        // Apply volume to samples
                        if playback.volume != 1.0 {
                            for sample in &mut samples {
                                *sample *= playback.volume;
                            }
                        }

                        // Push samples to oscilloscope buffer for visualization
                        for sample in &samples {
                            oscilloscope_buffer.push_sample(*sample);
                        }

                        // Push samples to audio device if available
                        if let Some(device) = &playback.audio_device {
                            // Generate stereo samples (duplicate mono for both channels)
                            let mut stereo_samples = Vec::with_capacity(samples.len() * 2);
                            for sample in &samples {
                                stereo_samples.push(*sample);
                                stereo_samples.push(*sample);
                            }

                            if let Err(e) = device.push_samples(stereo_samples.clone()) {
                                warn!("Failed to push samples to audio device: {}", e);
                            } else if should_log {
                                let fill = device.buffer_fill_level();
                                debug!(
                                    "Pushed {} stereo samples, buffer fill: {:.1}%",
                                    stereo_samples.len(),
                                    fill * 100.0
                                );
                            }
                        }

                        // Subtract the frame duration we just consumed
                        *time_since_last_frame -= frame_duration;
                    }

                    // Get current position after generation
                    let current_frame = player_locked.get_current_frame();
                    let frame_count_total = player_locked.frame_count();
                    playback.frame_position = current_frame as u32;

                    // Check if we've reached the end
                    if current_frame >= frame_count_total {
                        if settings.loop_enabled {
                            // Reset to beginning
                            let _ = player_locked.play();
                            playback.frame_position = 0;
                            *time_since_last_frame = 0.0;
                            info!("Song looped!");
                        } else {
                            playback.state = PlaybackState::Finished;
                            info!("Playback finished");
                        }
                    }
                }
                PlaybackState::Paused | PlaybackState::Idle | PlaybackState::Finished => {
                    // No playback - just update position display
                    playback.frame_position = player_locked.get_current_frame() as u32;
                }
            }
        }
    }
}
