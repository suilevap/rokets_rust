//! Clones along a path.

use bevy::math::Vec2;
use tilerider::actions::along;

#[test]
fn clones_spread_along_a_path_one_step_in_turned_from_its_way() {
    let path = [Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0)];
    let places = along(&path, 4, std::f32::consts::FRAC_PI_2);
    let points: Vec<Vec2> = places.iter().map(|p| p.0).collect();
    assert_eq!(points, [Vec2::new(2.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 2.0), Vec2::new(4.0, 4.0)]);
    // Going right, turned a quarter: facing up; going up: facing left.
    assert!((places[0].1 - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!((places[3].1 - std::f32::consts::PI).abs() < 1e-5);
    assert!(along(&path, 0, 0.0).is_empty());
}
