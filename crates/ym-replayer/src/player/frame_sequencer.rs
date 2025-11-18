/// Result of advancing the sequencer by one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceResult {
    /// Not enough samples accumulated to move to the next frame.
    NoFrameChange,
    /// Advanced to the next frame.
    FrameAdvanced,
    /// Reached the end and looped back to the configured loop start.
    Looped,
    /// Reached the end with no loop configured.
    Completed,
}

/// Manages YM register frames, timing, and loop state.
#[derive(Debug, Clone)]
pub struct FrameSequencer {
    frames: Vec<[u8; 16]>,
    current_frame: usize,
    samples_in_frame: u32,
    samples_per_frame: u32,
    loop_point: Option<usize>,
}

impl FrameSequencer {
    /// Create a new sequencer with no frames and the default 50Hz timing.
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            current_frame: 0,
            samples_in_frame: 0,
            samples_per_frame: 882,
            loop_point: None,
        }
    }

    /// Reset playback position to the beginning.
    pub fn reset_position(&mut self) {
        self.current_frame = 0;
        self.samples_in_frame = 0;
    }

    /// Clear all frames.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.reset_position();
        self.loop_point = None;
    }

    /// Load a new set of frames, resetting playback position.
    pub fn load_frames(&mut self, frames: Vec<[u8; 16]>) {
        self.frames = frames;
        self.reset_position();
    }

    /// Access the current frame slice.
    pub fn frames(&self) -> &[[u8; 16]] {
        &self.frames
    }

    /// Mutable access to frames (loader only).
    pub fn frames_mut(&mut self) -> &mut Vec<[u8; 16]> {
        &mut self.frames
    }

    /// Number of frames stored.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Whether any frames are available.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Current frame index (clamped to available frames).
    pub fn current_frame(&self) -> usize {
        if self.frames.is_empty() {
            0
        } else {
            self.current_frame.min(self.frames.len().saturating_sub(1))
        }
    }

    /// Get registers for the current frame.
    pub fn current_frame_regs(&self) -> Option<&[u8; 16]> {
        self.frames.get(self.current_frame)
    }

    /// Get registers for a specific frame.
    pub fn frame_at(&self, index: usize) -> Option<&[u8; 16]> {
        self.frames.get(index)
    }

    /// Samples per frame setting.
    pub fn samples_per_frame(&self) -> u32 {
        self.samples_per_frame
    }

    /// Current sample offset within the frame.
    pub fn samples_into_frame(&self) -> u32 {
        self.samples_in_frame
    }

    /// Update samples per frame (minimum 1 sample).
    pub fn set_samples_per_frame(&mut self, samples: u32) {
        self.samples_per_frame = samples.max(1);
        self.samples_in_frame = self.samples_in_frame.min(self.samples_per_frame - 1);
    }

    /// Loop point accessor.
    pub fn loop_point(&self) -> Option<usize> {
        self.loop_point
    }

    /// Set loop point if within range, otherwise disables looping.
    pub fn set_loop_point(&mut self, loop_point: Option<usize>) {
        self.loop_point = loop_point.filter(|&idx| idx < self.frames.len());
    }

    /// Advance by a single sample at the configured rate.
    pub fn advance_sample(&mut self) -> AdvanceResult {
        if self.frames.is_empty() {
            return AdvanceResult::Completed;
        }

        self.samples_in_frame += 1;
        if self.samples_in_frame < self.samples_per_frame {
            return AdvanceResult::NoFrameChange;
        }

        self.samples_in_frame = 0;

        if self.current_frame + 1 >= self.frames.len() {
            if let Some(loop_start) = self.loop_point {
                self.current_frame = loop_start;
                AdvanceResult::Looped
            } else {
                AdvanceResult::Completed
            }
        } else {
            self.current_frame += 1;
            AdvanceResult::FrameAdvanced
        }
    }

    /// Seek to a specific frame (clamped to available frames).
    pub fn seek(&mut self, frame: usize) {
        if self.frames.is_empty() {
            self.current_frame = 0;
        } else {
            self.current_frame = frame.min(self.frames.len().saturating_sub(1));
        }
        self.samples_in_frame = 0;
    }
}

impl Default for FrameSequencer {
    fn default() -> Self {
        Self::new()
    }
}
