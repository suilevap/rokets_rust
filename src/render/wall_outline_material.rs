//! Camera-facing corner ribbons for XNA's `LinePrimitive` wall borders.
//! One static mesh contains both flat mitered contours and upright quads;
//! only the upright vertices move, using the current render view on the GPU.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub(super) fn plugin(app: &mut App) {
    embedded_asset!(app, "wall_outline.wesl");
    app.add_plugins(MaterialPlugin::<WallOutlineMaterial>::default());
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub(super) struct WallOutlineMaterial {}

impl Material for WallOutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("wall_outline.wesl"))
            .with_source("embedded")
            .into()
    }

    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
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
