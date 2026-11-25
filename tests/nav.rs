//! The navigation graph and the path (`map::nav`): what `NavGraph` and
//! `NavPath` of the C# game did, without the app.

mod common;

use bevy::math::Vec2;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::nav::rects::{IRect, extract_all_rectangles};
use tilerider::map::nav::{Blocker, NavGraph, NavPath, Orientation};

fn grid(rows: &str) -> MapGrid {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// A map cell's middle in the world (cells are 1 unit, y up).
fn at(x: f32, y: f32) -> Vec2 {
    Vec2::new(x + 0.5, -(y + 0.5))
}

/// Two rooms joined by a corridor:
///
/// ```text
/// ***********
/// *....*....*
/// *....*....*
/// *.........*
/// *....*....*
/// *....*....*
/// ***********
/// ```
const TWO_ROOMS: &str = "\n***********\n*....*....*\n*....*....*\n*.........*\n*....*....*\n*....*....*\n***********\n";

fn free(g: &MapGrid, p: Vec2) -> bool {
    !g.is_wall(p.x.floor() as i32, (-p.y).floor() as i32)
}

#[test]
fn rectangles_start_where_the_priority_is_highest_and_cover_each_cell_once() {
    // 4 × 3, all usable, the highest priority at (1, 1): one rectangle.
    let mut priority = vec![1.0; 12];
    priority[5] = 2.0;
    let rects = extract_all_rectangles(4, 3, &priority, |_, _, _| true);
    assert_eq!(rects, vec![IRect::new(0, 0, 4, 3)]);

    // A wall column splits them; the left part is taken first, from the
    // highest cell, widening right, down, left, up.
    let usable = |x: i32, _: i32, _: f32| x != 2;
    let rects = extract_all_rectangles(5, 3, &[1.0; 15], usable);
    // Ties are taken in row order: from (0, 0), which grows right and down.
    assert_eq!(rects[0], IRect::new(0, 0, 2, 3));
    let covered: i32 = rects.iter().map(|r| r.w * r.h).sum();
    assert_eq!(covered, 12);
}

#[test]
fn rectangle_arithmetic_is_xnas() {
    let a = IRect::new(0, 0, 4, 4);
    let b = IRect::new(4, 0, 4, 4);
    assert!(!a.intersects(&b), "touching edges do not intersect");
    assert_eq!(a.intersection(&b), IRect::default());
    let c = IRect::new(3, 1, 4, 4);
    assert_eq!(a.intersection(&c), IRect::new(3, 1, 1, 3));
    assert_eq!(IRect::new(1, 1, 3, 5).center(), (2, 3), "the integer middle");
    let mut r = IRect::new(2, 2, 1, 1);
    r.inflate(1, 1);
    assert_eq!(r, IRect::new(1, 1, 3, 3));
}

#[test]
fn a_straight_way_inside_one_area() {
    let g = grid(TWO_ROOMS);
    let nav = NavGraph::from_grid(&g, 1.0);
    let (a, b) = (at(1.0, 1.0), at(3.0, 4.0));
    let path = nav.get_path(a, b).expect("a path");
    assert_eq!(path.points(), &[a, b]);
    assert!(path.blockers().is_empty());
}

#[test]
fn the_path_goes_through_the_corridor_and_only_over_floor() {
    let g = grid(TWO_ROOMS);
    let nav = NavGraph::from_grid(&g, 1.0);
    let (a, b) = (at(1.0, 1.0), at(8.0, 5.0));
    let path = nav.get_path(a, b).expect("a path");
    let points = path.points();
    assert_eq!(points[0], a);
    assert_eq!(*points.last().unwrap(), b);
    // It passes the corridor (row 3, the gap in the middle wall).
    assert!(points.iter().any(|p| (-p.y - 3.5).abs() < 0.6 && (4.0..6.0).contains(&p.x)), "{points:?}");
    for p in points {
        assert!(free(&g, *p), "{p:?} is in a wall");
    }
    // Every leg is clear of walls (the points are what the car steers at).
    for pair in points.windows(2) {
        assert!(!g.line_hits_wall((pair[0].x, -pair[0].y), (pair[1].x, -pair[1].y)), "{pair:?}");
    }
    assert!(path.full_length() >= a.distance(b));
}

#[test]
fn no_path_from_outside_the_map_or_from_a_wall() {
    let g = grid(TWO_ROOMS);
    let nav = NavGraph::from_grid(&g, 1.0);
    assert!(nav.get_path(Vec2::new(-5.0, 5.0), at(1.0, 1.0)).is_none());
    assert!(nav.get_path(at(0.0, 0.0), at(1.0, 1.0)).is_none(), "a wall is in no area");
    assert!(nav.get_path(at(1.0, 1.0), at(0.0, 0.0)).is_none());
    assert!(nav.get_nearest(Vec2::new(-5.0, 5.0)).is_none());
}

#[test]
fn separate_rooms_have_no_path() {
    let g = grid("\n*******\n*.*...*\n*.*...*\n*******\n");
    let nav = NavGraph::from_grid(&g, 1.0);
    assert!(nav.get_path(at(1.0, 1.0), at(4.0, 1.0)).is_none());
}

#[test]
fn a_crossing_between_areas_is_crossed_straight() {
    let g = grid(TWO_ROOMS);
    let nav = NavGraph::from_grid(&g, 1.0);
    // Some node sits in the overlap of two areas and has an axis.
    assert!(nav.nodes().iter().any(|n| n.orientation != Orientation::None));
    let path = nav.get_path(at(1.0, 1.0), at(8.0, 5.0)).unwrap();
    // A crossing adds two points on either side of itself, half a cell
    // apart each way: a pair of points a cell apart on the same row or column.
    let pairs = path.points().windows(2).filter(|w| (w[0].distance(w[1]) - 1.0).abs() < 1e-5).count();
    assert!(pairs >= 1, "{:?}", path.points());
}

/// Two rooms with a wide corridor between them (three rows): the corridor
/// touches both rooms and each of its two crossings has an axis.
const WIDE_CORRIDOR: &str = "\n*************\n*.....*.....*\n*.....*.....*\n*...........*\n*...........*\n*...........*\n*.....*.....*\n*.....*.....*\n*************\n";

#[test]
fn both_crossings_of_a_corridor_between_two_rooms_have_their_own_axis() {
    let g = grid(WIDE_CORRIDOR);
    let nav = NavGraph::from_grid(&g, 1.0);
    let unoriented = nav.nodes().iter().filter(|n| n.orientation == Orientation::None).count();
    assert_eq!(unoriented, 0, "{:?}", nav.nodes().iter().map(|n| (n.pos, n.orientation)).collect::<Vec<_>>());
    // The crossings are the vertical walls' gaps: crossed along x.
    assert!(nav.nodes().iter().all(|n| n.orientation == Orientation::Horizontal || n.orientation == Orientation::Vertical));
}

/// A pillar in an open room: two ways round it.
const PILLAR: &str = "\n*********\n*.......*\n*.......*\n*...*...*\n*.......*\n*.......*\n*********\n";

#[test]
fn weights_make_a_way_dear() {
    let g = grid(PILLAR);
    let mut nav = NavGraph::from_grid(&g, 1.0);
    let (a, b) = (at(1.0, 3.0), at(7.0, 3.0));
    let side = |p: &NavPath| p.points().iter().map(|p| -p.y).sum::<f32>() / p.points().len() as f32;
    let base = nav.get_path(a, b);
    if let Some(base) = base {
        // Make the whole upper half dear: it goes below.
        nav.set_weight(50.0, Vec2::new(2.0, -3.0), Vec2::new(6.0, 0.0));
        let dear = nav.get_path(a, b).expect("a path");
        assert!(side(&dear) >= side(&base), "{} then {}", side(&base), side(&dear));
        // `SetWeight` to the weight it has does nothing; back to 1 restores.
        nav.set_weight(1.0, Vec2::new(2.0, -3.0), Vec2::new(6.0, 0.0));
        let again = nav.get_path(a, b).unwrap();
        assert_eq!(again.points(), base.points());
    }
}

/// Two rooms with an alcove each and nothing between them.
const ALCOVES: &str = "\n***********\n*....*....*\n*....*....*\n*....*....*\n**.*****.**\n**.*****.**\n***********\n";

#[test]
fn linked_points_can_be_blocked_and_the_path_says_so() {
    let g = grid(ALCOVES);
    let mut nav = NavGraph::from_grid(&g, 1.0);
    let (a, b) = (at(1.0, 1.0), at(8.0, 3.0));
    assert!(nav.get_path(a, b).is_none(), "no way between the rooms");
    // A door in the wall between them.
    let (door_a, door_b) = (at(4.0, 2.0), at(6.0, 2.0));
    let links = nav.link_points(door_a, door_b, false, Some(Blocker(7)));
    assert_eq!(links.len(), 2, "both ways");
    for l in &links {
        nav.set_link(*l, 1.0, false);
    }
    let open = nav.get_path(a, b).expect("through the open door");
    assert!(open.points().contains(&door_a) && open.points().contains(&door_b), "{:?}", open.points());
    assert!(open.blockers().is_empty());
    // Closed, the search still takes it (it is the only way), and the path
    // lists what blocks it, once for the one link it takes.
    for l in &links {
        nav.set_link(*l, 32.0, true);
    }
    let closed = nav.get_path(a, b).expect("the search ignores a closed door");
    assert_eq!(closed.blockers(), &[Some(Blocker(7))]);
    // One way: only one link.
    assert_eq!(nav.link_points(at(1.0, 1.0), at(8.0, 3.0), true, None).len(), 1);
}

#[test]
fn a_door_in_an_area_with_no_nodes_leads_nowhere() {
    // `LinkPoints` joins its points to the nodes of their areas only; two
    // rooms that have none stay unreachable, as in C#.
    let g = grid("\n*********\n*...*...*\n*...*...*\n*********\n");
    let mut nav = NavGraph::from_grid(&g, 1.0);
    nav.link_points(at(3.0, 1.0), at(5.0, 1.0), false, None);
    assert!(nav.get_path(at(1.0, 1.0), at(6.0, 1.0)).is_none());
}

#[test]
fn get_nodes_is_a_square_around_the_point() {
    let g = grid(TWO_ROOMS);
    let nav = NavGraph::from_grid(&g, 1.0);
    let near: Vec<_> = nav.get_nodes(at(5.0, 3.0), 1.2).collect();
    assert!(!near.is_empty());
    assert!(near.iter().all(|n| (n.pos.x - 5.5).abs() < 1.2 && (n.pos.y + 3.5).abs() < 1.2));
    assert_eq!(nav.get_nodes(at(5.0, 3.0), 0.0).count(), 0);
}

#[test]
fn every_singleplayer_map_builds_and_connects_its_floor() {
    let maps = common::assets_root().join("Maps/Singleplayer");
    let mut built = 0;
    for entry in std::fs::read_dir(maps).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if path.extension().is_none_or(|e| e != "xml") {
            continue;
        }
        let (map, g) = common::load_map(&name);
        let cell = map.scale / 32.0;
        let nav = NavGraph::from_grid(&g, cell);
        built += 1;
        // Wherever a path is found it starts and ends where asked and crosses no wall.
        let floor: Vec<Vec2> = (0..g.height as i32)
            .step_by(7)
            .flat_map(|y| (0..g.width as i32).step_by(7).map(move |x| (x, y)))
            .filter(|&(x, y)| g.is_floor(x, y) && g.cell(x, y).is_some_and(|c| c.color.a != 0) && !g.is_wall(x, y))
            .map(|(x, y)| Vec2::new((x as f32 + 0.5) * cell, -(y as f32 + 0.5) * cell))
            .collect();
        for pair in floor.windows(2).take(40) {
            if let Some(path) = nav.get_path(pair[0], pair[1]) {
                assert_eq!(path.points()[0], pair[0], "{name}");
                assert_eq!(*path.points().last().unwrap(), pair[1], "{name}");
                assert!(path.full_length() + 1e-3 >= pair[0].distance(pair[1]), "{name}");
            }
        }
    }
    assert!(built > 20);
}

// ---- NavPath -------------------------------------------------------------

fn square(closed: bool) -> NavPath {
    NavPath::new(vec![Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)], closed)
}

#[test]
fn a_path_knows_its_length_and_start() {
    let p = square(false);
    assert_eq!(p.full_length(), 30.0);
    assert!(p.is_start() && !p.is_end());
    assert_eq!(p.current_position(), Vec2::ZERO);
    assert_eq!(p.last_position(), Vec2::new(0.0, 10.0));
    // A path with no length has already ended.
    assert!(NavPath::new(vec![Vec2::ONE, Vec2::ONE], false).is_end());
    assert!(NavPath::new(vec![], false).is_end());
}

#[test]
fn the_carrot_runs_along_the_path_and_stops_at_its_end() {
    let mut p = square(false);
    assert!(p.update_position(5.0));
    assert_eq!(p.current_position(), Vec2::new(5.0, 0.0));
    assert!(p.update_position(10.0));
    assert!((p.current_position() - Vec2::new(10.0, 5.0)).length() < 1e-5);
    assert_eq!(p.length(), 15.0);
    // Past the end: it stops on the last point and no longer moves on.
    assert!(!p.update_position(100.0));
    assert!(p.is_end());
    assert_eq!(p.current_position(), Vec2::new(0.0, 10.0));
    assert!(!p.update_position(1.0));
    // Backwards frees it.
    assert!(p.update_position(-2.0));
    assert!(!p.is_end());
}

#[test]
fn stepping_back_over_a_corner_keeps_the_carrot_on_the_path_where_the_distance_says() {
    let mut p = square(false);
    p.update_position(15.0);
    assert!((p.current_position() - Vec2::new(10.0, 5.0)).length() < 1e-5);
    // Back into the first side: 8 on from the start, not on its end.
    p.update_position(-8.0);
    assert!((p.current_position() - Vec2::new(7.0, 0.0)).length() < 1e-5, "{:?}", p.current_position());
    assert!((p.distance() - 7.0).abs() < 1e-5);
    // Forward again goes on from there.
    p.update_position(5.0);
    assert!((p.current_position() - Vec2::new(10.0, 2.0)).length() < 1e-5, "{:?}", p.current_position());
    // Back past the start stops at it.
    p.update_position(-50.0);
    assert!(p.is_start());
    assert_eq!(p.current_position(), Vec2::ZERO);
    p.update_position(3.0);
    assert!((p.current_position() - Vec2::new(3.0, 0.0)).length() < 1e-5);
    // The same over each corner of a closed one.
    let mut c = square(true);
    c.update_position(25.0);
    c.update_position(-15.0);
    assert!((c.current_position() - Vec2::new(10.0, 0.0)).length() < 1e-5, "{:?}", c.current_position());
}

#[test]
fn a_carrot_that_goes_on_too_far_is_pulled_back_in_the_same_update() {
    let mut p = NavPath::new(vec![Vec2::ZERO, Vec2::new(100.0, 0.0)], false);
    p.update_position(3.9);
    // The car is 3.9 behind, within the 4: the carrot goes on by 1, which is
    // too far ahead, so it comes back at once.
    p.update(Vec2::ZERO, 1.0, 2.0, 4.0);
    let gap = p.current_position().distance(Vec2::ZERO);
    assert!(gap <= 4.0, "{gap}");
    assert!(gap > 3.0, "and not much further: {gap}");
}

#[test]
fn a_closed_path_goes_round_for_ever() {
    let mut p = square(true);
    for _ in 0..100 {
        p.update_position(1.0);
    }
    assert!(!p.is_end());
    // The closing leg is the fourth side: 30 + 10 = 40 round, 100 = 20 into the third lap.
    let c = p.current_position();
    assert!((c - Vec2::new(10.0, 10.0)).length() < 1e-3, "{c:?}");
}

#[test]
fn update_keeps_the_carrot_ahead_of_the_car_and_follows_it_back() {
    let mut p = NavPath::new(vec![Vec2::ZERO, Vec2::new(100.0, 0.0)], false);
    // A car at the start: the carrot first runs out to min_distance.
    p.update(Vec2::ZERO, 0.0, 2.0, 4.0);
    let d = p.current_position().distance(Vec2::ZERO);
    assert!((2.0..=4.0).contains(&d), "{d}");
    // A car between the two distances: the carrot goes on by the car's speed.
    let before = p.current_position().x;
    p.update(Vec2::new(before - 3.0, 0.0), 0.1, 2.0, 4.0);
    assert!((p.current_position().x - before - 0.1).abs() < 1e-4);
    // A car left far behind drags the carrot back to within max_distance.
    let mut q = NavPath::new(vec![Vec2::ZERO, Vec2::new(100.0, 0.0)], false);
    q.update_position(50.0);
    for _ in 0..200 {
        q.update(Vec2::new(10.0, 0.0), 0.0, 2.0, 4.0);
    }
    let gap = q.current_position().distance(Vec2::new(10.0, 0.0));
    assert!(gap <= 4.0 + 0.5, "{gap}");
}

#[test]
fn add_point_extends_the_path_unless_it_is_hardly_a_move() {
    let mut p = NavPath::new(vec![Vec2::ZERO, Vec2::new(10.0, 0.0)], false);
    p.update_position(100.0);
    assert!(p.is_end());
    p.add_point(Vec2::new(10.5, 0.0));
    assert_eq!(p.points().len(), 2, "within a unit: ignored");
    assert!(p.is_end());
    p.add_point(Vec2::new(20.0, 0.0));
    assert_eq!(p.points().len(), 3);
    assert!(!p.is_end(), "a longer path is not at its end");
    assert_eq!(p.full_length(), 20.0);
    // An empty path takes any point as its start.
    let mut e = NavPath::new(vec![], false);
    e.add_point(Vec2::new(3.0, 4.0));
    assert_eq!(e.current_position(), Vec2::new(3.0, 4.0));
}

#[test]
fn smoothing_pulls_inner_points_to_the_curve() {
    let line = [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)];
    // The point `i` becomes the middle of the curve between `i - 1` and `i`.
    assert_eq!(NavPath::smooth(&line), vec![Vec2::ZERO, Vec2::new(0.5, 0.0), Vec2::new(2.0, 0.0)]);
    let corner = NavPath::smooth(&[Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0)]);
    assert_eq!(corner.len(), 3);
    assert!(corner[1].x < 2.0 && corner[1].y.abs() < 1.0, "{:?}", corner[1]);
    assert_eq!(NavPath::smooth(&[Vec2::ZERO, Vec2::ONE]).len(), 2);
}
