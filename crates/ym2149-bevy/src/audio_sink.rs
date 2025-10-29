//! Audio sink abstraction for pluggable output mechanisms
//!
//! This module defines the [`AudioSink`] trait, which allows users to bring their own
//! audio output implementations. The plugin ships with a [`RodioAudioSink`] that uses
//! the rodio library for real-time audio playback, but users can implement the trait
//! for custom outputs like WAV file writing, network streaming, or other mechanisms.
//!
//! # Using the Default Rodio Implementation
//!
//! The plugin automatically uses `RodioAudioSink` by default if no other sink is provided.
//!
//! # Implementing a Custom Sink
//!
//! ```ignore
//! use ym2149_bevy::audio_sink::AudioSink;
//! use std::sync::Arc;
//!
//! struct MyCustomSink {
//!     // Your fields here
//! }
//!
//! impl AudioSink for MyCustomSink {
//!     fn push_samples(&self, samples: Vec<f32>) -> Result<(), String> {
//!         // Write to file, network, etc.
//!         Ok(())
//!     }
//!
//!     fn pause(&self) {
//!         // Handle pause
//!     }
//!
//!     fn resume(&self) {
//!         // Handle resume
//!     }
//!
//!     fn buffer_fill_level(&self) -> f32 {
//!         // Return fill level 0.0 to 1.0
//!         0.5
//!     }
//! }
//! ```

use std::sync::Arc;

/// Trait for audio output mechanisms
///
/// Implement this trait to provide custom audio output for YM2149 playback.
/// The plugin uses this abstraction to allow users to bring their own audio
/// implementation while providing sensible defaults via [`RodioAudioSink`].
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` as they're accessed from Bevy systems.
pub trait AudioSink: Send + Sync {
    /// Push audio samples to the sink
    ///
    /// Samples are provided as mono f32 values. It's the sink's responsibility
    /// to handle stereo conversion, buffering, or file writing as needed.
    ///
    /// # Arguments
    ///
    /// * `samples` - Mono f32 audio samples to output
    ///
    /// # Errors
    ///
    /// Returns an error string if the push operation fails (e.g., buffer full,
    /// device error, file write error).
    fn push_samples(&self, samples: Vec<f32>) -> Result<(), String>;

    /// Pause audio output
    ///
    /// For real-time streaming, this stops the audio device.
    /// For file-based output, this may be a no-op.
    fn pause(&self);

    /// Resume audio output
    ///
    /// For real-time streaming, this resumes the audio device.
    /// For file-based output, this may be a no-op.
    fn resume(&self);

    /// Get the current buffer fill level
    ///
    /// Returns a value between 0.0 (empty) and 1.0 (full).
    /// This is used for diagnostic purposes and may inform the playback system
    /// about timing adjustments.
    ///
    /// For file-based output, this might always return 0.5.
    fn buffer_fill_level(&self) -> f32;
}

/// Default rodio-based audio sink implementation
///
/// This uses the rodio library to stream audio to the system's default audio device.
/// It's the default sink used by the plugin if no custom sink is provided.
pub mod rodio {
    use super::AudioSink;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use ym2149::streaming::{AudioDevice, RingBuffer};

    /// Rodio-based audio sink for real-time playback
    ///
    /// Note: This struct is wrapped in Arc<dyn AudioSink> by the plugin,
    /// so we don't need Arc fields for sharing - just Mutex for interior mutability.
    pub struct RodioAudioSink {
        /// Shared ring buffer for audio samples
        ring_buffer: Arc<Mutex<RingBuffer>>,
        /// The rodio-based audio device
        device: Mutex<Option<AudioDevice>>,
        /// Sample rate in Hz
        sample_rate: u32,
        /// Number of channels (1 = mono, 2 = stereo)
        channels: u16,
        /// Whether the device has been started
        started: Mutex<bool>,
    }

    // RodioAudioSink is Send + Sync because:
    // - ring_buffer is Arc<Mutex> which is Send + Sync
    // - device/started are parking_lot::Mutex which are Send + Sync
    // - sample_rate and channels are Copy types
    unsafe impl Send for RodioAudioSink {}
    unsafe impl Sync for RodioAudioSink {}

    impl RodioAudioSink {
        /// Create a new rodio audio sink with a pre-created ring buffer
        pub fn new_with_buffer(
            sample_rate: u32,
            channels: u16,
            ring_buffer: Arc<Mutex<RingBuffer>>,
        ) -> Self {
            Self {
                ring_buffer,
                device: Mutex::new(None),
                sample_rate,
                channels,
                started: Mutex::new(false),
            }
        }

        /// Create a new rodio audio sink
        ///
        /// This initializes a ring buffer with a size of ~250ms at the given sample rate.
        ///
        /// # Arguments
        ///
        /// * `sample_rate` - Sample rate in Hz (typically 44100)
        /// * `channels` - Number of channels (1 = mono, 2 = stereo)
        pub fn new(sample_rate: u32, channels: u16) -> Result<Self, String> {
            // Create ring buffer for audio samples
            // Use a larger buffer size for smooth playback: sample_rate / 4 gives ~250ms buffer at 44.1kHz
            let buffer_size = (sample_rate / 4) as usize;
            let ring_buffer = RingBuffer::new(buffer_size)
                .map_err(|e| format!("Failed to create ring buffer: {}", e))?;

            let ring_buffer = Arc::new(Mutex::new(ring_buffer));
            Ok(Self::new_with_buffer(sample_rate, channels, ring_buffer))
        }

        /// Start audio output
        pub fn start(&self) -> Result<(), String> {
            let mut started = self.started.lock();
            if !*started {
                let mut device_guard = self.device.lock();
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

        /// Check if device is active
        pub fn is_active(&self) -> bool {
            *self.started.lock()
        }
    }

    impl AudioSink for RodioAudioSink {
        fn push_samples(&self, samples: Vec<f32>) -> Result<(), String> {
            let mut ring_buffer = self.ring_buffer.lock();
            let _written = ring_buffer.write(&samples);
            Ok(())
        }

        fn pause(&self) {
            let mut device_guard = self.device.lock();
            if let Some(device) = device_guard.as_mut() {
                device.pause();
            }
        }

        fn resume(&self) {
            let mut device_guard = self.device.lock();
            if let Some(device) = device_guard.as_mut() {
                device.play();
            }
        }

        fn buffer_fill_level(&self) -> f32 {
            let ring_buffer = self.ring_buffer.lock();
            ring_buffer.fill_percentage()
        }
    }
}

/// Type alias for a boxed audio sink
///
/// This is useful when storing audio sinks in resources or when you want to
/// dynamically select between different sink implementations.
pub type BoxedAudioSink = Arc<dyn AudioSink>;

/// Helper function to create a boxed rodio sink
pub fn rodio_sink(sample_rate: u32, channels: u16) -> Result<BoxedAudioSink, String> {
    Ok(Arc::new(rodio::RodioAudioSink::new(sample_rate, channels)?))
}
