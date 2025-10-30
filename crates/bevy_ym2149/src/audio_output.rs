//! Audio output handling for YM2149 playback

use parking_lot::Mutex;
use std::sync::Arc;
use ym2149::streaming::{AudioDevice, RingBuffer};

/// Manages audio output for a single playback entity
#[derive(Clone)]
pub struct AudioOutputDevice {
    /// Shared ring buffer for audio samples (producer and consumer both use this)
    ring_buffer: Arc<Mutex<RingBuffer>>,
    /// The rodio-based audio device
    device: Arc<Mutex<Option<AudioDevice>>>,
    /// Sample rate in Hz
    sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    channels: u16,
    /// Whether the device has been started
    started: Arc<Mutex<bool>>,
}

// Ensure AudioOutputDevice is Send + Sync
unsafe impl Send for AudioOutputDevice {}
unsafe impl Sync for AudioOutputDevice {}

impl AudioOutputDevice {
    /// Create a new audio output device with a pre-created ring buffer
    pub fn new_with_buffer(
        sample_rate: u32,
        channels: u16,
        ring_buffer: Arc<Mutex<RingBuffer>>,
    ) -> Self {
        Self {
            ring_buffer,
            device: Arc::new(Mutex::new(None)),
            sample_rate,
            channels,
            started: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new audio output device
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self, String> {
        // Create ring buffer for audio samples
        // Use a larger buffer size for smooth playback: sample_rate / 4 gives ~250ms buffer at 44.1kHz
        // This provides enough headroom to handle timing variations and prevent underruns
        let buffer_size = (sample_rate / 4) as usize;
        let ring_buffer = RingBuffer::new(buffer_size)
            .map_err(|e| format!("Failed to create ring buffer: {}", e))?;

        let ring_buffer = Arc::new(Mutex::new(ring_buffer));

        Ok(Self::new_with_buffer(sample_rate, channels, ring_buffer))
    }

    /// Start audio output if not already started
    pub fn start(&self) -> Result<(), String> {
        let mut started = self.started.lock();
        if !*started {
            let mut device_guard = self.device.lock();

            // Share the same ring buffer with the AudioDevice
            // Both producer and consumer will use this exact instance
            let ring_buffer_clone = Arc::clone(&self.ring_buffer);

            match AudioDevice::new(self.sample_rate, self.channels, ring_buffer_clone) {
                Ok(dev) => {
                    *device_guard = Some(dev);
                    *started = true;
                    Ok(())
                }
                Err(e) => Err(format!("Failed to create audio device: {}", e)),
            }
        } else {
            Ok(())
        }
    }

    /// Push audio samples to the ring buffer
    pub fn push_samples(&self, samples: Vec<f32>) -> Result<(), String> {
        let mut ring_buffer = self.ring_buffer.lock();
        let _written = ring_buffer.write(&samples);
        Ok(())
    }

    /// Get the current ring buffer fill level (0.0 - 1.0)
    pub fn buffer_fill_level(&self) -> f32 {
        let ring_buffer = self.ring_buffer.lock();
        ring_buffer.fill_percentage()
    }

    /// Pause audio output
    pub fn pause(&self) {
        let mut device_guard = self.device.lock();
        if let Some(device) = device_guard.as_mut() {
            let _ = device.pause();
        }
    }

    /// Resume audio output
    pub fn resume(&self) {
        let mut device_guard = self.device.lock();
        if let Some(device) = device_guard.as_mut() {
            let _ = device.play();
        }
    }

    /// Check if device is active
    pub fn is_active(&self) -> bool {
        *self.started.lock()
    }
}
