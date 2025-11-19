//! Simplified PSG register representation plus helpers to extract channel data
#![allow(missing_docs)]

use std::fmt;

pub const CHANNEL_COUNT: usize = 3;
pub const MAX_SOFTWARE_PERIOD: u16 = 0x0FFF;
pub const MAX_HARDWARE_PERIOD: u16 = 0xFFFF;
pub const MAX_NOISE: u8 = 0x1F;
pub const HARDWARE_VOLUME_VALUE: u8 = 16;
pub const DEFAULT_HARDWARE_ENVELOPE: u8 = 8;

/// Structure holding AY/YM register state for one PSG
#[derive(Clone)]
pub struct PsgRegisters {
    volumes: [u8; CHANNEL_COUNT],
    software_periods: [u16; CHANNEL_COUNT],
    sound_open: [bool; CHANNEL_COUNT],
    noise_open: [bool; CHANNEL_COUNT],
    noise: u8,
    hardware_period: u16,
    hardware_envelope: u8,
    retrig: bool,
}

impl Default for PsgRegisters {
    fn default() -> Self {
        Self {
            volumes: [0; CHANNEL_COUNT],
            software_periods: [0; CHANNEL_COUNT],
            sound_open: [false; CHANNEL_COUNT],
            noise_open: [false; CHANNEL_COUNT],
            noise: 0,
            hardware_period: 0,
            hardware_envelope: DEFAULT_HARDWARE_ENVELOPE,
            retrig: false,
        }
    }
}

impl PsgRegisters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn correct(&mut self) {
        for volume in &mut self.volumes {
            *volume = (*volume).min(HARDWARE_VOLUME_VALUE);
        }
        for period in &mut self.software_periods {
            *period = (*period).min(MAX_SOFTWARE_PERIOD);
        }
        self.noise = self.noise.min(MAX_NOISE);
        self.hardware_envelope = self.hardware_envelope.clamp(0, 15);
        if self.hardware_envelope < DEFAULT_HARDWARE_ENVELOPE {
            self.hardware_envelope = DEFAULT_HARDWARE_ENVELOPE;
        }
    }

    pub fn set_volume(&mut self, channel: usize, volume: u8) {
        self.volumes[channel] = volume.min(HARDWARE_VOLUME_VALUE);
    }

    pub fn get_volume(&self, channel: usize) -> u8 {
        self.volumes[channel]
    }

    pub fn set_software_period(&mut self, channel: usize, period: u16) {
        self.software_periods[channel] = period.min(MAX_SOFTWARE_PERIOD);
    }

    pub fn get_software_period(&self, channel: usize) -> u16 {
        self.software_periods[channel]
    }

    pub fn set_noise(&mut self, noise: u8) {
        self.noise = noise.min(MAX_NOISE);
    }

    pub fn get_noise(&self) -> u8 {
        self.noise
    }

    pub fn set_mixer_noise_state(&mut self, channel: usize, open: bool) {
        self.noise_open[channel] = open;
    }

    pub fn get_mixer_noise_state(&self, channel: usize) -> bool {
        self.noise_open[channel]
    }

    pub fn set_mixer_sound_state(&mut self, channel: usize, open: bool) {
        self.sound_open[channel] = open;
    }

    pub fn get_mixer_sound_state(&self, channel: usize) -> bool {
        self.sound_open[channel]
    }

    pub fn set_hardware_period(&mut self, period: u16) {
        self.hardware_period = period;
    }

    pub fn get_hardware_period(&self) -> u16 {
        self.hardware_period
    }

    pub fn set_hardware_envelope_and_retrig(&mut self, envelope: u8, retrig: bool) {
        let mut envelope = envelope.clamp(0, 15);
        if envelope < DEFAULT_HARDWARE_ENVELOPE {
            envelope = DEFAULT_HARDWARE_ENVELOPE;
        }
        self.hardware_envelope = envelope;
        self.retrig = retrig;
    }

    pub fn get_hardware_envelope(&self) -> u8 {
        self.hardware_envelope
    }

    pub fn is_retrig(&self) -> bool {
        self.retrig
    }
}

/// Type of sound encoded in one channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundType {
    NoSoftwareNoHardware,
    SoftwareOnly,
    HardwareOnly,
    SoftwareAndHardware,
}

impl fmt::Display for SoundType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SoundType::NoSoftwareNoHardware => "noSoftwareNoHardware",
                SoundType::SoftwareOnly => "softwareOnly",
                SoundType::HardwareOnly => "hardwareOnly",
                SoundType::SoftwareAndHardware => "softwareAndHardware",
            }
        )
    }
}

/// Flattened channel registers derived from a PSG state
#[derive(Debug, Clone)]
pub struct ChannelOutputRegisters {
    volume: u8,
    noise: u8,
    sound_enabled: bool,
    software_period: u16,
    hardware_period: u16,
    hardware_envelope: u8,
    retrig: bool,
    sound_type: SoundType,
}

impl ChannelOutputRegisters {
    pub fn new(
        volume: u8,
        noise: u8,
        sound_enabled: bool,
        software_period: u16,
        hardware_period: u16,
        hardware_envelope: u8,
        retrig: bool,
    ) -> Self {
        let sound_type = Self::find_sound_type(sound_enabled, volume);
        Self {
            volume,
            noise,
            sound_enabled,
            software_period,
            hardware_period,
            hardware_envelope,
            retrig,
            sound_type,
        }
    }

    pub fn from_registers(channel_index: usize, registers: &PsgRegisters) -> Self {
        let volume = registers.get_volume(channel_index);
        let noise = if registers.get_mixer_noise_state(channel_index) {
            registers.get_noise()
        } else {
            0
        };
        let sound_enabled = registers.get_mixer_sound_state(channel_index);
        let software_period = registers.get_software_period(channel_index);
        let hardware_period = registers.get_hardware_period();
        let hardware_envelope = registers.get_hardware_envelope();
        let retrig = registers.is_retrig();
        Self::new(
            volume,
            noise,
            sound_enabled,
            software_period,
            hardware_period,
            hardware_envelope,
            retrig,
        )
    }

    pub fn sound_type(&self) -> SoundType {
        self.sound_type
    }

    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn noise(&self) -> u8 {
        self.noise
    }

    pub fn software_period(&self) -> u16 {
        self.software_period
    }

    pub fn hardware_period(&self) -> u16 {
        self.hardware_period
    }

    pub fn hardware_envelope(&self) -> u8 {
        self.hardware_envelope
    }

    pub fn retrig(&self) -> bool {
        self.retrig
    }

    pub fn sound_enabled(&self) -> bool {
        self.sound_enabled
    }

    fn find_sound_type(sound_enabled: bool, volume: u8) -> SoundType {
        if volume < HARDWARE_VOLUME_VALUE {
            if sound_enabled {
                SoundType::SoftwareOnly
            } else {
                SoundType::NoSoftwareNoHardware
            }
        } else if sound_enabled {
            SoundType::SoftwareAndHardware
        } else {
            SoundType::HardwareOnly
        }
    }
}
