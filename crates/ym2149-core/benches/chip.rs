//! Benchmarks for YM2149 chip hot path
//!
//! Run with: cargo bench --bench chip -p ym2149

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use ym2149::{Ym2149, Ym2149Backend};

fn bench_clock_iterations(c: &mut Criterion) {
    let mut group = c.benchmark_group("clock");

    // Setup chip with typical music settings
    let mut chip = Ym2149::new();
    chip.write_register(0, 0x10); // Tone A period low
    chip.write_register(1, 0x01); // Tone A period high (440 Hz)
    chip.write_register(7, 0x3E); // Mixer: enable tone A, disable noise
    chip.write_register(8, 0x0F); // Tone A volume max

    for iterations in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            iterations,
            |b, &iterations| {
                b.iter(|| {
                    for _ in 0..iterations {
                        chip.clock();
                        black_box(chip.get_sample());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_generate_samples(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_samples");

    let mut chip = Ym2149::new();
    chip.write_register(0, 0x10);
    chip.write_register(1, 0x01);
    chip.write_register(7, 0x3E);
    chip.write_register(8, 0x0F);

    for sample_count in [882, 4410, 44100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(sample_count),
            sample_count,
            |b, &sample_count| {
                b.iter(|| {
                    black_box(chip.generate_samples(sample_count));
                });
            },
        );
    }

    group.finish();
}

fn bench_register_updates(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    c.bench_function("write_register", |b| {
        b.iter(|| {
            chip.write_register(black_box(0), black_box(0x10));
            chip.write_register(black_box(1), black_box(0x01));
            chip.write_register(black_box(7), black_box(0x3E));
            chip.write_register(black_box(8), black_box(0x0F));
        });
    });
}

fn bench_music_frame(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    // Simulate a typical YM music frame update (R0-R15 = 16 registers)
    let frame_regs: [u8; 16] = [
        0x10, 0x01, // R0-R1: Tone A period (440 Hz)
        0x20, 0x02, // R2-R3: Tone B period (880 Hz)
        0x30, 0x03, // R4-R5: Tone C period (1320 Hz)
        0x10, // R6: Noise period
        0x3E, // R7: Mixer
        0x0F, // R8: Tone A volume
        0x0C, // R9: Tone B volume
        0x08, // R10: Tone C volume
        0x00, 0x10, // R11-R12: Envelope period
        0x09, // R13: Envelope shape
        0x00, 0x00, // R14-R15: I/O ports
    ];

    c.bench_function("music_frame_882_samples", |b| {
        b.iter(|| {
            // Load frame registers
            chip.load_registers(black_box(&frame_regs));

            // Generate one VBL frame (882 samples at 44.1kHz, 50Hz frame rate)
            for _ in 0..882 {
                chip.clock();
                black_box(chip.get_sample());
            }
        });
    });
}

fn bench_envelope_generation(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    // Setup for envelope testing
    chip.write_register(0, 0x10);
    chip.write_register(1, 0x01);
    chip.write_register(7, 0x3E); // Enable tone A
    chip.write_register(8, 0x10); // Volume controlled by envelope
    chip.write_register(11, 0x00); // Envelope period low
    chip.write_register(12, 0x10); // Envelope period high
    chip.write_register(13, 0x0E); // Envelope shape (triangular)

    c.bench_function("envelope_with_tone", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                chip.clock();
                black_box(chip.get_sample());
            }
        });
    });
}

fn bench_noise_generation(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    // Setup for noise testing
    chip.write_register(6, 0x10); // Noise period
    chip.write_register(7, 0x37); // Enable noise on channel A, B, C
    chip.write_register(8, 0x0F); // Max volume A
    chip.write_register(9, 0x0F); // Max volume B
    chip.write_register(10, 0x0F); // Max volume C

    c.bench_function("noise_three_channels", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                chip.clock();
                black_box(chip.get_sample());
            }
        });
    });
}

fn bench_mixed_tone_noise(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    // Setup for mixed tone + noise
    chip.write_register(0, 0x10);
    chip.write_register(1, 0x01);
    chip.write_register(6, 0x10); // Noise period
    chip.write_register(7, 0x36); // Enable tone+noise on channel A
    chip.write_register(8, 0x0F); // Max volume

    c.bench_function("tone_plus_noise", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                chip.clock();
                black_box(chip.get_sample());
            }
        });
    });
}

fn bench_channel_muting(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    // Setup all three channels
    chip.write_register(0, 0x10);
    chip.write_register(1, 0x01);
    chip.write_register(2, 0x20);
    chip.write_register(3, 0x02);
    chip.write_register(4, 0x30);
    chip.write_register(5, 0x03);
    chip.write_register(7, 0x38); // All tones enabled
    chip.write_register(8, 0x0F);
    chip.write_register(9, 0x0F);
    chip.write_register(10, 0x0F);

    c.bench_function("three_channels_with_muting", |b| {
        b.iter(|| {
            chip.set_channel_mute(1, black_box(true));
            for _ in 0..100 {
                chip.clock();
                black_box(chip.get_sample());
            }
            chip.set_channel_mute(1, black_box(false));
            for _ in 0..100 {
                chip.clock();
                black_box(chip.get_sample());
            }
        });
    });
}

fn bench_full_register_dump_load(c: &mut Criterion) {
    let mut chip = Ym2149::new();

    let regs: [u8; 16] = [
        0x10, 0x01, 0x20, 0x02, 0x30, 0x03, 0x10, 0x3E, 0x0F, 0x0C, 0x08, 0x00, 0x10, 0x09, 0x00,
        0x00,
    ];

    c.bench_function("register_dump_load_cycle", |b| {
        b.iter(|| {
            chip.load_registers(black_box(&regs));
            black_box(chip.dump_registers());
        });
    });
}

criterion_group!(
    benches,
    bench_clock_iterations,
    bench_generate_samples,
    bench_register_updates,
    bench_music_frame,
    bench_envelope_generation,
    bench_noise_generation,
    bench_mixed_tone_noise,
    bench_channel_muting,
    bench_full_register_dump_load
);
criterion_main!(benches);
