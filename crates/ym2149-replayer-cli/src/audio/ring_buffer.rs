//! Ring buffer for concurrent sample generation and playback
//!
//! A ring buffer (circular buffer) allows two threads to operate concurrently:
//! - Producer thread: Generates samples and writes to buffer
//! - Consumer thread: Reads samples from buffer and outputs to audio device
//!
//! Memory consumption is fixed at buffer_size * sizeof(f32) regardless of duration.
//! Uses mutex-based synchronization with atomic position tracking for visibility.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Error type for ring buffer operations
#[derive(Debug, Clone)]
pub struct RingBufferError(pub String);

impl std::fmt::Display for RingBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RingBufferError {}

/// Ring buffer for streaming audio samples
///
/// # Thread Safety
/// - One producer thread (sample generator)
/// - One consumer thread (audio playback)
/// - Uses parking_lot::Mutex for buffer access with atomic variables for position tracking
/// - Position tracking uses atomic operations for memory visibility without explicit locks
#[derive(Debug)]
pub struct RingBuffer {
    /// Shared buffer storage (protected by mutex for thread safety)
    buffer: Mutex<Vec<f32>>,
    /// Write position (producer)
    write_pos: AtomicUsize,
    /// Read position (consumer)
    read_pos: AtomicUsize,
    /// Capacity (power of 2 for efficient modulo operation)
    capacity: usize,
    /// Capacity mask for fast modulo: `pos & mask == pos % capacity`
    mask: usize,
}

impl RingBuffer {
    /// Create a new ring buffer
    /// Capacity will be rounded up to the next power of 2 for efficient operations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Requested capacity is 0
    /// - Requested capacity would exceed maximum safe allocation (512 MB)
    pub fn new(requested_capacity: usize) -> Result<Self, RingBufferError> {
        // Validate capacity
        if requested_capacity == 0 {
            return Err(RingBufferError(
                "Ring buffer capacity must be greater than 0".into(),
            ));
        }

        let capacity = requested_capacity.next_power_of_two();

        // Check for unreasonably large allocations (prevent OOM)
        // 512 MB worth of f32 samples
        const MAX_CAPACITY: usize = 512 * 1024 * 1024 / std::mem::size_of::<f32>();
        if capacity > MAX_CAPACITY {
            return Err(RingBufferError(format!(
                "Ring buffer capacity {capacity} exceeds maximum safe size {MAX_CAPACITY}"
            )));
        }

        let mask = capacity - 1;

        Ok(RingBuffer {
            buffer: Mutex::new(vec![0.0; capacity]),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            capacity,
            mask,
        })
    }

    /// Get the capacity of the buffer (used in tests)
    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of samples available to read (without blocking)
    pub fn available_read(&self) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Acquire);

        if write >= read {
            write - read
        } else {
            self.capacity - (read - write)
        }
    }

    /// Write samples to the buffer (producer)
    /// Returns the number of samples successfully written
    /// Returns 0 if buffer is full (would block on write)
    pub fn write(&self, samples: &[f32]) -> usize {
        let mut buf = self.buffer.lock();

        // Calculate available space while holding the lock (prevents TOCTOU race)
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        let available = if write_pos >= read_pos {
            self.capacity - (write_pos - read_pos) - 1
        } else {
            (read_pos - write_pos) - 1
        };

        let to_write = samples.len().min(available);

        if to_write == 0 {
            return 0;
        }

        let write_idx = write_pos & self.mask;

        // Check if we can write in one contiguous chunk
        if write_idx + to_write <= self.capacity {
            // Single write
            buf[write_idx..write_idx + to_write].copy_from_slice(&samples[..to_write]);
        } else {
            // Wrap-around write
            let first_part = self.capacity - write_idx;
            buf[write_idx..].copy_from_slice(&samples[..first_part]);
            buf[..to_write - first_part].copy_from_slice(&samples[first_part..to_write]);
        }

        drop(buf); // Release lock before updating position

        // Update write position (release semantics for visibility to reader)
        self.write_pos
            .store(write_pos + to_write, Ordering::Release);

        to_write
    }

    /// Read samples from the buffer (consumer)
    /// Returns the number of samples successfully read
    pub fn read(&self, dest: &mut [f32]) -> usize {
        let buf = self.buffer.lock();

        // Calculate available data while holding the lock (prevents TOCTOU race)
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        let available = if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            self.capacity - (read_pos - write_pos)
        };

        let to_read = dest.len().min(available);

        if to_read == 0 {
            return 0;
        }

        let read_idx = read_pos & self.mask;

        // Check if we can read in one contiguous chunk
        if read_idx + to_read <= self.capacity {
            // Single read
            dest[..to_read].copy_from_slice(&buf[read_idx..read_idx + to_read]);
        } else {
            // Wrap-around read
            let first_part = self.capacity - read_idx;
            dest[..first_part].copy_from_slice(&buf[read_idx..]);
            dest[first_part..to_read].copy_from_slice(&buf[..to_read - first_part]);
        }

        drop(buf); // Release lock before updating position

        // Update read position
        self.read_pos.store(read_pos + to_read, Ordering::Release);

        to_read
    }

    /// Drain and discard all samples from the buffer (used in tests)
    #[cfg(test)]
    pub fn flush(&self) {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        self.read_pos.store(write_pos, Ordering::Release);
    }

    /// Check if the buffer has any samples to read (used in tests)
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.available_read() == 0
    }

    /// Check if the buffer is full (used in tests)
    #[cfg(test)]
    pub fn is_full(&self) -> bool {
        self.capacity - self.available_read() - 1 == 0
    }

    /// Get fill percentage (0.0 to 1.0)
    pub fn fill_percentage(&self) -> f32 {
        (self.available_read() as f32) / (self.capacity as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_creation() {
        let rb = RingBuffer::new(1024).unwrap();
        assert_eq!(rb.capacity(), 1024);
        assert!(rb.is_empty());
        assert!(!rb.is_full());
    }

    #[test]
    fn test_ring_buffer_power_of_two() {
        let rb = RingBuffer::new(1000).unwrap();
        // Should round up to 1024
        assert_eq!(rb.capacity(), 1024);
    }

    #[test]
    fn test_write_and_read() {
        let rb = RingBuffer::new(16).unwrap();
        let samples = vec![0.1, 0.2, 0.3, 0.4];

        let written = rb.write(&samples);
        assert_eq!(written, 4);
        assert_eq!(rb.available_read(), 4);

        let mut dest = vec![0.0; 4];
        let read = rb.read(&mut dest);
        assert_eq!(read, 4);
        assert_eq!(dest, samples);
    }

    #[test]
    fn test_ring_buffer_wrap() {
        let rb = RingBuffer::new(16).unwrap();

        // Write, read, and write again to cause wrap-around
        let data1 = vec![1.0; 10];
        let data2 = vec![2.0; 8];

        let written1 = rb.write(&data1);
        assert_eq!(written1, 10);

        let mut buf = vec![0.0; 5];
        let read1 = rb.read(&mut buf);
        assert_eq!(read1, 5);
        assert_eq!(&buf[..], &data1[..5]);

        // Write more data (this will cause wrap-around)
        let written2 = rb.write(&data2);
        assert!(written2 > 0);

        // Read remaining
        let mut buf = vec![0.0; 15];
        let read2 = rb.read(&mut buf);
        assert!(read2 > 0);
    }

    #[test]
    fn test_fill_percentage() {
        let rb = RingBuffer::new(128).unwrap(); // Explicitly use power of 2
        assert_eq!(rb.fill_percentage(), 0.0);

        rb.write(&vec![1.0; 64]);
        let fill = rb.fill_percentage();
        assert!(fill > 0.45 && fill < 0.55, "Fill percentage {fill}");

        rb.write(&vec![1.0; 63]);
        let fill = rb.fill_percentage();
        // Should be nearly full (1 sample gap due to ring buffer invariant)
        assert!(fill > 0.95, "Fill percentage {fill}");
    }

    #[test]
    fn test_flush() {
        let rb = RingBuffer::new(16).unwrap();
        rb.write(&[1.0; 8]);
        assert!(!rb.is_empty());

        rb.flush();
        assert!(rb.is_empty());
    }

    #[test]
    fn test_zero_capacity_error() {
        let result = RingBuffer::new(0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("greater than 0"));
    }

    #[test]
    fn test_max_capacity_exceeded() {
        // Try to allocate too much memory
        let max_plus_one = (512 * 1024 * 1024 / std::mem::size_of::<f32>()) + 1;
        let result = RingBuffer::new(max_plus_one);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }
}
