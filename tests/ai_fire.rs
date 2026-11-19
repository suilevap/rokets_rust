//! `AlwaysShoot` and `ShootPattern` (`ai::fire`), and what the weapons
//! make of a trigger held by an enemy: the Minigun's bursts and reload, the
//! Railgun's pulses, no energy spent (`EnergyComponent` for `!LocalDevice`).

use tilerider::ai::fire::{BEAT, PATTERNS, ShootPattern};
use tilerider::weapon::model::{MINIGUN, Minigun, RAILGUN, Railgun, Shot};

const DT: f32 = 1.0 / 60.0;

#[test]
fn patterns_are_cut_into_silences_and_fires_of_quarter_seconds() {
    // "0000 1 0 1 0 1111": 1 s silent, then alternating beats, ending 1 s fire.
    let p = ShootPattern::new("0000 1 0 1 0 1111");
    assert_eq!(p.timing(), [4.0 * BEAT, BEAT, BEAT, BEAT, BEAT, 4.0 * BEAT]);
    // Starting with fire: a silence of no length first.
    let p = ShootPattern::new("1111 0 1 0000");
    assert_eq!(p.timing(), [0.0, 4.0 * BEAT, BEAT, BEAT, 4.0 * BEAT]);
    let p = ShootPattern::new("0000 1111 00 1111");
    assert_eq!(p.timing(), [4.0 * BEAT, 4.0 * BEAT, 2.0 * BEAT, 4.0 * BEAT]);
    assert_eq!(PATTERNS.len(), 4);
}

#[test]
fn the_trigger_is_held_in_the_fire_phases_only() {
    let mut p = ShootPattern::new("0000 1111");
    let mut held = Vec::new();
    for _ in 0..(2.0 * 2.0 / DT) as usize {
        held.push(p.step(DT));
    }
    let seconds = |on: bool| held.iter().filter(|h| **h == on).count() as f32 * DT;
    // Two rounds: two seconds silent, two firing, a step each way of slack.
    assert!((seconds(false) - 2.0).abs() < 0.1, "{}", seconds(false));
    assert!((seconds(true) - 2.0).abs() < 0.1, "{}", seconds(true));
    // The first second is quiet.
    assert!(held[..50].iter().all(|h| !h));
    assert!(held[70..90].iter().all(|h| *h), "firing from 1 s to 2 s");
}

#[test]
fn a_pattern_that_starts_with_fire_fires_at_once() {
    let mut p = ShootPattern::new("1111 0 1 0000");
    assert!(p.step(DT), "the zero-length silence is skipped in a step");
}

/// Holds the trigger for `secs` of an enemy's Minigun (no energy), the
/// seconds of every bullet.
fn minigun_bullets(secs: f32) -> Vec<f32> {
    let mut gun = Minigun::new(&MINIGUN, 0.0);
    let mut times = Vec::new();
    for i in 0..(secs / DT) as usize {
        gun.pull(&MINIGUN, None);
        if let Some(Shot::Bullet { .. }) = gun.step(DT, &MINIGUN, None, || 0.5) {
            times.push(i as f32 * DT);
        }
    }
    times
}

#[test]
fn an_enemy_minigun_fires_ten_bullets_then_waits_out_its_reload() {
    let times = minigun_bullets(6.0);
    // A burst of ten a tenth of a second apart, every 1.4 s.
    let burst = |from: f32, to: f32| times.iter().filter(|t| **t >= from && **t < to).count();
    assert_eq!(burst(0.0, 1.4), 10, "{times:?}");
    assert_eq!(burst(1.4, 2.8), 10);
    assert_eq!(burst(2.8, 4.2), 10);
    // They are spread over a second, then 0.4 s of quiet.
    let first: Vec<f32> = times.iter().cloned().filter(|t| *t < 1.4).collect();
    assert!(first.last().unwrap() - first[0] < 1.0 + 0.05);
    assert!(first.last().unwrap() - first[0] > 0.85);
    assert!(times.iter().all(|t| (*t % 1.4) < 1.05 || (*t % 1.4) > 1.39), "quiet 1.05..1.4 s into each round");
}

#[test]
fn an_enemy_railgun_fires_once_a_reload() {
    let mut gun = Railgun::new(&RAILGUN, 0.0);
    let mut shots = Vec::new();
    for i in 0..(6.0 / DT) as usize {
        if gun.pull(&RAILGUN, None) {
            shots.push(i as f32 * DT);
        }
        gun.step(DT);
    }
    assert_eq!(shots.len(), 4, "{shots:?}");
    for pair in shots.windows(2) {
        assert!((pair[1] - pair[0] - 1.5).abs() < 0.05, "{shots:?}");
    }
}
