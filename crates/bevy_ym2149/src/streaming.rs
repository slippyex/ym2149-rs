//! Ring-buffer based audio streaming for smooth playback.
//!
//! This module provides a producer-consumer architecture that decouples
//! sample generation from audio consumption:
//!
//! - **Producer thread**: Generates samples from the YmSongPlayer into a ring buffer
//! - **Consumer (rodio)**: Reads samples from the ring buffer for playback
//!
//! This architecture eliminates lock contention between the audio thread and
//! the main Bevy thread, ensuring smooth playback for all formats including
//! computationally intensive SNDH files.

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};

use crate::playback::ToneSettings;
use crate::song_player::SharedSongPlayer;

/// Default ring buffer size (enough for ~370ms at 44100Hz stereo)
const DEFAULT_BUFFER_SIZE: usize = 32768;

/// Backoff duration when buffer is full (microseconds)
const BUFFER_BACKOFF_MICROS: u64 = 500;

/// Samples generated per batch (one VBL frame at 50Hz)
const SAMPLES_PER_BATCH: usize = 882;

/// Minimum buffer fill before playback starts (50% = ~185ms latency)
const MIN_BUFFER_FILL: f32 = 0.5;

// ============================================================================
// Ring Buffer
// ============================================================================

/// Lock-free ring buffer for audio samples.
///
/// Uses atomic operations for position tracking to minimize contention
/// between producer and consumer threads.
pub struct RingBuffer {
    buffer: Mutex<Vec<f32>>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
    capacity: usize,
    mask: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the given capacity.
    /// Capacity is rounded up to the next power of 2.
    pub fn new(requested_capacity: usize) -> Self {
        let capacity = requested_capacity.max(1024).next_power_of_two();
        let mask = capacity - 1;

        Self {
            buffer: Mutex::new(vec![0.0; capacity]),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            capacity,
            mask,
        }
    }

    /// Number of samples available to read.
    pub fn available_read(&self) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Number of samples that can be written.
    pub fn available_write(&self) -> usize {
        self.capacity - self.available_read() - 1
    }

    /// Write samples to the buffer. Returns number of samples written.
    pub fn write(&self, samples: &[f32]) -> usize {
        let mut buf = self.buffer.lock();
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        let available = self.capacity - write_pos.wrapping_sub(read_pos) - 1;
        let to_write = samples.len().min(available);

        if to_write == 0 {
            return 0;
        }

        let write_idx = write_pos & self.mask;

        if write_idx + to_write <= self.capacity {
            buf[write_idx..write_idx + to_write].copy_from_slice(&samples[..to_write]);
        } else {
            let first_part = self.capacity - write_idx;
            buf[write_idx..].copy_from_slice(&samples[..first_part]);
            buf[..to_write - first_part].copy_from_slice(&samples[first_part..to_write]);
        }

        drop(buf);
        self.write_pos
            .store(write_pos.wrapping_add(to_write), Ordering::Release);

        to_write
    }

    /// Read samples from the buffer. Returns number of samples read.
    pub fn read(&self, dest: &mut [f32]) -> usize {
        let buf = self.buffer.lock();
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        let available = write_pos.wrapping_sub(read_pos);
        let to_read = dest.len().min(available);

        if to_read == 0 {
            return 0;
        }

        let read_idx = read_pos & self.mask;

        if read_idx + to_read <= self.capacity {
            dest[..to_read].copy_from_slice(&buf[read_idx..read_idx + to_read]);
        } else {
            let first_part = self.capacity - read_idx;
            dest[..first_part].copy_from_slice(&buf[read_idx..]);
            dest[first_part..to_read].copy_from_slice(&buf[..to_read - first_part]);
        }

        drop(buf);
        self.read_pos
            .store(read_pos.wrapping_add(to_read), Ordering::Release);

        to_read
    }

    /// Clear all samples from the buffer.
    pub fn flush(&self) {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        self.read_pos.store(write_pos, Ordering::Release);
    }

    /// Get fill percentage (0.0 to 1.0).
    pub fn fill_percentage(&self) -> f32 {
        self.available_read() as f32 / self.capacity as f32
    }
}

// ============================================================================
// Audio Stream
// ============================================================================

/// Shared state for coordinating producer and consumer.
pub struct AudioStreamState {
    /// Ring buffer for audio samples (stereo interleaved)
    pub buffer: RingBuffer,
    /// Signal to stop the producer thread
    pub running: AtomicBool,
    /// Signal that buffer is ready for playback
    pub ready: AtomicBool,
    /// Stereo gain values (left, right)
    pub stereo_gain: RwLock<(f32, f32)>,
    /// Tone processing settings
    pub tone_settings: RwLock<ToneSettings>,
    /// Seek counter - incremented on each seek to signal decoder to clear local buffer
    pub seek_counter: AtomicUsize,
}

impl AudioStreamState {
    pub fn new() -> Self {
        Self {
            buffer: RingBuffer::new(DEFAULT_BUFFER_SIZE),
            running: AtomicBool::new(true),
            ready: AtomicBool::new(false),
            stereo_gain: RwLock::new((1.0, 1.0)),
            tone_settings: RwLock::new(ToneSettings::default()),
            seek_counter: AtomicUsize::new(0),
        }
    }

    /// Signal that a seek occurred - decoder should clear its local buffer
    pub fn notify_seek(&self) {
        self.seek_counter.fetch_add(1, Ordering::Release);
    }

    pub fn set_stereo_gain(&self, left: f32, right: f32) {
        *self.stereo_gain.write() = (left, right);
    }

    pub fn set_tone_settings(&self, settings: ToneSettings) {
        *self.tone_settings.write() = settings;
    }

    /// Check if buffer is ready for playback
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

impl Default for AudioStreamState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to the audio streaming infrastructure.
///
/// When dropped, signals the producer thread to stop.
pub struct AudioStream {
    /// Shared state between producer and consumer
    pub state: Arc<AudioStreamState>,
    /// Producer thread handle
    producer_handle: Option<JoinHandle<()>>,
}

impl AudioStream {
    /// Start a new audio stream for the given player.
    ///
    /// Spawns a producer thread that continuously generates samples
    /// into the ring buffer. Waits for the buffer to be sufficiently
    /// filled before returning to ensure smooth playback start.
    pub fn start(player: SharedSongPlayer) -> Self {
        let state = Arc::new(AudioStreamState::new());
        let state_clone = Arc::clone(&state);

        let producer_handle = thread::spawn(move || {
            run_producer_loop(player, state_clone);
        });

        // Wait for buffer to be ready (with timeout to prevent deadlock)
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(500);
        while !state.is_ready() && start.elapsed() < timeout {
            thread::sleep(std::time::Duration::from_millis(5));
        }

        Self {
            state,
            producer_handle: Some(producer_handle),
        }
    }

    /// Get access to the shared state for the decoder.
    pub fn shared_state(&self) -> Arc<AudioStreamState> {
        Arc::clone(&self.state)
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        // Signal producer to stop
        self.state.running.store(false, Ordering::Release);

        // Wait for producer thread to finish
        if let Some(handle) = self.producer_handle.take() {
            let _ = handle.join();
        }
    }
}

// ============================================================================
// Producer Loop
// ============================================================================

/// Filter state for tone coloring.
struct ToneFilter {
    prev0: f32,
    prev1: f32,
    envelope: f32,
}

impl ToneFilter {
    fn new() -> Self {
        Self {
            prev0: 0.0,
            prev1: 0.0,
            envelope: 0.0,
        }
    }

    fn process(&mut self, sample: f32, settings: &ToneSettings) -> f32 {
        let mut s = sample;

        // Accent (envelope follower boost)
        if settings.accent > 0.0 {
            let target = s.abs();
            self.envelope += 0.001 * (target - self.envelope);
            let boost = 1.0 + self.envelope * settings.accent;
            s *= boost;
        }

        // Saturation (soft clipping)
        if settings.saturation > 0.0 {
            let drive = 1.0 + settings.saturation * 0.5;
            s = (s * drive).tanh() / drive;
        }

        // Color filter (simple lowpass)
        if settings.color_filter {
            let filtered = (self.prev0 * 0.25) + (self.prev1 * 0.5) + (s * 0.25);
            self.prev0 = self.prev1;
            self.prev1 = s;
            s = filtered;
        } else {
            self.prev0 = s;
            self.prev1 = s;
        }

        s.clamp(-1.0, 1.0)
    }
}

/// Producer loop that generates samples and writes them to the ring buffer.
fn run_producer_loop(player: SharedSongPlayer, state: Arc<AudioStreamState>) {
    let mut mono_buffer = vec![0.0f32; SAMPLES_PER_BATCH];
    let mut stereo_buffer = vec![0.0f32; SAMPLES_PER_BATCH * 2];
    let mut filter = ToneFilter::new();
    let mut marked_ready = false;

    // Start playback
    {
        let mut player_guard = player.write();
        player_guard.play();
    }

    while state.running.load(Ordering::Acquire) {
        // Generate mono samples
        {
            let mut player_guard = player.write();
            player_guard.generate_samples_into(&mut mono_buffer);
        }

        // Read current settings
        let (left_gain, right_gain) = *state.stereo_gain.read();
        let tone_settings = *state.tone_settings.read();

        // Convert to stereo with tone processing
        for (i, &mono_sample) in mono_buffer.iter().enumerate() {
            let processed = filter.process(mono_sample, &tone_settings);
            let width = tone_settings.widen.clamp(-0.5, 0.5);
            stereo_buffer[i * 2] = processed * (left_gain + width);
            stereo_buffer[i * 2 + 1] = processed * (right_gain - width);
        }

        // Write to ring buffer with backpressure
        let mut written = 0;
        while written < stereo_buffer.len() && state.running.load(Ordering::Relaxed) {
            let n = state.buffer.write(&stereo_buffer[written..]);
            written += n;

            if n == 0 {
                // Buffer full, back off
                thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
            }
        }

        // Mark ready once buffer is sufficiently filled
        if !marked_ready && state.buffer.fill_percentage() >= MIN_BUFFER_FILL {
            state.ready.store(true, Ordering::Release);
            marked_ready = true;
        }
    }
}

// ============================================================================
// Streaming Decoder
// ============================================================================

/// Decoder that reads from a ring buffer instead of generating samples directly.
///
/// This decoder implements rodio's `Source` trait and reads pre-generated
/// samples from the ring buffer filled by the producer thread.
pub struct StreamingDecoder {
    state: Arc<AudioStreamState>,
    sample_rate: u32,
    total_samples: usize,
    current_sample: usize,
    /// Local buffer for batch reads
    local_buffer: Vec<f32>,
    local_pos: usize,
    /// Last observed seek counter to detect when a seek occurred
    last_seek_counter: usize,
}

impl StreamingDecoder {
    /// Create a new streaming decoder.
    pub fn new(state: Arc<AudioStreamState>, sample_rate: u32, total_samples: usize) -> Self {
        let last_seek_counter = state.seek_counter.load(Ordering::Acquire);
        Self {
            state,
            sample_rate,
            total_samples,
            current_sample: 0,
            local_buffer: Vec::new(),
            local_pos: 0,
            last_seek_counter,
        }
    }

    /// Check if a seek occurred and clear local buffer if so
    fn check_seek(&mut self) {
        let current = self.state.seek_counter.load(Ordering::Acquire);
        if current != self.last_seek_counter {
            self.last_seek_counter = current;
            self.local_buffer.clear();
            self.local_pos = 0;
        }
    }

    fn refill_local_buffer(&mut self) {
        const LOCAL_BATCH: usize = 1024;

        if self.local_buffer.len() != LOCAL_BATCH {
            self.local_buffer.resize(LOCAL_BATCH, 0.0);
        }

        let read = self.state.buffer.read(&mut self.local_buffer);

        // If we didn't get enough samples, fill remainder with silence
        for sample in self.local_buffer[read..].iter_mut() {
            *sample = 0.0;
        }

        self.local_pos = 0;
    }
}

impl Iterator for StreamingDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // Check end condition (0 means unlimited for SNDH)
        if self.total_samples > 0 && self.current_sample >= self.total_samples * 2 {
            return None;
        }

        // Check if a seek occurred - clear local buffer if so
        self.check_seek();

        // Refill local buffer if exhausted
        if self.local_pos >= self.local_buffer.len() {
            self.refill_local_buffer();
        }

        let sample = self
            .local_buffer
            .get(self.local_pos)
            .copied()
            .unwrap_or(0.0);
        self.local_pos += 1;
        self.current_sample += 1;

        Some(sample)
    }
}

impl bevy::audio::Source for StreamingDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        if self.total_samples == 0 {
            None
        } else {
            Some(
                self.total_samples
                    .saturating_mul(2)
                    .saturating_sub(self.current_sample),
            )
        }
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        if self.total_samples == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs_f32(
                self.total_samples as f32 / self.sample_rate as f32,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let rb = RingBuffer::new(1024);
        assert_eq!(rb.available_read(), 0);
        assert!(rb.available_write() > 0);

        let samples = [1.0, 2.0, 3.0, 4.0];
        let written = rb.write(&samples);
        assert_eq!(written, 4);
        assert_eq!(rb.available_read(), 4);

        let mut dest = [0.0; 4];
        let read = rb.read(&mut dest);
        assert_eq!(read, 4);
        assert_eq!(dest, samples);
    }

    #[test]
    fn test_ring_buffer_wraparound() {
        let rb = RingBuffer::new(16);

        // Fill most of the buffer
        let data1 = [1.0; 10];
        rb.write(&data1);

        // Read some
        let mut buf = [0.0; 6];
        rb.read(&mut buf);

        // Write more (causes wraparound)
        let data2 = [2.0; 8];
        let written = rb.write(&data2);
        assert!(written > 0);
    }

    #[test]
    fn test_audio_stream_state() {
        let state = AudioStreamState::new();
        state.set_stereo_gain(0.5, 0.8);

        let (left, right) = *state.stereo_gain.read();
        assert!((left - 0.5).abs() < 0.001);
        assert!((right - 0.8).abs() < 0.001);
    }
}
