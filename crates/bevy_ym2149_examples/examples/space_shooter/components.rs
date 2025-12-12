//! ECS Components for the space shooter game

use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerBullet;

#[derive(Component)]
pub struct EnemyBullet;

#[derive(Component)]
pub struct Enemy {
    pub points: u32,
}

#[derive(Component)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
}

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

#[derive(Component)]
pub struct Booster;

#[derive(Component)]
pub struct Explosion;

#[derive(Component)]
pub struct Star {
    pub speed: f32,
}

#[derive(Component)]
pub struct DivingEnemy {
    pub target_x: f32,
    pub returning: bool,
    pub start_y: f32,
    pub original_x: f32,
    pub progress: f32,
    pub amplitude: f32,
    pub curve_dir: f32,
}

#[derive(Component)]
pub struct GameOverEnemy {
    pub phase: f32,
    pub amplitude: f32,
    pub frequency: f32,
    pub base_pos: Vec2,
    pub delay: f32,
    pub started: bool,
}

#[derive(Component)]
pub struct GameOverUi {
    pub base_top: f32,
}

#[derive(Component)]
pub struct FadingText {
    pub timer: Timer,
    pub color: Color,
    pub spawn_enemies: bool,
}

#[derive(Component)]
pub struct GameEntity;

#[derive(Component)]
pub struct CrtQuad;

#[derive(Component)]
pub struct LifeIcon;

#[derive(Component)]
pub struct DigitSprite {
    pub position: usize,
}

#[derive(Component, Clone, Copy, PartialEq)]
pub enum ScoreType {
    Score,
    HighScore,
}

#[derive(Component)]
pub struct WaveDigit;

#[derive(Component, Clone, Copy, PartialEq)]
pub enum UiMarker {
    Score,
    High,
    Lives,
    Wave,
    GameOver,
    Title,
    PressEnter,
    Subtitle,
}
