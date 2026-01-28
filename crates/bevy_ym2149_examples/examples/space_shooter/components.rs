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

#[derive(Component, Clone, Copy)]
pub struct EnemyHp {
    pub current: u8,
}

#[derive(Component)]
pub struct Boss {
    pub stage: u32,
    pub points: u32,
}

#[derive(Component, Clone, Copy)]
pub struct BossEnergy {
    pub current: i32,
    pub max: i32,
}

#[derive(Component)]
pub struct BossWeapon {
    pub timer: Timer,
}

#[derive(Component)]
pub struct BossTarget {
    pub target: Vec2,
    pub timer: Timer,
}

/// Boss floating/bobbing animation state
#[derive(Component)]
pub struct BossFloat {
    pub time: f32,
}

impl Default for BossFloat {
    fn default() -> Self {
        Self { time: 0.0 }
    }
}

#[derive(Component, Clone, Copy)]
pub struct BossEscort {
    pub angle: f32,
    pub radius: f32,
    pub speed: f32,
}

/// Boss enters rage mode at low HP - faster, more aggressive, red glow.
#[derive(Component)]
pub struct BossRage {
    pub active: bool,
    pub flash_timer: Timer,
}

impl Default for BossRage {
    fn default() -> Self {
        Self {
            active: false,
            flash_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
        }
    }
}

/// Boss charge attack - telegraphed big attack.
#[derive(Component)]
pub struct BossChargeAttack {
    pub state: ChargeState,
    pub timer: Timer,
    pub cooldown: Timer,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ChargeState {
    #[default]
    Ready,
    /// Boss is charging up (telegraph phase)
    Charging,
    /// Boss fires the big attack
    Firing,
    /// Cooldown before next charge
    Cooldown,
}

impl Default for BossChargeAttack {
    fn default() -> Self {
        Self {
            state: ChargeState::Ready,
            timer: Timer::from_seconds(1.5, TimerMode::Once),
            cooldown: Timer::from_seconds(6.0, TimerMode::Once),
        }
    }
}

/// Warning indicator that grows during boss charge attack.
#[derive(Component)]
pub struct ChargeWarning;

/// Boss HP phase tracking (triggers at 75%, 50%, 25%).
#[derive(Component)]
#[derive(Default)]
pub struct BossPhase {
    pub current: u8, // 0=full, 1=75%, 2=50%, 3=25%
    pub transition_timer: Option<Timer>,
}


/// Boss shield - temporarily invulnerable.
#[derive(Component)]
pub struct BossShield {
    pub active: bool,
    pub timer: Timer,
    pub cooldown: Timer,
}

impl Default for BossShield {
    fn default() -> Self {
        Self {
            active: false,
            timer: Timer::from_seconds(3.0, TimerMode::Once),
            cooldown: Timer::from_seconds(12.0, TimerMode::Once),
        }
    }
}

/// Boss movement pattern state.
#[derive(Component)]
pub struct BossMovementPattern {
    pub current: BossMoveType,
    pub timer: Timer,
    pub cooldown: Timer,
    pub start_pos: Vec2,
    pub target_pos: Vec2,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum BossMoveType {
    #[default]
    Normal,
    /// Quick dash to the side
    Dash,
    /// Dive toward player then retreat
    DiveBomb,
}

impl Default for BossMovementPattern {
    fn default() -> Self {
        Self {
            current: BossMoveType::Normal,
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            cooldown: Timer::from_seconds(5.0, TimerMode::Once),
            start_pos: Vec2::ZERO,
            target_pos: Vec2::ZERO,
        }
    }
}

/// Homing missile projectile.
#[derive(Component)]
pub struct HomingMissile {
    pub speed: f32,
    pub turn_rate: f32,
    pub lifetime: Timer,
    pub velocity: Vec2,
}

/// Bomb projectile that falls and explodes.
#[derive(Component)]
pub struct BossBomb {
    pub fall_speed: f32,
    pub explode_y: f32,
}

/// Bomb explosion ring pattern.
#[derive(Component)]
pub struct BombExplosion {
    pub timer: Timer,
    pub origin: Vec2,
    pub wave: u8,
}

/// Visual shield bubble around boss.
#[derive(Component)]
pub struct BossShieldBubble;

#[derive(Component, Clone, Copy)]
pub struct BulletDamage(pub u8);

#[derive(Component, Clone, Copy)]
pub struct BulletVelocity(pub Vec2);

#[derive(Component)]
pub struct DiveShooter {
    pub timer: Timer,
}

#[derive(Component)]
pub struct EscortShooter {
    pub timer: Timer,
}

/// Enemy performs a sinus-spiral approach and then returns to its original formation slot.
#[derive(Component, Clone, Copy)]
pub struct SpiralEnemy {
    pub target_x: f32,
    pub returning: bool,
    pub start_y: f32,
    pub original: Vec2,
    pub progress: f32,
    pub radius: f32,
    pub turns: f32,
    pub curve_dir: f32,
    pub wobble_amp: f32,
    pub wobble_freq: f32,
}

#[derive(Component)]
pub struct BossDeathFx {
    pub origin: Vec3,
    pub pulse: Timer,
    pub remaining: u8,
    pub total: u8,
}

#[derive(Component)]
pub struct TitleSceneEntity;

#[derive(Component, Clone, Copy)]
pub struct TitleDecor {
    pub base: Vec3,
    pub amp: Vec2,
    pub speed: Vec2,
    pub rot_speed: f32,
    pub scale_base: f32,
    pub scale_pulse: f32,
    pub alpha: f32,
}

#[derive(Component, Clone, Copy)]
pub struct TitleFlyby {
    pub dir: f32,
    pub progress: f32,
    pub duration: f32,
    pub wait: f32,
    pub wait_after: f32,
    pub y: f32,
    pub z: f32,
    pub scale: f32,
    pub alpha: f32,
    pub bob_amp: f32,
    pub bob_freq: f32,
    pub roll_amp: f32,
    pub roll_freq: f32,
    pub phase: f32,
}

#[derive(Component, Clone, Copy)]
pub struct TitleSpiralFlyby {
    pub dir: f32,
    pub progress: f32,
    pub duration: f32,
    pub wait: f32,
    pub wait_after: f32,
    pub base_y: f32,
    pub z: f32,
    pub scale: f32,
    pub alpha: f32,
    pub radius: f32,
    pub turns: f32,
    pub drift_amp: f32,
    pub drift_freq: f32,
    pub phase: f32,
}

// === Demoscene Effects ===

/// Raster bar effect (horizontal color bands).
#[derive(Component)]
pub struct RasterBar {
    pub index: usize,
    pub base_y: f32,
    pub speed: f32,
    pub phase: f32,
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

/// Twinkle parameters for star sprites.
#[derive(Component)]
pub struct StarTwinkle {
    pub phase: f32,
    pub speed: f32,
    pub base: f32,
    pub amplitude: f32,
    pub tint: Vec3,
}

/// Anchor position used for camera/parallax-relative background elements.
#[derive(Component)]
pub struct ParallaxAnchor {
    pub base_x: f32,
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

/// Marker for the exhaustion bar background
#[derive(Component)]
pub struct ExhaustionBarBg;

/// Marker for the exhaustion bar fill (energy remaining)
#[derive(Component)]
pub struct ExhaustionBarFill;

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
pub struct WaveDigit {
    pub position: usize,
}

// === Energy UI ===
#[derive(Component)]
pub struct EnergyBarBg;

#[derive(Component)]
pub struct EnergyBarFill {
    pub left_x: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Component)]
pub struct BossBarBg;

#[derive(Component)]
pub struct BossBarFill {
    pub left_x: f32,
    pub width: f32,
    pub height: f32,
}

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
    Combo,
}

/// Marker for UI drop shadows so animation systems can ignore them.
#[derive(Component)]
pub struct UiShadow;

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
    /// Restore player energy (green cross).
    Heal,
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

/// Pulse and drift parameters for nebula sprites.
#[derive(Component)]
pub struct NebulaPulse {
    pub phase: f32,
    pub speed: f32,
    pub base_alpha: f32,
    pub amplitude: f32,
}

/// Power-up animation (Bonuses-0001.png is a 5x5 grid; column = type, row = frame)
/// Frames step by 5: column 0 uses indices 0,5,10,15,20
#[derive(Component)]
pub struct PowerUpAnimation {
    pub first: usize,
    pub last: usize,
}

/// Shield bubble that protects the player during invincibility
#[derive(Component)]
pub struct ShieldBubble;

// === Bullet Trails / Impact FX ===

/// Emits bullet trail afterimages at a fixed cadence.
#[derive(Component)]
pub struct TrailEmitter {
    pub timer: Timer,
    pub alpha: f32,
}

/// A short-lived trail sprite that fades out.
#[derive(Component)]
pub struct TrailGhost {
    pub timer: Timer,
    pub start_alpha: f32,
}

/// A short-lived hit flash sprite that fades out quickly.
#[derive(Component)]
pub struct HitFlash {
    pub timer: Timer,
    pub start_alpha: f32,
    pub base_scale: Vec3,
}

/// Expanding ring burst used alongside explosions.
#[derive(Component)]
pub struct ExplosionRing {
    pub timer: Timer,
    pub start_scale: f32,
    pub end_scale: f32,
    pub start_alpha: f32,
}

/// Power-up pickup particle that bursts outward and fades.
#[derive(Component)]
pub struct PickupParticle {
    pub timer: Timer,
    pub velocity: Vec2,
    pub start_alpha: f32,
}

// === Wave Transition FX ===

/// Letterbox bar (top/bottom) used for wave transitions.
#[derive(Component)]
pub struct LetterboxBar {
    pub timer: Timer,
}

/// Center banner displayed during wave transitions.
#[derive(Component)]
pub struct WaveBanner {
    pub timer: Timer,
}

/// Player flyout state during wave transitions.
/// Player accelerates up, wraps to bottom, then flies back to start position.
#[derive(Component)]
pub struct WaveFlyout {
    pub phase: WaveFlyoutPhase,
    pub velocity: f32,
    pub target_y: f32,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum WaveFlyoutPhase {
    #[default]
    AccelerateUp,
    WrapToBottom,
    ReturnToPosition,
}

/// Enemy entrance animation - flies in from off-screen to formation position.
#[derive(Component)]
pub struct EnemyEntrance {
    pub start: Vec2,
    pub target: Vec2,
    pub progress: f32,
    pub duration: f32,
    pub pattern: EntrancePattern,
    pub delay: f32,
    pub sine_amp: f32,
    pub sine_freq: f32,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum EntrancePattern {
    #[default]
    FromTop,
    FromLeft,
    FromRight,
    SweepLeft,
    SweepRight,
}

/// Overall entrance formation style for a wave.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum EntranceFormation {
    /// Classic: top row sweeps, others from sides based on position
    #[default]
    ClassicSweep,
    /// All enemies cascade from top
    TopCascade,
    /// Left half from left, right half from right (pincer)
    Pincer,
    /// All from left side in sequence
    LeftWave,
    /// All from right side in sequence
    RightWave,
    /// Center columns first, then expanding outward
    CenterOut,
    /// Outer columns first, then collapsing inward
    OuterIn,
    /// Diagonal sweep from top-left to bottom-right
    DiagonalLeft,
    /// Diagonal sweep from top-right to bottom-left
    DiagonalRight,
}
