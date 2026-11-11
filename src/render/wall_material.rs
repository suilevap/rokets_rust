//! XNA's `BasicEffect` interpolated the baked wall colours in sRGB.
//! Convert in the fragment shader: converting each vertex first gives
//! differently curved gradients on neighbouring walls of unequal height.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct WallMaterial {}

impl Material for WallMaterial {
    fn fragment_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("wall_color.wesl"))
            .with_source("embedded")
            .into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub struct WallMaterialPlugin;

impl Plugin for WallMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "wall_color.wesl");
        app.add_plugins(MaterialPlugin::<WallMaterial>::default());
    }
}
