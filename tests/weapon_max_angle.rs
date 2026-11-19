//! `AdjustWeaponMaxAngle` (`weapon::max_angle`) and the wall rays it casts
//! (`MapGrid::ray_hits_wall`).

use std::f32::consts::PI;

use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::weapon::max_angle::{STEP, free_angle};

fn char_map(rows: &str) -> MapGrid {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

#[test]
fn an_open_gun_loses_the_last_step_of_its_angle() {
    // Nothing in the way: rays at 5°, 10° … 55° (60° is not under 60°).
    let angle = free_angle(PI / 3.0, |_| false);
    assert!((angle - 11.0 * STEP).abs() < 1e-5, "{}", angle.to_degrees());
}

#[test]
fn a_gun_boxed_in_ahead_may_not_turn_at_all() {
    assert_eq!(free_angle(PI / 3.0, |_| true), 0.0);
}

#[test]
fn the_wider_free_side_wins() {
    // A wall from 30° on the left (positive); the right is open.
    let angle = free_angle(PI / 3.0, |o| o >= 29.0_f32.to_radians());
    assert!((angle - 11.0 * STEP).abs() < 1e-5, "the open side, {}", angle.to_degrees());
    // Walls at 20° on the left and 30° on the right: 15° and 25°.
    let angle = free_angle(PI / 3.0, |o| (o > 0.0 && o >= 19.0_f32.to_radians()) || (o < 0.0 && o <= -29.0_f32.to_radians()));
    assert!((angle - 5.0 * STEP).abs() < 1e-5, "{}", angle.to_degrees());
}

#[test]
fn a_narrow_gun_is_stepped_up_to_its_own_angle() {
    // 5° of angle: no ray under it, so no free step either.
    assert_eq!(free_angle(5.0_f32.to_radians(), |_| false), 0.0);
    assert!((free_angle(16.0_f32.to_radians(), |_| false) - 3.0 * STEP).abs() < 1e-5);
}

#[test]
fn rays_meet_wall_cells() {
    // Cells of 2 units, y down. Row 2 is open from cell 1 to 5 with a wall
    // from cell 6 on (rows 1 and 3 have inside corners at its end).
    let grid = char_map("\n********\n*.....**\n*.....**\n*.....**\n********\n");
    let (cell, from) = (2.0, [3.0, 5.0]);
    // Right: the wall starts at x = 12, nine units away.
    assert!(!grid.ray_hits_wall(cell, from, [1.0, 0.0], 8.5));
    assert!(grid.ray_hits_wall(cell, from, [1.0, 0.0], 9.5));
    // Left: the frame at x = 2, a unit away.
    assert!(!grid.ray_hits_wall(cell, from, [-1.0, 0.0], 0.5));
    assert!(grid.ray_hits_wall(cell, from, [-1.0, 0.0], 1.5));
    // Past the map's edge is solid.
    assert!(grid.ray_hits_wall(cell, from, [1.0, 0.0], 40.0));
}

#[test]
fn rays_meet_the_wedge_in_an_inside_corner() {
    // The cell (1, 1) of an open room has walls above and to its left: its
    // corner there is filled by a wedge, the rest of the cell is open.
    let grid = char_map("\n*****\n*...*\n*...*\n*...*\n*****\n");
    assert_eq!(grid.edge(1, 1).map(|e| e.open), Some(2));
    let cell = 2.0;
    // Along the cell's far side (bottom right, open): free.
    assert!(!grid.ray_hits_wall(cell, [3.5, 3.5], [1.0, 0.0], 1.0));
    // A short ray across the wedge's corner (top left of the cell), from
    // the open cell below it: it turns up into the wedge.
    assert!(grid.ray_hits_wall(cell, [2.3, 3.8], [0.0, -1.0], 1.8));
    // A ray into the cell from the right at its middle height meets the
    // diagonal at half the cell.
    assert!(!grid.ray_hits_wall(cell, [5.9, 3.0], [-1.0, 0.0], 1.0));
    assert!(grid.ray_hits_wall(cell, [5.9, 3.0], [-1.0, 0.0], 3.9));
}
