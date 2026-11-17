//! Floor tiles next to walls lean on them.

use bevy::math::Vec3;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::render::map_mesh::wall_lean;

fn grid(rows: &str) -> MapGrid {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    MapGrid::build(&map, None)
}

/// Height (world z, up) of the tile's edge one unit from its centre
/// towards `side` (world x right, y up), after the lean.
fn edge_height(g: &MapGrid, x: i32, y: i32, side: Vec3) -> f32 {
    (wall_lean(&g.chamfer_distance(), g.width, x, y) * side).z
}

const ROOM: &str = "
**********
**********
**......**
**......**
**......**
**......**
**********
**********
";

#[test]
fn the_edge_by_the_wall_rises() {
    let g = grid(ROOM);
    // Wall to the left (x = 1), above (map y = 1, world up).
    assert!(edge_height(&g, 2, 3, Vec3::NEG_X) > 0.1, "left wall");
    assert!(edge_height(&g, 2, 3, Vec3::X) < -0.1);
    assert!(edge_height(&g, 4, 2, Vec3::Y) > 0.1, "wall above");
    assert!(edge_height(&g, 7, 4, Vec3::X) > 0.1, "right wall");
    assert!(edge_height(&g, 4, 5, Vec3::NEG_Y) > 0.1, "wall below");
}
