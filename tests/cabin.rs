//! The cabin sways on the chassis and settles.

use bevy::math::Vec2;
use tilerider::car::cabin::{CabinSway, Spring};

const DT: f32 = 1.0 / 60.0;

#[test]
fn a_pushed_spring_swings_back_and_settles() {
    let mut s = Spring {
        offset: Vec2::new(0.2, 0.0),
        speed: Vec2::ZERO,
    };
    let friction = 0.98;
    let mut crossed = false;
    for _ in 0..120 {
        s.step(DT, friction);
        crossed |= s.offset.x < 0.0;
    }
    assert!(crossed, "it overshoots once let go");
    for _ in 0..600 {
        s.step(DT, friction);
    }
    assert!(s.offset.length() < 1e-3, "settled: {}", s.offset);
}

#[test]
fn speeding_up_leaves_the_cabin_behind_then_it_settles() {
    let mut sway = CabinSway::new(1.0, 1.0);
    // Facing up (+y), speeding up along it for a quarter second.
    let angle = std::f32::consts::FRAC_PI_2;
    for i in 1..=15 {
        sway.update(DT, angle, Vec2::new(0.0, i as f32 * 0.5), 0.0);
    }
    let (offset, turn) = sway.pose(angle);
    assert!(offset.x < -0.01, "lags behind in the car's frame: {offset}");
    assert!(offset.y.abs() < 1e-4 && turn.abs() < 1e-4);
    for _ in 0..600 {
        sway.update(DT, angle, Vec2::new(0.0, 7.5), 0.0);
    }
    assert!(sway.pose(angle).0.length() < 1e-3);
}

#[test]
fn turning_swings_the_cabin_round() {
    let mut sway = CabinSway::new(1.0, 1.0);
    for i in 1..=15 {
        sway.update(DT, 0.0, Vec2::ZERO, i as f32 * 0.3);
    }
    let (_, turn) = sway.pose(0.0);
    assert!(turn < -0.005, "spinning left, the cabin lags right: {turn}");
}
