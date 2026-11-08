//! World units and map coordinates.

use bevy::math::Vec2;
use tilerider::units::{CellSize, map_degrees_to_world};

#[test]
fn converts_map_cells_to_the_world_and_back() {
    let cell = CellSize::from_map_scale(64.0);
    assert_eq!(cell.to_world(9.5, 3.5), Vec2::new(19.0, -7.0));
    assert_eq!(cell.to_cells(Vec2::new(19.0, -7.0)), (9.5, 3.5));
}

#[test]
fn map_directions_turn_counter_clockwise() {
    // As the C# game read them: angle = -DirectionDeg with y down.
    let up = Vec2::from_angle(map_degrees_to_world(90.0));
    assert!((up - Vec2::Y).length() < 1e-6, "{up}");
    let right = Vec2::from_angle(map_degrees_to_world(0.0));
    assert!((right - Vec2::X).length() < 1e-6);
}
