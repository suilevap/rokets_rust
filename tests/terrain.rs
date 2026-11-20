//! The floor plan the AI picks places from (`map::terrain`): the distance
//! field, the line of sight and the random picks of `MapManager`, without
//! the app.

use bevy::math::Vec2;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::{FREE_THRESHOLD, Terrain};

fn grid(rows: &str) -> MapGrid {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// Two rooms of 14 × 14 cells with a wall between them and a gap in it (a
/// door) at the bottom:
///
/// ```text
/// ******************************
/// *............*...............*
/// ...
/// *............................*
/// ```
fn two_rooms() -> MapGrid {
    let mut out = String::from("\n");
    let wide = 30;
    for y in 0..16 {
        for x in 0..wide {
            let edge = x == 0 || x == wide - 1 || y == 0 || y == 15;
            let divider = x == 15 && y < 13;
            out.push(if edge || divider { '*' } else { '.' });
        }
        out.push('\n');
    }
    grid(&out)
}

/// The world point in the middle of a map cell (cells are 1 unit, y up).
fn at(x: f32, y: f32) -> Vec2 {
    Vec2::new(x + 0.5, -(y + 0.5))
}

/// A stream of numbers in 0..1 that is the same every run.
fn stream(seed: u32) -> impl FnMut() -> f32 {
    let mut state = seed;
    move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[test]
fn the_field_is_positive_in_the_open_negative_in_walls_and_nothing_outside() {
    let terrain = Terrain::from_grid(&two_rooms(), 1.0);
    assert!(terrain.distance_at(at(7.0, 7.0)) > 5.0);
    assert!(terrain.distance_at(at(15.0, 3.0)) < 0.0, "inside the divider");
    assert_eq!(terrain.distance_at(at(0.0, 0.0)), 0.0, "the outer ring");
    assert_eq!(terrain.distance_at(Vec2::new(-5.0, 5.0)), 0.0, "outside");
    assert!(terrain.is_point_free(at(7.0, 7.0)));
    assert!(!terrain.is_point_free(at(14.0, 3.0)), "against the divider: under the threshold");
    assert!(terrain.distance_at(at(14.0, 3.0)) <= FREE_THRESHOLD);
}

#[test]
fn a_line_collides_exactly_as_the_grids_does() {
    let grid = two_rooms();
    let terrain = Terrain::from_grid(&grid, 1.0);
    let mut rand = stream(3);
    for _ in 0..2000 {
        let a = Vec2::new(rand() * 30.0, -rand() * 16.0);
        let b = Vec2::new(rand() * 30.0, -rand() * 16.0);
        assert_eq!(terrain.line_collides(a, b), grid.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]), "{a} {b}");
    }
    assert!(terrain.line_collides(at(5.0, 3.0), at(25.0, 3.0)), "through the divider");
    assert!(!terrain.line_collides(at(5.0, 14.0), at(25.0, 14.0)), "through the door");
    assert!(!terrain.line_collides(at(5.0, 5.0), at(5.0, 5.0)));
}

#[test]
fn free_points_are_roomy_free_and_six_cells_apart() {
    let grid = two_rooms();
    let terrain = Terrain::from_grid(&grid, 1.0);
    let points = terrain.free_points();
    assert!(points.len() >= 4, "{points:?}");
    for (i, p) in points.iter().enumerate() {
        assert!(terrain.distance_at(*p) > 1.0, "{p}");
        assert!(!grid.is_wall(p.x.floor() as i32, (-p.y).floor() as i32));
        for q in &points[i + 1..] {
            assert!(p.distance(*q) > 6.0, "{p} {q}");
        }
        // Roomiest first.
        if i > 0 {
            assert!(terrain.distance_at(points[i - 1]) >= terrain.distance_at(*p), "{points:?}");
        }
    }
}

#[test]
fn a_free_point_is_one_of_the_first_ten_near_or_the_last_or_none() {
    let terrain = Terrain::from_grid(&two_rooms(), 1.0);
    let near: Vec<Vec2> = terrain
        .free_points()
        .iter()
        .copied()
        .filter(|p| (p.x - 7.0).abs() < 100.0 && (p.y + 7.0).abs() < 100.0)
        .collect();
    assert!(near.len() > 2);
    // `Random(predicate, 10)`: index 3 is the third match, index 0 never
    // stops: the last of all.
    assert_eq!(terrain.random_free_point(at(7.0, 7.0), 100.0, &mut || 0.35), near[2]);
    assert_eq!(terrain.random_free_point(at(7.0, 7.0), 100.0, &mut || 0.05), *near.last().unwrap());
    // Index 9 is the ninth match, or the last when there are fewer.
    assert_eq!(terrain.random_free_point(at(7.0, 7.0), 100.0, &mut || 0.95), *near.get(8).unwrap_or(near.last().unwrap()));
    // None near: the origin, C#'s "no point".
    assert_eq!(terrain.random_free_point(Vec2::new(7.0, -7.0), 0.1, &mut || 0.5), Vec2::ZERO);
}

#[test]
fn a_random_visible_point_is_in_the_room_or_in_sight() {
    let terrain = Terrain::from_grid(&two_rooms(), 1.0);
    let mut rand = stream(11);
    for _ in 0..500 {
        let from = at(3.0 + (rand() * 9.0).floor(), 3.0 + (rand() * 9.0).floor());
        let spot = terrain.random_free_visible_point(from, 0.0, 9.0, &mut rand);
        let delta = (spot - from).abs();
        let room = terrain.distance_at(from);
        let in_room = delta.x <= room && delta.y <= room;
        assert!(in_room || !terrain.line_collides(from, spot), "{from} -> {spot}");
        assert!(delta.x <= 9.0 + 1e-3 && delta.y <= 9.0 + 1e-3, "{from} -> {spot}");
    }
}

#[test]
fn a_random_hidden_point_is_free_and_out_of_reach_or_behind_a_wall() {
    let terrain = Terrain::from_grid(&two_rooms(), 1.0);
    let mut rand = stream(5);
    let (pos, from) = (at(10.0, 6.0), at(20.0, 6.0));
    let mut behind = 0;
    for _ in 0..500 {
        let spot = terrain.random_free_non_visible_point(pos, from, 8.0, &mut rand);
        assert!(terrain.is_point_free(spot) || terrain.free_points().contains(&spot) || spot == Vec2::ZERO, "{spot}");
        // The ten tries found one (else a free point, which may be anywhere).
        if terrain.is_point_free(spot) && !terrain.free_points().contains(&spot) {
            let near = (spot - from).abs().max_element() < 8.0;
            assert!(!near || terrain.line_collides(from, spot), "{spot}");
            behind += usize::from(terrain.line_collides(from, spot));
        }
    }
    // The wall between the rooms hides the far side of the near one.
    assert!(behind > 100, "{behind}");
}

#[test]
fn the_field_is_in_world_units_and_the_map_hangs_below_the_origin() {
    // Cells of 2 units: the field is in units, and the rows go down the
    // world's y (C#'s y down, flipped once).
    let terrain = Terrain::from_grid(&two_rooms(), 2.0);
    assert!(terrain.distance_at(Vec2::new(2.0 * 7.5, -2.0 * 7.5)) > 10.0);
    assert!(terrain.distance_at(Vec2::new(2.0 * 7.5, 2.0 * 7.5)) == 0.0, "above the map is outside it");
}
