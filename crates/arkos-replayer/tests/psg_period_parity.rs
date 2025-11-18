#![cfg(feature = "extended-tests")]

use arkos_replayer::format::Note;
use arkos_replayer::psg;

fn period(psg_frequency: f64, reference_frequency: f64, note: Note) -> u16 {
    psg::calculate_period(psg_frequency, reference_frequency, note)
}

#[test]
fn psg_period_cpc() {
    let freq = 1_000_000.0;
    let reference = 440.0;

    assert_eq!(period(freq, reference, 0), 3822);
    assert_eq!(period(freq, reference, 1), 3608);
    assert_eq!(period(freq, reference, 2), 3405);
    assert_eq!(period(freq, reference, 3), 3214);
    assert_eq!(period(freq, reference, 11), 2025);
    assert_eq!(period(freq, reference, 12), 1911);
    assert_eq!(period(freq, reference, 20), 1204);
    assert_eq!(period(freq, reference, 48), 239);
    assert_eq!(period(freq, reference, 59), 127);
    assert_eq!(period(freq, reference, 84), 30);
    assert_eq!(period(freq, reference, 95), 16);
}

#[test]
fn psg_period_cpc_other_ref() {
    let freq = 1_000_000.0;
    let reference = 234.0;

    assert_eq!(period(freq, reference, 0), 7187);
    assert_eq!(period(freq, reference, 11), 3807);
    assert_eq!(period(freq, reference, 12), 3594);
    assert_eq!(period(freq, reference, 48), 449);
    assert_eq!(period(freq, reference, 84), 56);
    assert_eq!(period(freq, reference, 95), 30);
}

#[test]
fn psg_period_msx() {
    let freq = 1_789_773.0;
    let reference = 440.0;

    assert_eq!(period(freq, reference, 0), 6841);
    assert_eq!(period(freq, reference, 41), 641);
    assert_eq!(period(freq, reference, 48), 428);
    assert_eq!(period(freq, reference, 84), 53);
    assert_eq!(period(freq, reference, 95), 28);
}
