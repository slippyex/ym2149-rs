# Hardware Accuracy

This document describes how the ym2149-sndh-replayer emulates Atari ST/STE hardware and documents the accuracy of each component against real hardware specifications.

## Overview

The SNDH replayer emulates the following Atari ST/STE hardware:

| Component | Chip | Status |
|-----------|------|--------|
| Sound Generator | YM2149F | Verified |
| Multi-Function Peripheral | MC68901 MFP | Verified |
| DMA Audio | STE DAC | Verified |
| Audio Mixer | LMC1992 | Verified |
| CPU | MC68000 | Verified (via r68k/Musashi) |

## YM2149F PSG (Programmable Sound Generator)

The YM2149F is a 3-channel sound generator compatible with the AY-3-8910.

### Clock and Timing

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Master Clock | 2 MHz | ✓ Correct |
| Internal Divider | /8 | ✓ Correct |
| Internal Clock | 250 kHz | ✓ Correct |

### Tone Generator

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Period Range | 12-bit (0-4095) | ✓ Correct |
| Period 0 Behavior | Treated as 1 | ✓ Correct |
| Frequency Formula | f = master_clock / (16 × period) | ✓ Correct |
| Half-Amplitude | Period ≤ 1 outputs at 50% | ✓ Correct |

### Noise Generator

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| LFSR Width | 17-bit | ✓ Correct |
| LFSR Type | Galois (internal XOR) | ✓ Correct |
| Tap Positions | Bits 13 and 16 | ✓ Correct |
| Period Range | 5-bit (0-31) | ✓ Correct |
| Period 0 Behavior | Treated as 1 | ✓ Correct |
| Clock Rate | Half of tone rate | ✓ Correct |

**LFSR Algorithm:**
```
lsb = lfsr & 1
lfsr >>= 1
if lsb: lfsr ^= 0x12000  // taps at bits 13 and 16
```

Reference: [NESdev Forums - AY-3-8910 LFSR](https://archive.nes.science/nesdev-forums/f23/t18639.xhtml)

### Envelope Generator

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Period Range | 16-bit (0-65535) | ✓ Correct |
| Shape Register | 4-bit (16 values → 10 unique) | ✓ Correct |
| Step Rate | Same as tone | ✓ Correct |

**Envelope Shapes:**

| Register | Shape | Description |
|----------|-------|-------------|
| 0x00-0x03 | `\___` | Decay, hold low |
| 0x04-0x07 | `/___` | Attack, hold low |
| 0x08 | `\\\\` | Sawtooth down (continuous) |
| 0x09 | `\___` | Decay, hold low |
| 0x0A | `\/\/` | Triangle |
| 0x0B | `\¯¯¯` | Decay, hold high |
| 0x0C | `////` | Sawtooth up (continuous) |
| 0x0D | `/¯¯¯` | Attack, hold high |
| 0x0E | `/\/\` | Triangle inverted |
| 0x0F | `/___` | Attack, hold low |

### DAC Output

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Resolution | 5-bit (32 levels) | ✓ Correct |
| Curve | Logarithmic | ✓ Correct |
| DC Filtering | Sliding window | ✓ Correct |

### Mixer

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Tone Enable | Per-channel (inverted) | ✓ Correct |
| Noise Enable | Per-channel (inverted) | ✓ Correct |
| Output | Mono (all channels summed) | ✓ Correct |

### Register Map ($FF8800-$FF8803)

| Address | R/W | Function |
|---------|-----|----------|
| $FF8800 | R/W | Register select (active low accent) |
| $FF8802 | R/W | Register data |

## MC68901 MFP (Multi-Function Peripheral)

The MFP provides timers, interrupts, and GPIO for the Atari ST.

### Timer System

| Timer | Hardware Clock | Prescalers |
|-------|---------------|------------|
| Timer A | 2.4576 MHz | 4, 10, 16, 50, 64, 100, 200 |
| Timer B | 2.4576 MHz | 4, 10, 16, 50, 64, 100, 200 |
| Timer C | 2.4576 MHz | 4, 10, 16, 50, 64, 100, 200 |
| Timer D | 2.4576 MHz | 4, 10, 16, 50, 64, 100, 200 |

**Timer Frequency Calculation:**
```
f = 2457600 / (prescaler × data_register)
```

**Common SNDH Timer Rates:**

| Rate | Timer | Prescaler | Data | Actual Hz |
|------|-------|-----------|------|-----------|
| 50 Hz (VBL) | - | - | - | 50.0 |
| 200 Hz | A/B | 200 | 123 | 99.9 |
| 2 kHz | C/D | 50 | 25 | 1966.1 |

### Interrupt Registers

| Register | Address | Function |
|----------|---------|----------|
| GPIP | $FFFA01 | GPIO data |
| AER | $FFFA03 | Active edge (0=falling, 1=rising) |
| DDR | $FFFA05 | Data direction |
| IERA | $FFFA07 | Interrupt enable A |
| IERB | $FFFA09 | Interrupt enable B |
| IPRA | $FFFA0B | Interrupt pending A |
| IPRB | $FFFA0D | Interrupt pending B |
| ISRA | $FFFA0F | Interrupt in-service A |
| ISRB | $FFFA11 | Interrupt in-service B |
| IMRA | $FFFA13 | Interrupt mask A |
| IMRB | $FFFA15 | Interrupt mask B |
| VR | $FFFA17 | Vector register (bit 3 = S-bit for auto-EOI) |
| TACR | $FFFA19 | Timer A control |
| TBCR | $FFFA1B | Timer B control |
| TCDCR | $FFFA1D | Timer C/D control |
| TADR | $FFFA1F | Timer A data |
| TBDR | $FFFA21 | Timer B data |
| TCDR | $FFFA23 | Timer C data |
| TDDR | $FFFA25 | Timer D data |

### Implementation Notes

- Odd-address only access (matches real hardware)
- Proper interrupt acknowledge sequence
- End-of-interrupt handling (software and automatic via S-bit)
- Timer A/B event count modes supported
- GPI7 (mono detect) directly accessible

## STE DMA Audio (Microwire DAC)

The STE added DMA-driven audio playback capability.

### Sample Rates

| Register Value | Hardware Rate | Implementation |
|----------------|---------------|----------------|
| 0 | 6258 Hz | ✓ Correct |
| 1 | 12517 Hz | ✓ Correct |
| 2 | 25033 Hz | ✓ Correct |
| 3 | 50066 Hz | ✓ Correct |

### DMA Registers ($FF8900-$FF8921)

| Address | Function |
|---------|----------|
| $FF8901 | DMA control (bit 0 = enable, bit 1 = loop) |
| $FF8903 | Frame start address (high) |
| $FF8905 | Frame start address (mid) |
| $FF8907 | Frame start address (low) |
| $FF8909 | Frame counter (high) - read only |
| $FF890B | Frame counter (mid) - read only |
| $FF890D | Frame counter (low) - read only |
| $FF890F | Frame end address (high) |
| $FF8911 | Frame end address (mid) |
| $FF8913 | Frame end address (low) |
| $FF8921 | Sound mode (bit 7: 0=stereo, 1=mono; bits 1-0: rate) |

### Audio Format

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Sample Width | 8-bit signed | ✓ Correct |
| Channels | Mono or Stereo | ✓ Correct |
| Stereo Format | Interleaved L/R | ✓ Correct |
| DMA End Interrupt | Timer A / GPI7 | ✓ Correct |

## LMC1992 Audio Mixer

The LMC1992 provides volume and tone control via the Microwire interface.

### Microwire Protocol

| Parameter | Value |
|-----------|-------|
| Data Register | $FF8922 |
| Mask Register | $FF8924 |
| Command Width | 11 bits |
| Device Address | 10 (binary) |

**Command Format:**
```
Bit 10-9: Device address (10 for LMC1992)
Bit 8-6:  Function code
Bit 5-0:  Data value
```

### Function Codes

| Code | Function | Data Range | Description |
|------|----------|------------|-------------|
| 000 | Input Select | 0-3 | Audio source mixing |
| 001 | Bass | 0-12 | -12dB to +12dB @ 100Hz |
| 010 | Treble | 0-12 | -12dB to +12dB @ 10kHz |
| 011 | Master Volume | 0-40 | -80dB to 0dB (2dB steps) |
| 100 | Right Volume | 0-20 | -40dB to 0dB (2dB steps) |
| 101 | Left Volume | 0-20 | -40dB to 0dB (2dB steps) |

### Input Select (Mix Control)

| Value | Function | Notes |
|-------|----------|-------|
| 00 | DMA + YM2149 (-12dB) | -12dB broken on real HW (same as 01) |
| 01 | DMA + YM2149 | Default mode |
| 10 | DMA only | YM2149 muted |
| 11 | Reserved | Undefined behavior |

### Volume Curves

**Master Volume (0-40):**
```
dB = (value - 40) × 2
gain = 10^(dB / 20)
```

**Left/Right Volume (0-20):**
```
dB = (value - 20) × 2
gain = 10^(dB / 20)
```

### Bass/Treble EQ

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Bass Frequency | 118.3 Hz | ✓ Cascaded first-order low-shelf filters |
| Treble Frequency | 8439 Hz | ✓ Cascaded first-order high-shelf filters |
| Filter Slope | 12 dB/octave | ✓ Two-stage cascading |
| Range | ±12 dB | ✓ 2dB steps (13 levels) |
| Flat Setting | Value 6 | ✓ 0dB (unity gain) |

**Note:** The Atari STE uses 0.0068µF capacitors which affect the turnover frequencies.
Values empirically measured from real Atari STE hardware. Two cascaded first-order
shelving filters provide the characteristic 12dB/octave slope of the analog LMC1992.

Reference: [LMC1992 Datasheet](https://media.digikey.com/pdf/Data%20Sheets/National%20Semiconductor%20PDFs/LMC1992.pdf)

## MC68000 CPU

The CPU emulation is provided by r68k (based on Musashi).

### Timing

| Parameter | Hardware | Implementation |
|-----------|----------|----------------|
| Clock | 8 MHz | ✓ Correct |
| Bus Cycle | 4 cycles minimum | ✓ Correct (GLUE/MMU alignment) |
| Instruction Timing | Cycle-accurate tables | ✓ Correct (Musashi-derived) |

### Memory Map

| Range | Size | Device |
|-------|------|--------|
| $000000-$3FFFFF | 4 MB | RAM |
| $FF8800-$FF88FF | 256 B | YM2149 PSG |
| $FF8900-$FF8925 | 38 B | STE DMA Audio |
| $FF8922-$FF8925 | 4 B | LMC1992 Microwire |
| $FFFA00-$FFFA2F | 48 B | MC68901 MFP |

## Stereo Output

### Signal Path

```
YM2149 (mono) ──┬──► LMC1992 ──► Left Output
                │
STE DAC (L) ────┘

YM2149 (mono) ──┬──► LMC1992 ──► Right Output
                │
STE DAC (R) ────┘
```

**Important:** The YM2149 outputs a mono signal that is sent to both left and right channels equally. This matches real Atari ST/STE hardware. Some emulators offer "ACB stereo" (Channel A→Left, B→Center, C→Right) but this is **not** hardware-accurate.

## Known Limitations

### Not Implemented (Acceptable for SNDH)

- MFP USART (serial communication) - not used by SNDH
- MFP parallel port - not used by SNDH
- Blitter - not used by audio playback
- Video shifter - not needed for audio

### Simplifications

- STE DMA end interrupt routing slightly simplified (pulses both Timer A and GPI7)
- LMC1992 -12dB mix mode (value 00) treated same as normal mix (matches broken real HW)

## References

- [Atari ST Hardware Description](https://info-coach.fr/atari/hardware/STE-HW.php)
- [NESdev - AY-3-8910 LFSR Analysis](https://archive.nes.science/nesdev-forums/f23/t18639.xhtml)
- [LMC1992 Datasheet](https://media.digikey.com/pdf/Data%20Sheets/National%20Semiconductor%20PDFs/LMC1992.pdf)
- [SNDH v2.2 Specification](http://sndh.atari.org/)
