//! World units and map coordinates.

use bevy::math::{Vec2, Vec3};
use tilerider::units::{CellSize, from_cs, from_cs_rotation, map_degrees_to_world};

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

/// XNA's `CreateRotationX/Y/Z` on a row vector `v * M`, as C# applied them.
fn xna_rotate(v: Vec3, x: f32, y: f32, z: f32) -> Vec3 {
    let rx = |v: Vec3, a: f32| Vec3::new(v.x, v.y * a.cos() - v.z * a.sin(), v.y * a.sin() + v.z * a.cos());
    let ry = |v: Vec3, a: f32| Vec3::new(v.x * a.cos() + v.z * a.sin(), v.y, -v.x * a.sin() + v.z * a.cos());
    let rz = |v: Vec3, a: f32| Vec3::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos(), v.z);
    rz(ry(rx(v, x), y), z)
}

#[test]
fn csharp_turns_land_where_csharp_put_them() {
    let (x, y, z) = (0.3, -0.7, 1.1);
    for p in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(0.4, -1.3, 2.0)] {
        let theirs = from_cs(xna_rotate(p, x, y, z));
        let ours = from_cs_rotation(x, y, z) * from_cs(p);
        assert!((theirs - ours).length() < 1e-5, "{p}: {theirs} vs {ours}");
    }
    // A C# point below the floor (depth > 0), down the screen, is below
    // the ground and towards -y here.
    assert_eq!(from_cs(Vec3::new(1.0, 2.0, 3.0)), Vec3::new(1.0, -2.0, -3.0));
}
