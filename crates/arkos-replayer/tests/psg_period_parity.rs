use arkos_replayer::format::Note;
use arkos_replayer::psg;

fn period(psg_frequency: f64, reference_frequency: f64, note: Note) -> u16 {
    psg::calculate_period(psg_frequency, reference_frequency, note)
}

#[test]
fn psg_period_cpc() {
    let freq = 1_000_000.0;
    let reference = 440.0;

    assert_eq!(period(freq, reference, (0 * 12 + 0) as Note), 3822);
    assert_eq!(period(freq, reference, (0 * 12 + 1) as Note), 3608);
    assert_eq!(period(freq, reference, (0 * 12 + 2) as Note), 3405);
    assert_eq!(period(freq, reference, (0 * 12 + 3) as Note), 3214);
    assert_eq!(period(freq, reference, (0 * 12 + 11) as Note), 2025);
    assert_eq!(period(freq, reference, (1 * 12 + 0) as Note), 1911);
    assert_eq!(period(freq, reference, (1 * 12 + 8) as Note), 1204);
    assert_eq!(period(freq, reference, (4 * 12 + 0) as Note), 239);
    assert_eq!(period(freq, reference, (4 * 12 + 11) as Note), 127);
    assert_eq!(period(freq, reference, (7 * 12 + 0) as Note), 30);
    assert_eq!(period(freq, reference, (7 * 12 + 11) as Note), 16);
}

#[test]
fn psg_period_cpc_other_ref() {
    let freq = 1_000_000.0;
    let reference = 234.0;

    assert_eq!(period(freq, reference, (0 * 12 + 0) as Note), 7187);
    assert_eq!(period(freq, reference, (0 * 12 + 11) as Note), 3807);
    assert_eq!(period(freq, reference, (1 * 12 + 0) as Note), 3594);
    assert_eq!(period(freq, reference, (4 * 12 + 0) as Note), 449);
    assert_eq!(period(freq, reference, (7 * 12 + 0) as Note), 56);
    assert_eq!(period(freq, reference, (7 * 12 + 11) as Note), 30);
}

#[test]
fn psg_period_msx() {
    let freq = 1_789_773.0;
    let reference = 440.0;

    assert_eq!(period(freq, reference, (0 * 12 + 0) as Note), 6841);
    assert_eq!(period(freq, reference, (3 * 12 + 5) as Note), 641);
    assert_eq!(period(freq, reference, (4 * 12 + 0) as Note), 428);
    assert_eq!(period(freq, reference, (7 * 12 + 0) as Note), 53);
    assert_eq!(period(freq, reference, (7 * 12 + 11) as Note), 28);
}
