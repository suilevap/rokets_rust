//! The outland: what shows between the floor tiles and around the map.
//!
//! C# origin: `OutlandPrimitive`, built in `TilesMap.AddTiles2` (2013,
//! "OutlandColors in map XML"). It is a funnel under the map: its bottom is
//! the map's rectangle 32 units below the ground, in the background colour;
//! its sides rise outwards to 32 units above the ground, 16 cells past the
//! map's edge, where each outer corner takes one of the map's
//! `OutlandColor*` colours. Seen through the gaps between tiles and past the
//! walls in perspective, its sides give each map its own coloured
//! gradients.
//!
//! Light: in C# the light map is a layer at ground level that multiplies
//! what lies under it on screen, over the map and 4 cells around it
//! (`LightManager.BorderCells`); it is full light over the void around the
//! map's ground (`LightMap::sample_layer`). So the outland seen through a
//! gap takes the light of the gap, not of the point under it, and shows
//! its own colours around the map. [`OutlandMaterial`] does the same: each
//! pixel takes the light where the line of sight crosses the ground; parts
//! above the ground, or seen past the light layer's edge, are unlit.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::ImageSampler;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;

use crate::content::xna::XColor;
use crate::game::LoadedMap;
use crate::light::bake::TEXELS_PER_CELL;
use crate::render::map_mesh::{BACKGROUND, MeshBuilder, lerp3};

/// How far below the ground the bottom lies (`ToSimUnits(1024)`).
const DEPTH: f32 = 32.0;
/// How far above the ground the outer rim rises.
const RISE: f32 = 32.0;
/// How far past the map's edge the rim lies, in cells.
const REACH: i32 = 16;
/// How far past the map's edge the light layer reaches, in cells.
const LIGHT_BORDER: usize = 4;

/// The outland's colours multiplied by the light layer at ground level.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct OutlandMaterial {
    /// The light layer's top-left corner in the world (x, y) and its size
    /// (width, height), y running down the map.
    #[uniform(0)]
    pub area: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub light: Handle<Image>,
}

impl Material for OutlandMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://tilerider/render/outland.wesl".into()
    }
}

pub struct OutlandPlugin;

impl Plugin for OutlandPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "outland.wesl");
        app.add_plugins(MaterialPlugin::<OutlandMaterial>::default());
    }
}

/// The light layer as a texture and the material that uses it.
pub fn outland_material(map: &LoadedMap, images: &mut Assets<Image>) -> OutlandMaterial {
    let (w, h, texels) = map.light.layer_texels(LIGHT_BORDER);
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
        // The light is in sRGB like the C# one; sampled as linear it
        // multiplies the linear colours as it did the sRGB ones.
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let cell = map.cell.0;
    let per_cell = TEXELS_PER_CELL as f32;
    OutlandMaterial {
        area: Vec4::new(
            -(LIGHT_BORDER as f32) * cell,
            LIGHT_BORDER as f32 * cell,
            w as f32 / per_cell * cell,
            h as f32 / per_cell * cell,
        ),
        light: images.add(image),
    }
}

/// The outland's colours at the outer corners, `[left-top, right-top,
/// right-bottom, left-bottom]` in map orientation, as sRGB.
pub fn corner_colors(map: &LoadedMap) -> [[f32; 3]; 4] {
    let d = &map.data;
    [d.outland_color_l_t, d.outland_color_r_t, d.outland_color_r_b, d.outland_color_l_b].map(srgb)
}

fn srgb(c: XColor) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

/// Builds the outland of a `width × height`-cell map, unlit.
pub fn build_outland(width: usize, height: usize, cell: f32, corners: [[f32; 3]; 4]) -> Mesh {
    let (w, h) = (width as f32, height as f32);
    // Cells past the map's edge (0 inside it): the funnel's sides are
    // planes from the bottom's edge to the rim's, meeting on diagonals.
    let outside = |x: f32, y: f32| (-x).max(x - w).max(-y).max(y - h).max(0.0);
    let point = |x: i32, y: i32| {
        let (x, y) = (x as f32, y as f32);
        let t = outside(x, y) / REACH as f32;
        let z = -DEPTH + (DEPTH + RISE) * t;
        // The rim's colour here, blended across the rim's corners as the
        // C# quads were, then from the background at the bottom to it.
        let span = REACH as f32;
        let fx = ((x + span) / (w + 2.0 * span)).clamp(0.0, 1.0);
        let fy = ((y + span) / (h + 2.0 * span)).clamp(0.0, 1.0);
        let rim = lerp3(lerp3(corners[0], corners[1], fx), lerp3(corners[3], corners[2], fx), fy);
        (Vec3::new(x * cell, -y * cell, z), lerp3(BACKGROUND, rim, t))
    };
    let mut mb = MeshBuilder::default();
    for y in -REACH..height as i32 + REACH {
        for x in -REACH..width as i32 + REACH {
            // Split each quad along the diagonal the funnel folds on, so
            // the corner quads lie flat on its sides.
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let p = if (cx < w / 2.0) == (cy < h / 2.0) {
                [(x, y), (x, y + 1), (x + 1, y + 1), (x + 1, y)]
            } else {
                [(x, y + 1), (x + 1, y + 1), (x + 1, y), (x, y)]
            };
            let v = p.map(|(px, py)| point(px, py));
            mb.quad(v.map(|(pos, _)| pos), v.map(|(_, color)| color));
        }
    }
    mb.build()
}
