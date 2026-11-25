//! The magnet's rope: holds its cargo seven segments behind the car.

use bevy::math::Vec2;
use tilerider::gadget::rope::{LENGTH, MAX_LENGTH, POINTS, Rope};

#[test]
fn a_stretched_rope_pulls_the_cargo_towards_the_car() {
    let mut rope = Rope::new(Vec2::ZERO);
    rope.lay(Vec2::ZERO, Vec2::new(LENGTH * 2.0, 0.0));
    let mut force = Vec2::ZERO;
    for _ in 0..60 {
        force = rope.step(Vec2::ZERO, Vec2::new(LENGTH * 2.0, 0.0));
    }
    assert!(force.x < -1.0 && force.y.abs() < 1e-3, "{force}");
}

#[test]
fn like_a_tow_bar_it_keeps_the_cargo_seven_segments_away() {
    // C#'s eight points keep a segment apart: seven segments out the rope
    // rests; nearer it pushes the cargo out, gently; further it pulls,
    // the harder the further.
    let force = |d: f32| {
        let mut rope = Rope::new(Vec2::ZERO);
        let to = Vec2::new(d, 0.0);
        rope.lay(Vec2::ZERO, to);
        (0..120).fold(Vec2::ZERO, |_, _| rope.step(Vec2::ZERO, to)).x
    };
    let rest = LENGTH * (POINTS - 1) as f32 / POINTS as f32;
    assert_eq!(force(rest), 0.0);
    for d in [rest * 0.3, rest * 0.7] {
        let push = force(d);
        assert!(push > 0.0 && push < 50.0, "a gentle push at {d}: {push}");
    }
    assert!(force(LENGTH * 2.0) < force(LENGTH * 1.25) && force(LENGTH * 1.25) < 0.0, "a growing pull");
}

#[test]
fn the_far_end_reaches_no_further_than_the_rope_stretches() {
    let end = Rope::reach(Vec2::ZERO, Vec2::new(100.0, 0.0));
    assert!((end.x - MAX_LENGTH).abs() < 1e-4);
    assert_eq!(Rope::reach(Vec2::ZERO, Vec2::new(1.0, 1.0)), Vec2::new(1.0, 1.0));
}

#[test]
fn laid_on_a_line_the_points_are_spread_evenly_and_the_first_step_is_gentle() {
    let (from, to) = (Vec2::ZERO, Vec2::new(4.0, 0.0));
    let mut rope = Rope::new(Vec2::ZERO);
    rope.lay(from, to);
    let points = rope.points();
    assert_eq!((points[0], points[POINTS - 1]), (from, to));
    let gap = (points[1] - points[0]).length();
    for w in points.windows(2) {
        assert!(((w[1] - w[0]).length() - gap).abs() < 1e-5, "{points:?}");
    }
    let force = rope.step(from, to);
    assert!(force.x > -500.0 && force.x <= 0.0, "no yank: {force}");
}
