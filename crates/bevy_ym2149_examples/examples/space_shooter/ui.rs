//! UI spawning and update systems

use bevy::prelude::*;

use super::components::*;
use super::config::BOSS_WAVE_INTERVAL;
use super::resources::*;
use super::spawning::*;

type ComboUiQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Text,
        &'static mut TextColor,
        &'static mut TextFont,
        &'static mut Visibility,
        &'static UiMarker,
        Option<&'static UiShadow>,
    ),
>;

pub fn spawn_ui(cmd: &mut Commands, fonts: &FontAssets) {
    let ui_elements = [
        (
            "",
            24.0,
            Color::WHITE,
            Val::Px(15.0),
            Some(Val::Px(20.0)),
            None,
            false,
            UiMarker::Score,
        ),
        (
            "",
            24.0,
            Color::srgb(1.0, 0.8, 0.0),
            Val::Px(15.0),
            None,
            None,
            false,
            UiMarker::High,
        ),
        (
            "",
            0.0,
            Color::NONE,
            Val::Px(0.0),
            None,
            None,
            false,
            UiMarker::Lives,
        ),
        (
            "WAVE 1",
            18.0,
            Color::srgb(0.6, 0.6, 1.0),
            Val::Px(45.0),
            Some(Val::Px(20.0)),
            None,
            false,
            UiMarker::Wave,
        ),
        (
            "GAME OVER",
            64.0,
            Color::srgb(1.0, 0.2, 0.2),
            Val::Percent(40.0),
            None,
            None,
            false,
            UiMarker::GameOver,
        ),
        (
            "SPACE  SHOOTER",
            72.0,
            Color::srgb(0.2, 0.8, 1.0),
            Val::Percent(20.0),
            None,
            None,
            true,
            UiMarker::Title,
        ),
        (
            "Press  ENTER  to  Start",
            36.0,
            Color::srgb(1.0, 1.0, 0.2),
            Val::Percent(45.0),
            None,
            None,
            true,
            UiMarker::PressEnter,
        ),
        (
            "ARROWS  MOVE   SPACE  FIRE   ENTER  START   C  CRT   M  MUSIC",
            22.0,
            Color::srgba(0.7, 0.85, 1.0, 0.85),
            Val::Percent(65.0),
            None,
            None,
            true,
            UiMarker::Subtitle,
        ),
        (
            "COMBO  x2",
            28.0,
            Color::srgb(1.0, 0.3, 0.9),
            Val::Px(80.0),
            None,
            None,
            false,
            UiMarker::Combo,
        ),
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

        let visibility = if vis {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        // Cheap but effective drop shadow for better readability (especially with CRT off).
        if size > 0.0 && color != Color::NONE && !txt.is_empty() {
            let mut shadow_node = node.clone();
            shadow_node.margin = UiRect {
                left: Val::Px(2.0),
                top: Val::Px(2.0),
                ..default()
            };
            cmd.spawn((
                Text::new(txt),
                TextFont {
                    font: fonts.arcade.clone(),
                    font_size: size,
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                TextLayout::new_with_justify(bevy::text::Justify::Center),
                shadow_node,
                visibility,
                marker,
                UiShadow,
            ));
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
            visibility,
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
            Text::new(format!("{score}")),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 64.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        // Wave display
        parent.spawn((
            Text::new(format!("WAVE  {wave}")),
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
                for (txt, width) in [
                    ("NO", 60.0),
                    ("NAME", 100.0),
                    ("SCORE", 180.0),
                    ("WAVE", 60.0),
                ] {
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
                0 => Color::srgb(1.0, 0.2, 0.2), // Red (1st)
                1 => Color::srgb(1.0, 1.0, 0.2), // Yellow (2nd)
                2 => Color::srgb(0.2, 1.0, 0.2), // Green (3rd)
                _ => Color::srgb(0.5, 0.5, 0.5), // Gray
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
            Text::new("Press  ENTER  to  return"),
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

pub fn update_ui(
    gd: Res<GameData>,
    mut last_wave: Local<u32>,
    mut q: Query<(&mut Text, &UiMarker)>,
) {
    if *last_wave == gd.wave {
        return;
    }
    *last_wave = gd.wave;

    for (mut t, m) in q.iter_mut() {
        if *m == UiMarker::Wave {
            t.0 = format!("WAVE {}", gd.wave);
        }
    }
}

pub fn combo_ui_update(
    time: Res<Time>,
    combo: Res<ComboTracker>,
    mut last_combo: Local<u32>,
    mut pulse: Local<f32>,
    mut q: ComboUiQuery<'_, '_>,
) {
    const PULSE_DURATION: f32 = 0.28;
    let dt = time.delta_secs();
    *pulse = (*pulse - dt).max(0.0);

    let count = combo.count;
    if count > *last_combo {
        *pulse = PULSE_DURATION;
        *last_combo = count;
    } else if combo.timer <= 0.0 {
        *last_combo = 0;
    }

    for (mut text, mut color, mut font, mut vis, marker, shadow) in q.iter_mut() {
        if *marker != UiMarker::Combo {
            continue;
        }

        if combo.count <= 1 || combo.timer <= 0.0 {
            *vis = Visibility::Hidden;
            continue;
        }

        *vis = Visibility::Visible;
        text.0 = format!("COMBO  x{}", combo.count);

        let alpha = (combo.timer / 1.0).clamp(0.0, 1.0);
        let t = time.elapsed_secs();
        let glow = (t * 8.0).sin() * 0.15 + 0.85;
        // Bigger "pop" on each new combo, then zoom while fading out.
        let pulse_t = (*pulse / PULSE_DURATION).clamp(0.0, 1.0);
        let pulse_pop = 1.0 + pulse_t.powf(0.6) * 0.95;
        let fade_zoom = 1.0 + (1.0 - alpha).powf(1.6) * 0.85;
        font.font_size = 28.0 * pulse_pop * fade_zoom;
        if shadow.is_some() {
            color.0 = Color::srgba(0.0, 0.0, 0.0, alpha * 0.55);
        } else {
            color.0 = Color::srgba(1.0, 0.3, 0.9, alpha * glow);
        }
    }
}

pub fn spawn_wave_transition_fx(
    cmd: &mut Commands,
    fonts: &FontAssets,
    screen: &ScreenSize,
    wave: u32,
    duration: f32,
) {
    let duration = duration.max(0.4);

    for is_top in [true, false] {
        cmd.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: if is_top { Val::Px(0.0) } else { Val::Auto },
                bottom: if is_top { Val::Auto } else { Val::Px(0.0) },
                width: Val::Percent(100.0),
                height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            LetterboxBar {
                timer: Timer::from_seconds(duration, TimerMode::Once),
            },
        ));
    }

    // Center banner with subtle background.
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        WaveBanner {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        },
    ))
    .with_children(|parent| {
        let label = if wave % BOSS_WAVE_INTERVAL == 0 {
            format!("BOSS  {}", (wave / BOSS_WAVE_INTERVAL).max(1))
        } else {
            format!("WAVE  {wave}")
        };

        // Container with background + border; text as child so it doesn't appear as a separate "block".
        parent
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.1, 0.14, 0.0)),
                BorderColor::all(Color::srgba(0.35, 0.6, 1.0, 0.0)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(label),
                    TextFont {
                        font: fonts.arcade.clone(),
                        font_size: 46.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.45, 0.8, 1.0, 0.0)),
                    TextLayout::new_with_justify(bevy::text::Justify::Center),
                ));
            });
    });

    // Subtle full-screen tint overlay behind UI to reinforce the transition (no extra shader needed).
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Px(screen.width),
            height: Val::Px(screen.height),
            ..default()
        },
        BackgroundColor(Color::srgba(0.35, 0.55, 1.0, 0.0)),
        WaveBanner {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        },
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn wave_transition_update(
    mut cmd: Commands,
    time: Res<Time>,
    mut set: ParamSet<(
        Query<(Entity, &mut Node, &mut BackgroundColor, &mut LetterboxBar), Without<WaveBanner>>,
        Query<
            (Entity, &mut BackgroundColor, &mut WaveBanner),
            (Without<Children>, Without<LetterboxBar>),
        >,
        Query<
            (&mut BackgroundColor, &mut BorderColor),
            (Without<WaveBanner>, Without<LetterboxBar>),
        >,
    )>,
    mut banners: Query<(Entity, &Children, &mut WaveBanner)>,
    child_nodes: Query<&Children>,
    mut texts: Query<(&mut TextColor, &mut TextFont, Option<&UiShadow>)>,
) {
    for (entity, mut node, mut bg, mut bar) in set.p0().iter_mut() {
        bar.timer.tick(time.delta());
        let progress = bar.timer.fraction().clamp(0.0, 1.0);

        // Ease in/out.
        let ease = if progress < 0.25 {
            progress / 0.25
        } else if progress > 0.75 {
            (1.0 - progress) / 0.25
        } else {
            1.0
        };

        let target = 62.0;
        node.height = Val::Px(target * ease);
        bg.0 = Color::srgba(0.0, 0.0, 0.0, 0.85 * ease);

        if bar.timer.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }

    for (entity, children, mut banner) in banners.iter_mut() {
        banner.timer.tick(time.delta());
        let progress = banner.timer.fraction().clamp(0.0, 1.0);
        let ease = if progress < 0.2 {
            progress / 0.2
        } else if progress > 0.8 {
            (1.0 - progress) / 0.2
        } else {
            1.0
        };

        for child in children.iter() {
            if let Ok((mut bg, mut border)) = set.p2().get_mut(child) {
                bg.0 = Color::srgba(0.08, 0.1, 0.14, 0.75 * ease);
                *border = BorderColor::all(Color::srgba(0.35, 0.6, 1.0, 0.9 * ease));
            }

            // Text lives inside the banner panel node; update its children too.
            let panel_children = child_nodes.get(child).ok();
            for &text_entity in panel_children.into_iter().flatten() {
                if let Ok((mut text_color, mut font, shadow)) = texts.get_mut(text_entity) {
                    let is_shadow = shadow.is_some();
                    let alpha = if is_shadow { 0.55 } else { 1.0 };
                    let tint = if is_shadow {
                        Color::srgba(0.0, 0.0, 0.0, alpha * ease)
                    } else {
                        Color::srgba(0.45, 0.8, 1.0, alpha * ease)
                    };
                    text_color.0 = tint;

                    // Mild breathing to feel alive.
                    if !is_shadow {
                        let t = time.elapsed_secs();
                        font.font_size = 46.0 * (1.0 + (t * 2.0).sin() * 0.02);
                    }
                }
            }
        }

        if banner.timer.just_finished() {
            for child in children.iter() {
                cmd.entity(child).try_despawn();
            }
            cmd.entity(entity).try_despawn();
        }
    }

    for (entity, mut bg, mut overlay) in set.p1().iter_mut() {
        overlay.timer.tick(time.delta());
        let progress = overlay.timer.fraction().clamp(0.0, 1.0);
        let ease = if progress < 0.2 {
            progress / 0.2
        } else if progress > 0.8 {
            (1.0 - progress) / 0.2
        } else {
            1.0
        };
        bg.0 = Color::srgba(0.35, 0.55, 1.0, 0.12 * ease);
        if overlay.timer.just_finished() {
            cmd.entity(entity).try_despawn();
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
    mut last: Local<(u32, u32)>,
    mut score_digits: Query<(&mut Sprite, &DigitSprite, &ScoreType)>,
) {
    if last.0 == gd.score && last.1 == gd.high_score {
        return;
    }
    last.0 = gd.score;
    last.1 = gd.high_score;

    let score_str = format!("{:06}", gd.score.min(999999));
    let high_str = format!("{:06}", gd.high_score.min(999999));

    for (mut sprite, digit_sprite, score_type) in score_digits.iter_mut() {
        let value_str = match score_type {
            ScoreType::Score => &score_str,
            ScoreType::HighScore => &high_str,
        };

        if let Some(digit_char) = value_str.chars().nth(digit_sprite.position)
            && let Some(atlas) = &mut sprite.texture_atlas
        {
            let digit = digit_char.to_digit(10).unwrap_or(0) as u8;
            atlas.index = digit_to_atlas_index(digit);
        }
    }
}

pub fn update_wave_digits(
    gd: Res<GameData>,
    mut last_wave: Local<u32>,
    mut wave_digits: Query<(&mut Sprite, &WaveDigit)>,
) {
    if *last_wave == gd.wave {
        return;
    }
    *last_wave = gd.wave;

    let wave_str = format!("{:02}", gd.wave.min(99));
    for (mut sprite, digit_sprite) in wave_digits.iter_mut() {
        if let Some(digit_char) = wave_str.chars().nth(digit_sprite.position)
            && let Some(atlas) = &mut sprite.texture_atlas
        {
            let digit = digit_char.to_digit(10).unwrap_or(0) as u8;
            atlas.index = digit_to_atlas_index(digit);
        }
    }
}

pub fn spawn_powerups_ui(
    cmd: &mut Commands,
    fonts: &FontAssets,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
) {
    use super::spawning::POWERUP_ANIM;

    // Text colors matching Bonuses-0001.png columns.
    const POWERUP_TEXT_COLORS: [Color; 5] = [
        Color::srgb(0.3, 0.95, 0.3),  // Heal - green cross
        Color::srgb(1.0, 0.3, 0.3),   // RapidFire - red star
        Color::srgb(1.0, 0.35, 0.95), // TripleShot - magenta
        Color::srgb(0.3, 0.6, 1.0),   // SpeedBoost - blue shield
        Color::srgb(1.0, 0.9, 0.2),   // PowerShot - yellow 2x
    ];

    // Title
    let title_top = 8.0;
    cmd.spawn((
        Text::new("POWER  UPS"),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 56.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.0)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(title_top),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WavyText {
            line_index: 0,
            base_top: title_top,
        },
        PowerUpsUi,
    ));

    // (description, visual_index)
    let powerups = [
        ("Restore  energy", 0),   // Heal
        ("Faster  shooting", 1),  // Rapid Fire
        ("Spread  pattern", 2),   // Triple Shot
        ("Faster  movement", 3),  // Speed Boost
        ("Stronger  bullets", 4), // Power Shot
    ];

    let sprite_x = -100.0;

    for (i, (desc, visual_idx)) in powerups.iter().enumerate() {
        let (sprite_index, _) = POWERUP_ANIM[*visual_idx];
        let text_color = POWERUP_TEXT_COLORS[*visual_idx];

        // Text position as percentage from top
        let text_top = 23.0 + i as f32 * 11.0;

        // Convert percentage to world Y coordinate
        // top% from screen top = half_height - (top% / 100 * height)
        let base_y = screen.half_height * (1.0 - 2.0 * text_top / 100.0) - 20.0;

        // Power-up sprite (2D entity, sprites are already colored)
        cmd.spawn((
            Sprite {
                image: sprites.powerup_texture.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: sprites.powerup_layout.clone(),
                    index: sprite_index,
                }),
                ..default()
            },
            Transform::from_xyz(sprite_x, base_y, 5.0).with_scale(Vec3::splat(1.75)), // 32x32 * 1.75 ≈ 56px
            WavySprite {
                line_index: i + 1,
                base_y,
            },
            PowerUpsUi,
        ));

        // Description text (UI)
        cmd.spawn((
            Text::new(*desc),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 28.0,
                ..default()
            },
            TextColor(text_color),
            TextLayout::new_with_justify(bevy::text::Justify::Left),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(text_top),
                left: Val::Percent(52.0),
                ..default()
            },
            WavyText {
                line_index: i + 1,
                base_top: text_top,
            },
            PowerUpsUi,
        ));
    }

    // Back hint
    let hint_top = 78.0;
    cmd.spawn((
        Text::new("Press  ENTER  to  return"),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(hint_top),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WavyText {
            line_index: 5,
            base_top: hint_top,
        },
        PowerUpsUi,
    ));
}

pub fn spawn_enemy_scores_ui(
    cmd: &mut Commands,
    fonts: &FontAssets,
    sprites: &SpriteAssets,
    screen: &ScreenSize,
) {
    use super::config::SPRITE_SCALE;

    // Title
    let title_top = 8.0;
    cmd.spawn((
        Text::new("ENEMY  SCORES"),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 56.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.0)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(title_top),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WavyText {
            line_index: 0,
            base_top: title_top,
        },
        EnemyScoresUi,
    ));

    // (texture, layout, last_frame, score_text, color)
    let enemies: [(_, _, usize, &str, Color); 3] = [
        (
            &sprites.enemy_lips_texture,
            &sprites.enemy_lips_layout,
            4,
            "100 pts",
            Color::srgb(1.0, 0.2, 0.2),
        ),
        (
            &sprites.enemy_bonbon_texture,
            &sprites.enemy_bonbon_layout,
            3,
            "50 pts",
            Color::srgb(1.0, 1.0, 0.2),
        ),
        (
            &sprites.enemy_alan_texture,
            &sprites.enemy_alan_layout,
            5,
            "25 pts",
            Color::srgb(0.2, 1.0, 0.2),
        ),
    ];

    let sprite_x = -100.0;

    for (i, (texture, layout, last_frame, pts, color)) in enemies.iter().enumerate() {
        // Text position as percentage from top
        let text_top = 24.0 + i as f32 * 14.0;

        // Convert percentage to world Y coordinate
        let base_y = screen.half_height * (1.0 - 2.0 * text_top / 100.0) - 20.0;

        // Animated enemy sprite (2D entity) - slower animation for readability
        cmd.spawn((
            Sprite::from_atlas_image(
                (*texture).clone(),
                TextureAtlas {
                    layout: (*layout).clone(),
                    index: 0,
                },
            ),
            Transform::from_xyz(sprite_x, base_y, 5.0).with_scale(Vec3::splat(SPRITE_SCALE)),
            AnimationIndices {
                first: 0,
                last: *last_frame,
            },
            AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)), // Slower animation
            WavySprite {
                line_index: i + 1,
                base_y,
            },
            EnemyScoresUi,
        ));

        // Score text (UI)
        cmd.spawn((
            Text::new(*pts),
            TextFont {
                font: fonts.arcade.clone(),
                font_size: 42.0,
                ..default()
            },
            TextColor(*color),
            TextLayout::new_with_justify(bevy::text::Justify::Left),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(text_top),
                left: Val::Percent(52.0),
                ..default()
            },
            WavyText {
                line_index: i + 1,
                base_top: text_top,
            },
            EnemyScoresUi,
        ));
    }

    // Bonus info
    let bonus_top = 68.0;
    cmd.spawn((
        Text::new("DIVING  x2"),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 1.0)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(bonus_top),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WavyText {
            line_index: 4,
            base_top: bonus_top,
        },
        EnemyScoresUi,
    ));

    // Back hint
    let hint_top = 78.0;
    cmd.spawn((
        Text::new("Press  ENTER  to  return"),
        TextFont {
            font: fonts.arcade.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(hint_top),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WavyText {
            line_index: 5,
            base_top: hint_top,
        },
        EnemyScoresUi,
    ));
}
