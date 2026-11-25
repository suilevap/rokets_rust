//! Private checks of `Terrain::at`, the cell a world point lies in.

use bevy::math::Vec2;

use super::*;
use crate::content::map::MapData;
use crate::map::grid::MapGrid;

fn terrain() -> Terrain {
    let rows = "\n########\n#......#\n#......#\n#......#\n#......#\n########\n";
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>48</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>#:#FFFF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    Terrain::from_grid(&MapGrid::build(&map, None), 2.0)
}

#[test]
fn a_cell_is_the_one_a_point_lies_in() {
    let t = terrain();
    assert_eq!(t.at(Vec2::new(0.1, -0.1)), (0, 0));
    assert_eq!(t.at(Vec2::new(1.9, -3.9)), (0, 1));
    assert_eq!(t.at(Vec2::new(2.0, -4.0)), (1, 2), "on an edge, the later cell");
}

#[test]
fn points_left_of_or_above_the_map_are_in_cells_with_negative_indices() {
    let t = terrain();
    assert_eq!(t.at(Vec2::new(-0.1, 1.0)), (-1, -1));
    assert_eq!(t.at(Vec2::new(-2.1, 0.1)), (-2, -1));
    assert_eq!(t.at(Vec2::new(0.5, 0.5)), (0, -1), "above the map is not row 0");
}

#[test]
fn nothing_is_known_outside_the_map() {
    let t = terrain();
    for p in [Vec2::new(-0.5, -3.0), Vec2::new(3.0, 0.5), Vec2::new(-9.0, 9.0)] {
        assert_eq!(t.distance_at(p), 0.0, "{p}");
        assert_eq!(t.gradient_at(p), Vec2::ZERO, "{p}");
    }
}
