//! Metadata extraction tool for YM2149 chiptune files.
//!
//! Scans directories and extracts metadata from YM, SNDH, AY, and AKS files
//! using the same parsers as the main library.
//!
//! Optionally generates waveform peaks and audio fingerprints for instant
//! visualization in the web player.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use walkdir::WalkDir;

use ym2149_arkos_replayer::load_aks;
use ym2149_ay_replayer::AyPlayer;
use ym2149_common::{ChiptunePlayer, ChiptunePlayerBase};
use ym2149_sndh_replayer::{is_sndh_data, load_sndh, SndhFile};
use ym2149_ym_replayer::load_song;

// Waveform generation constants
const WAVEFORM_BARS: usize = 400; // Higher resolution for smoother waveform
const SAMPLE_RATE: u32 = 44100;

#[derive(Parser)]
#[command(name = "ym-metadata")]
#[command(about = "Extract metadata from YM2149 chiptune files")]
struct Args {
    /// Directory to scan
    #[arg(short, long)]
    dir: PathBuf,

    /// Output JSON file
    #[arg(short, long)]
    output: PathBuf,

    /// Base path to strip from file paths (for relative paths in output)
    #[arg(short, long)]
    base: Option<PathBuf>,

    /// Pretty print JSON output
    #[arg(long)]
    pretty: bool,

    /// Generate waveform peaks and fingerprints for web player visualization
    #[arg(long)]
    waveforms: bool,
}

#[derive(Serialize, Clone)]
struct TrackMetadata {
    path: String,
    title: String,
    author: String,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<String>,
    #[serde(skip_serializing_if = "is_one")]
    subsongs: u32,
    #[serde(skip_serializing_if = "is_three")]
    channels: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f32>,
    collection: String,
    /// Waveform peaks as base64-encoded bytes (0-255 per bar)
    #[serde(skip_serializing_if = "Option::is_none")]
    w: Option<String>,
    /// Audio fingerprint for similarity matching
    #[serde(skip_serializing_if = "Option::is_none")]
    fp: Option<Fingerprint>,
}

#[derive(Serialize, Clone)]
struct Fingerprint {
    /// Average amplitude (0.0-1.0)
    amp: f32,
    /// Note density (zero crossings per second) - indicates pitch/frequency content
    density: u32,
    /// Amplitude variance (0.0-1.0) - how dynamic the song is
    variance: f32,
    /// Peak-to-average ratio - how "punchy" the song is
    punch: f32,
    /// Brightness (0.0-1.0) - high vs low frequency content
    brightness: f32,
    /// Energy histogram (8 bins) - distribution of amplitude levels
    #[serde(skip_serializing_if = "Option::is_none")]
    hist: Option<[u8; 8]>,
    /// Section energies (4 quarters) - song structure fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    sections: Option<[u8; 4]>,
    /// Tempo indicator (peaks per second) - rhythm signature
    #[serde(skip_serializing_if = "Option::is_none")]
    tempo: Option<u16>,
    // === New spectral and rhythm features ===
    /// Spectral centroid (0-1) - center of mass of spectrum (low=bassy, high=bright)
    #[serde(skip_serializing_if = "Option::is_none")]
    centroid: Option<f32>,
    /// Spectral flatness (0-1) - 0=tonal, 1=noise-like
    #[serde(skip_serializing_if = "Option::is_none")]
    flatness: Option<f32>,
    /// Spectral bands [bass, low-mid, high-mid, treble] (0-255 each)
    #[serde(skip_serializing_if = "Option::is_none")]
    bands: Option<[u8; 4]>,
    /// Chroma features - 12-bin pitch class histogram (C, C#, D, ..., B)
    #[serde(skip_serializing_if = "Option::is_none")]
    chroma: Option<[u8; 12]>,
    /// Rhythm regularity (0-1) - how consistent the beat pattern is
    #[serde(skip_serializing_if = "Option::is_none")]
    rhythm_reg: Option<f32>,
    /// Rhythm strength (0-1) - how prominent/strong the beat is
    #[serde(skip_serializing_if = "Option::is_none")]
    rhythm_str: Option<f32>,
    /// MFCCs - Mel-Frequency Cepstral Coefficients (13 coefficients, industry standard for timbre)
    #[serde(skip_serializing_if = "Option::is_none")]
    mfcc: Option<[i8; 13]>,
    /// MFCC Deltas - How timbre changes over time (13 coefficients)
    #[serde(skip_serializing_if = "Option::is_none")]
    mfcc_d: Option<[i8; 13]>,
    /// MFCC Delta-Deltas - Acceleration of timbre changes (13 coefficients)
    #[serde(skip_serializing_if = "Option::is_none")]
    mfcc_dd: Option<[i8; 13]>,
    /// Chromagram - Pitch class distribution over 8 time segments (8 × 12 = 96 values)
    /// Captures melodic/harmonic progression through the song
    #[serde(skip_serializing_if = "Option::is_none")]
    chromagram: Option<Vec<u8>>,
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

fn is_three(n: &u32) -> bool {
    *n == 3
}

#[derive(Serialize)]
struct CollectionInfo {
    id: String,
    name: String,
    description: String,
    format: String,
    #[serde(rename = "trackCount")]
    track_count: usize,
}

#[derive(Serialize)]
struct Catalog {
    version: String,
    generated: String,
    collections: Vec<CollectionInfo>,
    tracks: Vec<TrackMetadata>,
}

/// Waveform and fingerprint data
struct WaveformData {
    /// Base64-encoded peaks (0-255 per bar)
    waveform: String,
    /// Audio fingerprint
    fingerprint: Fingerprint,
}

// FFT size for spectral analysis (power of 2)
const FFT_SIZE: usize = 2048;

/// Convert frequency to MIDI note number
fn freq_to_midi(freq: f32) -> f32 {
    if freq <= 0.0 {
        return 0.0;
    }
    69.0 + 12.0 * (freq / 440.0).log2()
}

/// Convert FFT bin index to frequency
fn bin_to_freq(bin: usize, sample_rate: u32, fft_size: usize) -> f32 {
    bin as f32 * sample_rate as f32 / fft_size as f32
}

/// Compute spectral features from FFT magnitudes
fn compute_spectral_features(
    magnitudes: &[f32],
    sample_rate: u32,
    fft_size: usize,
) -> (f32, f32, [f32; 4], [f32; 12]) {
    let nyquist_bin = fft_size / 2;

    // Spectral centroid: weighted average of frequencies
    let mut weighted_sum: f64 = 0.0;
    let mut magnitude_sum: f64 = 0.0;
    for (i, &mag) in magnitudes.iter().take(nyquist_bin).enumerate() {
        let freq = bin_to_freq(i, sample_rate, fft_size);
        weighted_sum += freq as f64 * mag as f64;
        magnitude_sum += mag as f64;
    }
    let centroid = if magnitude_sum > 0.0 {
        // Normalize to 0-1 range (assuming max useful freq ~10kHz for chiptunes)
        ((weighted_sum / magnitude_sum) / 10000.0).min(1.0) as f32
    } else {
        0.0
    };

    // Spectral flatness: geometric mean / arithmetic mean
    // High value = noise-like, low value = tonal
    let epsilon = 1e-10;
    let mut log_sum: f64 = 0.0;
    let mut lin_sum: f64 = 0.0;
    let mut count = 0;
    for &mag in magnitudes.iter().take(nyquist_bin).skip(1) {
        // Skip DC
        if mag > epsilon as f32 {
            log_sum += (mag as f64 + epsilon).ln();
            lin_sum += mag as f64;
            count += 1;
        }
    }
    let flatness = if count > 0 && lin_sum > 0.0 {
        let geometric_mean = (log_sum / count as f64).exp();
        let arithmetic_mean = lin_sum / count as f64;
        (geometric_mean / arithmetic_mean).min(1.0) as f32
    } else {
        0.0
    };

    // Spectral bands: energy in 4 frequency ranges
    // Bass: 20-200Hz, Low-mid: 200-800Hz, High-mid: 800-4000Hz, Treble: 4000-10000Hz
    let band_edges = [20.0, 200.0, 800.0, 4000.0, 10000.0];
    let mut bands = [0.0f32; 4];
    for (i, &mag) in magnitudes.iter().take(nyquist_bin).enumerate() {
        let freq = bin_to_freq(i, sample_rate, fft_size);
        for (band_idx, window) in band_edges.windows(2).enumerate() {
            if freq >= window[0] && freq < window[1] {
                bands[band_idx] += mag * mag; // Energy = magnitude squared
                break;
            }
        }
    }
    // Normalize bands
    let band_max = bands.iter().cloned().fold(0.0f32, f32::max).max(0.001);
    for band in &mut bands {
        *band /= band_max;
    }

    // Chroma features: 12-bin pitch class histogram
    let mut chroma = [0.0f32; 12];
    for (i, &mag) in magnitudes.iter().take(nyquist_bin).enumerate() {
        let freq = bin_to_freq(i, sample_rate, fft_size);
        if freq >= 60.0 && freq <= 4000.0 {
            // Focus on musical range
            let midi = freq_to_midi(freq);
            let pitch_class = (midi.round() as i32 % 12).unsigned_abs() as usize;
            if pitch_class < 12 {
                chroma[pitch_class] += mag * mag;
            }
        }
    }
    // Normalize chroma
    let chroma_max = chroma.iter().cloned().fold(0.0f32, f32::max).max(0.001);
    for c in &mut chroma {
        *c /= chroma_max;
    }

    (centroid, flatness, bands, chroma)
}

// ============================================================================
// MFCC (Mel-Frequency Cepstral Coefficients) Implementation
// ============================================================================

/// Number of Mel filterbank channels
const MEL_FILTERS: usize = 26;
/// Number of MFCC coefficients to keep (standard is 13)
const NUM_MFCC: usize = 13;

/// Convert frequency in Hz to Mel scale
fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convert Mel scale to frequency in Hz
fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

/// Create Mel filterbank matrix
/// Returns a matrix where each row is a triangular filter in the Mel scale
fn create_mel_filterbank(
    num_filters: usize,
    fft_size: usize,
    sample_rate: u32,
    low_freq: f32,
    high_freq: f32,
) -> Vec<Vec<f32>> {
    let nyquist = fft_size / 2;

    // Convert frequency range to Mel scale
    let low_mel = hz_to_mel(low_freq);
    let high_mel = hz_to_mel(high_freq.min(sample_rate as f32 / 2.0));

    // Create equally spaced points in Mel scale
    let mel_points: Vec<f32> = (0..=num_filters + 1)
        .map(|i| low_mel + (high_mel - low_mel) * i as f32 / (num_filters + 1) as f32)
        .collect();

    // Convert back to Hz and then to FFT bin indices
    let bin_points: Vec<usize> = mel_points
        .iter()
        .map(|&mel| {
            let hz = mel_to_hz(mel);
            ((hz * fft_size as f32 / sample_rate as f32) as usize).min(nyquist - 1)
        })
        .collect();

    // Create triangular filters
    let mut filterbank = vec![vec![0.0f32; nyquist]; num_filters];

    for i in 0..num_filters {
        let start = bin_points[i];
        let center = bin_points[i + 1];
        let end = bin_points[i + 2];

        // Rising edge
        if center > start {
            for k in start..center {
                filterbank[i][k] = (k - start) as f32 / (center - start) as f32;
            }
        }

        // Falling edge
        if end > center {
            for k in center..=end.min(nyquist - 1) {
                filterbank[i][k] = (end - k) as f32 / (end - center) as f32;
            }
        }
    }

    filterbank
}

/// Apply DCT-II (Discrete Cosine Transform Type 2) to get cepstral coefficients
fn dct_ii(input: &[f32], num_coeffs: usize) -> Vec<f32> {
    let n = input.len();
    let mut output = Vec::with_capacity(num_coeffs);

    for k in 0..num_coeffs {
        let mut sum = 0.0f64;
        for (i, &x) in input.iter().enumerate() {
            sum += x as f64 * (std::f64::consts::PI * k as f64 * (2.0 * i as f64 + 1.0) / (2.0 * n as f64)).cos();
        }
        output.push(sum as f32);
    }

    output
}

/// Compute MFCCs from FFT magnitudes
/// Returns 13 MFCC coefficients (industry standard for audio similarity)
fn compute_mfcc(
    magnitudes: &[f32],
    sample_rate: u32,
    fft_size: usize,
) -> [f32; NUM_MFCC] {
    let nyquist = fft_size / 2;

    // Create Mel filterbank (focus on 60-8000 Hz for chiptunes)
    let filterbank = create_mel_filterbank(MEL_FILTERS, fft_size, sample_rate, 60.0, 8000.0);

    // Apply filterbank to get Mel-scale energies
    let mut mel_energies = vec![0.0f32; MEL_FILTERS];
    for (i, filter) in filterbank.iter().enumerate() {
        let mut energy = 0.0f32;
        for (k, &mag) in magnitudes.iter().take(nyquist).enumerate() {
            energy += mag * mag * filter.get(k).copied().unwrap_or(0.0);
        }
        // Log compression (add small epsilon to avoid log(0))
        mel_energies[i] = (energy + 1e-10).ln();
    }

    // Apply DCT to get cepstral coefficients
    let mfcc_vec = dct_ii(&mel_energies, NUM_MFCC);

    // Convert to fixed-size array
    let mut mfcc = [0.0f32; NUM_MFCC];
    for (i, &v) in mfcc_vec.iter().take(NUM_MFCC).enumerate() {
        mfcc[i] = v;
    }

    mfcc
}

/// Normalize MFCCs to i8 range (-128 to 127) for compact storage
fn normalize_mfcc(mfcc: &[f32; NUM_MFCC]) -> [i8; NUM_MFCC] {
    // Find the range of values
    let max_abs = mfcc.iter().map(|&x| x.abs()).fold(0.0f32, f32::max).max(0.001);

    // Scale to -127..127 range
    let mut normalized = [0i8; NUM_MFCC];
    for (i, &v) in mfcc.iter().enumerate() {
        normalized[i] = ((v / max_abs) * 127.0).clamp(-127.0, 127.0) as i8;
    }

    normalized
}

// ============================================================================
// MFCC Deltas and Chromagram
// ============================================================================

/// Number of time segments for chromagram
const CHROMAGRAM_SEGMENTS: usize = 8;

/// Compute MFCC deltas (first derivative) from a sequence of MFCC frames
/// Returns average delta across all frames
fn compute_mfcc_delta(mfcc_frames: &[[f32; NUM_MFCC]]) -> [f32; NUM_MFCC] {
    let mut delta = [0.0f32; NUM_MFCC];

    if mfcc_frames.len() < 3 {
        return delta;
    }

    // Compute delta using regression over window of 2 frames on each side
    // delta[t] = (c[t+1] - c[t-1]) / 2
    let mut count = 0;
    for t in 1..(mfcc_frames.len() - 1) {
        for i in 0..NUM_MFCC {
            delta[i] += (mfcc_frames[t + 1][i] - mfcc_frames[t - 1][i]) / 2.0;
        }
        count += 1;
    }

    // Average
    if count > 0 {
        for d in &mut delta {
            *d /= count as f32;
        }
    }

    delta
}

/// Compute MFCC delta-deltas (second derivative / acceleration)
fn compute_mfcc_delta_delta(mfcc_frames: &[[f32; NUM_MFCC]]) -> [f32; NUM_MFCC] {
    let mut delta_delta = [0.0f32; NUM_MFCC];

    if mfcc_frames.len() < 5 {
        return delta_delta;
    }

    // First compute deltas for each frame
    let mut deltas: Vec<[f32; NUM_MFCC]> = Vec::with_capacity(mfcc_frames.len() - 2);
    for t in 1..(mfcc_frames.len() - 1) {
        let mut d = [0.0f32; NUM_MFCC];
        for i in 0..NUM_MFCC {
            d[i] = (mfcc_frames[t + 1][i] - mfcc_frames[t - 1][i]) / 2.0;
        }
        deltas.push(d);
    }

    // Then compute delta of deltas
    if deltas.len() >= 3 {
        let mut count = 0;
        for t in 1..(deltas.len() - 1) {
            for i in 0..NUM_MFCC {
                delta_delta[i] += (deltas[t + 1][i] - deltas[t - 1][i]) / 2.0;
            }
            count += 1;
        }

        if count > 0 {
            for dd in &mut delta_delta {
                *dd /= count as f32;
            }
        }
    }

    delta_delta
}

/// Normalize delta/delta-delta to i8 range
fn normalize_delta(delta: &[f32; NUM_MFCC]) -> [i8; NUM_MFCC] {
    let max_abs = delta.iter().map(|&x| x.abs()).fold(0.0f32, f32::max).max(0.001);

    let mut normalized = [0i8; NUM_MFCC];
    for (i, &v) in delta.iter().enumerate() {
        normalized[i] = ((v / max_abs) * 127.0).clamp(-127.0, 127.0) as i8;
    }

    normalized
}

/// Compute chromagram - chroma features over time segments
/// Returns 8 segments × 12 pitch classes = 96 values
fn compute_chromagram(
    all_samples: &[f32],
    sample_rate: u32,
    fft_size: usize,
) -> Vec<u8> {
    if all_samples.len() < fft_size * CHROMAGRAM_SEGMENTS {
        return vec![0u8; CHROMAGRAM_SEGMENTS * 12];
    }

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Hann window
    let hann: Vec<f32> = (0..fft_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos()))
        .collect();

    let segment_size = all_samples.len() / CHROMAGRAM_SEGMENTS;
    let mut chromagram = Vec::with_capacity(CHROMAGRAM_SEGMENTS * 12);

    for seg in 0..CHROMAGRAM_SEGMENTS {
        let seg_start = seg * segment_size;
        let seg_end = (seg_start + segment_size).min(all_samples.len());

        // Compute average chroma for this segment using multiple windows
        let mut segment_chroma = [0.0f32; 12];
        let mut window_count = 0;

        let mut pos = seg_start;
        while pos + fft_size <= seg_end {
            // Apply window and FFT
            let mut buffer: Vec<Complex<f32>> = all_samples[pos..pos + fft_size]
                .iter()
                .zip(hann.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();

            fft.process(&mut buffer);

            // Compute chroma from this window
            for (i, c) in buffer.iter().take(fft_size / 2).enumerate() {
                let freq = i as f32 * sample_rate as f32 / fft_size as f32;
                if freq >= 60.0 && freq <= 4000.0 {
                    let mag = c.norm();
                    let midi = freq_to_midi(freq);
                    let pitch_class = (midi.round() as i32 % 12).unsigned_abs() as usize;
                    if pitch_class < 12 {
                        segment_chroma[pitch_class] += mag * mag;
                    }
                }
            }

            window_count += 1;
            pos += fft_size / 2; // 50% overlap
        }

        // Normalize segment chroma
        if window_count > 0 {
            let max_val = segment_chroma.iter().cloned().fold(0.0f32, f32::max).max(0.001);
            for c in &mut segment_chroma {
                *c = (*c / max_val).min(1.0);
            }
        }

        // Convert to u8 and add to chromagram
        for &c in &segment_chroma {
            chromagram.push((c * 255.0) as u8);
        }
    }

    chromagram
}

/// Compute rhythm features from amplitude envelope
fn compute_rhythm_features(envelope: &[f32], duration: f32) -> (f32, f32) {
    if envelope.len() < 100 || duration < 1.0 {
        return (0.0, 0.0);
    }

    // Compute autocorrelation at rhythmic lags (60-200 BPM range)
    let samples_per_second = envelope.len() as f32 / duration;
    let min_lag = (samples_per_second * 60.0 / 200.0) as usize; // 200 BPM
    let max_lag = (samples_per_second * 60.0 / 60.0) as usize; // 60 BPM
    let max_lag = max_lag.min(envelope.len() / 2);

    if min_lag >= max_lag || max_lag < 10 {
        return (0.0, 0.0);
    }

    // Normalize envelope (mean=0, std=1)
    let mean: f32 = envelope.iter().sum::<f32>() / envelope.len() as f32;
    let variance: f32 = envelope.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / envelope.len() as f32;
    let std = variance.sqrt().max(0.001);

    let normalized: Vec<f32> = envelope.iter().map(|x| (x - mean) / std).collect();

    // Find autocorrelation peaks
    let mut autocorr = Vec::with_capacity(max_lag - min_lag);
    for lag in min_lag..max_lag {
        let mut sum: f64 = 0.0;
        let mut count = 0;
        for i in 0..(envelope.len() - lag) {
            sum += (normalized[i] * normalized[i + lag]) as f64;
            count += 1;
        }
        if count > 0 {
            autocorr.push((sum / count as f64) as f32);
        }
    }

    if autocorr.is_empty() {
        return (0.0, 0.0);
    }

    // Rhythm strength: max autocorrelation value
    let rhythm_strength = autocorr
        .iter()
        .cloned()
        .fold(0.0f32, f32::max)
        .max(0.0)
        .min(1.0);

    // Rhythm regularity: find peaks and measure consistency
    let mut peaks = Vec::new();
    for i in 1..(autocorr.len() - 1) {
        if autocorr[i] > autocorr[i - 1] && autocorr[i] > autocorr[i + 1] && autocorr[i] > 0.1 {
            peaks.push(i + min_lag);
        }
    }

    let rhythm_regularity = if peaks.len() >= 2 {
        // Measure how evenly spaced the peaks are
        let intervals: Vec<usize> = peaks.windows(2).map(|w| w[1] - w[0]).collect();
        let avg_interval: f32 = intervals.iter().sum::<usize>() as f32 / intervals.len() as f32;
        let interval_variance: f32 = intervals
            .iter()
            .map(|&i| (i as f32 - avg_interval).powi(2))
            .sum::<f32>()
            / intervals.len() as f32;
        // Low variance = regular rhythm
        (1.0 - (interval_variance.sqrt() / avg_interval).min(1.0)).max(0.0)
    } else {
        0.0
    };

    (rhythm_regularity, rhythm_strength)
}

// Rhythm envelope resolution: 50 samples per second for good BPM detection
const RHYTHM_ENVELOPE_RATE: f32 = 50.0;

/// Generate waveform peaks and fingerprint from audio samples
fn generate_waveform<P: ChiptunePlayer>(player: &mut P, duration: f32) -> WaveformData {
    // Scan the entire song for accurate waveform representation
    let total_samples = (duration * SAMPLE_RATE as f32) as usize;
    let samples_per_bar = total_samples / WAVEFORM_BARS;

    let mut peaks = Vec::with_capacity(WAVEFORM_BARS);
    let mut total_amp: f64 = 0.0;
    let mut total_amp_squared: f64 = 0.0;
    let mut prev_sample: f32 = 0.0;
    let mut zero_crossings: u32 = 0;
    let mut samples_processed: usize = 0;
    let mut max_peak_overall: f32 = 0.0;
    let mut total_diff: f64 = 0.0;
    let mut diff_count: usize = 0;

    // For tempo detection
    let mut prev_bar_peak: f32 = 0.0;
    let mut peak_count: u32 = 0;
    const PEAK_THRESHOLD: f32 = 0.15;

    // Collect samples for FFT analysis
    let mut all_samples: Vec<f32> = Vec::with_capacity(total_samples);

    // High-resolution amplitude envelope for rhythm analysis (50 Hz)
    let rhythm_envelope_size = (duration * RHYTHM_ENVELOPE_RATE) as usize;
    let samples_per_rhythm_frame = (SAMPLE_RATE as f32 / RHYTHM_ENVELOPE_RATE) as usize;
    let mut rhythm_envelope: Vec<f32> = Vec::with_capacity(rhythm_envelope_size.max(1));
    let mut rhythm_frame_energy: f32 = 0.0;
    let mut rhythm_frame_samples: usize = 0;

    for bar_idx in 0..WAVEFORM_BARS {
        let samples = player.generate_samples(samples_per_bar);
        let mut max_peak: f32 = 0.0;

        for (i, &sample) in samples.iter().enumerate() {
            let abs = sample.abs();
            if abs > max_peak {
                max_peak = abs;
            }
            total_amp += abs as f64;
            total_amp_squared += (abs * abs) as f64;

            // Accumulate rhythm envelope at 50 Hz
            rhythm_frame_energy += abs;
            rhythm_frame_samples += 1;
            if rhythm_frame_samples >= samples_per_rhythm_frame {
                rhythm_envelope.push(rhythm_frame_energy / rhythm_frame_samples as f32);
                rhythm_frame_energy = 0.0;
                rhythm_frame_samples = 0;
            }

            // Zero crossing detection (every 4th sample for speed)
            if i % 4 == 0 {
                if (prev_sample < 0.0 && sample >= 0.0) || (prev_sample >= 0.0 && sample < 0.0) {
                    zero_crossings += 1;
                }
                total_diff += (sample - prev_sample).abs() as f64;
                diff_count += 1;
                prev_sample = sample;
            }
        }

        // Store samples for FFT (subsample to limit memory)
        if all_samples.len() < 44100 * 30 {
            // Max 30 seconds for FFT
            all_samples.extend(samples.iter().step_by(4).cloned());
        }

        if max_peak > max_peak_overall {
            max_peak_overall = max_peak;
        }

        if bar_idx > 0 && max_peak > prev_bar_peak + PEAK_THRESHOLD {
            peak_count += 1;
        }
        prev_bar_peak = max_peak;

        peaks.push((max_peak.min(1.0) * 255.0) as u8);
        samples_processed += samples.len();
    }

    // Flush remaining rhythm frame
    if rhythm_frame_samples > 0 {
        rhythm_envelope.push(rhythm_frame_energy / rhythm_frame_samples as f32);
    }

    // === Basic features (existing) ===
    let avg_amp = if samples_processed > 0 {
        (total_amp / samples_processed as f64) as f32
    } else {
        0.0
    };

    let variance = if samples_processed > 0 {
        let mean_squared = total_amp_squared / samples_processed as f64;
        let squared_mean = (avg_amp * avg_amp) as f64;
        ((mean_squared - squared_mean).max(0.0).sqrt() as f32).min(1.0)
    } else {
        0.0
    };

    let density = if duration > 0.0 {
        (zero_crossings as f32 / duration) as u32
    } else {
        0
    };

    let punch = if avg_amp > 0.001 {
        (max_peak_overall / avg_amp).min(10.0)
    } else {
        1.0
    };

    let brightness = if diff_count > 0 {
        let avg_diff = (total_diff / diff_count as f64) as f32;
        (avg_diff * 2.0).min(1.0)
    } else {
        0.0
    };

    // Energy histogram
    let mut histogram = [0u32; 8];
    for &peak in &peaks {
        let bin = (peak / 32).min(7) as usize;
        histogram[bin] += 1;
    }
    let hist_max = histogram.iter().max().copied().unwrap_or(1).max(1);
    let hist: [u8; 8] = histogram.map(|v| ((v * 255) / hist_max) as u8);

    // Section energies
    let quarter_size = WAVEFORM_BARS / 4;
    let mut sections = [0u8; 4];
    for (i, section) in sections.iter_mut().enumerate() {
        let start = i * quarter_size;
        let end = start + quarter_size;
        let sum: u32 = peaks[start..end].iter().map(|&p| p as u32).sum();
        let avg = sum / quarter_size as u32;
        *section = avg.min(255) as u8;
    }

    let tempo = if duration > 0.0 {
        ((peak_count as f32 / duration) * 60.0) as u16
    } else {
        0
    };

    // === FFT-based spectral features ===
    let effective_sample_rate = SAMPLE_RATE / 4; // /4 because subsampled

    let (centroid, flatness, bands, chroma, mfcc, mfcc_d, mfcc_dd) = if all_samples.len() >= FFT_SIZE {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        // Accumulate spectral features over multiple windows
        let mut total_magnitudes = vec![0.0f32; FFT_SIZE / 2];
        let mut window_count = 0;

        // Store MFCCs for each window (for delta computation)
        let mut mfcc_frames: Vec<[f32; NUM_MFCC]> = Vec::new();

        // Hann window for smoother FFT
        let hann: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos()))
            .collect();

        // Process multiple windows across the song
        let step = all_samples.len().saturating_sub(FFT_SIZE) / 20.max(1); // ~20 windows
        let step = step.max(FFT_SIZE / 2); // At least 50% overlap

        let mut pos = 0;
        while pos + FFT_SIZE <= all_samples.len() {
            // Apply window and convert to complex
            let mut buffer: Vec<Complex<f32>> = all_samples[pos..pos + FFT_SIZE]
                .iter()
                .zip(hann.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();

            fft.process(&mut buffer);

            // Extract magnitudes for this window
            let magnitudes: Vec<f32> = buffer.iter().take(FFT_SIZE / 2).map(|c| c.norm()).collect();

            // Accumulate for average spectrum
            for (i, &mag) in magnitudes.iter().enumerate() {
                total_magnitudes[i] += mag;
            }

            // Compute MFCC for this window
            let mfcc_frame = compute_mfcc(&magnitudes, effective_sample_rate, FFT_SIZE);
            mfcc_frames.push(mfcc_frame);

            window_count += 1;
            pos += step;
        }

        if window_count > 0 {
            // Average magnitudes
            for mag in &mut total_magnitudes {
                *mag /= window_count as f32;
            }

            // Compute spectral features from average spectrum
            let (c, f, b, ch) = compute_spectral_features(&total_magnitudes, effective_sample_rate, FFT_SIZE);

            // Compute average MFCC
            let mut avg_mfcc = [0.0f32; NUM_MFCC];
            for frame in &mfcc_frames {
                for (i, &v) in frame.iter().enumerate() {
                    avg_mfcc[i] += v;
                }
            }
            for m in &mut avg_mfcc {
                *m /= mfcc_frames.len() as f32;
            }
            let mfcc_normalized = normalize_mfcc(&avg_mfcc);

            // Compute MFCC deltas and delta-deltas
            let delta = compute_mfcc_delta(&mfcc_frames);
            let delta_delta = compute_mfcc_delta_delta(&mfcc_frames);
            let delta_normalized = normalize_delta(&delta);
            let delta_delta_normalized = normalize_delta(&delta_delta);

            (Some(c), Some(f), Some(b), Some(ch), Some(mfcc_normalized), Some(delta_normalized), Some(delta_delta_normalized))
        } else {
            (None, None, None, None, None, None, None)
        }
    } else {
        (None, None, None, None, None, None, None)
    };

    // === Chromagram - pitch class distribution over time ===
    let chromagram = if all_samples.len() >= FFT_SIZE * CHROMAGRAM_SEGMENTS {
        Some(compute_chromagram(&all_samples, effective_sample_rate, FFT_SIZE))
    } else {
        None
    };

    // === Rhythm features (using high-resolution 50Hz envelope) ===
    let (rhythm_reg, rhythm_str) = compute_rhythm_features(&rhythm_envelope, duration);

    // Convert bands and chroma to u8 arrays
    let bands_u8: Option<[u8; 4]> = bands.map(|b| b.map(|v| (v * 255.0) as u8));
    let chroma_u8: Option<[u8; 12]> = chroma.map(|c| c.map(|v| (v * 255.0) as u8));

    WaveformData {
        waveform: BASE64.encode(&peaks),
        fingerprint: Fingerprint {
            amp: (avg_amp * 1000.0).round() / 1000.0,
            density,
            variance: (variance * 1000.0).round() / 1000.0,
            punch: (punch * 100.0).round() / 100.0,
            brightness: (brightness * 1000.0).round() / 1000.0,
            hist: Some(hist),
            sections: Some(sections),
            tempo: Some(tempo),
            // New spectral features
            centroid,
            flatness,
            bands: bands_u8,
            chroma: chroma_u8,
            rhythm_reg: Some((rhythm_reg * 1000.0).round() / 1000.0),
            rhythm_str: Some((rhythm_str * 1000.0).round() / 1000.0),
            // MFCCs for timbre similarity
            mfcc,
            // MFCC deltas for timbre dynamics
            mfcc_d,
            mfcc_dd,
            // Chromagram for melodic progression
            chromagram,
        },
    }
}

fn detect_collection(path: &Path) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.contains("sndh") {
        Some(("sndh", "SNDH Collection", "Atari ST/STE music from the SNDH archive", "SNDH"))
    } else if path_str.contains("projectay") {
        Some(("ay", "Project AY", "ZX Spectrum AY music", "AY"))
    } else if path_str.contains("arkos") {
        Some(("arkos", "Arkos Tracker", "Arkos Tracker 2 songs", "AKS"))
    } else if path_str.contains("/ym/") || path_str.contains("\\ym\\") || path_str.ends_with("/ym") {
        Some(("ym", "YM Collection", "YM format chiptunes", "YM"))
    } else {
        None
    }
}

fn extract_metadata(path: &Path, base_path: &Path, gen_waveforms: bool) -> Option<TrackMetadata> {
    let ext = path.extension()?.to_str()?.to_lowercase();

    let data = fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }

    let relative_path = path.strip_prefix(base_path).unwrap_or(path);
    let path_str = relative_path.to_string_lossy().replace('\\', "/");

    // Detect collection from path
    let (collection_id, _, _, _format_name) = detect_collection(path)?;

    // Extract artist hint from directory structure
    let artist_hint = path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| {
            // Clean up artist names like "Hippel.Jochen (Mad Max)" -> "Mad Max"
            if let Some(start) = s.find('(') {
                if let Some(end) = s.find(')') {
                    return s[start + 1..end].to_string();
                }
            }
            // Or "LastName.FirstName" -> "FirstName LastName"
            if s.contains('.') && !s.ends_with('.') {
                let parts: Vec<&str> = s.split('.').collect();
                if parts.len() == 2 {
                    return format!("{} {}", parts[1], parts[0]);
                }
            }
            s.to_string()
        });

    match ext.as_str() {
        "sndh" => extract_sndh_metadata(&data, path_str, collection_id, artist_hint, gen_waveforms),
        "ym" => extract_ym_metadata(&data, path_str, collection_id, artist_hint, path, gen_waveforms),
        "ay" => extract_ay_metadata(&data, path_str, collection_id, artist_hint, gen_waveforms),
        "aks" => extract_aks_metadata(&data, path_str, collection_id, artist_hint, gen_waveforms),
        _ => None,
    }
}

fn extract_sndh_metadata(data: &[u8], path: String, collection: &str, artist_hint: Option<String>, gen_waveforms: bool) -> Option<TrackMetadata> {
    if !is_sndh_data(data) {
        return None;
    }

    let sndh = SndhFile::parse(data).ok()?;
    let meta = &sndh.metadata;

    let title = meta.title.clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.rsplit('/').next().unwrap_or(&path)
                .trim_end_matches(".sndh")
                .trim_end_matches(".SNDH")
                .to_string()
        });

    let author = meta.author.clone()
        .filter(|s| !s.is_empty())
        .or(artist_hint)
        .unwrap_or_else(|| "Unknown".to_string());

    // Calculate duration from frame count and player rate
    let duration = meta.subsong_frames.first()
        .filter(|&&f| f > 0)
        .map(|&frames| frames as f32 / meta.player_rate as f32)
        .or_else(|| {
            // Fallback to TIME durations if FRMS not available
            meta.subsong_durations.first().map(|&d| d as f32)
        });

    // Generate waveform if requested
    let (w, fp) = if gen_waveforms {
        if let Ok(mut player) = load_sndh(data, SAMPLE_RATE) {
            let _ = player.init_subsong(1);
            player.play(); // Must start playback before generating samples
            let dur = duration.unwrap_or(180.0);
            let wave_data = generate_waveform(&mut player, dur);
            (Some(wave_data.waveform), Some(wave_data.fingerprint))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Some(TrackMetadata {
        path,
        title,
        author,
        format: "SNDH".to_string(),
        year: meta.year.clone().filter(|s| !s.is_empty()),
        subsongs: meta.subsong_count.max(1) as u32,
        channels: 3,
        duration_seconds: duration,
        collection: collection.to_string(),
        w,
        fp,
    })
}

fn extract_ym_metadata(data: &[u8], path: String, collection: &str, artist_hint: Option<String>, file_path: &Path, gen_waveforms: bool) -> Option<TrackMetadata> {
    // Try to load as YM file
    let (mut player, summary) = load_song(data).ok()?;

    let info = player.info();

    let title = info.map(|i| i.song_name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            file_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    let author = info.map(|i| i.author.clone())
        .filter(|s| !s.is_empty())
        .or(artist_hint)
        .unwrap_or_else(|| "Unknown".to_string());

    let duration = player.get_duration_seconds();

    // Generate waveform if requested
    let (w, fp) = if gen_waveforms && duration > 0.0 {
        player.play(); // Must start playback before generating samples
        let wave_data = generate_waveform(&mut player, duration);
        (Some(wave_data.waveform), Some(wave_data.fingerprint))
    } else {
        (None, None)
    };

    Some(TrackMetadata {
        path,
        title,
        author,
        format: summary.format.to_string(),
        year: None,
        subsongs: 1,
        channels: 3,
        duration_seconds: if duration > 0.0 { Some(duration) } else { None },
        collection: collection.to_string(),
        w,
        fp,
    })
}

fn extract_ay_metadata(data: &[u8], path: String, collection: &str, artist_hint: Option<String>, gen_waveforms: bool) -> Option<TrackMetadata> {
    let (mut player, meta) = AyPlayer::load_from_bytes(data, 0).ok()?;

    let title = if meta.song_name.is_empty() {
        path.rsplit('/').next().unwrap_or(&path)
            .trim_end_matches(".ay")
            .trim_end_matches(".AY")
            .to_string()
    } else {
        meta.song_name.clone()
    };

    let author = if meta.author.is_empty() {
        artist_hint.unwrap_or_else(|| "Unknown".to_string())
    } else {
        meta.author.clone()
    };

    let duration = meta.frame_count.map(|f| f as f32 / 50.0);

    // Generate waveform if requested
    let (w, fp) = if gen_waveforms {
        if let Some(dur) = duration {
            let _ = player.play(); // Must start playback before generating samples
            let wave_data = generate_waveform(&mut player, dur);

            // Skip AY files that produce silence (likely Z80 emulation failures)
            if wave_data.fingerprint.amp < 0.001 {
                return None;
            }

            (Some(wave_data.waveform), Some(wave_data.fingerprint))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Some(TrackMetadata {
        path,
        title,
        author,
        format: "AY".to_string(),
        year: None,
        subsongs: meta.song_count as u32,
        channels: 3,
        duration_seconds: duration,
        collection: collection.to_string(),
        w,
        fp,
    })
}

fn extract_aks_metadata(data: &[u8], path: String, collection: &str, artist_hint: Option<String>, gen_waveforms: bool) -> Option<TrackMetadata> {
    let song = load_aks(data).ok()?;

    let title = if song.metadata.title.is_empty() {
        path.rsplit('/').next().unwrap_or(&path)
            .trim_end_matches(".aks")
            .trim_end_matches(".AKS")
            .to_string()
    } else {
        song.metadata.title.clone()
    };

    let author = if song.metadata.author.is_empty() {
        artist_hint.unwrap_or_else(|| "Unknown".to_string())
    } else {
        song.metadata.author.clone()
    };

    let duration = song.subsongs.first().map(|s| {
        s.end_position as f32 / s.replay_frequency_hz
    });

    // Channel count = PSG count * 3 channels per PSG
    let channels = song.subsongs.first()
        .map(|s| (s.psgs.len() * 3) as u32)
        .unwrap_or(3);

    // Generate waveform if requested
    let (w, fp) = if gen_waveforms {
        if let Some(dur) = duration {
            if let Ok(mut player) = ym2149_arkos_replayer::ArkosPlayer::new(song.clone(), 0) {
                let _ = player.play(); // Must start playback before generating samples
                let wave_data = generate_waveform(&mut player, dur);
                (Some(wave_data.waveform), Some(wave_data.fingerprint))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Some(TrackMetadata {
        path,
        title,
        author,
        format: "AKS".to_string(),
        year: None,
        subsongs: song.subsongs.len() as u32,
        channels,
        duration_seconds: duration,
        collection: collection.to_string(),
        w,
        fp,
    })
}

fn main() {
    let args = Args::parse();

    let base_path = args.base.unwrap_or_else(|| args.dir.clone());
    let gen_waveforms = args.waveforms;

    eprintln!("Scanning {}...", args.dir.display());
    if gen_waveforms {
        eprintln!("Waveform generation: ENABLED");
    }

    // Collect all files first
    let files: Vec<PathBuf> = WalkDir::new(&args.dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());
            matches!(ext.as_deref(), Some("ym") | Some("sndh") | Some("ay") | Some("aks"))
        })
        .map(|e| e.into_path())
        .collect();

    eprintln!("Found {} files to scan", files.len());

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let tracks: Mutex<Vec<TrackMetadata>> = Mutex::new(Vec::new());

    // Process files in parallel
    files.par_iter().for_each(|path| {
        if let Some(meta) = extract_metadata(path, &base_path, gen_waveforms) {
            tracks.lock().unwrap().push(meta);
        }
        pb.inc(1);
    });

    pb.finish_with_message("Scan complete");

    let mut tracks = tracks.into_inner().unwrap();

    // Sort: collection, author, title
    tracks.sort_by(|a, b| {
        let col_order = ["sndh", "ym", "ay", "arkos"];
        let col_a = col_order.iter().position(|&c| c == a.collection).unwrap_or(99);
        let col_b = col_order.iter().position(|&c| c == b.collection).unwrap_or(99);

        col_a.cmp(&col_b)
            .then_with(|| a.author.to_lowercase().cmp(&b.author.to_lowercase()))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });

    // Deduplicate by (collection, author, title) - keep first occurrence
    let before_dedup = tracks.len();
    tracks.dedup_by(|a, b| {
        a.collection == b.collection
            && a.author.to_lowercase() == b.author.to_lowercase()
            && a.title.to_lowercase() == b.title.to_lowercase()
    });
    let removed = before_dedup - tracks.len();
    if removed > 0 {
        eprintln!("Removed {} duplicates", removed);
    }

    // Count per collection
    let mut collection_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for track in &tracks {
        *collection_counts.entry(&track.collection).or_insert(0) += 1;
    }

    let collections = vec![
        CollectionInfo {
            id: "sndh".to_string(),
            name: "SNDH Collection".to_string(),
            description: "Atari ST/STE music from the SNDH archive".to_string(),
            format: "SNDH".to_string(),
            track_count: *collection_counts.get("sndh").unwrap_or(&0),
        },
        CollectionInfo {
            id: "ym".to_string(),
            name: "YM Collection".to_string(),
            description: "YM format chiptunes".to_string(),
            format: "YM".to_string(),
            track_count: *collection_counts.get("ym").unwrap_or(&0),
        },
        CollectionInfo {
            id: "ay".to_string(),
            name: "Project AY".to_string(),
            description: "ZX Spectrum AY music".to_string(),
            format: "AY".to_string(),
            track_count: *collection_counts.get("ay").unwrap_or(&0),
        },
        CollectionInfo {
            id: "arkos".to_string(),
            name: "Arkos Tracker".to_string(),
            description: "Arkos Tracker 2 songs".to_string(),
            format: "AKS".to_string(),
            track_count: *collection_counts.get("arkos").unwrap_or(&0),
        },
    ];

    let catalog = Catalog {
        version: "1.1".to_string(),
        generated: chrono::Utc::now().to_rfc3339(),
        collections: collections.into_iter().filter(|c| c.track_count > 0).collect(),
        tracks,
    };

    eprintln!("Writing {} tracks to {}", catalog.tracks.len(), args.output.display());

    let json = if args.pretty {
        serde_json::to_string_pretty(&catalog).unwrap()
    } else {
        serde_json::to_string(&catalog).unwrap()
    };

    fs::write(&args.output, &json).expect("Failed to write output");

    // Also write minified version
    if args.pretty {
        let min_path = args.output.with_extension("min.json");
        let min_json = serde_json::to_string(&catalog).unwrap();
        fs::write(&min_path, &min_json).expect("Failed to write minified output");
        eprintln!("Minified: {} ({:.1} KB)", min_path.display(), min_json.len() as f64 / 1024.0);
    }

    for col in &catalog.collections {
        eprintln!("  {}: {} tracks", col.name, col.track_count);
    }
}
