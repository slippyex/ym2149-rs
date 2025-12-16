//! ECS Components for the space shooter game

use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

/// Player is invincible (after respawn)
#[derive(Component)]
pub struct Invincible {
    pub timer: Timer,
}

impl Invincible {
    pub fn new(duration: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        }
    }
}

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

#[derive(Component, Clone, Copy)]
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

// === Name Entry UI ===
#[derive(Component)]
pub struct NameEntryUi;

#[derive(Component)]
pub struct NameEntryChar {
    pub index: usize,
}

// === High Scores UI ===
#[derive(Component)]
pub struct HighScoresUi;

#[derive(Component)]
pub struct HighScoreRow(pub usize);

// === Power-ups Info UI ===
#[derive(Component)]
pub struct PowerUpsUi;

// === Enemy Scores UI ===
#[derive(Component)]
pub struct EnemyScoresUi;

// === Wavy Text Animation ===
#[derive(Component)]
pub struct WavyText {
    pub line_index: usize,
    pub base_top: f32,
}

// === Wavy Sprite Animation (for info screen sprites) ===
#[derive(Component)]
pub struct WavySprite {
    pub line_index: usize,
    pub base_y: f32,
}

// === Quit Confirmation UI ===
#[derive(Component)]
pub struct QuitConfirmUi;

// === Visual Effects ===
#[derive(Component)]
pub struct GameCamera;

#[derive(Component)]
pub struct ScorePopup {
    pub timer: Timer,
    pub start_y: f32,
}

#[derive(Component)]
pub struct StarLayer(pub u8); // 0 = far/slow, 1 = mid, 2 = near/fast

// === Power-ups ===
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerUpType {
    /// Faster shooting rate
    RapidFire,
    /// Triple shot (spread)
    TripleShot,
    /// Speed boost for movement
    SpeedBoost,
    /// Stronger bullets (more damage/bigger)
    PowerShot,
}

#[derive(Component)]
pub struct PowerUp {
    pub kind: PowerUpType,
}

#[derive(Component)]
pub struct SideBooster;

/// Screen flash overlay for power-up collection
#[derive(Component)]
pub struct ScreenFlashOverlay;

/// Nebula background cloud
#[derive(Component)]
pub struct Nebula {
    pub speed: f32,
}

/// Power-up animation (4x4 grid, column = type, row = frame)
/// Frames step by 4: column 0 uses indices 0,4,8,12
#[derive(Component)]
pub struct PowerUpAnimation {
    pub first: usize,
    pub last: usize,
}

/// Shield bubble that protects the player during invincibility
#[derive(Component)]
pub struct ShieldBubble;
