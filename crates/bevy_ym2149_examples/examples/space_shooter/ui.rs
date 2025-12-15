//! UI spawning and update systems

use bevy::prelude::*;

use super::components::*;
use super::resources::*;
use super::spawning::*;

pub fn spawn_ui(cmd: &mut Commands, fonts: &FontAssets) {
    let ui_elements = [
        ("", 24.0, Color::WHITE, Val::Px(15.0), Some(Val::Px(20.0)), None, false, UiMarker::Score),
        ("", 24.0, Color::srgb(1.0, 0.8, 0.0), Val::Px(15.0), None, None, false, UiMarker::High),
        ("", 0.0, Color::NONE, Val::Px(0.0), None, None, false, UiMarker::Lives),
        ("WAVE 1", 18.0, Color::srgb(0.6, 0.6, 1.0), Val::Px(45.0), Some(Val::Px(20.0)), None, false, UiMarker::Wave),
        ("GAME OVER", 64.0, Color::srgb(1.0, 0.2, 0.2), Val::Percent(40.0), None, None, false, UiMarker::GameOver),
        ("SPACE  SHOOTER", 72.0, Color::srgb(0.2, 0.8, 1.0), Val::Percent(20.0), None, None, true, UiMarker::Title),
        ("Press  ENTER  to  Start", 36.0, Color::srgb(1.0, 1.0, 0.2), Val::Percent(45.0), None, None, true, UiMarker::PressEnter),
        ("", 0.0, Color::NONE, Val::Percent(0.0), None, None, false, UiMarker::Subtitle),
    ];

    for (txt, size, color, top, left, right, vis, marker) in ui_elements {
        let mut node = Node {
            position_type: PositionType::Absolute,
            top,
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        };
        if let Some(l) = left {
            node.left = l;
            node.width = Val::Auto;
        }
        if let Some(r) = right {
            node.right = r;
            node.width = Val::Auto;
        }
        cmd.spawn((
            Text::new(txt),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
            TextLayout::new_with_justify(bevy::text::Justify::Center),
            node,
            if vis { Visibility::Visible } else { Visibility::Hidden },
            marker,
        ));
    }
}

pub fn spawn_name_entry_ui(cmd: &mut Commands, fonts: &FontAssets, score: u32, wave: u32) {
    // Container
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(25.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        NameEntryUi,
    ))
    .with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("NEW  HIGH  SCORE"),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 48.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.8, 0.0)),
        ));

        // Score display
        parent.spawn((
            Text::new(format!("{}", score)),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 64.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        // Wave display
        parent.spawn((
            Text::new(format!("WAVE  {}", wave)),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 32.0,
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 1.0)),
        ));

        // Instruction
        parent.spawn((
            Text::new("ENTER  YOUR  NAME"),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 28.0,
                ..default()
            },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));

        // Name entry row
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(20.0),
                ..default()
            })
            .with_children(|row| {
                for i in 0..3 {
                    row.spawn((
                        Text::new("A"),
                        TextFont {
                            font: fonts.arcade.clone(),
                            font_size: 72.0,
                            ..default()
                        },
                        TextColor(if i == 0 {
                            Color::srgb(1.0, 1.0, 0.2)
                        } else {
                            Color::srgb(0.5, 0.5, 0.5)
                        }),
                        NameEntryChar { index: i },
                        NameEntryUi,
                    ));
                }
            });

        // Controls hint
        parent.spawn((
            Text::new("UP DOWN  to  change    LEFT RIGHT  to  move    ENTER  to  confirm"),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 18.0,
                ..default()
            },
            TextColor(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        ));
    });
}

pub fn spawn_high_scores_ui(cmd: &mut Commands, fonts: &FontAssets, scores: &HighScoreList) {
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(10.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(15.0),
            ..default()
        },
        HighScoresUi,
    ))
    .with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("HIGH  SCORES"),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 56.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.8, 0.0)),
        ));

        // Header row
        parent
            .spawn(Node {
                width: Val::Px(500.0),
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::top(Val::Px(20.0)),
                ..default()
            })
            .with_children(|row| {
                for (txt, width) in [("NO", 60.0), ("NAME", 100.0), ("SCORE", 180.0), ("WAVE", 60.0)]
                {
                    row.spawn((
                        Text::new(txt),
                        TextFont {
                            font: fonts.arcade.clone(),
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.6, 1.0)),
                        Node {
                            width: Val::Px(width),
                            ..default()
                        },
                    ));
                }
            });

        // Score entries
        for (i, entry) in scores.entries.iter().enumerate().take(10) {
            let color = match i {
                0 => Color::srgb(1.0, 0.2, 0.2),  // Red (1st)
                1 => Color::srgb(1.0, 1.0, 0.2),  // Yellow (2nd)
                2 => Color::srgb(0.2, 1.0, 0.2),  // Green (3rd)
                _ => Color::srgb(0.5, 0.5, 0.5),  // Gray
            };

            parent
                .spawn((
                    Node {
                        width: Val::Px(500.0),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                    HighScoreRow(i),
                    HighScoresUi,
                ))
                .with_children(|row| {
                    let items = [
                        (format!("{:2}", i + 1), 60.0),
                        (entry.name.clone(), 100.0),
                        (format!("{:06}", entry.score), 180.0),
                        (format!("{:02}", entry.wave), 60.0),
                    ];
                    for (txt, width) in items {
                        row.spawn((
                            Text::new(txt),
                            TextFont {
                                font: fonts.arcade.clone(),
                                font_size: 28.0,
                                ..default()
                            },
                            TextColor(color),
                            Node {
                                width: Val::Px(width),
                                ..default()
                            },
                            HighScoreRow(i),
                            HighScoresUi,
                        ));
                    }
                });
        }


        // Back hint
        parent.spawn((
            Text::new("Press  ESCAPE  to  return"),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgba(0.5, 0.5, 0.5, 0.8)),
            Node {
                margin: UiRect::top(Val::Px(30.0)),
                ..default()
            },
        ));
    });
}

pub fn update_ui(gd: Res<GameData>, mut q: Query<(&mut Text, &UiMarker)>) {
    for (mut t, m) in q.iter_mut() {
        if *m == UiMarker::Wave {
            t.0 = format!("WAVE {}", gd.wave);
        }
    }
}

pub fn update_life_icons(
    mut cmd: Commands,
    gd: Res<GameData>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    life_icons: Query<Entity, With<LifeIcon>>,
) {
    let current_count = life_icons.iter().count() as u32;

    if current_count != gd.lives {
        for entity in life_icons.iter() {
            cmd.entity(entity).try_despawn();
        }
        spawn_life_icons(&mut cmd, &sprites, &screen, gd.lives);
    }
}

pub fn update_score_digits(
    gd: Res<GameData>,
    mut score_digits: Query<(&mut Sprite, &DigitSprite, &ScoreType)>,
) {
    let score_str = format!("{:06}", gd.score.min(999999));
    let high_str = format!("{:06}", gd.high_score.min(999999));

    for (mut sprite, digit_sprite, score_type) in score_digits.iter_mut() {
        let value_str = match score_type {
            ScoreType::Score => &score_str,
            ScoreType::HighScore => &high_str,
        };

        if let Some(digit_char) = value_str.chars().nth(digit_sprite.position) {
            if let Some(atlas) = &mut sprite.texture_atlas {
                let digit = digit_char.to_digit(10).unwrap_or(0) as u8;
                atlas.index = digit_to_atlas_index(digit);
            }
        }
    }
}

pub fn update_wave_digits(
    mut cmd: Commands,
    gd: Res<GameData>,
    sprites: Res<SpriteAssets>,
    screen: Res<ScreenSize>,
    wave_digits: Query<Entity, With<WaveDigit>>,
) {
    let current_digits: Vec<_> = wave_digits.iter().collect();
    if current_digits.len() != 2 {
        return;
    }

    for entity in current_digits {
        cmd.entity(entity).try_despawn();
    }
    spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);
}
