//! Shadows of moving things: which lamp throws them, and their shape.

use bevy::math::Vec2;
use tilerider::content::map::MapData;
use tilerider::light::bake::{Lamp, LightFrame};
use tilerider::light::shadow_lamps::ShadowLamps;
use tilerider::map::grid::MapGrid;
use tilerider::render::shadow::streak_shape;

const CELL: f32 = 2.0;

fn grid(rows: &str) -> MapGrid {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    MapGrid::build(&map, None)
}

fn lamp(x: f32, y: f32, radius: f32, dynamic: bool) -> Lamp {
    Lamp {
        x,
        y,
        radius,
        color: [1.0; 3],
        use_for_dynamic_shadow: dynamic,
        additive_rate: 0.0,
    }
}

const ROOM: &str = "
********************
*..................*
*..................*
*..................*
*........*.........*
*........*.........*
*........*.........*
********************
";

fn lamps_of(lamps: &[Lamp]) -> (MapGrid, ShadowLamps) {
    let grid = grid(ROOM);
    let frame = LightFrame::new(grid.width, grid.height, CELL);
    let shadows = ShadowLamps::build(&grid, &frame, lamps);
    (grid, shadows)
}

#[test]
fn the_nearer_lamp_that_sees_a_spot_throws_its_shadows() {
    let (_, lamps) = lamps_of(&[lamp(3.0, 2.0, 30.0, true), lamp(16.0, 2.0, 30.0, true)]);
    assert_eq!(lamps.lamp_at(5.5, 2.5, CELL), Some(0));
    assert_eq!(lamps.lamp_at(14.5, 2.5, CELL), Some(1));
    // Behind the pillar from the first lamp: only the second sees it.
    assert_eq!(lamps.lamp_at(10.5, 6.5, CELL), Some(1));
}

#[test]
fn only_lamps_for_dynamic_shadows_count_and_only_within_reach() {
    let (_, lamps) = lamps_of(&[lamp(3.0, 2.0, 4.0, true), lamp(16.0, 2.0, 30.0, false)]);
    assert_eq!(lamps.lamp_at(4.5, 2.5, CELL), Some(0));
    assert_eq!(lamps.lamp_at(14.5, 2.5, CELL), None, "out of the first's box, the second casts none");
    assert_eq!(lamps.lamp_at(-1.0, 2.5, CELL), None, "off the map");
}

#[test]
fn the_streak_points_away_from_the_lamp_and_fades_at_both_ends() {
    let car = Vec2::new(2.0, 1.2);
    let at = |d: f32| streak_shape(Vec2::new(d, 0.0), 0.0, car, Vec2::ZERO, 40.0, 100.0);
    let mid = at(20.0).expect("halfway out");
    assert!(mid.dir.abs() < 1e-5, "away from the lamp: {}", mid.dir);
    assert!((mid.alpha - 1.0).abs() < 1e-5, "darkest halfway: {}", mid.alpha);
    let near = at(8.0).unwrap();
    let far = at(37.0).unwrap();
    assert!(near.alpha < mid.alpha && far.alpha < mid.alpha);
    assert!(far.texel.x < mid.texel.x, "shorter near the radius");
    assert!(at(40.5).is_none(), "beyond the radius");

    let behind = streak_shape(Vec2::new(-20.0, 0.0), 0.0, car, Vec2::ZERO, 40.0, 100.0).unwrap();
    assert!((behind.dir.abs() - std::f32::consts::PI).abs() < 1e-5);
}

#[test]
fn side_on_casters_throw_wider_streaks() {
    let car = Vec2::new(2.0, 1.0);
    let shape = |angle: f32| streak_shape(Vec2::new(10.0, 0.0), angle, car, Vec2::ZERO, 20.0, 100.0).unwrap();
    let end_on = shape(0.0);
    let side_on = shape(std::f32::consts::FRAC_PI_2);
    let diagonal = shape(std::f32::consts::FRAC_PI_4 + 0.46);
    assert!(side_on.texel.y > end_on.texel.y * 1.9);
    assert!(diagonal.texel.y > side_on.texel.y, "the diagonal is widest");
}

#[test]
fn close_to_the_lamp_the_streak_splits_then_fades_out() {
    let car = Vec2::new(2.0, 1.2);
    let at = |d: f32| streak_shape(Vec2::new(d, 0.0), 0.0, car, Vec2::ZERO, 20.0, 100.0);
    assert_eq!(at(18.0).unwrap().split, 0.0, "far: one streak");
    let close = at(5.0).unwrap();
    assert!(close.split > 0.0, "close: a penumbra pair");
    assert!(at(0.6).is_none(), "right under the lamp: none");
}

#[test]
fn walls_cut_the_streak_short() {
    let car = Vec2::new(2.0, 1.2);
    let open = streak_shape(Vec2::new(10.0, 0.0), 0.0, car, Vec2::ZERO, 20.0, 100.0).unwrap();
    let walled = streak_shape(Vec2::new(10.0, 0.0), 0.0, car, Vec2::ZERO, 20.0, 1.0).unwrap();
    assert!(walled.texel.x < open.texel.x / 2.0);
}
