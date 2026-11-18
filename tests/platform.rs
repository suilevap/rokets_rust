//! Moving platforms: still, then out and back each half period.

use bevy::math::Vec2;
use tilerider::platform::MovingPlatform;

#[test]
fn a_platform_waits_half_a_period_then_goes_out_and_back() {
    let mut p = MovingPlatform::new(Vec2::ZERO, Vec2::new(6.0, 0.0), 4.0);
    let mut at = Vec2::ZERO;
    let mut run = |p: &mut MovingPlatform, secs: f32| {
        for _ in 0..(secs * 64.0).round() as usize {
            at += p.step(1.0 / 64.0) / 64.0;
        }
        at
    };
    assert_eq!(run(&mut p, 1.0), Vec2::ZERO, "still for the first half period");
    let out = run(&mut p, 3.0);
    assert!((out.x - 6.0).abs() < 0.2, "out after another half period: {out}");
    let back = run(&mut p, 2.0);
    assert!(back.x.abs() < 0.2, "back after the next: {back}");
}
