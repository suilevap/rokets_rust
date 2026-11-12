//! The light layer: the light map laid over the map at ground level,
//! multiplying what lies under it on screen.
//!
//! C# origin: `LightManager` draws its final light target over the map and
//! `BorderCells` (4) cells around it, stretched over that rectangle at `DepthConst.Light`, with a multiply
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
use crate::render::layers::{LIGHT_DEPTH_BIAS, LIGHT_Z};

/// A light image as a texture: sRGB like the C# light (multiplying or
/// adding linear values does to the sRGB ones what C# did, or near it),
/// sampled linearly.
pub fn light_texture((w, h, texels): (usize, usize, Vec<[f32; 3]>)) -> Image {
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
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// The quad the light is laid on: the map and its border, as C# stretched
/// its light targets over them; at `z`.
pub fn light_quad(map: &LoadedMap, z: f32) -> (Mesh, Transform) {
    let [left, top, width, height] = map.light.layer_rect();
    let size = Vec2::new(width, height) * map.cell.0;
    let corner = map.world(left, top);
    let center = corner + Vec2::new(size.x, -size.y) / 2.0;
    (Mesh::from(Rectangle::from_size(size)), Transform::from_translation(center.extend(z)))
}

/// The layer's quad and material.
pub fn build_light_layer(map: &LoadedMap, images: &mut Assets<Image>) -> (Mesh, StandardMaterial, Transform) {
    let (mesh, transform) = light_quad(map, LIGHT_Z);
    let material = StandardMaterial {
        base_color_texture: Some(images.add(light_texture(map.light.layer_image()))),
        unlit: true,
        alpha_mode: AlphaMode::Multiply,
        depth_bias: LIGHT_DEPTH_BIAS,
        ..default()
    };
    (mesh, material, transform)
}
