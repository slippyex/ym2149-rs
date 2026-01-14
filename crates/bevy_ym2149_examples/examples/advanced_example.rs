//! Advanced YM2149 playback example with visualization
//!
//! This example demonstrates advanced features of the bevy_ym2149 plugin including:
//! - Real-time visualization (oscilloscope, channel display, spectrum analysis)
//! - File drag-and-drop loading
//! - Keyboard-based playback control

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_ym2149::{
    PatternTrigger, PatternTriggerSet, PatternTriggered, PlaybackState, Ym2149Playback,
    Ym2149Plugin, Ym2149Settings,
};
use bevy_ym2149_examples::{embedded_asset_plugin, example_plugins_with_window};
use bevy_ym2149_viz::{
    ProgressBarContainer, Ym2149VizPlugin, create_channel_visualization,
    create_detailed_channel_display, create_oscilloscope, create_status_display,
};

fn main() {
    App::new()
        .add_plugins(embedded_asset_plugin())
        .add_plugins(example_plugins_with_window(Window {
            title: "YM2149 goes Bevy".into(),
            ..default()
        }))
        .add_plugins(Ym2149Plugin::default())
        .add_plugins(Ym2149VizPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_file_drop,
                playback_controls,
                progress_bar_seek,
                log_pattern_hits,
            ),
        )
        .run();
}

/// Set up the initial scene with a YM2149 playback entity and visualization
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn a camera for the window
    commands.spawn(Camera2d);

    // Create title/instructions display (positioned below top panel)
    commands.spawn((
        Text::new(
            "Controls:\n\
             - Drag & Drop: Load YM/AKS/AY/SNDH file\n\
             - Click Progress Bar: Seek to position\n\
             - SPACE: Play/Pause\n\
             - R: Restart\n\
             - L: Toggle Looping\n\
             - S: Toggle Saturation\n\
             - A: Toggle Accent Boost\n\
             - B: Toggle Stereo Widen\n\
             - C: Toggle Color Filter\n\
             - LEFT/RIGHT: Previous/Next Subsong (AKS/SNDH)\n\
             - UP/DOWN: Volume Control\n\
             - Pattern hits are logged to the console",
        ),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.88, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(130.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    // Create a playback entity
    // Load the default song via Bevy's asset system
    let asset_handle = asset_server.load("music/ND-Toxygene.ym");
    let playback = Ym2149Playback::from_asset(asset_handle);
    let triggers = PatternTriggerSet::from_patterns(vec![
        PatternTrigger::new("Channel A Accent", 0)
            .with_min_amplitude(0.35)
            .with_cooldown(4),
        PatternTrigger::new("Lead A4", 1)
            .with_min_amplitude(0.2)
            .with_frequency(440.0, 12.0)
            .with_cooldown(6),
    ]);
    commands.spawn((playback, triggers));

    // Create detailed channel information display
    create_detailed_channel_display(&mut commands);

    // Create oscilloscope display
    create_oscilloscope(&mut commands);

    // Create channel visualizations (3 channels for YM2149)
    let _channels = create_channel_visualization(&mut commands, 3);

    // Create top panel with song info and status display
    create_status_display(&mut commands);
}

/// Handle keyboard input for playback control
fn playback_controls(
    mut playbacks: Query<&mut Ym2149Playback>,
    mut settings: ResMut<Ym2149Settings>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if let Some(mut playback) = playbacks.iter_mut().next() {
        // Play/Pause toggle on spacebar
        if keyboard.just_pressed(KeyCode::Space) {
            // Resume from pause without resetting; otherwise start from beginning.
            // If a song ended, restart from frame 0.
            let target_state = playback.state;
            if target_state == PlaybackState::Paused {
                playback.play(); // do not reset frame
            } else if target_state == PlaybackState::Playing {
                playback.pause();
            } else {
                playback.restart();
                playback.play();
            }
        }

        // Restart on 'R'
        if keyboard.just_pressed(KeyCode::KeyR) {
            playback.restart();
            playback.play();
        }

        // Toggle looping on 'L'
        if keyboard.just_pressed(KeyCode::KeyL) {
            settings.loop_enabled = !settings.loop_enabled;
            info!(
                "Looping {}",
                if settings.loop_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }

        // Tone shaping toggles
        if keyboard.just_pressed(KeyCode::KeyS) {
            let mut tone = playback.tone_settings();
            tone.saturation = if tone.saturation > 0.0 { 0.0 } else { 0.15 };
            playback.set_tone_settings(tone);
            info!(
                "Soft saturation {}",
                if tone.saturation > 0.0 {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        if keyboard.just_pressed(KeyCode::KeyA) {
            let mut tone = playback.tone_settings();
            tone.accent = if tone.accent > 0.0 { 0.0 } else { 0.25 };
            playback.set_tone_settings(tone);
            info!(
                "Accent boost {}",
                if tone.accent > 0.0 {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        if keyboard.just_pressed(KeyCode::KeyB) {
            let mut tone = playback.tone_settings();
            tone.widen = if tone.widen > 0.0 { 0.0 } else { 0.15 };
            playback.set_tone_settings(tone);
            info!(
                "Stereo widen {}",
                if tone.widen > 0.0 {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        if keyboard.just_pressed(KeyCode::KeyC) {
            let mut tone = playback.tone_settings();
            tone.color_filter = !tone.color_filter;
            playback.set_tone_settings(tone);
            info!(
                "Color filter {}",
                if tone.color_filter {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }

        // Volume control with arrow keys
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            let new_volume = (playback.volume + 0.1).min(1.0);
            playback.set_volume(new_volume);
            info!("Volume: {:.0}%", new_volume * 100.0);
        }

        if keyboard.just_pressed(KeyCode::ArrowDown) {
            let new_volume = (playback.volume - 0.1).max(0.0);
            playback.set_volume(new_volume);
            info!("Volume: {:.0}%", new_volume * 100.0);
        }

        // Subsong navigation with left/right arrow keys (AKS/SNDH files)
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            let count = playback.subsong_count();
            if count > 1 {
                if let Some(new_subsong) = playback.next_subsong() {
                    info!("Switched to subsong {}/{}", new_subsong, count);
                }
            } else {
                info!("No subsongs available (count={})", count);
            }
        }

        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            let count = playback.subsong_count();
            if count > 1 {
                if let Some(new_subsong) = playback.prev_subsong() {
                    info!("Switched to subsong {}/{}", new_subsong, count);
                }
            } else {
                info!("No subsongs available (count={})", count);
            }
        }
    }
}

fn handle_file_drop(
    mut drop_events: MessageReader<FileDragAndDrop>,
    mut playbacks: Query<&mut Ym2149Playback>,
) {
    for event in drop_events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let path_str = path_buf.to_string_lossy().to_string();
            let lower = path_str.to_lowercase();
            let is_ym = lower.ends_with(".ym");
            let is_aks = lower.ends_with(".aks");
            let is_ay = lower.ends_with(".ay");
            let is_sndh = lower.ends_with(".sndh");
            if !(is_ym || is_aks || is_ay || is_sndh) {
                warn!("Dropped file is not a YM/AKS/AY/SNDH file: {}", path_str);
                continue;
            }

            if let Some(mut playback) = playbacks.iter_mut().next() {
                playback.set_source_path(path_str.clone());
                playback.play();
                if is_ay {
                    info!(
                        "Loaded AY file (ZX-only; CPC AY tracks will report unsupported firmware): {}",
                        path_str
                    );
                } else if is_sndh {
                    info!(
                        "Loaded SNDH file (Atari ST native 68000 code): {}",
                        path_str
                    );
                } else {
                    info!("Loaded song from drag-and-drop: {}", path_str);
                }
            }
        }
    }
}

fn log_pattern_hits(mut hits: MessageReader<PatternTriggered>) {
    for hit in hits.read() {
        info!(
            "Pattern '{}' hit on channel {} (amp {:.2}, freq {:?})",
            hit.pattern_id, hit.channel, hit.amplitude, hit.frequency
        );
    }
}

/// Handle clicks on the progress bar to seek to the clicked position.
fn progress_bar_seek(
    mut playbacks: Query<&mut Ym2149Playback>,
    progress_bars: Query<
        (&Interaction, &GlobalTransform, &ComputedNode),
        (With<ProgressBarContainer>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    for (interaction, transform, computed) in &progress_bars {
        if *interaction != Interaction::Pressed {
            continue;
        }

        // Get the bar's position and size in screen coordinates
        let bar_pos = transform.translation().truncate();
        let bar_size = computed.size();

        // Calculate local X position within the bar (Bevy UI uses center origin)
        let local_x = cursor_pos.x - (bar_pos.x - bar_size.x / 2.0);
        let percentage = (local_x / bar_size.x).clamp(0.0, 1.0);

        // Seek all playback entities to the clicked position
        for mut playback in &mut playbacks {
            if playback.seek_percentage(percentage) {
                info!("Seeked to {:.1}%", percentage * 100.0);
            }
        }
    }
}
