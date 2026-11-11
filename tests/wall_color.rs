//! Wall colours stay continuous where the height of a straight wall changes.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use tilerider::content::map::MapData;
use tilerider::game::LoadedMap;
use tilerider::light::{bake::LightMap, raster::Canvas};
use tilerider::map::grid::MapGrid;
use tilerider::render::map_mesh::build_map_meshes;
use tilerider::units::CellSize;

struct Quad {
    positions: [Vec3; 4],
    colors: [Vec3; 4],
}

fn linear_gray(value: f32) -> Vec3 {
    Vec4::from_array(Color::srgb(value, value, value).to_linear().to_f32_array()).truncate()
}

fn walls(rows: &str, ambient: &str) -> Vec<Quad> {
    let data = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold><AmbientLight>{ambient}</AmbientLight>
        <CharMapDictionary>#:#FFFFFFFF;+:#80FFFFFF;.:#100000A0</CharMapDictionary>
        <CharMap>{rows}</CharMap></Asset></XnaContent>"#
    ))
    .unwrap();
    let grid = MapGrid::build(&data, None);
    let light = LightMap::bake(&data, &grid, &Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let map = LoadedMap {
        data,
        grid,
        light,
        cell: CellSize::from_map_scale(32.0),
        cars: default(),
        list: default(),
        sprites: default(),
    };
    let (_, mesh) = build_map_meshes(&map);
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("colours");
    };
    positions
        .chunks_exact(4)
        .zip(colors.chunks_exact(4))
        .map(|(p, c)| Quad {
            positions: std::array::from_fn(|i| Vec3::from(p[i])),
            colors: std::array::from_fn(|i| Vec4::from(c[i]).truncate()),
        })
        .collect()
}

fn upright(q: &Quad) -> bool {
    q.positions
        .iter()
        .any(|p| (p.z - q.positions[0].z).abs() > 1e-5)
}

/// Interpolate a rasterizer varying along the common vertical edge.
fn edge_color(q: &Quad, x: f32, z: f32) -> Vec3 {
    let mut edge: Vec<_> = q
        .positions
        .iter()
        .zip(q.colors)
        .filter(|(p, _)| (p.x - x).abs() < 1e-5)
        .collect();
    assert_eq!(edge.len(), 2);
    edge.sort_by(|a, b| a.0.z.total_cmp(&b.0.z));
    let [(a, ca), (b, cb)] = [edge[0], edge[1]];
    assert!(a.z <= z && z <= b.z);
    ca.lerp(cb, (z - a.z) / (b.z - a.z))
}

#[test]
fn adjacent_wall_heights_have_the_same_color_on_the_shared_edge() {
    for ambient in ["FF404040", "FF808080", "FFFFFFFF"] {
        let walls = walls("....\n.+#.\n....", ambient);
        let front: Vec<_> = walls
            .iter()
            .filter(|q| upright(q) && q.positions.iter().all(|p| (p.y + 2.0).abs() < 1e-5))
            .collect();
        assert_eq!(front.len(), 2);
        // The two faces meet at x=2. The whole visible common edge must
        // have the same colour, not only its ground and top vertices.
        for z in [0.25, 1.0, 2.0, 2.03125] {
            let a = edge_color(front[0], 2.0, z);
            let b = edge_color(front[1], 2.0, z);
            assert!(
                a.distance(b) < 1e-5,
                "height {z}, ambient {ambient}: {a} vs {b}"
            );
        }
    }
}

#[test]
fn a_step_keeps_the_tall_faces_fade_anchored_at_the_floor() {
    let walls = walls("....\n.#+.\n....", "FFFFFFFF");
    let step = walls
        .iter()
        .find(|q| upright(q) && q.positions.iter().all(|p| (p.x - 2.0).abs() < 1e-5))
        .unwrap();
    assert_eq!(
        step.positions
            .iter()
            .map(|p| p.z)
            .fold(f32::INFINITY, f32::min),
        0.0
    );
    let foot = step
        .positions
        .iter()
        .zip(step.colors)
        .filter(|(p, _)| p.z == 0.0);
    for (_, color) in foot {
        assert!(
            color.distance(linear_gray(0.5)) < 1e-5,
            "XNA's ground colour: {color}"
        );
    }
}

#[test]
fn bright_wall_and_wedge_light_saturates_before_the_side_fade() {
    // White in white light produces 1.5 before XNA's Color clamps it.
    // Side feet should be 0.5, not 0.75, and tops should remain white.
    let walls = walls("####\n#...\n#...\n#...", "FFFFFFFF");
    assert!(
        walls.iter().any(|q| q.positions[2] == q.positions[3]),
        "a wedge top"
    );
    for q in walls {
        for (p, color) in q.positions.iter().zip(q.colors) {
            assert!(color.min_element() >= 0.0 && color.max_element() <= 1.0);
            if !upright(&q) {
                assert!(color.distance(Vec3::ONE) < 1e-5, "top: {color}");
            } else if p.z == 0.0 {
                assert!(color.distance(linear_gray(0.5)) < 1e-5, "foot: {color}");
            }
        }
    }
}

#[test]
fn walls_only_draw_a_face_above_a_lower_corner_wedge() {
    for (rows, count) in [("####\n#...\n#...\n#...", 0), ("####\n+...\n+...\n+...", 1)] {
        let walls = walls(rows, "FFFFFFFF");
        // The wall at (1,0) faces the wedge at (1,1). An equal-height
        // face is entirely hidden and causes a dark roof seam in WebGL;
        // a taller wall still needs the visible face above the wedge.
        let faces = walls
            .iter()
            .filter(|q| {
                upright(q)
                    && q.positions
                        .iter()
                        .all(|p| (p.y + 1.0).abs() < 1e-5 && p.x >= 1.0 && p.x <= 2.0)
            })
            .count();
        assert_eq!(faces, count, "{rows}");
    }
}
