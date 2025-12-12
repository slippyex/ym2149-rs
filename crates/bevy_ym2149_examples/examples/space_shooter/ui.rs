//! UI spawning and update systems

use bevy::prelude::*;

use super::components::*;
use super::resources::*;
use super::spawning::*;

pub fn spawn_ui(cmd: &mut Commands) {
    let ui_elements = [
        ("", 24.0, Color::WHITE, Val::Px(15.0), Some(Val::Px(20.0)), None, false, UiMarker::Score),
        ("", 24.0, Color::srgb(1.0, 0.8, 0.0), Val::Px(15.0), None, None, false, UiMarker::High),
        ("", 0.0, Color::NONE, Val::Px(0.0), None, None, false, UiMarker::Lives),
        ("WAVE 1", 18.0, Color::srgb(0.6, 0.6, 1.0), Val::Px(45.0), Some(Val::Px(20.0)), None, false, UiMarker::Wave),
        ("GAME OVER\n\nPress R to Restart", 48.0, Color::srgb(1.0, 0.2, 0.2), Val::Percent(45.0), None, None, false, UiMarker::GameOver),
        ("SPACE SHOOTER", 64.0, Color::srgb(0.2, 0.8, 1.0), Val::Percent(25.0), None, None, true, UiMarker::Title),
        ("Press ENTER to Start", 32.0, Color::srgb(1.0, 1.0, 0.2), Val::Percent(50.0), None, None, true, UiMarker::PressEnter),
        ("Music: Mad Max - Lethal Xcess (STe) | C: Toggle CRT", 18.0, Color::srgba(0.7, 0.7, 0.7, 0.9), Val::Percent(65.0), None, None, true, UiMarker::Subtitle),
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
            TextFont { font_size: size, ..default() },
            TextColor(color),
            TextLayout::new_with_justify(bevy::text::Justify::Center),
            node,
            if vis { Visibility::Visible } else { Visibility::Hidden },
            marker,
        ));
    }
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
            cmd.entity(entity).despawn();
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
        cmd.entity(entity).despawn();
    }
    spawn_wave_digits(&mut cmd, &sprites, &screen, gd.wave);
}
