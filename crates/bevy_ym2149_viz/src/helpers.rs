pub(crate) use ym2149_common::channel_period as get_channel_period;
pub(crate) use ym2149_common::period_to_frequency;

pub(crate) const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub(crate) fn frequency_to_note(freq: f32) -> Option<String> {
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

pub(crate) fn format_note_label(note: Option<&str>) -> String {
    let value = match note {
        Some(n) if !n.is_empty() => n,
        _ => "--",
    };
    format!("{value:^4}")
}

pub(crate) fn format_freq_label(freq: Option<f32>) -> String {
    if let Some(value) = freq {
        format!("{value:>6.1}Hz")
    } else {
        " --.-Hz".to_string()
    }
}
