//! The darkening at the foot of walls and the distance field it comes from.

use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::render::wall_ao::wall_ao;

fn grid(rows: &str) -> MapGrid {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>48</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>#:#FFFF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    MapGrid::build(&map, None)
}

const ROOM: &str = "\n########\n#......#\n#......#\n#......#\n#......#\n#......#\n########\n";

#[test]
fn distance_counts_cells_to_the_nearest_wall() {
    let g = grid(ROOM);
    let d = g.chamfer_distance();
    let at = |x: usize, y: usize| d[y * g.width + x];
    assert_eq!(at(0, 0), -1.0, "a wall");
    assert_eq!(at(1, 1), 1.0, "next to walls");
    assert_eq!(at(2, 2), 2.0, "two cells in");
    assert_eq!(at(3, 3), 3.0, "three cells in");
}

#[test]
fn walls_darken_half_and_floor_beside_them_a_quarter() {
    let g = grid(ROOM);
    let ao = wall_ao(&g);
    let at = |x: usize, y: usize| ao[y * g.width + x];
    // Only the outer ring stays clear, on every side (C# left two columns
    // and rows clear on the left and top, one on the right and bottom).
    for (x, y) in [(0, 3), (7, 3), (3, 0), (3, 6)] {
        assert_eq!(at(x, y), 0.0, "outer cell {x},{y}");
    }
    for (x, y) in [(1, 3), (6, 3), (3, 1), (3, 5)] {
        assert_eq!(at(x, y), 0.25, "floor beside the border wall {x},{y}");
    }
    // Inside: floor one cell from a wall is a quarter dark, further on clear.
    assert_eq!(at(2, 2), 0.0, "two cells from walls");
    let g = grid("\n########\n#......#\n#......#\n#..#...#\n#......#\n#......#\n########\n");
    let ao = wall_ao(&g);
    let at = |x: usize, y: usize| ao[y * g.width + x];
    assert_eq!(at(3, 3), 0.5, "the wall cell");
    assert_eq!(at(4, 3), 0.25, "beside it");
    assert_eq!(at(4, 4), 0.25, "diagonally beside it");
}
