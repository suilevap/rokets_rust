//! The light layer: the light map laid over the map at ground level,
//! multiplying what lies under it on screen.
//!
//! C# origin: `LightManager` draws its final light target over the map and
//! `BorderCells` (4) cells around it at `DepthConst.Light`, with a multiply
//! blend (`RendererConst.LightBlendState`). Over the void around the map's
//! ground the light is full (`MapManager.AddOutSideLight`), so the
//! outland shows its own colours there. Floor tiles and the outland lie
//! under it; walls rise above it and light their own vertices; sprites
//! are drawn after it and carry their light in their tint.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::LoadedMap;
use crate::light::bake::TEXELS_PER_CELL;
use crate::render::layers::{LIGHT_DEPTH_BIAS, LIGHT_Z};

/// How far past the map's edge the layer reaches, in cells.
const BORDER: usize = 4;

/// The layer's quad and material.
pub fn build_light_layer(map: &LoadedMap, images: &mut Assets<Image>) -> (Mesh, StandardMaterial, Transform) {
    let (w, h, texels) = map.light.layer_texels(BORDER);
    let data = texels
        .iter()
        .flat_map(|t| {
            let [r, g, b] = t.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            [r, g, b, 255]
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        // sRGB like the C# light; multiplying linear values multiplies the
        // sRGB ones the same way.
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let cell = map.cell.0;
    let size = Vec2::new(w as f32, h as f32) / TEXELS_PER_CELL as f32 * cell;
    // The quad's top-left corner is map cell (-BORDER, -BORDER).
    let corner = Vec2::new(-(BORDER as f32) * cell, BORDER as f32 * cell);
    let center = corner + Vec2::new(size.x, -size.y) / 2.0;
    let material = StandardMaterial {
        base_color_texture: Some(images.add(image)),
        unlit: true,
        alpha_mode: AlphaMode::Multiply,
        depth_bias: LIGHT_DEPTH_BIAS,
        ..default()
    };
    (
        Mesh::from(Rectangle::from_size(size)),
        material,
        Transform::from_translation(center.extend(LIGHT_Z)),
    )
}
