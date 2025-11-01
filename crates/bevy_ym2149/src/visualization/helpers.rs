pub(super) const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;
pub(super) const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub(super) fn get_channel_period(lo: u8, hi: u8) -> Option<u16> {
    let period = (((hi as u16) & 0x0F) << 8) | (lo as u16);
    if period == 0 {
        None
    } else {
        Some(period)
    }
}

pub(super) fn period_to_frequency(period: u16) -> f32 {
    PSG_MASTER_CLOCK_HZ / (16.0 * period as f32)
}

pub(super) fn frequency_to_note(freq: f32) -> Option<String> {
    if !freq.is_finite() || freq <= 0.0 {
        return None;
    }
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    let midi_rounded = midi.round();
    if !(0.0..=127.0).contains(&midi_rounded) {
        return None;
    }
    let midi_int = midi_rounded as i32;
    let note_index = ((midi_int % 12) + 12) % 12;
    let octave = (midi_int / 12) - 1;
    Some(format!("{}{}", NOTE_NAMES[note_index as usize], octave))
}

pub(super) fn format_note_label(note: Option<&str>) -> String {
    let value = match note {
        Some(n) if !n.is_empty() => n,
        _ => "--",
    };
    format!("{:^4}", value)
}

pub(super) fn format_freq_label(freq: Option<f32>) -> String {
    if let Some(value) = freq {
        format!("{:>6.1}Hz", value)
    } else {
        " --.-Hz".to_string()
    }
}
