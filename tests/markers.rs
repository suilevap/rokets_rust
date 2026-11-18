//! Goal markers: an arrow off the car towards the goal, when it is far.

use bevy::math::Vec2;
use tilerider::markers::arrow;

#[test]
fn the_arrow_points_at_far_goals_and_fades_in() {
    assert_eq!(arrow(Vec2::ZERO, 1.0, Vec2::new(3.0, 0.0)), None, "near: no arrow");
    let (at, angle, alpha) = arrow(Vec2::ZERO, 1.0, Vec2::new(0.0, 8.0)).unwrap();
    assert!((at - Vec2::new(0.0, 2.25)).length() < 1e-5, "the car's radius + 40 px out: {at}");
    assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!((alpha - 0.5).abs() < 1e-5, "half faded in 4 units past the scan");
    assert_eq!(arrow(Vec2::ZERO, 1.0, Vec2::new(100.0, 0.0)).unwrap().2, 1.0);
}
