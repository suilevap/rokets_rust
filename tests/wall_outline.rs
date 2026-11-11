//! The walls' black outline.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::render::wall_outline::build_wall_outline;

/// `#` a wall 6 units tall, `+` one 2 units tall, `.` floor.
fn grid(rows: &str) -> MapGrid {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>#:#FFFF0000;+:#80FF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    MapGrid::build(&map, None)
}

/// Every strip as two opposite corners.
fn strips(grid: &MapGrid) -> Vec<[Vec3; 2]> {
    let mesh = build_wall_outline(grid, 1.0);
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions");
    };
    // Each line is a strip of four corners; two opposite ones stand for it.
    p.chunks(4)
        .map(|s| [Vec3::from(s[0]), Vec3::from(s[2])])
        .collect()
}

/// The strips lying flat: lines along edges.
fn segments(grid: &MapGrid) -> Vec<[Vec3; 2]> {
    strips(grid)
        .into_iter()
        .filter(|s| (s[0].z - s[1].z).abs() < 1e-4)
        .collect()
}

/// The upright corner edges.
fn uprights(grid: &MapGrid) -> Vec<[Vec3; 2]> {
    // Each is a column of four standing faces; one stands for it.
    strips(grid)
        .into_iter()
        .filter(|s| (s[0].z - s[1].z).abs() >= 1e-4)
        .step_by(4)
        .collect()
}

#[test]
fn a_lone_wall_gets_its_top_and_foot_on_every_side() {
    let g = grid("\n...\n.#.\n...\n");
    let lines = segments(&g);
    assert_eq!(lines.len(), 8);
    let h = g.cell(1, 1).unwrap().height;
    let tops = lines.iter().filter(|l| l[0].z > h).count();
    let feet = lines.iter().filter(|l| l[0].z < 0.1).count();
    assert_eq!((tops, feet), (4, 4));
}

#[test]
fn walls_of_one_height_share_no_line() {
    // Two cells side by side: the shared side has no line.
    assert_eq!(segments(&grid("\n....\n.##.\n....\n")).len(), 6 * 2);
}

#[test]
fn a_step_between_walls_gets_top_lines_but_no_foot() {
    // The short wall and the tall one each outline their top along the
    // step; neither side of it reaches the floor.
    let lines = segments(&grid("\n....\n.#+.\n....\n"));
    let at_step = lines
        .iter()
        .filter(|l| (l[0].x - 2.0).abs() < 0.1 && (l[1].x - 2.0).abs() < 0.1)
        .count();
    assert_eq!(at_step, 2);
    assert_eq!(lines.len(), 6 * 2 + 2);
}

#[test]
fn lines_are_strips_of_a_fixed_width_in_the_world() {
    let lines = segments(&grid("\n...\n.#.\n...\n"));
    // A top line along the upper side: 3 / 32 units across.
    let top = lines
        .iter()
        .find(|l| l[0].z > 1.0 && (l[0].y - l[1].y).abs() > 0.05)
        .unwrap();
    assert!(
        ((top[0].y - top[1].y).abs() - 3.0 / 32.0).abs() < 1e-4,
        "{top:?}"
    );
}

#[test]
fn a_side_against_a_wedge_gets_no_line() {
    // `x` is a floor cell with walls up and left: a corner wedge.
    let g = grid("\n####\n#...\n#...\n");
    let wedge_cell = (1, 1);
    assert!(g.edge(wedge_cell.0, wedge_cell.1).is_some());
    let lines = segments(&g);
    // The wall above the wedge (1, 0) has no foot line along y = 1 over x 1..2.
    let feet_over_wedge = lines
        .iter()
        .filter(|l| {
            l[0].z < 0.1
                && l[0].y.abs() > 0.9
                && l[0].y.abs() < 1.2
                && l[0].x.min(l[1].x) > 0.9
                && l[0].x.max(l[1].x) < 2.1
        })
        .count();
    assert_eq!(feet_over_wedge, 0);
}

#[test]
fn outer_corners_get_a_vertical_edge_down_the_shorter_side() {
    let g = grid("\n...\n.#.\n...\n");
    let h = g.cell(1, 1).unwrap().height;
    let up = uprights(&g);
    assert_eq!(up.len(), 4, "one per corner of a lone wall");
    for s in &up {
        assert!(
            s[0].z > h - 0.1 && s[1].z.abs() < 1e-3,
            "from the top to the floor: {s:?}"
        );
    }
    // At a step from a tall wall to a short one, the corners on the step
    // only go down to the short wall's top.
    let g = grid("\n....\n.#+.\n....\n");
    let short = g.cell(2, 1).unwrap().height;
    let at_step = uprights(&g)
        .into_iter()
        .filter(|s| (s[0].x - 2.0).abs() < 0.1)
        .collect::<Vec<_>>();
    assert_eq!(at_step.len(), 2);
    assert!(
        at_step.iter().all(|s| (s[1].z - short).abs() < 1e-3),
        "{at_step:?}"
    );
}
