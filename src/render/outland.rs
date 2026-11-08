//! The outland: what shows between the floor tiles and around the map.
//!
//! C# origin: `OutlandPrimitive`, built first in `TilesMap.AddTiles2`
//! (2013, "OutlandColors in map XML"). A funnel of four quads under the
//! map: its inner edge is the map's rectangle 32 units below the ground,
//! in the background colour; its outer edge is 16 cells further out and
//! 32 units above the ground, its corners in the map's `OutlandColor*`.
//! Vertex colours blend between them. Seen in perspective past the walls
//! and through the gaps between tiles, it gives each map its own coloured
//! gradients. Inside it, the clear colour (the background colour) shows.
//!
//! It is unlit: the light layer (`render/light_layer`) darkens it on
//! screen where it lies under the map.

use bevy::prelude::*;

use crate::content::xna::XColor;
use crate::game::LoadedMap;
use crate::render::map_mesh::{BACKGROUND, MeshBuilder};

/// How far below the ground the inner edge lies (`ToSimUnits(1024)`).
const DEPTH: f32 = 32.0;
/// How far above the ground the outer edge lies.
const RISE: f32 = 32.0;
/// How far past the map's edge the outer edge lies, in cells.
const REACH: f32 = 16.0;

/// The outland's colours at the outer corners, `[left-top, right-top,
/// right-bottom, left-bottom]` in map orientation, as sRGB.
pub fn corner_colors(map: &LoadedMap) -> [[f32; 3]; 4] {
    let d = &map.data;
    [d.outland_color_l_t, d.outland_color_r_t, d.outland_color_r_b, d.outland_color_l_b].map(srgb)
}

fn srgb(c: XColor) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

/// Builds the outland of a `width × height`-cell map: four quads, each
/// from an inner edge to the outer one, split as the C# quads were.
pub fn build_outland(width: usize, height: usize, cell: f32, corners: [[f32; 3]; 4]) -> Mesh {
    let (w, h) = (width as f32 * cell, height as f32 * cell);
    let r = REACH * cell;
    // Left-top, right-top, right-bottom, left-bottom; map y down, world y up.
    let inner = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| Vec3::new(x, -y, -DEPTH));
    let outer = [(-r, -r), (w + r, -r), (w + r, h + r), (-r, h + r)].map(|(x, y)| Vec3::new(x, -y, RISE));
    let mut mb = MeshBuilder::default();
    for i in 0..4 {
        let k = (i + 1) % 4;
        mb.quad(
            [inner[i], outer[i], outer[k], inner[k]],
            [BACKGROUND, corners[i], corners[k], BACKGROUND],
        );
    }
    mb.build()
}
