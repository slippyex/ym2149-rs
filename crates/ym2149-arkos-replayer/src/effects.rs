//! Effect types and handling for Arkos Tracker
//!
//! Effects modify playback behavior. Some are "immediate" (applied when read),
//! others are "trailing" (continuously applied every tick).

use crate::fixed_point::FixedPoint;

/// Effect types in Arkos Tracker 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    /// No effect
    None,
    /// Set volume (0-15)
    Volume,
    /// Volume slide in (fade in)
    VolumeIn,
    /// Volume slide out (fade out)
    VolumeOut,
    /// Pitch slide up (lower period = higher pitch)
    PitchUp,
    /// Fast pitch slide up
    FastPitchUp,
    /// Pitch slide down (higher period = lower pitch)
    PitchDown,
    /// Fast pitch slide down
    FastPitchDown,
    /// Pitch glide to new note
    PitchGlide,
    /// Use pitch table
    PitchTable,
    /// Reset effects
    Reset,
    /// Force instrument speed
    ForceInstrumentSpeed,
    /// Force arpeggio speed
    ForceArpeggioSpeed,
    /// Force pitch table speed
    ForcePitchTableSpeed,
    /// 3-note arpeggio (inline)
    Arpeggio3Notes,
    /// 4-note arpeggio (inline)
    Arpeggio4Notes,
    /// Use arpeggio table
    ArpeggioTable,
    /// Legato (don't retrigger instrument)
    Legato,
}

impl EffectType {
    /// Parse from effect name string
    pub fn from_name(name: &str) -> Self {
        match name {
            "volume" => Self::Volume,
            "volumeIn" => Self::VolumeIn,
            "volumeOut" => Self::VolumeOut,
            "pitchUp" => Self::PitchUp,
            "fastPitchUp" => Self::FastPitchUp,
            "pitchDown" => Self::PitchDown,
            "fastPitchDown" => Self::FastPitchDown,
            "pitchGlide" => Self::PitchGlide,
            "pitchTable" => Self::PitchTable,
            "reset" => Self::Reset,
            "forceInstrumentSpeed" => Self::ForceInstrumentSpeed,
            "forceArpeggioSpeed" => Self::ForceArpeggioSpeed,
            "forcePitchTableSpeed" => Self::ForcePitchTableSpeed,
            "arpeggio3Notes" => Self::Arpeggio3Notes,
            "arpeggio4Notes" => Self::Arpeggio4Notes,
            "arpeggioTable" => Self::ArpeggioTable,
            "legato" => Self::Legato,
            _ => Self::None,
        }
    }

    /// Number of hex digits used to encode this effect in legacy formats
    pub fn digit_count(self) -> u8 {
        match self {
            EffectType::None => 0,
            EffectType::Volume | EffectType::Reset => 1,
            EffectType::PitchTable
            | EffectType::ArpeggioTable
            | EffectType::Arpeggio3Notes
            | EffectType::ForceArpeggioSpeed
            | EffectType::ForcePitchTableSpeed
            | EffectType::ForceInstrumentSpeed
            | EffectType::Legato => 2,
            EffectType::VolumeIn
            | EffectType::VolumeOut
            | EffectType::PitchUp
            | EffectType::PitchDown
            | EffectType::PitchGlide
            | EffectType::FastPitchUp
            | EffectType::FastPitchDown
            | EffectType::Arpeggio4Notes => 3,
        }
    }

    /// Convert a legacy raw hex value into the logical value used by AT3
    pub fn decode_legacy_value(self, raw_value: i32) -> i32 {
        match self.digit_count() {
            0 => 0,
            1 => (raw_value >> 8) & 0xF,
            2 => (raw_value >> 4) & 0xFF,
            _ => raw_value & 0xFFF,
        }
    }
}

/// State for volume slide effects
#[derive(Debug, Clone, Default)]
pub struct VolumeSlide {
    /// Current track volume (0-15)
    pub current: FixedPoint,
    /// Slide amount per tick
    pub slide: FixedPoint,
}

impl VolumeSlide {
    /// Create with initial volume
    pub fn new(initial_volume: u8) -> Self {
        Self {
            current: FixedPoint::from_int(initial_volume as i32),
            slide: FixedPoint::default(),
        }
    }

    /// Set volume directly
    pub fn set(&mut self, volume: u8) {
        self.current = FixedPoint::from_int(volume.min(15) as i32);
        self.slide.reset();
    }

    /// Start volume in (fade in)
    pub fn volume_in(&mut self, speed: u16) {
        self.slide = FixedPoint::from_digits(speed);
    }

    /// Start volume out (fade out)
    pub fn volume_out(&mut self, speed: u16) {
        self.slide = FixedPoint::from_digits(speed);
        self.slide.negate();
    }

    /// Set current volume from fixed-point value
    pub fn set_fixed(&mut self, value: FixedPoint) {
        self.current = value;
        self.current.clamp(15);
    }

    /// Apply slide for one tick
    pub fn apply_slide(&mut self) {
        self.current += self.slide;
        self.current.clamp(15);
    }

    /// Get current volume as u8
    pub fn get(&self) -> u8 {
        self.current.integer_part().clamp(0, 15) as u8
    }

    /// Reset slide
    pub fn reset_slide(&mut self) {
        self.slide.reset();
    }
}

/// State for pitch slide effects
#[derive(Debug, Clone, Default)]
pub struct PitchSlide {
    /// Current pitch offset (added to base period)
    pub current: FixedPoint,
    /// Slide amount per tick
    pub slide: FixedPoint,
}

impl PitchSlide {
    /// Start pitch up (period decreases)
    pub fn pitch_up(&mut self, speed: u16) {
        self.slide = FixedPoint::from_digits(speed);
        self.slide.negate(); // Negative = period down = pitch up
    }

    /// Start fast pitch up
    pub fn fast_pitch_up(&mut self, speed: u16) {
        let fast_speed = ((speed as u32) << 4).min(u16::MAX as u32) as u16;
        self.pitch_up(fast_speed);
    }

    /// Start pitch down (period increases)
    pub fn pitch_down(&mut self, speed: u16) {
        self.slide = FixedPoint::from_digits(speed);
    }

    /// Start fast pitch down
    pub fn fast_pitch_down(&mut self, speed: u16) {
        let fast_speed = ((speed as u32) << 4).min(u16::MAX as u32) as u16;
        self.pitch_down(fast_speed);
    }

    /// Apply slide for one tick
    pub fn apply_slide(&mut self) {
        self.current += self.slide;
    }

    /// Reset only the slide component, preserving the current offset
    pub fn reset_slide(&mut self) {
        self.slide.reset();
    }

    /// Get current pitch as i16
    pub fn get(&self) -> i16 {
        self.current.integer_part() as i16
    }

    /// Reset
    pub fn reset(&mut self) {
        self.current.reset();
        self.slide.reset();
    }
}

/// State for glide effect
#[derive(Debug, Clone, Default)]
pub struct GlideState {
    /// Whether glide is active
    pub active: bool,
    /// Glide speed (per tick)
    pub speed: FixedPoint,
    /// Initial period at glide start
    pub initial_period: u16,
    /// Goal period to reach
    pub goal_period: u16,
    /// Final pitch value when goal is reached
    pub final_pitch: i16,
    /// Direction: true if period is increasing (pitch down)
    pub period_increasing: bool,
}

impl GlideState {
    /// Start a new glide
    pub fn start(&mut self, current_period: u16, goal_period: u16, current_pitch: i16) {
        self.active = true;
        self.initial_period = current_period;
        self.goal_period = goal_period;
        let delta = goal_period as i32 - current_period as i32;
        self.final_pitch = delta.clamp(i16::MIN as i32, i16::MAX as i32) as i16;

        let actual_period = current_period as i32 + current_pitch as i32;
        self.period_increasing = (goal_period as i32) >= actual_period;
    }

    /// Stop glide
    pub fn stop(&mut self) {
        self.active = false;
        self.speed.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_type_from_name() {
        assert_eq!(EffectType::from_name("volume"), EffectType::Volume);
        assert_eq!(EffectType::from_name("pitchUp"), EffectType::PitchUp);
        assert_eq!(EffectType::from_name("unknown"), EffectType::None);
    }

    #[test]
    fn test_volume_slide() {
        let mut vol = VolumeSlide::new(10);
        assert_eq!(vol.get(), 10);

        vol.volume_in(0x0110); // +1.0625 per tick (1 + 0x10/256)
        vol.apply_slide();
        assert_eq!(vol.get(), 11);

        vol.volume_out(0x0200); // -2 per tick
        vol.apply_slide();
        assert_eq!(vol.get(), 9);
    }

    #[test]
    fn test_pitch_slide_up() {
        let mut pitch = PitchSlide::default();
        pitch.pitch_up(0x0100); // -1 per tick
        pitch.apply_slide();
        assert_eq!(pitch.get(), -1);
    }
}
