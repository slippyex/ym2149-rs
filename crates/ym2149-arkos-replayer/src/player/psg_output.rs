//! PSG register output handling.
//!
//! This module handles writing channel data to the YM2149 PSG registers,
//! including tone periods, volumes, mixer settings, noise, and hardware envelopes.

use super::sample_voice::HardwareEnvelopeState;
use crate::channel_player::{ChannelFrame, ChannelOutput};
use ym2149::ym2149::PsgBank;

/// Writes channel frames to PSG registers.
///
/// This follows the C++ SongPlayer behavior where:
/// 1. Per-channel registers (period, volume, envelope) are written individually
/// 2. Mixer and noise are accumulated across all 3 channels then written once per PSG
pub(crate) fn write_frames_to_psg(
    psg_bank: &mut PsgBank,
    frames: &[ChannelFrame],
    hardware_envelope_state: &mut [HardwareEnvelopeState],
) {
    for psg_idx in 0..psg_bank.psg_count() {
        let base_channel = psg_idx * 3;

        for ch_in_psg in 0..3 {
            let channel_idx = base_channel + ch_in_psg;
            if channel_idx < frames.len() {
                write_channel_registers(
                    psg_bank,
                    channel_idx,
                    &frames[channel_idx].psg,
                    hardware_envelope_state,
                );
            }
        }

        let start = base_channel.min(frames.len());
        write_mixer_for_psg(psg_bank, psg_idx, &frames[start..]);
    }
}

/// Write channel registers (period, volume) but NOT mixer or noise.
///
/// This matches C++ SongPlayer::fillWithChannelResult behavior.
fn write_channel_registers(
    psg_bank: &mut PsgBank,
    channel_idx: usize,
    output: &ChannelOutput,
    hardware_envelope_state: &mut [HardwareEnvelopeState],
) {
    let psg_idx = channel_idx / 3;
    let channel_in_psg = channel_idx % 3;

    if psg_idx >= psg_bank.psg_count() {
        return;
    }

    let psg = psg_bank.get_chip_mut(psg_idx);

    // 1. Write tone period (registers 0-5)
    // C++: psgRegistersToFill.setSoftwarePeriod(targetChannel, inputChannelRegisters.getSoftwarePeriod());
    let reg_base = channel_in_psg * 2;
    psg.write_register(reg_base as u8, (output.software_period & 0xFF) as u8);
    psg.write_register(
        (reg_base + 1) as u8,
        ((output.software_period >> 8) & 0x0F) as u8,
    );

    // 2. Write volume (registers 8-10)
    // C++: psgRegistersToFill.setVolume(targetChannel, volume0To16);
    if output.volume == 16 {
        // Hardware envelope mode
        psg.write_register((8 + channel_in_psg) as u8, 0x10);

        // 3. Write hardware envelope period (registers 11-12)
        // C++: psgRegistersToFill.setHardwarePeriod(hardwarePeriod);
        psg.write_register(11, (output.hardware_period & 0xFF) as u8);
        psg.write_register(12, ((output.hardware_period >> 8) & 0xFF) as u8);

        // 4. Write hardware envelope shape (register 13)
        // C++: psgRegistersToFill.setHardwareEnvelopeAndRetrig(hardwareEnvelope, retrig);
        if let Some(state) = hardware_envelope_state.get_mut(psg_idx) {
            let shape = output.hardware_envelope & 0x0F;
            let should_write = output.hardware_retrig || state.last_shape != shape;
            if should_write {
                psg.write_register(13, shape);
                state.last_shape = shape;
            }
        }
    } else {
        // Software volume (0-15)
        psg.write_register((8 + channel_in_psg) as u8, output.volume & 0x0F);
    }

    // NOTE: Mixer (register 7) and Noise (register 6) are written ONCE per PSG in write_mixer_for_psg
}

/// Write mixer and noise registers for all 3 channels of a PSG.
///
/// This matches C++ behavior where mixer is modified per-channel then written once.
///
/// C++ flow (in SongPlayer::fillWithChannelResult, called 3x per PSG):
/// 1. For each channel: setMixerNoiseState() - modifies mixer bits 3-5
/// 2. For each channel: setMixerSoundState() - modifies mixer bits 0-2
/// 3. Mixer register written ONCE after all channels processed
/// 4. Noise register written with last channel's noise value
fn write_mixer_for_psg(psg_bank: &mut PsgBank, psg_idx: usize, channel_frames: &[ChannelFrame]) {
    if psg_idx >= psg_bank.psg_count() {
        return;
    }

    let psg = psg_bank.get_chip_mut(psg_idx);

    // Start with all channels disabled (matches C++ PsgRegisters constructor: 0b111111)
    // Note: C++ uses 0x3F (0b00111111), NOT 0xFF!
    let mut mixer: u8 = 0x3F;
    let mut noise_value: u8 = 0;

    // Process all 3 channels - each modifies the same mixer variable
    // This matches C++ setMixerNoiseState() and setMixerSoundState()
    for (ch_in_psg, frame) in channel_frames.iter().take(3).enumerate() {
        let output = &frame.psg;
        // C++: psgRegistersToFill.setMixerNoiseState(targetChannel, isNoise);
        // C++: if (open) mixer &= ~(1 << bitRank); else mixer |= (1 << bitRank);
        if output.noise > 0 {
            // Noise enabled: clear bit (0 = enabled)
            mixer &= !(1 << (ch_in_psg + 3));
            // Save noise value - last channel with noise wins
            noise_value = output.noise;
        } else {
            // Noise disabled: set bit (1 = disabled)
            mixer |= 1 << (ch_in_psg + 3);
        }

        // C++: psgRegistersToFill.setMixerSoundState(targetChannel, isSound);
        // C++: if (open) mixer &= ~(1 << bitRank); else mixer |= (1 << bitRank);
        if output.sound_open {
            // Tone enabled: clear bit (0 = enabled)
            mixer &= !(1 << ch_in_psg);
        } else {
            // Tone disabled: set bit (1 = disabled)
            mixer |= 1 << ch_in_psg;
        }
    }

    // Write noise period (register 6) BEFORE mixer
    // C++: if (isNoise) psgRegistersToFill.setNoise(noise);
    if noise_value > 0 {
        psg.write_register(6, noise_value & 0x1F);
    }

    // Write mixer register (register 7) ONCE for all 3 channels
    psg.write_register(7, mixer);
}

/// Convert channel frames to PSG register array (for testing).
#[cfg(all(test, feature = "extended-tests"))]
pub(crate) fn frames_to_registers(
    psg_idx: usize,
    frames: &[ChannelFrame],
    prev: &mut [u8; 16],
) -> [u8; 16] {
    let base_channel = psg_idx * 3;
    let mut regs = *prev;
    let mut mixer: u8 = 0x3F;
    let mut noise_value: Option<u8> = None;

    for offset in 0..3 {
        let channel_idx = base_channel + offset;
        if let Some(frame) = frames.get(channel_idx) {
            let output = &frame.psg;
            regs[offset * 2] = (output.software_period & 0xFF) as u8;
            regs[offset * 2 + 1] = ((output.software_period >> 8) & 0x0F) as u8;

            if output.volume == 16 {
                regs[8 + offset] = 0x10;
                regs[11] = (output.hardware_period & 0xFF) as u8;
                regs[12] = ((output.hardware_period >> 8) & 0xFF) as u8;
                regs[13] = output.hardware_envelope & 0x0F;
            } else {
                regs[8 + offset] = output.volume & 0x0F;
            }

            if output.noise > 0 {
                mixer &= !(1 << (offset + 3));
                noise_value = Some(output.noise & 0x1F);
            } else {
                mixer |= 1 << (offset + 3);
            }

            if output.sound_open {
                mixer &= !(1 << offset);
            } else {
                mixer |= 1 << offset;
            }
        }
    }

    if let Some(noise) = noise_value {
        regs[6] = noise;
    }
    regs[7] = mixer;
    *prev = regs;
    regs
}
