//! Buttons: a second on one presses it, leaving releases it.

use tilerider::button::{Press, Pressing};

/// Looks a quarter of a second apart, the same one standing (or nobody).
fn looks(p: &mut Press, standing: Option<Option<i64>>, n: usize) -> Vec<Pressing> {
    (0..n).filter_map(|_| p.look(standing)).collect()
}

#[test]
fn a_second_on_it_presses_it_once_with_a_light_a_quarter() {
    let mut p = Press::new(true);
    let car = Some(Some(4001));
    assert_eq!(looks(&mut p, car, 3), [Pressing::Light(1), Pressing::Light(2), Pressing::Light(3)]);
    assert!(!p.pressed);
    assert_eq!(looks(&mut p, car, 3), [Pressing::Press(Some(4001))]);
    assert!(p.pressed);
    assert_eq!(looks(&mut p, None, 2), [Pressing::Release(Some(4001))]);
    assert!(!p.pressed && p.charge == 0.0);
}

#[test]
fn leaving_early_starts_again() {
    let mut p = Press::new(true);
    looks(&mut p, Some(Some(1)), 3);
    assert_eq!(looks(&mut p, None, 1), [], "not pressed: nothing to release");
    assert_eq!(p.lights, 0);
    assert_eq!(looks(&mut p, Some(Some(2)), 5).last(), Some(&Pressing::Press(Some(2))));
}

#[test]
fn without_a_release_it_stays_pressed() {
    let mut p = Press::new(false);
    looks(&mut p, Some(Some(1)), 5);
    assert_eq!(looks(&mut p, None, 2), []);
    assert!(p.pressed);
    assert_eq!(looks(&mut p, Some(Some(1)), 8), [], "pressed once only");
}
