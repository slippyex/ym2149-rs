//! Channel Player - Plays one channel with complete effect handling
//!
//! This is a 1:1 port of the C++ ChannelPlayer class. Each channel has its own
//! player that manages notes, instruments, effects, arpeggios, pitch tables, etc.
//!
//! Key responsibilities:
//! - Read cells from tracks and apply note/instrument/effects
//! - Manage instrument playback (envelope advancement)
//! - Handle all track effects (volume, pitch, arpeggio, etc.)
//! - Calculate final PSG periods and volumes
//!
//! The channel player is completely self-contained and doesn't know about
//! the song structure - it only receives cells and produces PSG parameters.

use std::sync::Arc;

use crate::effect_context::LineContext;
use crate::effects::{EffectType, GlideState, PitchSlide, VolumeSlide};
use crate::expression::{
    ExpressionReader, InlineArpeggio, get_arpeggio_metadata, get_pitch_metadata,
    read_arpeggio_value, read_pitch_value,
};
use crate::fixed_point::FixedPoint;
use crate::format::{AksSong, Cell, ChannelLink, InstrumentType, Note, SongFormat};
use crate::psg;

/// Output from channel player (PSG parameters for one channel)
#[derive(Debug, Clone, Default)]
pub struct ChannelOutput {
    /// Volume (0-15 for software, 16 for hardware envelope)
    pub volume: u8,
    /// Noise period (0-31, 0 = no noise)
    pub noise: u8,
    /// Whether tone channel is open (audible)
    pub sound_open: bool,
    /// Software period (tone frequency)
    pub software_period: u16,
    /// Hardware envelope period
    pub hardware_period: u16,
    /// Hardware envelope shape (0-15)
    pub hardware_envelope: u8,
    /// Whether to retrigger hardware envelope
    pub hardware_retrig: bool,
}

/// Sample playback parameters emitted by a channel
#[derive(Debug, Clone)]
pub struct SamplePlaybackParams {
    /// PCM data in -1.0..1.0 range
    pub data: Arc<Vec<f32>>,
    /// Loop start index (inclusive)
    pub loop_start: usize,
    /// Loop end index (inclusive)
    pub loop_end: usize,
    /// Whether the sample loops
    pub looping: bool,
    /// Native sample frequency
    pub sample_frequency_hz: u32,
    /// Target playback frequency
    pub pitch_hz: f32,
    /// Amplification ratio
    pub amplification: f32,
    /// Volume (0-15)
    pub volume: u8,
    /// PSG sample player frequency
    pub sample_player_frequency_hz: f32,
    /// Reference frequency in Hz
    pub reference_frequency_hz: f32,
    /// Start from beginning
    pub play_from_start: bool,
    /// Whether this playback is high priority (digidrum)
    pub high_priority: bool,
}

/// Sample command emitted by a channel for this tick
#[derive(Debug, Clone, Default)]
pub enum SampleCommand {
    /// No change
    #[default]
    None,
    /// Play or update a sample voice
    Play(SamplePlaybackParams),
    /// Stop any currently playing sample
    Stop,
}

/// Complete channel frame result (PSG + optional sample)
#[derive(Debug, Clone)]
pub struct ChannelFrame {
    /// PSG register set generated for this tick
    pub psg: ChannelOutput,
    /// Sample command emitted alongside the PSG changes
    pub sample: SampleCommand,
}

impl Default for ChannelFrame {
    fn default() -> Self {
        Self {
            psg: ChannelOutput::default(),
            sample: SampleCommand::None,
        }
    }
}

/// Complete state for one channel
pub struct ChannelPlayer {
    /// Channel index in song
    _channel_index: usize,

    /// Reference to song data
    song: Arc<AksSong>,

    // Current note state
    /// Base note from track (0-95, 255 = no note)
    base_note: Note,
    /// Track note (base + arpeggio)
    track_note: Note,
    /// Note that was just played (for new note detection)
    new_played_note: Option<Note>,

    // Volume state
    volume_slide: VolumeSlide,

    // Pitch state
    pitch_slide: PitchSlide,
    glide: GlideState,
    pitch_from_pitch_table: i16,

    // Noise
    noise: u8,
    sound_open: bool,

    // Instrument state
    current_instrument_id: Option<usize>,
    instrument_current_line: usize,
    instrument_current_tick: u8,
    instrument_cell_volume: u8,
    forced_instrument_speed: Option<u8>,

    // Hardware envelope state
    hardware_envelope: u8,
    hardware_retrig: bool,
    hardware_period: u16,
    software_period: u16,

    // Sample state
    sample_voice: Option<SampleVoiceState>,
    sample_restart_pending: bool,
    sample_was_active: bool,

    // Arpeggio state
    inline_arpeggio: InlineArpeggio,
    use_inline_arpeggio: bool,
    table_arpeggio_id: Option<usize>,
    arpeggio_reader: ExpressionReader,

    // Pitch table state
    pitch_table_id: Option<usize>,
    pitch_reader: ExpressionReader,

    // PSG frequency for this channel
    psg_frequency: f32,
    reference_frequency: f32,
    sample_player_frequency: f32,
}

impl ChannelPlayer {
    /// Create new channel player
    pub fn new(
        channel_index: usize,
        song: Arc<AksSong>,
        psg_frequency: f32,
        reference_frequency: f32,
        sample_player_frequency: f32,
    ) -> Self {
        Self {
            _channel_index: channel_index,
            song,
            base_note: 255,
            track_note: 255,
            new_played_note: None,
            volume_slide: VolumeSlide::new(15),
            pitch_slide: PitchSlide::default(),
            glide: GlideState::default(),
            pitch_from_pitch_table: 0,
            noise: 0,
            sound_open: false,
            current_instrument_id: None,
            instrument_current_line: 0,
            instrument_current_tick: 0,
            instrument_cell_volume: 0,
            forced_instrument_speed: None,
            hardware_envelope: 0,
            hardware_retrig: false,
            hardware_period: 0,
            software_period: 0,
            sample_voice: None,
            sample_restart_pending: false,
            sample_was_active: false,
            inline_arpeggio: InlineArpeggio::empty(),
            use_inline_arpeggio: false,
            table_arpeggio_id: Some(0), // Index 0 = empty arpeggio
            arpeggio_reader: ExpressionReader::new(),
            pitch_table_id: Some(0), // Index 0 = empty pitch table
            pitch_reader: ExpressionReader::new(),
            psg_frequency,
            reference_frequency,
            sample_player_frequency,
        }
    }

    /// Apply effect context for the current line
    pub fn apply_line_context(&mut self, context: &LineContext) {
        if let Some(pitch_id) = context.pitch_table()
            && self.pitch_table_id != Some(pitch_id)
        {
            self.pitch_table_id = Some(pitch_id);
            self.update_pitch_metadata(true);
            self.pitch_reader.reset();
        }

        if let Some(inline) = context.inline_arpeggio()
            && (!self.use_inline_arpeggio || self.inline_arpeggio != *inline)
        {
            self.inline_arpeggio = inline.clone();
            self.use_inline_arpeggio = true;
            self.update_arpeggio_metadata(true);
            self.arpeggio_reader.reset();
        } else if let Some(arpeggio_id) = context.arpeggio_table()
            && (self.use_inline_arpeggio || self.table_arpeggio_id != Some(arpeggio_id))
        {
            self.use_inline_arpeggio = false;
            self.table_arpeggio_id = Some(arpeggio_id);
            self.update_arpeggio_metadata(true);
            self.arpeggio_reader.reset();
        }

        self.volume_slide.set_fixed(context.volume());
    }

    /// Play one frame - this is called every tick
    ///
    /// # Arguments
    ///
    /// * `cell` - Optional cell to read (on first tick of line)
    /// * `is_first_tick` - Whether this is first tick of line
    /// * `still_within_line` - Whether we're still within speed limit (for slides)
    ///
    /// # Returns
    ///
    /// PSG output parameters for this channel
    pub fn play_frame(
        &mut self,
        cell: Option<&Cell>,
        transposition: i8,
        is_first_tick: bool,
        still_within_line: bool,
    ) -> ChannelFrame {
        self.new_played_note = None;
        let mut frame = ChannelFrame::default();

        // Read cell if provided (first tick)
        if let Some(cell) = cell
            && is_first_tick
        {
            #[cfg(test)]
            if cell.index == 38 && self._channel_index == 2 {
                println!(
                    "[dbg] channel {} reading cell idx {} note {} instrument {}",
                    self._channel_index, cell.index, cell.note, cell.instrument
                );
            }
            self.read_cell_and_set_states(cell, transposition);
        }

        // Apply trailing effects (arpeggio, pitch, volume slides)
        self.manage_trailing_effects(still_within_line);

        // Play instrument frame
        self.play_instrument_frame();

        // Build output
        let output = ChannelOutput {
            volume: self.instrument_cell_volume,
            noise: self.noise,
            sound_open: self.sound_open,
            software_period: self.software_period,
            hardware_period: self.hardware_period,
            hardware_envelope: self.hardware_envelope,
            hardware_retrig: self.hardware_retrig,
        };
        frame.psg = output;
        frame.sample = self.emit_sample_command();
        frame
    }

    /// Reset to stop sound
    pub fn stop_sound(&mut self) {
        self.current_instrument_id = None;
        self.table_arpeggio_id = Some(0);
        self.pitch_table_id = Some(0);
        self.volume_slide.set(15);
        self.reset_inline_arpeggio();
        self.noise = 0;
        self.sound_open = false;
        self.instrument_cell_volume = 0;
        self.sample_voice = None;
        self.sample_restart_pending = false;
        self.sample_was_active = false;
    }

    // ============================================================================
    // CELL READING & EFFECT PROCESSING
    // ============================================================================

    /// Read cell and update internal state
    fn read_cell_and_set_states(&mut self, cell: &Cell, transposition: i8) {
        // Check for glide effect first (special handling)
        let has_glide = self.check_for_glide_effect(cell);

        // Process note
        if cell.note != 255 {
            let note = self.apply_transposition(cell.note, transposition);
            self.new_played_note = Some(note);

            if has_glide {
                self.pitch_slide.reset_slide();
                // Glide: set goal period but don't change current note/instrument
                self.setup_glide_goal(note);
            } else {
                // Normal note: update base note and reset slides
                let note_changed = self.base_note != note;
                self.base_note = note;
                #[cfg(test)]
                if cell.index == 38 && self._channel_index == 2 {
                    println!(
                        "[dbg] channel {} base_note set to {} track_note {}",
                        self._channel_index, self.base_note, self.track_note
                    );
                }
                self.pitch_slide.reset();
                self.volume_slide.reset_slide();
                self.glide.stop();

                // If there's an instrument, start it
                if cell.instrument_present {
                    self.start_instrument(cell.instrument);
                } else if note_changed {
                    self.current_instrument_id = None;
                }
            }
        }

        // Process effects
        for effect in &cell.effects {
            self.process_effect(effect);
        }
    }

    /// Check if cell has glide effect
    fn check_for_glide_effect(&mut self, cell: &Cell) -> bool {
        for effect in &cell.effects {
            let effect_type = EffectType::from_name(&effect.name);
            if effect_type == EffectType::PitchGlide {
                let speed = effect.logical_value.clamp(0, 0xFFFF) as u16;
                // Store speed but don't activate yet (needs goal note)
                self.glide.speed = FixedPoint::from_digits(speed);
                return true;
            }
        }
        false
    }

    /// Setup glide to target note
    fn setup_glide_goal(&mut self, goal_note: Note) {
        let current_period = self.calculate_period_for_note(self.base_note);
        let goal_period = self.calculate_period_for_note(goal_note);
        let current_pitch = self.pitch_slide.get();

        self.glide.start(current_period, goal_period, current_pitch);
    }

    /// Start playing an instrument
    fn start_instrument(&mut self, instrument_index: usize) {
        if instrument_index == usize::MAX {
            self.stop_sound();
            return;
        }
        if instrument_index >= self.song.instruments.len() {
            return;
        }

        self.current_instrument_id = Some(instrument_index);
        self.instrument_current_line = 0;
        self.instrument_current_tick = 0;
        self.forced_instrument_speed = None;

        // Reset arpeggio and pitch table positions
        self.update_arpeggio_metadata(true);
        self.arpeggio_reader.reset();

        self.update_pitch_metadata(true);
        self.pitch_reader.reset();
    }

    /// Process a single effect
    fn process_effect(&mut self, effect: &crate::format::Effect) {
        let effect_type = EffectType::from_name(&effect.name);
        let raw_value = effect.logical_value.clamp(0, 0xFFFF) as u16;
        let value_u8 = raw_value.min(u8::MAX as u16) as u8;

        match effect_type {
            EffectType::Volume => {
                self.volume_slide.set(value_u8.min(15));
            }
            EffectType::VolumeIn => {
                self.volume_slide.volume_in(raw_value);
            }
            EffectType::VolumeOut => {
                self.volume_slide.volume_out(raw_value);
            }
            EffectType::PitchUp => {
                self.pitch_slide.pitch_up(raw_value);
                self.glide.stop();
            }
            EffectType::FastPitchUp => {
                self.pitch_slide.fast_pitch_up(raw_value);
                self.glide.stop();
            }
            EffectType::PitchDown => {
                self.pitch_slide.pitch_down(raw_value);
                self.glide.stop();
            }
            EffectType::FastPitchDown => {
                self.pitch_slide.fast_pitch_down(raw_value);
                self.glide.stop();
            }
            EffectType::PitchTable => {
                self.pitch_table_id = Some(value_u8 as usize);
                self.update_pitch_metadata(true);
                self.pitch_reader.reset();
            }
            EffectType::Reset => {
                self.reset_effects(value_u8);
            }
            EffectType::ForceInstrumentSpeed => {
                self.forced_instrument_speed = Some(value_u8);
            }
            EffectType::ForceArpeggioSpeed => {
                self.arpeggio_reader.set_forced_speed(Some(value_u8));
            }
            EffectType::ForcePitchTableSpeed => {
                self.pitch_reader.set_forced_speed(Some(value_u8));
            }
            EffectType::Arpeggio3Notes => {
                self.inline_arpeggio = InlineArpeggio::from_3_notes(value_u8);
                self.use_inline_arpeggio = true;
                self.arpeggio_reader.set_forced_speed(None);
                self.arpeggio_reader.reset();
            }
            EffectType::Arpeggio4Notes => {
                self.inline_arpeggio = InlineArpeggio::from_4_notes(value_u8);
                self.use_inline_arpeggio = true;
                self.arpeggio_reader.set_forced_speed(None);
                self.arpeggio_reader.reset();
            }
            EffectType::ArpeggioTable => {
                self.use_inline_arpeggio = false;
                self.table_arpeggio_id = Some(value_u8 as usize);
                self.update_arpeggio_metadata(true);
                self.arpeggio_reader.reset();
            }
            EffectType::PitchGlide => {
                // Already handled in check_for_glide_effect
            }
            EffectType::Legato => {
                // TODO: Implement legato (don't retrigger instrument)
            }
            EffectType::None => {}
        }
    }

    /// Reset all effects
    fn reset_effects(&mut self, inverted_volume: u8) {
        let volume = 15 - (inverted_volume & 0x0F);
        self.volume_slide.set(volume);
        self.pitch_slide.reset();
        self.reset_inline_arpeggio();
        self.use_inline_arpeggio = false;
        self.table_arpeggio_id = Some(0);
        self.pitch_table_id = Some(0);
    }

    fn reset_inline_arpeggio(&mut self) {
        self.inline_arpeggio.clear();
        self.inline_arpeggio.set_speed(0);
        self.arpeggio_reader.reset();
    }

    // ============================================================================
    // TRAILING EFFECTS (applied every tick)
    // ============================================================================

    /// Apply continuous effects
    fn manage_trailing_effects(&mut self, still_within_line: bool) {
        // Read arpeggio first (defines track note)
        let arpeggio_offset = self.read_arpeggio_and_advance();
        self.track_note = if self.base_note == 255 {
            255
        } else {
            self.wrap_note((self.base_note as i32) + arpeggio_offset as i32)
        };

        let legacy_song = self.song.format == SongFormat::Legacy;
        let apply_pitch_slide = legacy_song || self.new_played_note.is_none();

        // Apply slides if still within line
        if still_within_line {
            if self.glide.active {
                self.manage_trailing_glide();
            } else if apply_pitch_slide {
                self.pitch_slide.apply_slide();
            }
            self.volume_slide.apply_slide();
        }

        // Read pitch table
        self.pitch_from_pitch_table = self.read_pitch_table_and_advance();
    }

    /// Apply glide effect
    fn manage_trailing_glide(&mut self) {
        // Apply glide slide first (matches C++ ordering)
        if self.glide.period_increasing {
            self.pitch_slide.current += self.glide.speed;
        } else {
            self.pitch_slide.current -= self.glide.speed;
        }

        let current_period =
            (self.glide.initial_period as i32 + self.pitch_slide.current.integer_part()) as u16;

        if current_period == self.glide.goal_period {
            self.finish_glide();
        } else if self.glide.goal_period > current_period {
            if !self.glide.period_increasing {
                self.finish_glide();
            }
        } else if self.glide.period_increasing {
            self.finish_glide();
        }
    }

    fn finish_glide(&mut self) {
        self.pitch_slide.current = FixedPoint::from_int(self.glide.final_pitch as i32);
        self.pitch_slide.reset_slide();
        self.glide.stop();
    }

    // ============================================================================
    // ARPEGGIO & PITCH TABLE
    // ============================================================================

    /// Read current arpeggio value and advance
    fn read_arpeggio_and_advance(&mut self) -> i8 {
        self.update_arpeggio_metadata(false);

        // Clamp index to valid range
        let end_index = self.arpeggio_reader.end;
        if self.arpeggio_reader.current_index() > end_index {
            self.arpeggio_reader.set_current_index(0);
        }

        // Read value
        let value = self.read_arpeggio_value();

        // Advance
        self.arpeggio_reader.advance();

        value
    }

    /// Read current arpeggio value
    fn read_arpeggio_value(&self) -> i8 {
        let index = self.arpeggio_reader.current_index();

        if self.use_inline_arpeggio {
            self.inline_arpeggio.get(index)
        } else if let Some(arp_id) = self.table_arpeggio_id {
            read_arpeggio_value(&self.song, arp_id, index)
        } else {
            0
        }
    }

    /// Update arpeggio metadata from song
    fn update_arpeggio_metadata(&mut self, reset_speed: bool) {
        if reset_speed {
            self.arpeggio_reader.set_forced_speed(None);
        }

        let (speed, loop_start, end) = if self.use_inline_arpeggio {
            (
                self.inline_arpeggio.speed(),
                self.inline_arpeggio.loop_start(),
                self.inline_arpeggio.end(),
            )
        } else if let Some(arp_id) = self.table_arpeggio_id {
            get_arpeggio_metadata(&self.song, arp_id)
        } else {
            (0, 0, 0)
        };

        self.arpeggio_reader.update_metadata(speed, loop_start, end);
    }

    /// Read current pitch table value and advance
    fn read_pitch_table_and_advance(&mut self) -> i16 {
        self.update_pitch_metadata(false);

        // Clamp index to valid range
        let end_index = self.pitch_reader.end;
        if self.pitch_reader.current_index() > end_index {
            self.pitch_reader.set_current_index(0);
        }

        // Read value
        let value = self.read_pitch_value();

        // Advance
        self.pitch_reader.advance();

        value
    }

    /// Read current pitch value
    fn read_pitch_value(&self) -> i16 {
        if let Some(pitch_id) = self.pitch_table_id {
            read_pitch_value(&self.song, pitch_id, self.pitch_reader.current_index())
        } else {
            0
        }
    }

    /// Update pitch metadata from song
    fn update_pitch_metadata(&mut self, reset_speed: bool) {
        if reset_speed {
            self.pitch_reader.set_forced_speed(None);
        }

        if let Some(pitch_id) = self.pitch_table_id {
            let (speed, loop_start, end) = get_pitch_metadata(&self.song, pitch_id);
            self.pitch_reader.update_metadata(speed, loop_start, end);
        }
    }

    // ============================================================================
    // INSTRUMENT PLAYBACK
    // ============================================================================

    /// Play current instrument frame
    fn play_instrument_frame(&mut self) {
        let Some(inst_id) = self.current_instrument_id else {
            self.noise = 0;
            self.sound_open = false;
            self.instrument_cell_volume = 0;
            self.sample_voice = None;
            return;
        };

        if inst_id >= self.song.instruments.len() {
            return;
        }

        match self.song.instruments[inst_id].instrument_type {
            InstrumentType::Psg => {
                self.play_psg_instrument_frame(inst_id);
                self.sample_voice = None;
            }
            InstrumentType::Digi => {
                self.play_sample_instrument_frame(inst_id);
            }
        }
    }

    fn play_psg_instrument_frame(&mut self, inst_id: usize) {
        let instrument = &self.song.instruments[inst_id];

        if self.instrument_current_line >= instrument.cells.len() {
            return;
        }

        let cell = instrument.cells[self.instrument_current_line].clone();
        let instrument_speed = instrument.speed;
        let instrument_is_retrig = instrument.is_retrig;
        let instrument_is_looping = instrument.is_looping;
        let instrument_loop_start = instrument.loop_start_index;
        let instrument_end = instrument.end_index;

        self.hardware_retrig = self.new_played_note.is_some()
            && self.instrument_current_tick == 0
            && instrument_is_retrig;

        self.noise = cell.noise;
        self.sound_open = !matches!(
            cell.link,
            ChannelLink::HardwareOnly | ChannelLink::NoSoftwareNoHardware
        );

        match cell.link {
            ChannelLink::NoSoftwareNoHardware => {
                // Matches Arkos: keep last periods, only update envelope volume.
                self.instrument_cell_volume = cell.volume;
            }
            ChannelLink::SoftwareOnly => {
                self.instrument_cell_volume = cell.volume;
            }
            ChannelLink::HardwareOnly
            | ChannelLink::SoftwareAndHardware
            | ChannelLink::SoftwareToHardware
            | ChannelLink::HardwareToSoftware => {
                self.instrument_cell_volume = 16;
                self.hardware_envelope = cell.hardware_envelope;

                if !self.hardware_retrig {
                    self.hardware_retrig = cell.is_retrig && self.instrument_current_tick == 0;
                }
            }
        }

        self.calculate_periods_for_link_mode(&cell);

        if self.instrument_cell_volume <= 15 {
            let inst_vol = self.instrument_cell_volume as i16;
            let track_vol = self.volume_slide.get() as i16;
            let combined = inst_vol - (15 - track_vol);
            self.instrument_cell_volume = combined.clamp(0, 15) as u8;
        }

        let speed = self.forced_instrument_speed.unwrap_or(instrument_speed);

        self.instrument_current_tick += 1;
        if self.instrument_current_tick > speed {
            self.instrument_current_tick = 0;
            self.instrument_current_line += 1;

            if self.instrument_current_line > instrument_end {
                if instrument_is_looping {
                    self.instrument_current_line = instrument_loop_start;
                } else {
                    self.instrument_current_line = 0;
                    self.current_instrument_id = None;
                }
            }
        }
    }

    /// Calculate software and hardware periods based on link mode
    fn calculate_periods_for_link_mode(&mut self, cell: &crate::format::InstrumentCell) {
        use ChannelLink::*;

        match cell.link {
            NoSoftwareNoHardware => {
                // Arkos keeps the previous periods to avoid register glitches.
            }

            SoftwareOnly | SoftwareAndHardware | SoftwareToHardware => {
                // Calculate software period
                self.software_period = self.calculate_software_period(cell);
                #[cfg(test)]
                if self._channel_index == 2 && self.software_period == 0 {
                    println!(
                        "[dbg] channel {} computed period 0 for Software link {:?}, note {} base {} track {}",
                        self._channel_index,
                        cell.link,
                        self.track_note,
                        self.base_note,
                        self.track_note
                    );
                }

                // For SoftToHard, derive hardware from software
                if matches!(cell.link, SoftwareToHardware) {
                    let ratio = cell.ratio;
                    let mut hw_period = self.software_period >> ratio;

                    // Round up if needed
                    if ratio > 0 && ((self.software_period >> (ratio - 1)) & 1) != 0 {
                        hw_period += 1;
                    }

                    // Apply hardware desync pitch
                    self.hardware_period =
                        (hw_period as i32 - cell.secondary_pitch as i32).clamp(0, 0xFFFF) as u16;
                } else if matches!(cell.link, SoftwareAndHardware) {
                    // For SoftAndHard, calculate hardware period independently
                    self.hardware_period = self.calculate_hardware_period(cell);
                }
            }

            HardwareOnly | HardwareToSoftware => {
                // Calculate hardware period
                self.hardware_period = self.calculate_hardware_period(cell);

                // For HardToSoft, derive software from hardware
                if matches!(cell.link, HardwareToSoftware) {
                    let ratio = cell.ratio;
                    let sw_period = self.hardware_period << ratio;

                    // Apply software desync pitch
                    self.software_period =
                        (sw_period as i32 - cell.primary_pitch as i32).clamp(0, 0xFFF) as u16;
                }
            }
        }
    }

    fn play_sample_instrument_frame(&mut self, inst_id: usize) {
        let instrument = &self.song.instruments[inst_id];
        let Some(sample) = instrument.sample.as_ref() else {
            self.sample_voice = None;
            return;
        };

        if self.track_note == 255 {
            self.sample_voice = None;
            return;
        }

        self.noise = 0;
        self.sound_open = false;
        self.instrument_cell_volume = 0;

        let restart = self.new_played_note.is_some();
        let mut pitch_hz = self.calculate_frequency_for_note(self.track_note);
        let signed_pitch = self.pitch_slide.get() as f32;
        pitch_hz = (pitch_hz - signed_pitch).max(0.0);

        let voice = SampleVoiceState {
            data: Arc::clone(&sample.data),
            loop_start: sample.loop_start_index,
            loop_end: sample.end_index,
            looping: sample.is_looping,
            sample_frequency_hz: sample.frequency_hz,
            amplification: sample.amplification_ratio,
            pitch_hz,
            volume_4bits: self.volume_slide.get().min(15),
            reference_frequency_hz: self.reference_frequency,
            sample_player_frequency_hz: self.sample_player_frequency,
            high_priority: false,
        };

        self.sample_voice = Some(voice);
        if restart {
            self.sample_restart_pending = true;
        }
    }

    fn emit_sample_command(&mut self) -> SampleCommand {
        if let Some(voice) = self.sample_voice.as_ref() {
            if voice.pitch_hz <= 0.0 || voice.data.is_empty() {
                self.sample_voice = None;
            } else {
                let params = voice.to_params(self.sample_restart_pending);
                self.sample_restart_pending = false;
                self.sample_was_active = true;
                return SampleCommand::Play(params);
            }
        }

        self.sample_restart_pending = false;
        if self.sample_was_active {
            self.sample_was_active = false;
            SampleCommand::Stop
        } else {
            SampleCommand::None
        }
    }

    /// Calculate software period from cell
    fn calculate_software_period(&self, cell: &crate::format::InstrumentCell) -> u16 {
        // Check for forced period
        if cell.primary_period != 0 {
            return cell.primary_period.max(0) as u16;
        }

        // Calculate from note
        let mut note = self.track_note as i32;
        note += cell.primary_arpeggio_note_in_octave as i32;
        note += (cell.primary_arpeggio_octave as i32) * 12;
        note = self.wrap_note(note) as i32;

        let raw_period = psg::calculate_period(
            self.psg_frequency as f64,
            self.reference_frequency as f64,
            note as u8,
        );
        let mut period = raw_period as i32;

        // Add pitches (track pitch is ADDED, others are SUBTRACTED)
        period += self.pitch_slide.get() as i32;
        period -= self.pitch_from_pitch_table as i32;
        period -= cell.primary_pitch as i32;

        period.clamp(0, 0xFFF) as u16
    }

    /// Calculate hardware period from cell
    fn calculate_hardware_period(&self, cell: &crate::format::InstrumentCell) -> u16 {
        // Check for forced period
        if cell.secondary_period != 0 {
            return cell.secondary_period.max(0) as u16;
        }

        // Calculate from note
        let mut note = self.track_note as i32;
        note += cell.secondary_arpeggio_note_in_octave as i32;
        note += (cell.secondary_arpeggio_octave as i32) * 12;
        note = self.wrap_note(note) as i32;

        let raw_period = psg::calculate_period(
            self.psg_frequency as f64,
            self.reference_frequency as f64,
            note as u8,
        );
        let mut period = raw_period as i32;

        // Add pitches
        period += self.pitch_slide.get() as i32;
        period -= self.pitch_from_pitch_table as i32;
        period -= cell.secondary_pitch as i32;

        period.clamp(0, 0xFFFF) as u16
    }

    // ============================================================================
    // HELPERS
    // ============================================================================

    /// Calculate PSG period for a note using shared PSG helper (software range)
    fn calculate_period_for_note(&self, note: Note) -> u16 {
        let raw = psg::calculate_period(
            self.psg_frequency as f64,
            self.reference_frequency as f64,
            note,
        );
        #[cfg(test)]
        if self._channel_index == 2 && note == 255 {
            println!(
                "[dbg] channel {} calculate_period note {} base {} track {}",
                self._channel_index, note, self.base_note, self.track_note
            );
        }
        raw.min(0x0FFF)
    }

    fn calculate_frequency_for_note(&self, note: Note) -> f32 {
        if note == 255 {
            return 0.0;
        }

        const START_OCTAVE: i32 = -3;
        const NOTES_IN_OCTAVE: i32 = 12;

        let octave = (note as i32 / NOTES_IN_OCTAVE) + START_OCTAVE;
        let note_in_octave = (note as i32 % NOTES_IN_OCTAVE) + 1;

        ((self.reference_frequency as f64)
            * 2.0_f64.powf((octave as f64) + ((note_in_octave as f64 - 10.0) / 12.0)))
            as f32
    }

    /// Clamp note to Arkos Tracker range (0-127)
    fn wrap_note(&self, note: i32) -> u8 {
        psg::clamp_note(note)
    }

    fn apply_transposition(&self, note: Note, transposition: i8) -> Note {
        if note == 255 {
            return note;
        }

        let adjusted = note as i32 + transposition as i32;
        self.wrap_note(adjusted) as Note
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct ChannelDebugState {
    pub instrument_id: Option<usize>,
    pub instrument_line: usize,
    pub instrument_tick: u8,
    pub noise: u8,
    pub volume: u8,
    pub sound_open: bool,
    pub base_note: Note,
    pub track_note: Note,
    pub pitch_from_pitch_table: i16,
    pub track_pitch: i16,
    pub volume_slide: u8,
    pub use_inline_arpeggio: bool,
    pub table_arpeggio_id: Option<usize>,
    pub inline_arpeggio_len: usize,
    pub arpeggio_index: usize,
    pub pitch_table_id: Option<usize>,
}

#[cfg(test)]
impl ChannelPlayer {
    pub(crate) fn debug_state(&self) -> ChannelDebugState {
        ChannelDebugState {
            instrument_id: self.current_instrument_id,
            instrument_line: self.instrument_current_line,
            instrument_tick: self.instrument_current_tick,
            noise: self.noise,
            volume: self.instrument_cell_volume,
            sound_open: self.sound_open,
            base_note: self.base_note,
            track_note: self.track_note,
            pitch_from_pitch_table: self.pitch_from_pitch_table,
            track_pitch: self.pitch_slide.get(),
            volume_slide: self.volume_slide.get(),
            use_inline_arpeggio: self.use_inline_arpeggio,
            table_arpeggio_id: self.table_arpeggio_id,
            inline_arpeggio_len: self.inline_arpeggio.len(),
            arpeggio_index: self.arpeggio_reader.current_index(),
            pitch_table_id: self.pitch_table_id,
        }
    }
}

#[derive(Clone)]
struct SampleVoiceState {
    data: Arc<Vec<f32>>,
    loop_start: usize,
    loop_end: usize,
    looping: bool,
    sample_frequency_hz: u32,
    amplification: f32,
    pitch_hz: f32,
    volume_4bits: u8,
    reference_frequency_hz: f32,
    sample_player_frequency_hz: f32,
    high_priority: bool,
}

impl SampleVoiceState {
    fn to_params(&self, play_from_start: bool) -> SamplePlaybackParams {
        SamplePlaybackParams {
            data: Arc::clone(&self.data),
            loop_start: self.loop_start,
            loop_end: self.loop_end,
            looping: self.looping,
            sample_frequency_hz: self.sample_frequency_hz,
            pitch_hz: self.pitch_hz,
            amplification: self.amplification,
            volume: self.volume_4bits,
            sample_player_frequency_hz: self.sample_player_frequency_hz,
            reference_frequency_hz: self.reference_frequency_hz,
            play_from_start,
            high_priority: self.high_priority,
        }
    }
}
