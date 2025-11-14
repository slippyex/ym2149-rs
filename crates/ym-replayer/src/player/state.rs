//! Playback State Management
//!
//! This module handles frame position tracking, loop point management,
//! and state transitions during YM file playback.

use super::{AdvanceResult, PlaybackController, PlaybackState, ym_player::Ym6PlayerGeneric};
use crate::Result;
use ym2149::Ym2149Backend;

impl<B: Ym2149Backend> Ym6PlayerGeneric<B> {
    /// Set loop frame for looping playback
    pub fn set_loop_frame(&mut self, frame: usize) {
        if let Some(tracker) = self.tracker.as_mut() {
            if tracker.total_frames == 0 {
                tracker.loop_enabled = false;
                tracker.loop_frame = 0;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = 0;
                }
                return;
            }

            if frame < tracker.total_frames {
                tracker.loop_enabled = true;
                tracker.loop_frame = frame;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = frame as u32;
                }
            } else {
                tracker.loop_enabled = false;
                tracker.loop_frame = 0;
                if let Some(info) = self.info.as_mut() {
                    info.loop_frame = 0;
                }
            }
        } else if frame < self.sequencer.frame_count() {
            self.sequencer.set_loop_point(Some(frame));
            if let Some(info) = self.info.as_mut() {
                info.loop_frame = frame as u32;
            }
        } else {
            self.sequencer.set_loop_point(None);
            if let Some(info) = self.info.as_mut() {
                info.loop_frame = 0;
            }
        }
    }

    /// Get the number of frames
    pub fn frame_count(&self) -> usize {
        if let Some(tracker) = &self.tracker {
            tracker.total_frames
        } else {
            self.sequencer.frame_count()
        }
    }

    #[allow(missing_docs)]
    pub fn samples_per_frame_value(&self) -> u32 {
        self.sequencer.samples_per_frame()
    }

    #[allow(missing_docs)]
    pub fn loop_point_value(&self) -> Option<usize> {
        if self.is_tracker_mode {
            self.tracker.as_ref().and_then(|tracker| {
                if tracker.loop_enabled {
                    Some(tracker.loop_frame)
                } else {
                    None
                }
            })
        } else {
            self.sequencer.loop_point()
        }
    }

    /// Advance frame counter and handle looping
    pub(in crate::player) fn advance_frame(&mut self) {
        match self.sequencer.advance_sample() {
            AdvanceResult::Completed => {
                self.state = PlaybackState::Stopped;
            }
            _ => {}
        }
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> usize {
        if let Some(tracker) = &self.tracker {
            if tracker.total_frames == 0 {
                0
            } else {
                tracker
                    .current_frame
                    .min(tracker.total_frames.saturating_sub(1))
            }
        } else {
            self.sequencer.current_frame()
        }
    }

    /// Get playback position as a percentage (0.0 to 1.0)
    pub fn get_playback_position(&self) -> f32 {
        if self.is_tracker_mode {
            if let Some(tracker) = &self.tracker {
                if tracker.total_frames == 0 {
                    0.0
                } else {
                    (tracker.current_frame.min(tracker.total_frames) as f32)
                        / (tracker.total_frames as f32)
                }
            } else {
                0.0
            }
        } else if self.sequencer.is_empty() {
            0.0
        } else {
            (self.sequencer.current_frame() as f32) / (self.sequencer.frame_count() as f32)
        }
    }
}

impl<B: Ym2149Backend> PlaybackController for Ym6PlayerGeneric<B> {
    fn play(&mut self) -> Result<()> {
        if self.is_tracker_mode {
            if let Some(tracker) = self.tracker.as_mut() {
                tracker.samples_until_update = 0.0;
                tracker.current_frame = tracker.current_frame.min(tracker.total_frames);
            }
            self.state = PlaybackState::Playing;
        } else if !self.sequencer.is_empty() {
            self.state = PlaybackState::Playing;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.state = PlaybackState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = PlaybackState::Stopped;
        self.sequencer.reset_position();
        self.vbl.reset();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.reset();
        }
        Ok(())
    }

    fn state(&self) -> PlaybackState {
        self.state
    }
}
