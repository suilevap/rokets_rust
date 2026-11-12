//! Camera-facing ribbons, generalised from XNA's `LinePrimitive` wall borders.
//!
//! Add [`LinePlugin`], build a mesh with [`LineMeshBuilder`], then spawn it
//! with `Mesh3d` and `MeshMaterial3d<LineMaterial>`. Widths are in world units
//! and independent of the entity's scale. Segments may point in any direction;
//! the GPU turns their two-triangle ribbons towards each render view.
//!
//! Fixed quads can share the same mesh/material, for example mitered joins.
//! Because centre-line bounds omit the shader's expansion, use a conservative
//! `Aabb` or `NoFrustumCulling` on the entity (see `examples/lines.rs`).

use bevy::asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub struct LinePlugin;

impl Plugin for LinePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "lines.wesl");
        app.add_plugins(MaterialPlugin::<LineMaterial>::default());
    }
}

/// Unlit line colour and separation from coplanar geometry.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct LineMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// World units towards the camera, perpendicular to each segment's axis.
    /// Zero by default; a small positive value prevents coplanar fighting.
    #[uniform(0)]
    pub camera_offset: f32,
}

impl Default for LineMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::WHITE,
            camera_offset: 0.0,
        }
    }
}

impl Material for LineMaterial {
    fn vertex_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("lines.wesl"))
            .with_source("embedded")
            .into()
    }

    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.color.alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// A batch of independent ribbons and optional fixed quads.
///
/// Positions store segment endpoints, normals store the unit segment axis,
/// and UVs store (signed half-width, billboard flag). Fixed quads have flag 0;
/// ribbons have flag 1, so tiny widths do not get confused with fixed geometry.
#[derive(Default)]
pub struct LineMeshBuilder {
    positions: Vec<[f32; 3]>,
    axes: Vec<[f32; 3]>,
    widths: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl LineMeshBuilder {
    /// Append a two-triangle segment. Coincident/non-finite endpoints and
    /// non-positive/non-finite widths are ignored, producing no invalid vertices.
    pub fn add_line(&mut self, from: Vec3, to: Vec3, width: f32) {
        if !from.is_finite() || !to.is_finite() || !width.is_finite() || width <= 0.0 {
            return;
        }
        let Some(axis) = (to - from).try_normalize() else {
            return;
        };
        self.push([from, from, to, to], axis, width * 0.5, 1.0);
    }

    /// Append a fixed quad, in perimeter order, without camera-facing expansion.
    pub fn add_quad(&mut self, vertices: [Vec3; 4]) {
        self.push(vertices, Vec3::Z, 0.0, 0.0);
    }

    fn push(&mut self, vertices: [Vec3; 4], axis: Vec3, half_width: f32, flag: f32) {
        let base = self.positions.len() as u32;
        self.positions.extend(vertices.map(|v| v.to_array()));
        self.axes.extend([axis.to_array(); 4]);
        self.widths
            .extend([-half_width, half_width, half_width, -half_width].map(|w| [w, flag]));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.axes)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.widths)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}
