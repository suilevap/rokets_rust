//! The map grid built from a `CharMap`.

use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;

fn char_map(dict: &str, rows: &str) -> MapData {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>{dict}</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapData::parse(&xml).unwrap()
}

#[test]
fn char_map_builds_walls_and_floor() {
    let map = char_map("*:#FFFF0000;.:#100000A0", "\n****\n*..*\n****\n");
    let grid = MapGrid::build(&map, None);
    assert_eq!((grid.width, grid.height), (4, 3));
    assert!(grid.is_wall(0, 0));
    assert!(!grid.is_wall(1, 1));
    assert!(grid.is_wall(-1, 1), "outside the map is solid");
    assert_eq!(grid.distance_to_wall(1, 1), 0.5);
    assert!(grid.is_floor(1, 1));
    assert!(!grid.is_floor(0, 0));
    let covered: usize = grid.wall_rects().iter().map(|r| r.2 * r.3).sum();
    assert_eq!(covered, 10);
    // Between three walls there is no corner to round off.
    assert_eq!(grid.edge(1, 1), None);
}

#[test]
fn inside_corners_get_wedges() {
    let map = char_map("*:#FFFF0000;.:#100000A0", "\n*****\n*...*\n*...*\n*...*\n*****\n");
    let grid = MapGrid::build(&map, None);
    // Walls up and left: the bottom-right corner stays open.
    assert_eq!(grid.edge(1, 1).map(|e| e.open), Some(2));
    assert_eq!(grid.edge(3, 1).map(|e| e.open), Some(3));
    assert_eq!(grid.edge(3, 3).map(|e| e.open), Some(0));
    assert_eq!(grid.edge(1, 3).map(|e| e.open), Some(1));
    assert_eq!(grid.edge(2, 2), None);
    assert!(grid.edge(1, 1).unwrap().height > 0.0);
}

#[test]
fn ground_is_drawn_cells_and_the_gaps_between_them() {
    // `_` is undrawn: the void, unless drawn cells close it in on a line.
    let map = char_map("*:#FFFF0000;.:#100000A0;_:#00000000", "\n__*__\n_*_*_\n__*__\n");
    let grid = MapGrid::build(&map, None);
    assert!(grid.is_ground(2, 0), "a wall is ground");
    assert!(grid.is_ground(2, 1), "a gap between walls is ground");
    assert!(!grid.is_ground(0, 0), "the corner is void");
    assert!(!grid.is_floor(2, 0), "but a wall is not floor");
}
