//! Audio device integration using rodio
//!
//! Provides playback of samples to the system audio device with proper
//! synchronization with the sample ring buffer.

use crate::Result;
use rodio::{OutputStream, Sink, Source};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Audio source that reads from the ring buffer
struct RingBufferSource {
    ring_buffer: Arc<parking_lot::Mutex<super::RingBuffer>>,
    current_pos: usize,
    sample_rate: u32,
    channels: u16,
    finished: Arc<AtomicBool>,
    /// Internal buffer for efficient batch reading (reduces lock contention)
    buffer: Vec<f32>,
    /// Current position in the internal buffer
    buffer_pos: usize,
}

impl RingBufferSource {
    fn new(
        ring_buffer: Arc<parking_lot::Mutex<super::RingBuffer>>,
        sample_rate: u32,
        channels: u16,
        finished: Arc<AtomicBool>,
    ) -> Self {
        RingBufferSource {
            ring_buffer,
            current_pos: 0,
            sample_rate,
            channels,
            finished,
            buffer: vec![0.0f32; 4096],
            buffer_pos: 4096, // Start by reading new batch
        }
    }
}

impl Source for RingBufferSource {
    fn current_frame_len(&self) -> Option<usize> {
        // Return None to indicate stream continues as long as data is available
        let buffer = self.ring_buffer.lock();
        let available = buffer.available_read();
        if available > 0 {
            Some(available)
        } else {
            Some(4096) // Return a reasonable chunk size
        }
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        // We don't know total duration upfront
        None
    }
}

impl Iterator for RingBufferSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.finished.load(Ordering::Relaxed) {
            return None;
        }

        // Check if we need to refill the internal buffer
        if self.buffer_pos >= self.buffer.len() {
            // Refill internal buffer from ring buffer (batch read)
            let mut ring_buffer = self.ring_buffer.lock();
            let read = ring_buffer.read(&mut self.buffer);
            drop(ring_buffer);

            if read > 0 {
                self.buffer_pos = 0;
            } else {
                // Ring buffer underrun - return silence to keep stream alive
                self.buffer_pos = 0;
                self.buffer.fill(0.0);
            }
        }

        // Return next sample from internal buffer
        if self.buffer_pos < self.buffer.len() {
            let sample = self.buffer[self.buffer_pos];
            self.buffer_pos += 1;
            self.current_pos += 1;
            Some(sample)
        } else {
            // Shouldn't reach here, but handle gracefully
            Some(0.0)
        }
    }
}

/// Audio playback device using rodio
pub struct AudioDevice {
    _stream: OutputStream,
    _sink: Sink,
    running: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl AudioDevice {
    /// Create a new audio device and start playback
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz (typically 44100)
    /// * `channels` - Number of audio channels (typically 1 for mono, 2 for stereo)
    /// * `ring_buffer` - Reference to the ring buffer containing samples
    ///
    /// # Returns
    /// A new AudioDevice that plays samples from the ring buffer to the system audio device.
    pub fn new(
        sample_rate: u32,
        channels: u16,
        ring_buffer: Arc<parking_lot::Mutex<super::RingBuffer>>,
    ) -> Result<Self> {
        // Create output stream
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to create audio stream: {}", e))?;

        // Create sink for playback
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        // Create finished signal for shutdown coordination
        let finished = Arc::new(AtomicBool::new(false));

        // Create the source that reads from ring buffer
        let source =
            RingBufferSource::new(ring_buffer, sample_rate, channels, Arc::clone(&finished));

        // Play the source
        sink.append(source);

        let running = Arc::new(AtomicBool::new(true));

        Ok(AudioDevice {
            _stream: stream,
            _sink: sink,
            running,
            finished,
        })
    }

    /// Pause playback
    pub fn pause(&self) {
        self._sink.pause();
    }

    /// Resume playback
    pub fn play(&self) {
        self._sink.play();
    }

    /// Check if audio device is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Wait for playback to finish (blocks until sink is empty)
    pub fn wait_for_finish(&self) {
        self._sink.sleep_until_end();
    }

    /// Signal that no more samples will be produced
    /// This allows the playback stream to properly terminate instead of playing silence forever
    pub fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }
}

impl Drop for AudioDevice {
    fn drop(&mut self) {
        // Pause on drop
        self.pause();
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::RingBuffer;

    fn try_audio_device(
        buffer_len: usize,
        sample_rate: u32,
        channels: u16,
    ) -> Option<(AudioDevice, Arc<parking_lot::Mutex<RingBuffer>>)> {
        let ring_buffer = Arc::new(parking_lot::Mutex::new(
            RingBuffer::new(buffer_len).expect("Failed to create ring buffer"),
        ));

        match AudioDevice::new(sample_rate, channels, Arc::clone(&ring_buffer)) {
            Ok(device) => Some((device, ring_buffer)),
            Err(err) => {
                eprintln!(
                    "Skipping streaming::audio_device test (audio backend unavailable): {}",
                    err
                );
                None
            }
        }
    }

    #[test]
    fn test_audio_device_creation() {
        let Some((device, _ring)) = try_audio_device(4096, 44100, 1) else {
            return;
        };

        assert!(
            device.is_running(),
            "Audio device should be running after creation"
        );
    }

    #[test]
    fn test_finish_signal() {
        let Some((device, _ring)) = try_audio_device(4096, 44100, 1) else {
            return;
        };

        // Finish signal should stop the playback stream
        device.finish();
        // Note: We can't directly test that the iterator gets the signal without
        // accessing private fields, but the finish() call should succeed
    }

    #[test]
    fn test_pause_and_play() {
        let Some((device, _ring)) = try_audio_device(4096, 44100, 1) else {
            return;
        };

        // Test pause
        device.pause();
        assert!(
            device.is_running(),
            "Device should still be marked running after pause"
        );

        // Test play
        device.play();
        assert!(
            device.is_running(),
            "Device should still be marked running after play"
        );
    }

    #[test]
    fn test_ring_buffer_source_creation() {
        let ring_buffer = Arc::new(parking_lot::Mutex::new(
            RingBuffer::new(4096).expect("Failed to create ring buffer"),
        ));
        let finished = Arc::new(AtomicBool::new(false));

        let source = RingBufferSource::new(ring_buffer, 44100, 1, finished);

        assert_eq!(source.sample_rate(), 44100);
        assert_eq!(source.channels(), 1);
        assert!(source.current_frame_len().is_some());
    }

    #[test]
    fn test_ring_buffer_source_silence_on_underrun() {
        let ring_buffer = Arc::new(parking_lot::Mutex::new(
            RingBuffer::new(4096).expect("Failed to create ring buffer"),
        ));
        let finished = Arc::new(AtomicBool::new(false));

        let mut source = RingBufferSource::new(ring_buffer, 44100, 1, finished);

        // With empty ring buffer, should return silence (0.0) instead of None
        let sample = source.next();
        assert!(
            sample.is_some(),
            "Source should return Some value on buffer underrun"
        );
        assert_eq!(
            sample.unwrap(),
            0.0,
            "Source should return silence (0.0) on underrun"
        );
    }

    #[test]
    fn test_ring_buffer_source_finished_signal() {
        let ring_buffer = Arc::new(parking_lot::Mutex::new(
            RingBuffer::new(4096).expect("Failed to create ring buffer"),
        ));
        let finished = Arc::new(AtomicBool::new(false));

        let mut source =
            RingBufferSource::new(Arc::clone(&ring_buffer), 44100, 1, Arc::clone(&finished));

        // Initially should return samples or silence
        assert!(source.next().is_some());

        // Signal finished
        finished.store(true, Ordering::Relaxed);

        // After finished signal, iterator should return None
        assert_eq!(
            source.next(),
            None,
            "Source should return None after finished signal"
        );
    }

    #[test]
    fn test_audio_device_drop_pauses() {
        let Some((device, _ring)) = try_audio_device(4096, 44100, 1) else {
            return;
        };
        let running = device.is_running();
        assert!(running, "Device should be running before drop");

        drop(device);
        // Drop should have called pause() and set running to false
        // (We can't directly verify without accessing private fields)
    }

    #[test]
    fn test_stereo_audio_device() {
        let Some((_device, _ring)) = try_audio_device(8192, 44100, 2) else {
            return;
        };
        let source = RingBufferSource::new(
            Arc::new(parking_lot::Mutex::new(
                RingBuffer::new(8192).expect("Failed to create ring buffer"),
            )),
            44100,
            2,
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(
            source.channels(),
            2,
            "Source should report 2 channels for stereo"
        );
    }

    #[test]
    fn test_various_sample_rates() {
        let sample_rates = vec![22050, 44100, 48000, 96000];
        let mut succeeded = false;

        for rate in sample_rates {
            let Some((_device, _ring)) = try_audio_device(4096, rate, 1) else {
                continue;
            };
            succeeded = true;
            let source = RingBufferSource::new(
                Arc::new(parking_lot::Mutex::new(
                    RingBuffer::new(4096).expect("Failed to create ring buffer"),
                )),
                rate,
                1,
                Arc::new(AtomicBool::new(false)),
            );
            assert_eq!(
                source.sample_rate(),
                rate,
                "Source should report correct sample rate"
            );
        }

        if !succeeded {
            eprintln!(
                "Skipping sample rate checks (audio backend unavailable for all tested rates)"
            );
        }
    }
}
