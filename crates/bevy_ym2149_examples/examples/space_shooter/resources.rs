//! Game resources

use bevy::prelude::*;
use directories::ProjectDirs;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ym2149_gist_replayer::{GistPlayer, GistSound};

use super::components::PowerUpType;
use super::config::{EXTRA_LIFE_SCORE, FadePhase, MAX_HIGH_SCORES, STARTING_LIVES};
use super::crt::CrtMaterial;

/// Cached window dimensions - updated on resize
#[derive(Resource)]
pub struct ScreenSize {
    pub width: f32,
    pub height: f32,
    pub half_width: f32,
    pub half_height: f32,
}

/// Debug: start at a specific wave (CLI: --wave N or --boss)
#[derive(Resource, Default)]
pub struct DebugStartWave(pub Option<u32>);

impl ScreenSize {
    pub fn from_window(w: &Window) -> Self {
        Self {
            width: w.width(),
            height: w.height(),
            half_width: w.width() / 2.0,
            half_height: w.height() / 2.0,
        }
    }
}

#[derive(Resource)]
pub struct GameData {
    pub score: u32,
    pub high_score: u32,
    pub lives: u32,
    pub wave: u32,
    pub enemy_direction: f32,
    pub next_extra_life: u32,
}

/// Timers that tick every frame during gameplay.
///
/// Keeping these out of `GameData` avoids triggering `resource_changed::<GameData>` every frame,
/// which would otherwise cause UI update systems to run constantly.
#[derive(Resource, Debug)]
pub struct GameTimers {
    pub shoot: Timer,
    pub enemy_shoot: Timer,
    pub dive: Timer,
    pub spiral: Timer,
}

impl Default for GameTimers {
    fn default() -> Self {
        Self {
            shoot: Timer::from_seconds(0.0, TimerMode::Once),
            enemy_shoot: Timer::from_seconds(1.5, TimerMode::Once),
            dive: Timer::from_seconds(3.0, TimerMode::Once),
            spiral: Timer::from_seconds(6.0, TimerMode::Once),
        }
    }
}

/// Player energy (health) within a life.
#[derive(Resource, Clone, Copy, Debug)]
pub struct PlayerEnergy {
    pub current: u8,
    pub max: u8,
}

/// Per-run offset used to rotate wave formation patterns (avoids predictable ordering).
#[derive(Resource, Clone, Copy, Debug)]
pub struct WavePatternRotation {
    pub offset: usize,
}

impl Default for WavePatternRotation {
    fn default() -> Self {
        let mut rng = SmallRng::from_os_rng();
        Self {
            offset: rng.random_range(0..1024),
        }
    }
}

/// Queue of power-ups to spawn over time (used for wave-clear drops while enforcing an on-screen cap).
#[derive(Resource, Default, Debug)]
pub struct PowerUpDropQueue(pub VecDeque<PowerUpType>);

/// Cooldown for spawning queued power-ups so players can't vacuum all wave-clear drops instantly.
#[derive(Resource, Debug)]
pub struct PowerUpSpawnCooldown(pub Timer);

impl Default for PowerUpSpawnCooldown {
    fn default() -> Self {
        Self(Timer::from_seconds(0.0, TimerMode::Once))
    }
}

/// Between waves we show a short "wave cleared" and wait before spawning the next wave.
#[derive(Resource, Debug)]
pub struct WaveIntermission {
    pub timer: Timer,
    pub pending_wave: Option<u32>,
}

impl Default for WaveIntermission {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            pending_wave: None,
        }
    }
}

impl WaveIntermission {
    pub fn is_active(&self) -> bool {
        self.pending_wave.is_some()
    }

    pub fn start(&mut self, next_wave: u32, seconds: f32) {
        self.pending_wave = Some(next_wave);
        self.timer = Timer::from_seconds(seconds.max(0.0), TimerMode::Once);
    }

    pub fn cancel(&mut self) {
        self.pending_wave = None;
        self.timer = Timer::from_seconds(0.0, TimerMode::Once);
    }
}

impl Default for PlayerEnergy {
    fn default() -> Self {
        Self { current: 5, max: 5 }
    }
}

impl PlayerEnergy {
    pub fn reset(&mut self) {
        self.current = self.max;
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    /// Returns `true` if the hit depleted energy (player should lose a life).
    pub fn take_hit(&mut self) -> bool {
        self.current = self.current.saturating_sub(1);
        self.current == 0
    }

    /// Returns `true` if energy actually increased.
    pub fn heal(&mut self, amount: u8) -> bool {
        let before = self.current;
        self.current = self.current.saturating_add(amount).min(self.max);
        self.current != before
    }

    pub fn fraction(&self) -> f32 {
        if self.max == 0 {
            return 0.0;
        }
        (self.current as f32 / self.max as f32).clamp(0.0, 1.0)
    }
}

impl GameData {
    pub fn new() -> Self {
        Self {
            lives: STARTING_LIVES,
            wave: 1,
            enemy_direction: 1.0,
            next_extra_life: EXTRA_LIFE_SCORE,
            score: 0,
            high_score: 0,
        }
    }

    pub fn reset(&mut self) {
        let hs = self.high_score;
        *self = Self::new();
        self.high_score = hs;
    }

    pub fn dive_interval(&self) -> f32 {
        // Galaxian-like: readable early, ramps up later.
        let w = (self.wave.saturating_sub(1) as f32).sqrt();
        (3.2 - w * 0.55).max(0.95)
    }

    pub fn max_divers(&self) -> usize {
        let w = (self.wave.saturating_sub(1) as f32).sqrt();
        (1 + (w * 1.25) as usize).min(7)
    }

    pub fn enemy_shoot_rate(&self) -> f32 {
        let w = (self.wave.saturating_sub(1) as f32).sqrt();
        (1.45 - w * 0.32).max(0.55)
    }

    pub fn spiral_interval(&self) -> f32 {
        // Spirals start later and ramp gently.
        let w = (self.wave.saturating_sub(8) as f32).max(0.0).sqrt();
        (5.0 - w * 0.55).max(1.8)
    }

    pub fn max_spirals(&self) -> usize {
        let w = (self.wave.saturating_sub(8) as f32).max(0.0).sqrt();
        (1 + (w * 1.2) as usize).min(6)
    }
}

impl Default for GameData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Resource, Default)]
pub struct MusicFade {
    pub target_subsong: Option<usize>,
    pub phase: FadePhase,
    pub timer: f32,
}

#[derive(Resource)]
pub struct CrtState {
    pub enabled: bool,
}

#[derive(Resource)]
pub struct CrtMaterialHandle(pub Handle<CrtMaterial>);

#[derive(Resource)]
pub struct SceneRenderTarget(pub Handle<Image>);

#[derive(Resource)]
pub struct Sfx {
    pub laser: GistSound,
    pub explode: GistSound,
    pub death: GistSound,
    pub powerup_pickup: GistSound,
}

#[derive(Resource, Clone)]
pub struct GistRes(pub Arc<Mutex<GistPlayer>>);

/// All sprite sheet assets for the game
#[derive(Resource)]
pub struct SpriteAssets {
    pub player_texture: Handle<Image>,
    pub player_layout: Handle<TextureAtlasLayout>,
    pub booster_texture: Handle<Image>,
    pub booster_layout: Handle<TextureAtlasLayout>,
    pub booster_left_texture: Handle<Image>,
    pub booster_left_layout: Handle<TextureAtlasLayout>,
    pub booster_right_texture: Handle<Image>,
    pub booster_right_layout: Handle<TextureAtlasLayout>,
    pub enemy_alan_texture: Handle<Image>,
    pub enemy_alan_layout: Handle<TextureAtlasLayout>,
    pub enemy_bonbon_texture: Handle<Image>,
    pub enemy_bonbon_layout: Handle<TextureAtlasLayout>,
    pub enemy_lips_texture: Handle<Image>,
    pub enemy_lips_layout: Handle<TextureAtlasLayout>,
    pub player_bullet_texture: Handle<Image>,
    pub player_bullet_layout: Handle<TextureAtlasLayout>,
    pub triple_shot_texture: Handle<Image>,
    pub triple_shot_layout: Handle<TextureAtlasLayout>,
    pub power_shot_texture: Handle<Image>,
    pub power_shot_layout: Handle<TextureAtlasLayout>,
    pub enemy_bullet_texture: Handle<Image>,
    pub enemy_bullet_layout: Handle<TextureAtlasLayout>,
    pub explosion_texture: Handle<Image>,
    pub explosion_layout: Handle<TextureAtlasLayout>,
    pub life_icon_texture: Handle<Image>,
    pub number_font_texture: Handle<Image>,
    pub number_font_layout: Handle<TextureAtlasLayout>,
    pub powerup_texture: Handle<Image>,
    pub powerup_layout: Handle<TextureAtlasLayout>,
    pub boss_texture: Handle<Image>,
    pub boss_layout: Handle<TextureAtlasLayout>,
    pub bubble_texture: Handle<Image>,
    pub ring_texture: Handle<Image>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyType {
    Alan,
    BonBon,
    Lips,
}

/// Font assets for arcade-style text
#[derive(Resource, Clone)]
pub struct FontAssets {
    pub arcade: Handle<Font>,
}

/// Single high score entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    pub name: String,
    pub score: u32,
    pub wave: u32,
}

/// Persistent high score list
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreList {
    pub entries: Vec<HighScoreEntry>,
}

impl Default for HighScoreList {
    fn default() -> Self {
        Self {
            entries: (0..MAX_HIGH_SCORES)
                .map(|i| HighScoreEntry {
                    name: "AAA".to_string(),
                    score: 1000 * (MAX_HIGH_SCORES - i) as u32,
                    wave: 1,
                })
                .collect(),
        }
    }
}

impl HighScoreList {
    fn save_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ym2149", "space_shooter")
            .map(|dirs| dirs.data_dir().join("highscores.json"))
    }

    pub fn load() -> Self {
        Self::save_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::save_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, json);
            }
        }
    }

    pub fn is_high_score(&self, score: u32) -> bool {
        score > 0
            && (self.entries.len() < MAX_HIGH_SCORES
                || self.entries.last().is_some_and(|e| score > e.score))
    }

    /// Add a score and return its position in the list (0-indexed)
    pub fn add_score(&mut self, name: String, score: u32, wave: u32) -> usize {
        let name_for_search = name.clone();
        let entry = HighScoreEntry { name, score, wave };
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        self.entries.truncate(MAX_HIGH_SCORES);
        self.save();
        // Find the position of the newly added score
        self.entries
            .iter()
            .position(|e| e.score == score && e.name == name_for_search)
            .unwrap_or(0)
    }
}

/// Tracks the index of a newly entered high score for highlighting
#[derive(Resource, Default)]
pub struct NewHighScoreIndex(pub Option<usize>);

/// Timer for auto-cycling screens (10 seconds)
#[derive(Resource)]
pub struct ScreenCycleTimer(pub Timer);

impl Default for ScreenCycleTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(10.0, TimerMode::Once))
    }
}

/// Current name entry state
#[derive(Resource)]
pub struct NameEntryState {
    pub chars: [char; 3],
    pub position: usize,
}

impl Default for NameEntryState {
    fn default() -> Self {
        Self {
            chars: ['A', 'A', 'A'],
            position: 0,
        }
    }
}

/// Whether the game is in attract mode (cycling between title and high scores)
#[derive(Resource)]
pub struct AttractMode {
    pub active: bool,
}

impl Default for AttractMode {
    fn default() -> Self {
        Self { active: true } // Start in attract mode
    }
}

/// Screen fade state for smooth transitions
#[derive(Resource)]
pub struct ScreenFade {
    pub alpha: f32,
    pub fading_in: bool,
    pub fading_out: bool,
    pub timer: Timer,
}

impl Default for ScreenFade {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            fading_in: true,
            fading_out: false,
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }
}

impl ScreenFade {
    pub const FADE_DURATION: f32 = 0.5;

    pub fn start_fade_in(&mut self) {
        self.fading_in = true;
        self.fading_out = false;
        self.alpha = 0.0;
        self.timer = Timer::from_seconds(Self::FADE_DURATION, TimerMode::Once);
    }

    pub fn start_fade_out(&mut self) {
        self.fading_in = false;
        self.fading_out = true;
        self.timer = Timer::from_seconds(Self::FADE_DURATION, TimerMode::Once);
    }
}

/// Tracks active power-ups for the player
#[derive(Resource, Default)]
pub struct PowerUpState {
    pub rapid_fire: bool,
    pub triple_shot: bool,
    pub speed_boost: bool,
    pub power_shot: bool,
}

impl PowerUpState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn has_any(&self) -> bool {
        self.rapid_fire || self.triple_shot || self.speed_boost || self.power_shot
    }
}

/// Quit confirmation dialog state
#[derive(Resource, Default)]
pub struct QuitConfirmation {
    pub showing: bool,
}

/// CLI setting: whether music is enabled
#[derive(Resource)]
pub struct MusicEnabled(pub bool);

/// Screen shake effect state
#[derive(Resource, Default)]
pub struct ScreenShake {
    pub trauma: f32,     // Current shake intensity (0.0 - 1.0)
    pub decay_rate: f32, // How fast trauma decays
}

impl ScreenShake {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
        self.decay_rate = 5.0; // Default decay
    }
}

/// Combo tracking for rapid kills
#[derive(Resource, Default)]
pub struct ComboTracker {
    pub count: u32,
    pub timer: f32,
}

impl ComboTracker {
    const COMBO_WINDOW: f32 = 1.0; // Seconds to maintain combo

    pub fn add_kill(&mut self) {
        self.count += 1;
        self.timer = Self::COMBO_WINDOW;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.count = 0;
            }
        }
    }

    pub fn current(&self) -> u32 {
        self.count.max(1)
    }
}

/// Screen flash effect for power-up collection
#[derive(Resource, Default)]
pub struct ScreenFlash {
    pub timer: f32,
    pub duration: f32,
    pub color: Color,
}

impl ScreenFlash {
    pub fn trigger(&mut self, duration: f32, color: Color) {
        let duration = duration.max(0.001);
        self.timer = duration;
        self.duration = duration;
        self.color = color;
    }

    pub fn strength(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

/// Delayed player respawn timer
#[derive(Resource, Default)]
pub struct PlayerRespawnTimer {
    pub timer: Option<Timer>,
}

impl PlayerRespawnTimer {
    pub const RESPAWN_DELAY: f32 = 2.0; // Seconds before respawn

    pub fn start(&mut self) {
        self.timer = Some(Timer::from_seconds(Self::RESPAWN_DELAY, TimerMode::Once));
    }

    pub fn tick(&mut self, dt: std::time::Duration) -> bool {
        if let Some(ref mut timer) = self.timer {
            timer.tick(dt);
            if timer.just_finished() {
                self.timer = None;
                return true; // Ready to respawn
            }
        }
        false
    }
}

/// Boost power-up drop rate after player respawns
#[derive(Resource, Default)]
pub struct PowerUpDropBoost {
    pub timer: Option<Timer>,
}

impl PowerUpDropBoost {
    pub const BOOST_DURATION: f32 = 12.0; // Seconds of boosted drop rate
    pub const BOOST_MULTIPLIER: f32 = 4.0; // 4x drop rate during boost

    pub fn activate(&mut self) {
        self.timer = Some(Timer::from_seconds(Self::BOOST_DURATION, TimerMode::Once));
    }

    pub fn tick(&mut self, dt: std::time::Duration) {
        if let Some(ref mut timer) = self.timer {
            timer.tick(dt);
            if timer.just_finished() {
                self.timer = None;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer.is_some()
    }

    pub fn drop_chance(&self) -> f32 {
        if self.is_active() {
            crate::space_shooter::config::POWERUP_DROP_CHANCE * Self::BOOST_MULTIPLIER
        } else {
            crate::space_shooter::config::POWERUP_DROP_CHANCE
        }
    }
}

/// Firing exhaustion system - continuous fire becomes less effective over time
#[derive(Resource)]
pub struct FiringExhaustion {
    /// Current exhaustion level (0.0 = fresh, 1.0 = fully exhausted)
    pub level: f32,
    /// Whether player is currently firing
    pub is_firing: bool,
}

impl Default for FiringExhaustion {
    fn default() -> Self {
        Self {
            level: 0.0,
            is_firing: false,
        }
    }
}

impl FiringExhaustion {
    /// Rate at which exhaustion builds up per second while firing
    pub const EXHAUST_RATE: f32 = 0.25;
    /// Rate at which exhaustion recovers per second when not firing
    pub const RECOVERY_RATE: f32 = 0.5;
    /// Exhaustion threshold where slowdown begins (30%)
    pub const SLOWDOWN_THRESHOLD: f32 = 0.3;
    /// Maximum fire rate penalty at full exhaustion (3x slower)
    pub const MAX_PENALTY: f32 = 3.0;

    /// Update exhaustion based on firing state
    pub fn update(&mut self, dt: f32, firing: bool) {
        self.is_firing = firing;
        if firing {
            self.level = (self.level + Self::EXHAUST_RATE * dt).min(1.0);
        } else {
            self.level = (self.level - Self::RECOVERY_RATE * dt).max(0.0);
        }
    }

    /// Get fire rate multiplier (1.0 = normal, higher = slower)
    pub fn fire_rate_multiplier(&self) -> f32 {
        if self.level <= Self::SLOWDOWN_THRESHOLD {
            1.0
        } else {
            // Linear interpolation from 1.0 to MAX_PENALTY
            let t = (self.level - Self::SLOWDOWN_THRESHOLD) / (1.0 - Self::SLOWDOWN_THRESHOLD);
            1.0 + t * (Self::MAX_PENALTY - 1.0)
        }
    }

    /// Get display percentage (0.0 to 1.0, inverted for "energy remaining")
    pub fn energy_remaining(&self) -> f32 {
        1.0 - self.level
    }
}
