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
//! Light: the C# light layer multiplies everything under it, over the map
//! and 4 cells around it (`LightManager.BorderCells`), and nothing beyond.
//! Over the void around the map's ground it is full light
//! (`LightMap::sample_layer`), so the outland is dark only under the play
//! area and shows its own colours around it. Here the light is baked into
//! the vertices, sampled straight above each point.

use bevy::prelude::*;

use crate::game::LoadedMap;
use crate::content::xna::XColor;
use crate::light::bake::LightMap;
use crate::map::grid::MapGrid;
use crate::render::map_mesh::{BACKGROUND, MeshBuilder, lerp3, mul};

/// How far below the ground the bottom lies (`ToSimUnits(1024)`).
const DEPTH: f32 = 32.0;
/// How far above the ground the outer rim rises.
const RISE: f32 = 32.0;
/// How far past the map's edge the rim lies, in cells.
const REACH: i32 = 16;
/// How far past the map's edge the light layer reaches, in cells.
const LIGHT_BORDER: f32 = 4.0;

/// The outland's colours at the outer corners, `[left-top, right-top,
/// right-bottom, left-bottom]` in map orientation, as sRGB.
pub fn corner_colors(map: &LoadedMap) -> [[f32; 3]; 4] {
    let d = &map.data;
    [d.outland_color_l_t, d.outland_color_r_t, d.outland_color_r_b, d.outland_color_l_b].map(srgb)
}

fn srgb(c: XColor) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

/// Builds the outland of a map.
pub fn build_outland(grid: &MapGrid, light: &LightMap, cell: f32, corners: [[f32; 3]; 4]) -> Mesh {
    let (width, height) = (grid.width, grid.height);
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
            let lit = outside(cx, cy) < LIGHT_BORDER;
            let v = p.map(|(px, py)| point(px, py));
            let colors = std::array::from_fn(|i| {
                let (px, py) = p[i];
                if lit {
                    mul(v[i].1, light.sample_layer(px as f32, py as f32))
                } else {
                    v[i].1
                }
            });
            mb.quad(v.map(|(pos, _)| pos), colors);
        }
    }
    mb.build()
}
