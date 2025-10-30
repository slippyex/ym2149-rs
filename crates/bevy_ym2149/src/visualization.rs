//! Visualization components for YM2149 playback

use crate::playback::{PlaybackState, Ym2149Playback};
use bevy::prelude::*;

const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Extract period value from YM2149 registers (2 bytes per channel)
fn get_channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | (lo as u16);
    if period == 0 {
        None
    } else {
        Some(period)
    }
}

/// Convert YM2149 period to frequency in Hz
fn period_to_frequency(period: u16) -> f32 {
    PSG_MASTER_CLOCK_HZ / (16.0 * period as f32)
}

/// Convert frequency to musical note (e.g., "C4", "A#5")
fn frequency_to_note(freq: f32) -> Option<String> {
    if !freq.is_finite() || freq <= 0.0 {
        return None;
    }
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    let midi_rounded = midi.round();
    if !(0.0..=127.0).contains(&midi_rounded) {
        return None;
    }
    let midi_int = midi_rounded as i32;
    let note_index = ((midi_int % 12) + 12) % 12;
    let octave = (midi_int / 12) - 1;
    Some(format!("{}{}", NOTE_NAMES[note_index as usize], octave))
}

/// Component for displaying playback status
#[derive(Component)]
pub struct PlaybackStatusDisplay;

/// Component for displaying detailed channel information (like CLI output)
#[derive(Component)]
pub struct DetailedChannelDisplay;

/// Component for displaying song information
#[derive(Component)]
pub struct SongInfoDisplay;

/// Component for displaying channel information
#[derive(Component)]
pub struct ChannelVisualization {
    pub channel_index: usize,
    pub output_level: f32,
}

/// Component for the channel bar visualization fill
#[derive(Component)]
pub struct ChannelBar {
    pub channel_index: usize,
}

/// Component for oscilloscope visualization
#[derive(Component)]
pub struct Oscilloscope;

/// Resource to store recent audio samples for oscilloscope
#[derive(Resource, Clone)]
pub struct OscilloscopeBuffer {
    samples: Vec<f32>,
    capacity: usize,
    index: usize,
}

impl OscilloscopeBuffer {
    /// Create a new oscilloscope buffer with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![0.0; capacity],
            capacity,
            index: 0,
        }
    }

    /// Add a sample to the buffer
    pub fn push_sample(&mut self, sample: f32) {
        self.samples[self.index] = sample.clamp(-1.0, 1.0);
        self.index = (self.index + 1) % self.capacity;
    }

    /// Get the current samples in order
    pub fn get_samples(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.capacity);
        for i in 0..self.capacity {
            let idx = (self.index + i) % self.capacity;
            result.push(self.samples[idx]);
        }
        result
    }
}

/// System to update song info display
pub fn update_song_info(
    playbacks: Query<&Ym2149Playback>,
    mut song_display: Query<&mut Text, With<SongInfoDisplay>>,
) {
    if let Some(playback) = playbacks.iter().next() {
        for mut text in song_display.iter_mut() {
            let song_text = if playback.song_title.is_empty() {
                "(loading...)".to_string()
            } else {
                format!(
                    "{}\nArtist: {}",
                    playback.song_title.trim(),
                    if playback.song_author.is_empty() {
                        "(unknown)".to_string()
                    } else {
                        playback.song_author.trim().to_string()
                    }
                )
            };
            text.0 = song_text;
        }
    }
}

/// System to update playback status display
pub fn update_status_display(
    playbacks: Query<&Ym2149Playback>,
    mut status_display: Query<&mut Text, With<PlaybackStatusDisplay>>,
) {
    if let Some(playback) = playbacks.iter().next() {
        for mut text in status_display.iter_mut() {
            let state_str = match playback.state {
                PlaybackState::Idle => "Idle",
                PlaybackState::Playing => "[>]",
                PlaybackState::Paused => "[||]",
                PlaybackState::Finished => "[END]",
            };

            let frame_pos = playback.frame_position;
            let volume_percent = (playback.volume * 100.0) as u32;
            let buffer_fill = if let Some(device) = &playback.audio_device {
                (device.buffer_fill_level() * 100.0) as u32
            } else {
                0
            };

            let status_text = format!(
                "Status: {}\n\
                 Frame: {}\n\
                 Volume: {}%\n\
                 Buffer: {}%",
                state_str, frame_pos, volume_percent, buffer_fill
            );

            text.0 = status_text;
        }
    }
}

/// System to update channel output levels
pub fn update_channel_levels(
    playbacks: Query<&Ym2149Playback>,
    mut channel_visualizations: Query<&mut ChannelVisualization>,
) {
    if let Some(playback) = playbacks.iter().next() {
        if let Some(player) = &playback.player {
            let player_locked = player.lock();
            let chip = player_locked.get_chip();
            let (ch1, ch2, ch3) = chip.get_channel_outputs();

            for mut channel_viz in channel_visualizations.iter_mut() {
                let level = match channel_viz.channel_index {
                    0 => ch1.abs() * 2.5,
                    1 => ch2.abs() * 2.5,
                    2 => ch3.abs() * 2.5,
                    _ => 0.0,
                };
                channel_viz.output_level = level;
            }
        }
    }
}

/// System to update detailed channel information display
pub fn update_detailed_channel_display(
    playbacks: Query<&Ym2149Playback>,
    mut channel_display: Query<&mut Text, With<DetailedChannelDisplay>>,
) {
    if let Some(playback) = playbacks.iter().next() {
        if let Some(player) = &playback.player {
            let player_locked = player.lock();
            let chip = player_locked.get_chip();
            let regs = chip.dump_registers();
            let mixer_r7 = regs[7];

            let amp_a = regs[8] & 0x0F;
            let amp_b = regs[9] & 0x0F;
            let amp_c = regs[10] & 0x0F;

            let tone_a = (mixer_r7 & 0x01) == 0;
            let tone_b = (mixer_r7 & 0x02) == 0;
            let tone_c = (mixer_r7 & 0x04) == 0;

            let noise_a = (mixer_r7 & 0x08) == 0;
            let noise_b = (mixer_r7 & 0x10) == 0;
            let noise_c = (mixer_r7 & 0x20) == 0;

            let env_a = (regs[8] & 0x10) != 0;
            let env_b = (regs[9] & 0x10) != 0;
            let env_c = (regs[10] & 0x10) != 0;

            // Extract period and calculate frequency/note for each channel
            let period_a = get_channel_period(regs[0], regs[1]);
            let period_b = get_channel_period(regs[2], regs[3]);
            let period_c = get_channel_period(regs[4], regs[5]);

            let (freq_str_a, note_a) = if let Some(period) = period_a {
                let freq = period_to_frequency(period);
                let note = frequency_to_note(freq).unwrap_or_default();
                (format!("{:7.1}Hz", freq), note)
            } else {
                ("  --  Hz".to_string(), "--".to_string())
            };

            let (freq_str_b, note_b) = if let Some(period) = period_b {
                let freq = period_to_frequency(period);
                let note = frequency_to_note(freq).unwrap_or_default();
                (format!("{:7.1}Hz", freq), note)
            } else {
                ("  --  Hz".to_string(), "--".to_string())
            };

            let (freq_str_c, note_c) = if let Some(period) = period_c {
                let freq = period_to_frequency(period);
                let note = frequency_to_note(freq).unwrap_or_default();
                (format!("{:7.1}Hz", freq), note)
            } else {
                ("  --  Hz".to_string(), "--".to_string())
            };

            // Update detailed display
            for mut text in channel_display.iter_mut() {
                let detailed_text = format!(
                    "A: T:{} N:{} A:{:2} E:{}                        {:<4} {:<3}\n\
                     B: T:{} N:{} A:{:2} E:{}                        {:<4} {:<3}\n\
                     C: T:{} N:{} A:{:2} E:{}                        {:<4} {:<3}",
                    if tone_a { "[x]" } else { "[ ]" },
                    if noise_a { "[x]" } else { "[ ]" },
                    amp_a,
                    if env_a { "[x]" } else { "[ ]" },
                    note_a,
                    freq_str_a,
                    if tone_b { "[x]" } else { "[ ]" },
                    if noise_b { "[x]" } else { "[ ]" },
                    amp_b,
                    if env_b { "[x]" } else { "[ ]" },
                    note_b,
                    freq_str_b,
                    if tone_c { "[x]" } else { "[ ]" },
                    if noise_c { "[x]" } else { "[ ]" },
                    amp_c,
                    if env_c { "[x]" } else { "[ ]" },
                    note_c,
                    freq_str_c,
                );
                text.0 = detailed_text;
            }
        }
    }
}

/// System to update channel visualization bar widths based on output levels
pub fn update_channel_bars(
    mut channel_bars: Query<(&mut Node, &ChannelBar)>,
    channel_visualizations: Query<&ChannelVisualization>,
) {
    for (mut node, bar) in channel_bars.iter_mut() {
        if let Some(channel_viz) = channel_visualizations
            .iter()
            .find(|c| c.channel_index == bar.channel_index)
        {
            // Scale the bar width based on output level (0.0 to 1.0)
            // Clamp to reasonable range and normalize
            let normalized_level = channel_viz.output_level.clamp(0.0, 1.0);
            let bar_width = normalized_level * 110.0; // Max 110px out of 120px container
            node.width = Val::Px(bar_width);
        }
    }
}

/// Create a top panel container with song info and status side-by-side
pub fn create_status_display(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                right: Val::Px(10.0),
                height: Val::Auto,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
        ))
        .with_children(|parent| {
            // Song info display (left side)
            parent.spawn((
                Text::new("Song: (loading...)\nArtist: (unknown)"),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                SongInfoDisplay,
            ));

            // Status display (right side)
            parent.spawn((
                Text::new("Status: Idle\nFrame: 0\nVolume: 100%\nBuffer: 0%"),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                PlaybackStatusDisplay,
            ));
        })
        .id()
}

/// Create song information display (deprecated - now part of status display)
pub fn create_song_info_display(_commands: &mut Commands) -> Entity {
    // This function is kept for backwards compatibility but is no longer needed
    Entity::PLACEHOLDER
}

/// Create detailed channel information display
pub fn create_detailed_channel_display(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Text::new(
                "A: T:[ ] N:[ ] A: 0 E:[ ]                        --   --  Hz\n\
                 B: T:[ ] N:[ ] A: 0 E:[ ]                        --   --  Hz\n\
                 C: T:[ ] N:[ ] A: 0 E:[ ]                        --   --  Hz",
            ),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(150.0),
                left: Val::Px(10.0),
                width: Val::Auto,
                ..default()
            },
            DetailedChannelDisplay,
        ))
        .id()
}

/// Create a frequency spectrum analyzer display
pub fn create_oscilloscope(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(280.0),
                right: Val::Px(10.0),
                width: Val::Px(340.0),
                height: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)), // Dark background
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("~ Oscilloscope ~"),
                Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
            ));

            // Waveform display canvas
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(100.0),
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.0, 0.05, 0.0)), // Dark green background
                    Oscilloscope,
                ))
                .with_children(|canvas| {
                    // Create 256 waveform sample points
                    for _ in 0..256 {
                        canvas.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(1.0),
                                height: Val::Px(1.0),
                                left: Val::Px(0.0),
                                top: Val::Px(50.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.0, 1.0, 0.2)), // Bright oscilloscope green
                        ));
                    }
                });
        })
        .id()
}

/// System to update oscilloscope display with actual waveform data
pub fn update_oscilloscope(
    oscilloscope_buffer: Res<OscilloscopeBuffer>,
    oscilloscope_displays: Query<&Children, With<Oscilloscope>>,
    mut nodes: Query<&mut Node>,
) {
    // Get the samples from the buffer
    let samples = oscilloscope_buffer.get_samples();

    if !samples.is_empty() {
        // Update waveform display
        for oscilloscope_canvas in oscilloscope_displays.iter() {
            let children: Vec<_> = oscilloscope_canvas.iter().collect();

            for (point_index, &child) in children.iter().enumerate() {
                if point_index < samples.len() {
                    // Map sample value (-1 to 1) to display position
                    let sample_value = samples[point_index].clamp(-1.0, 1.0);

                    // Calculate x position (spread across width)
                    let x_pos = (point_index as f32 / samples.len() as f32) * 324.0; // 340 - 16px padding

                    // Calculate y position (inverted so positive = up)
                    // Sample -1.0 -> top (0px), 0.0 -> middle (50px), 1.0 -> bottom (100px)
                    let y_pos = 50.0 - (sample_value * 50.0);

                    // Update node position
                    if let Ok(mut node) = nodes.get_mut(child) {
                        node.left = Val::Px(x_pos);
                        node.top = Val::Px(y_pos);
                    }
                }
            }
        }
    }
}

/// Create channel visualization bars with proper flexbox layout
pub fn create_channel_visualization(commands: &mut Commands, num_channels: usize) -> Vec<Entity> {
    // Create main container for all channels
    let container_id = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Px(10.0),
                right: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
        ))
        .id();

    let mut channel_ids = Vec::new();

    for i in 0..num_channels {
        let channel_row_id = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
            ))
            .with_children(|parent| {
                // Channel label (fixed width)
                parent.spawn((
                    Text::new(format!("{}", ['A', 'B', 'C'][i])),
                    Node {
                        width: Val::Px(20.0),
                        ..default()
                    },
                ));

                // Bar container (flex to fill available space, but capped)
                parent
                    .spawn((
                        Node {
                            width: Val::Px(120.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
                    ))
                    .with_children(|parent| {
                        // Animated fill bar
                        parent.spawn((
                            Node {
                                height: Val::Percent(100.0),
                                width: Val::Px(0.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.0, 1.0, 0.4)),
                            ChannelBar { channel_index: i },
                        ));
                    });

                // Visualization component for tracking
                parent.spawn(ChannelVisualization {
                    channel_index: i,
                    output_level: 0.0,
                });
            })
            .id();

        channel_ids.push(channel_row_id);
        commands.entity(container_id).add_child(channel_row_id);
    }

    channel_ids
}
