//! The walls' black outline.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::render::wall_outline::build_wall_outline;

/// `#` a wall 6 units tall, `+` one 2 units tall, `.` floor.
fn map(rows: &str) -> MapData {
    MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>#:#FFFF0000;+:#80FF0000;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap()
}

fn grid(rows: &str) -> MapGrid {
    MapGrid::build(&map(rows), None)
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
    strips(grid)
        .into_iter()
        .filter(|s| (s[0].z - s[1].z).abs() >= 1e-4)
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
                && l[1].y.abs() > 0.9
                && l[1].y.abs() < 1.2
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
            s[0].z > h - 0.1 && s[1].z.abs() < 0.01,
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
        at_step.iter().all(|s| (s[1].z - short).abs() < 0.01),
        "{at_step:?}"
    );
}

/// Full quads for checking the geometry at a join, not only its bounds.
fn quads(grid: &MapGrid, cell: f32) -> Vec<[Vec3; 4]> {
    let mesh = build_wall_outline(grid, cell);
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions");
    };
    p.chunks_exact(4)
        .map(|s| std::array::from_fn(|i| Vec3::from(s[i])))
        .collect()
}

fn flat(q: &[Vec3; 4]) -> bool {
    (q[0].z - q[2].z).abs() < 1e-5
}

/// Closed contours must share both ends of each cross-section exactly.
/// Extending independent rectangles leaves overlaps/spikes at these joins.
fn assert_closed_contours(grid: &MapGrid, cell: f32) {
    let ends: Vec<_> = quads(grid, cell)
        .into_iter()
        .filter(flat)
        .flat_map(|q| [[q[0], q[1]], [q[3], q[2]]])
        .collect();
    for (i, [a, b]) in ends.iter().enumerate() {
        assert!(
            ends.iter()
                .enumerate()
                .any(|(j, [c, d])| { i != j && a.distance(*c) < 1e-5 && b.distance(*d) < 1e-5 }),
            "unmatched join {a:?}–{b:?} at cell size {cell}"
        );
    }
}

#[test]
fn map_boundary_is_outlined_on_all_four_sides() {
    let g = grid("#");
    assert_eq!(segments(&g).len(), 8, "top and foot on every outside face");
    assert_eq!(uprights(&g).len(), 4, "outside corners reach the ground");
    assert_closed_contours(&g, 1.0);
}

#[test]
fn map_boundary_lines_have_wall_faces_behind_them() {
    use tilerider::game::LoadedMap;
    use tilerider::light::{bake::LightMap, raster::Canvas};
    use tilerider::render::map_mesh::build_map_meshes;
    use tilerider::units::CellSize;

    let data = map("#");
    let grid = MapGrid::build(&data, None);
    let light = LightMap::bake(&data, &grid, &Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let loaded = LoadedMap {
        data,
        grid,
        light,
        cell: CellSize::from_map_scale(64.0),
        cars: default(),
        list: default(),
        sprites: default(),
    };
    let (_, walls) = build_map_meshes(&loaded);
    let Some(VertexAttributeValues::Float32x3(p)) = walls.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    let faces: Vec<_> = p
        .chunks_exact(4)
        .filter(|q| q.iter().any(|v| (v[2] - q[0][2]).abs() > 1e-5))
        .collect();
    assert_eq!(
        faces.len(),
        4,
        "each boundary edge needs a face, not a floating outline"
    );
    for q in faces {
        assert!(q.iter().any(|v| v[2] == 0.0));
        assert!(q.iter().any(|v| v[2] == loaded.grid.height_at(0, 0)));
    }
}

#[test]
fn right_angle_and_straight_joins_have_shared_endpoints() {
    for rows in ["...\n.#.\n...", "....\n.##.\n....", "....\n.#+.\n...."] {
        for cell in [1.0, 2.0, 3.0] {
            assert_closed_contours(&grid(rows), cell);
        }
    }
}

#[test]
fn diagonal_wedges_join_the_straight_outline_in_all_orientations() {
    for rows in [
        "####\n#...\n#...\n#...",
        "####\n...#\n...#\n...#",
        "...#\n...#\n...#\n####",
        "#...\n#...\n#...\n####",
        // A wedge lower than one of its two neighbours: contours at
        // different heights must stay separate, but meet at their own joins.
        "#+++\n#...\n#...\n#...",
    ] {
        let g = grid(rows);
        assert!((0..g.height as i32).any(|y| (0..g.width as i32).any(|x| g.edge(x, y).is_some())));
        for cell in [1.0, 2.0, 3.0] {
            assert_closed_contours(&g, cell);
        }
    }
}

#[test]
fn touching_blocks_keep_separate_finite_contours() {
    // Transparent cells do not generate wedges between the blocks.
    let g = grid("# \n #");
    assert_eq!(uprights(&g).len(), 8);
    assert_closed_contours(&g, 1.0);
    assert!(quads(&g, 1.0).iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn uprights_are_single_quads_with_camera_facing_widths() {
    let g = grid("...\n.#.\n...");
    let mesh = build_wall_outline(&g, 1.0);
    let Some(VertexAttributeValues::Float32x2(widths)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        panic!("signed billboard widths");
    };
    // Eight horizontal edges and four uprights, all just two triangles.
    assert_eq!(mesh.count_vertices(), 12 * 4);
    assert_eq!(mesh.indices().unwrap().len(), 12 * 6);
    let quads = quads(&g, 1.0);
    let mut upright_count = 0;
    for (q, widths) in quads.iter().zip(widths.chunks_exact(4)) {
        if flat(q) {
            assert!(widths.iter().all(|uv| uv[0] == 0.0));
        } else {
            upright_count += 1;
            // Both sides start on the actual corner, not an offset ribbon
            // that can turn edge-on; the material expands them at render time.
            assert_eq!(q[0], q[1]);
            assert_eq!(q[2], q[3]);
            assert_eq!(q[0].truncate(), q[2].truncate());
            assert_eq!(
                widths,
                &[
                    [-1.5 / 32.0, 0.0],
                    [1.5 / 32.0, 0.0],
                    [1.5 / 32.0, 0.0],
                    [-1.5 / 32.0, 0.0]
                ]
            );
        }
    }
    assert_eq!(upright_count, 4);
}

fn covers(q: &[Vec3; 4], p: Vec3) -> bool {
    if (q[0].z - p.z).abs() > 1e-5 {
        return false;
    }
    let side: [f32; 4] = std::array::from_fn(|i| {
        (q[(i + 1) % 4] - q[i])
            .truncate()
            .perp_dot((p - q[i]).truncate())
    });
    side.iter().all(|&s| s >= -1e-5) || side.iter().all(|&s| s <= 1e-5)
}

#[test]
fn upright_endpoints_meet_the_flat_corner_joins() {
    for rows in [
        "...\n.#.\n...",
        "....\n.#+.\n....",
        "####\n#...\n#...\n#...",
    ] {
        let quads = quads(&grid(rows), 1.0);
        let joins: Vec<_> = quads.iter().filter(|q| flat(q)).collect();
        for q in quads.iter().filter(|q| !flat(q)) {
            for p in &q[..2] {
                assert!(
                    joins.iter().any(|join| covers(join, *p)),
                    "uncovered top {p}"
                );
            }
            // A step can hide the lower end under the shorter wall;
            // ground-level ends must meet the foot outline itself.
            for p in q[2..].iter().filter(|p| p.z < 0.0) {
                assert!(
                    joins.iter().any(|join| covers(join, *p)),
                    "uncovered foot {p}"
                );
            }
        }
    }
}

#[test]
fn feet_straddle_the_same_contour_as_the_tops() {
    let quads = quads(&grid("...\n.#.\n..."), 1.0);
    for q in quads.iter().filter(|q| flat(q) && q[0].z > 1.0) {
        assert!(quads.iter().any(|foot| {
            flat(foot)
                && foot[0].z < 0.0
                && q.iter()
                    .zip(foot)
                    .all(|(a, b)| a.truncate().distance(b.truncate()) < 1e-5)
        }));
    }
}
