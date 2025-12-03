//! Builder functions for creating YM2149 visualization UI widgets.
//!
//! Use these helpers to spawn pre-configured UI hierarchies for displaying
//! playback status, oscilloscope waveforms, and channel information.

use crate::components::*;
use crate::helpers::{format_freq_label, format_note_label};
use bevy::prelude::*;

// UI Layout Constants
const UI_PADDING: f32 = 10.0;
const UI_MARGIN_SMALL: f32 = 6.0;
const UI_MARGIN_MEDIUM: f32 = 12.0;
const UI_MARGIN_LARGE: f32 = 14.0;

// UI Colors
const PANEL_BG_DARK: Color = Color::srgba(0.0, 0.0, 0.0, 0.3);
const PANEL_BG_DARKER: Color = Color::srgba(0.01, 0.01, 0.02, 0.95);
const BADGE_PANEL_BG: Color = Color::srgba(0.05, 0.05, 0.07, 0.75);
const OSCILLOSCOPE_BG: Color = Color::srgb(0.02, 0.06, 0.1);
const BADGE_BG_DARK: Color = Color::srgba(0.1, 0.12, 0.18, 0.8);
const BADGE_BAR_BG: Color = Color::srgba(0.18, 0.2, 0.24, 0.6);
const GRID_COLOR: Color = Color::srgba(0.12, 0.18, 0.2, 0.4);
const GRID_COLOR_BRIGHT: Color = Color::srgba(0.12, 0.18, 0.2, 0.85);
const GRID_COLOR_MID: Color = Color::srgba(0.12, 0.18, 0.2, 0.6);
const GRID_COLOR_DIM: Color = Color::srgba(0.12, 0.18, 0.2, 0.32);
const CHANNEL_LABEL_COLOR: Color = Color::srgb(0.74, 0.82, 0.9);

/// Create a combined status bar with song info on the left and playback status on the right.
///
/// Returns the root entity of the status display.
pub fn create_status_display(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(UI_PADDING),
                left: Val::Px(UI_PADDING),
                right: Val::Px(UI_PADDING),
                height: Val::Auto,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(PANEL_BG_DARK),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Song: (loading...)\nArtist: (unknown)"),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                SongInfoDisplay,
            ));

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

/// Create a standalone song info display showing title and artist.
///
/// Returns the text entity.
pub fn create_song_info_display(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Text::new("Song: (loading...)\nArtist: (unknown)"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                left: Val::Px(10.0),
                ..default()
            },
            SongInfoDisplay,
        ))
        .id()
}

/// Create a multi-line text display showing detailed channel state (registers, volumes, etc).
///
/// Returns the text entity.
pub fn create_detailed_channel_display(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Text::new(""),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(160.0),
                left: Val::Px(10.0),
                width: Val::Auto,
                ..default()
            },
            DetailedChannelDisplay,
        ))
        .id()
}

/// Create an oscilloscope widget showing real-time waveforms for all three channels.
///
/// Includes a grid background, per-channel waveform layers, and amplitude badges.
/// Returns the root panel entity.
pub fn create_oscilloscope(commands: &mut Commands) -> Entity {
    const CHANNEL_COLOR_RGB: [Vec3; 3] = [
        Vec3::new(1.0, 0.4, 0.4),
        Vec3::new(0.35, 1.0, 0.45),
        Vec3::new(0.45, 0.65, 1.0),
    ];

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(280.0),
                right: Val::Px(UI_PADDING),
                width: Val::Px(324.0),
                height: Val::Px(220.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(UI_PADDING)),
                ..default()
            },
            BackgroundColor(PANEL_BG_DARKER),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("~ Oscilloscope ~"),
                Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(UI_MARGIN_SMALL)),
                    ..default()
                },
            ));

            let half_height = OSCILLOSCOPE_HEIGHT / 2.0;
            panel
                .spawn((Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(OSCILLOSCOPE_HEIGHT),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(UI_MARGIN_MEDIUM),
                    margin: UiRect::bottom(Val::Px(UI_MARGIN_LARGE)),
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Px(72.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(UI_MARGIN_MEDIUM),
                            padding: UiRect::axes(
                                Val::Px(UI_MARGIN_SMALL),
                                Val::Px(UI_MARGIN_SMALL),
                            ),
                            ..default()
                        },
                        BackgroundColor(BADGE_PANEL_BG),
                    ))
                    .with_children(|badges| {
                        for channel_index in 0..3 {
                            badges
                                .spawn((Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(UI_MARGIN_SMALL),
                                    ..default()
                                },))
                                .with_children(|column| {
                                    let label_char = char::from(b'A' + channel_index as u8);
                                    column.spawn((
                                        Text::new(format!("CH {}", label_char)),
                                        TextFont {
                                            font_size: 12.0,
                                            ..default()
                                        },
                                        TextColor(CHANNEL_LABEL_COLOR),
                                    ));

                                    column
                                        .spawn((Node {
                                            flex_direction: FlexDirection::Row,
                                            column_gap: Val::Px(UI_MARGIN_SMALL),
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },))
                                        .with_children(|row| {
                                            row.spawn((
                                                Node {
                                                    width: Val::Px(36.0),
                                                    height: Val::Px(6.0),
                                                    ..default()
                                                },
                                                BackgroundColor(BADGE_BAR_BG),
                                                ChannelBadge {
                                                    channel: channel_index,
                                                    kind: BadgeKind::Amplitude,
                                                },
                                            ));

                                            row.spawn((
                                                Node {
                                                    width: Val::Px(12.0),
                                                    height: Val::Px(12.0),
                                                    ..default()
                                                },
                                                BorderRadius::all(Val::Px(UI_MARGIN_SMALL)),
                                                BackgroundColor(BADGE_BG_DARK),
                                                ChannelBadge {
                                                    channel: channel_index,
                                                    kind: BadgeKind::HighFreq,
                                                },
                                            ));
                                        });
                                });
                        }
                    });

                    row.spawn((Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|scope_column| {
                            scope_column
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(OSCILLOSCOPE_HEIGHT),
                                        position_type: PositionType::Relative,
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BackgroundColor(OSCILLOSCOPE_BG),
                                    Oscilloscope,
                                ))
                                .with_children(|canvas| {
                                    canvas
                                        .spawn((Node {
                                            position_type: PositionType::Absolute,
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },))
                                        .with_children(|grid| {
                                            for i in 0..=4 {
                                                grid.spawn((
                                                    Node {
                                                        position_type: PositionType::Absolute,
                                                        width: Val::Percent(100.0),
                                                        height: Val::Px(if i == 2 {
                                                            2.0
                                                        } else {
                                                            1.0
                                                        }),
                                                        top: Val::Percent(i as f32 * 25.0),
                                                        ..default()
                                                    },
                                                    BackgroundColor(if i == 2 {
                                                        GRID_COLOR_BRIGHT
                                                    } else {
                                                        GRID_COLOR
                                                    }),
                                                    OscilloscopeGridLine,
                                                ));
                                            }
                                            for i in 0..=8 {
                                                grid.spawn((
                                                    Node {
                                                        position_type: PositionType::Absolute,
                                                        width: Val::Px(if i == 4 {
                                                            2.0
                                                        } else {
                                                            1.0
                                                        }),
                                                        height: Val::Percent(100.0),
                                                        left: Val::Percent(i as f32 * 12.5),
                                                        ..default()
                                                    },
                                                    BackgroundColor(if i == 4 {
                                                        GRID_COLOR_MID
                                                    } else {
                                                        GRID_COLOR_DIM
                                                    }),
                                                    OscilloscopeGridLine,
                                                ));
                                            }
                                        });

                                    for (channel_index, base) in
                                        CHANNEL_COLOR_RGB.iter().enumerate()
                                    {
                                        let base = *base;
                                        canvas
                                            .spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    width: Val::Percent(100.0),
                                                    height: Val::Percent(100.0),
                                                    ..default()
                                                },
                                                OscilloscopeChannel {
                                                    index: channel_index,
                                                    base_color: base,
                                                },
                                            ))
                                            .with_children(|layer| {
                                                for sample_index in 0..OSCILLOSCOPE_RESOLUTION {
                                                    layer.spawn((
                                                        Node {
                                                            position_type: PositionType::Absolute,
                                                            width: Val::Px(2.0),
                                                            height: Val::Px(2.0),
                                                            left: Val::Px(0.0),
                                                            top: Val::Px(half_height),
                                                            ..default()
                                                        },
                                                        BackgroundColor(Color::srgba(
                                                            base.x, base.y, base.z, 0.0,
                                                        )),
                                                        OscilloscopePoint {
                                                            channel: channel_index,
                                                            index: sample_index,
                                                        },
                                                    ));
                                                }

                                                layer.spawn((
                                                    Node {
                                                        position_type: PositionType::Absolute,
                                                        width: Val::Px(10.0),
                                                        height: Val::Px(10.0),
                                                        left: Val::Px(0.0),
                                                        top: Val::Px(half_height),
                                                        ..default()
                                                    },
                                                    BorderRadius::all(Val::Px(5.0)),
                                                    BackgroundColor(Color::srgba(
                                                        base.x, base.y, base.z, 0.0,
                                                    )),
                                                    OscilloscopeHead {
                                                        channel: channel_index,
                                                    },
                                                ));
                                            });
                                    }
                                });
                        });
                });
        })
        .id()
}

/// Create a channel visualization panel with progress bar, note labels, and spectrum bars.
///
/// Returns the entity IDs of the individual channel column containers.
pub fn create_channel_visualization(commands: &mut Commands, num_channels: usize) -> Vec<Entity> {
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

    commands.entity(container_id).with_children(|parent| {
        parent
            .spawn((Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::ZERO, Val::Px(6.0)),
                ..default()
            },))
            .with_children(|progress_col| {
                progress_col
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.12, 0.14, 0.8)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.85, 0.95)),
                            SongProgressFill,
                        ));
                    });

                progress_col.spawn((
                    Text::new("Progress 000%"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.72, 0.82, 0.9)),
                    SongProgressLabel,
                ));

                progress_col.spawn((
                    Text::new("Looping: off"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.78, 0.86)),
                    LoopStatusLabel,
                ));
            });
    });

    let mut channel_ids = Vec::new();
    let channel_width = if num_channels == 0 {
        100.0
    } else {
        100.0 / num_channels as f32
    };
    let initial_note = format_note_label(None);
    let initial_freq = format_freq_label(None);

    commands.entity(container_id).with_children(|parent| {
        parent
            .spawn((Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::top(Val::Px(6.0)),
                ..default()
            },))
            .with_children(|row| {
                for channel_index in 0..num_channels {
                    let label_char = char::from(b'A' + channel_index as u8);
                    let column_entity = row
                        .spawn((Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            width: Val::Percent(channel_width),
                            min_width: Val::Px(96.0),
                            ..default()
                        },))
                        .with_children(|column| {
                            column.spawn((
                                Text::new(format!("CH {}", label_char)),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.72, 0.82, 0.9)),
                            ));

                            column.spawn((
                                Text::new(initial_note.clone()),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.85, 0.9, 0.95)),
                                ChannelNoteLabel {
                                    channel: channel_index,
                                },
                            ));

                            column.spawn((
                                Text::new(initial_freq.clone()),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.65, 0.75, 0.88)),
                                ChannelFreqLabel {
                                    channel: channel_index,
                                },
                            ));

                            column
                                .spawn((
                                    Node {
                                        flex_direction: FlexDirection::Row,
                                        column_gap: Val::Px(4.0),
                                        align_items: AlignItems::FlexEnd,
                                        height: Val::Px(60.0),
                                        padding: UiRect::new(
                                            Val::Px(2.0),
                                            Val::Px(2.0),
                                            Val::Px(6.0),
                                            Val::Px(4.0),
                                        ),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.35)),
                                ))
                                .with_children(|bar_row| {
                                    for bin in 0..16 {
                                        bar_row.spawn((
                                            Node {
                                                width: Val::Px(10.0),
                                                height: Val::Px(6.0),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.08, 0.11, 0.13, 0.8)),
                                            SpectrumBar {
                                                channel: channel_index,
                                                bin,
                                            },
                                        ));
                                    }
                                });
                        })
                        .id();

                    channel_ids.push(column_entity);
                }
            });
    });

    channel_ids
}
