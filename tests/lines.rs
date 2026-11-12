//! Reusable line meshes, independent of maps and the world's up direction.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use tilerider::render::lines::LineMeshBuilder;

#[test]
fn segments_preserve_endpoints_widths_and_arbitrary_axes() {
    let from = Vec3::new(2.0, -3.0, 4.0);
    let segments = [
        (Vec3::X, 0.1),
        (Vec3::Y, 0.2),
        (Vec3::Z, 0.3),
        (Vec3::new(1.0, -2.0, 3.0), 0.4),
    ];
    let mut builder = LineMeshBuilder::default();
    for (direction, width) in segments {
        builder.add_line(from, from + direction, width);
    }
    let mesh = builder.build();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    let Some(VertexAttributeValues::Float32x3(axes)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("axes");
    };
    let Some(VertexAttributeValues::Float32x2(widths)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        panic!("widths");
    };
    assert_eq!(mesh.count_vertices(), segments.len() * 4);
    assert_eq!(mesh.indices().unwrap().len(), segments.len() * 6);
    for (i, (direction, width)) in segments.into_iter().enumerate() {
        let base = 4 * i;
        assert_eq!(
            &positions[base..base + 4],
            &[
                from.to_array(),
                from.to_array(),
                (from + direction).to_array(),
                (from + direction).to_array()
            ]
        );
        for j in 0..4 {
            let axis = Vec3::from(axes[base + j]);
            assert!((axis.length() - 1.0).abs() < 1e-5);
            assert!(axis.dot(direction.normalize()) > 0.99999);
            assert_eq!(widths[base + j][1], 1.0);
            assert!((widths[base + j][0].abs() * 2.0 - width).abs() < 1e-5);
        }
    }
    assert!(
        mesh.indices()
            .unwrap()
            .iter()
            .all(|i| i < mesh.count_vertices())
    );
}

#[test]
fn fixed_joins_and_very_thin_ribbons_can_share_one_mesh() {
    let vertices = [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Z];
    let mut builder = LineMeshBuilder::default();
    builder.add_quad(vertices);
    builder.add_line(Vec3::ZERO, Vec3::X, 0.00001);
    let mesh = builder.build();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    let Some(VertexAttributeValues::Float32x2(widths)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    else {
        panic!("widths");
    };
    assert_eq!(&positions[..4], &vertices.map(|v| v.to_array()));
    assert!(widths[..4].iter().all(|uv| uv[1] == 0.0));
    assert!(widths[4..].iter().all(|uv| uv[1] == 1.0 && uv[0] != 0.0));
}

#[test]
fn empty_or_invalid_segments_do_not_poison_a_batch() {
    let mut builder = LineMeshBuilder::default();
    for (from, to, width) in [
        (Vec3::ZERO, Vec3::ZERO, 1.0),
        (Vec3::ZERO, Vec3::X, 0.0),
        (Vec3::ZERO, Vec3::X, -1.0),
        (Vec3::ZERO, Vec3::X, f32::NAN),
        (Vec3::ZERO, Vec3::X, f32::INFINITY),
        (Vec3::NAN, Vec3::X, 1.0),
        (Vec3::ZERO, Vec3::INFINITY, 1.0),
    ] {
        builder.add_line(from, to, width);
    }
    let empty = builder.build();
    assert_eq!(empty.count_vertices(), 0);
    assert_eq!(empty.indices().unwrap().len(), 0);
}
