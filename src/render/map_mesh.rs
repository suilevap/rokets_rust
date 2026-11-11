//! The map's look: the floor of loose tiles and the solid walls.
//!
//! Intent (from `TilesMap.AddTiles2` and `Primitive3d`):
//! - **The floor is tiles**, one per cell, each slightly smaller than its
//!   cell and slightly tilted, so the background shows between them. Tiles
//!   sink deeper and shrink more the further they are from walls; near a
//!   wall they lean towards it. One in ten open spots gets a double tile;
//!   one in twenty tiles drops deep and dark.
//! - **Walls are blocks** as tall as their cell's alpha says, with faces
//!   only where the neighbour is lower, darkening towards the floor; their
//!   black outline is `render/wall_outline`.
//! - **The floor stays under everything else**: however a tile tilts, it
//!   is lowered until no corner rises above `layers::FLOOR_TOP`, below
//!   every sprite layer, as the C# renderer kept the floor below them.
//! - **Light**: floor tiles are unlit here; the light layer
//!   (`render/light_layer`) multiplies them on screen. Walls take
//!   `colour × light + light / 2` (the "Mul_Add05" vertex light).
//!
//! Walls keep their vertex colours in sRGB through interpolation, as XNA
//! did; `wall_material` converts them to linear in the fragment shader.
//! The other meshes use Bevy's usual linear vertex colours.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::f32::consts::PI;

use crate::game::LoadedMap;
use crate::light::bake::LightMap;
use crate::map::grid::MapGrid;
use crate::render::layers::FLOOR_TOP;

/// `MapManager.BackgroundColor` (CornflowerBlue): the clear colour, the
/// bottom of the outland, and what pale tiles are mixed from.
pub const BACKGROUND: [f32; 3] = [100.0 / 255.0, 149.0 / 255.0, 237.0 / 255.0];
/// `Settings.WallFadeStart` / `WallFadeEnd`.
const WALL_FADE_START: f32 = 0.5;
const WALL_FADE_END: f32 = 0.95;
/// How far tiles sink, per C# depth unit (`DepthConst.FloorMaxDepthScale`):
/// `amount × cell × 2 + 1` units below the walls' base, so in perspective
/// the floor reads smaller than the cars and walls above it.
const FLOOR_DEPTH_SCALE: f32 = 1.0;

#[derive(Default)]
pub(crate) struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// A quad from four corners (counter-clockwise seen from its front),
    /// each with an sRGB colour.
    pub(crate) fn quad(&mut self, corners: [Vec3; 4], colors: [[f32; 3]; 4]) {
        let colors = colors.map(|c| {
            Color::srgb(
                c[0].clamp(0.0, 1.0),
                c[1].clamp(0.0, 1.0),
                c[2].clamp(0.0, 1.0),
            )
            .to_linear()
            .to_f32_array()
        });
        self.quad_encoded(corners, colors);
    }

    /// Wall colours are converted after interpolation by `WallMaterial`.
    fn wall_quad(&mut self, corners: [Vec3; 4], colors: [[f32; 3]; 4]) {
        self.quad_encoded(corners, colors.map(|c| [c[0], c[1], c[2], 1.0]));
    }

    fn quad_encoded(&mut self, corners: [Vec3; 4], colors: [[f32; 4]; 4]) {
        let n = (corners[1] - corners[0])
            .cross(corners[3] - corners[0])
            .normalize_or_zero();
        let base = self.positions.len() as u32;
        for (p, c) in corners.iter().zip(colors) {
            self.positions.push(p.to_array());
            self.normals.push(n.to_array());
            self.colors.push(c);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub(crate) fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

pub(crate) fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// `RenderSettings` "Mul_Add05": XNA's Color saturates before the
/// side-face fade, rather than letting overbright light leak into it.
fn wall_light(c: [f32; 3], l: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| (c[i] * l[i] + l[i] / 2.0).clamp(0.0, 1.0))
}

/// Builds the floor and wall meshes of a loaded map.
pub fn build_map_meshes(map: &LoadedMap) -> (Mesh, Mesh) {
    let mut rng = SmallRng::seed_from_u64(
        map.data
            .name
            .bytes()
            .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)),
    );
    let floor = build_floor(&map.grid, map.cell.0, &mut rng);
    let walls = build_walls(&map.grid, &map.light, map.cell.0);
    (floor, walls)
}

fn build_floor(grid: &MapGrid, cell: f32, rng: &mut SmallRng) -> Mesh {
    let mut mb = MeshBuilder::default();
    let mut done = vec![false; (grid.width + 1) * (grid.height + 1)];
    let idx = |x: usize, y: usize| y * (grid.width + 1) + x;
    for y in 0..grid.height {
        for x in 0..grid.width {
            let (xi, yi) = (x as i32, y as i32);
            if !grid.is_floor(xi, yi) || done[idx(x, y)] {
                continue;
            }
            done[idx(x, y)] = true;
            // `MapManager.CreateDistanceField` measures from the cell's corner
            // in cells × cell size, at least one cell for open ground: about
            // our centre distance + ½. `amount = clamp(d / (512·cell/96))`, the
            // cell size cancels: tiles far from walls shrink and sink most.
            let d = grid.distance_to_wall(xi, yi) + 0.5;
            let amount = (d * 96.0 / 512.0).clamp(0.2, 1.0);
            let mut center = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let mut size = 1.0;
            let big = grid.is_floor(xi + 1, yi + 1)
                && grid.is_floor(xi + 1, yi)
                && grid.is_floor(xi, yi + 1)
                && grid.distance_to_wall(xi + 1, yi + 1) > 0.0
                && !done[idx(x + 1, y)]
                && !done[idx(x, y + 1)]
                && !done[idx(x + 1, y + 1)]
                && rng.random_bool(0.1);
            if big {
                for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
                    done[idx(x + dx, y + dy)] = true;
                }
                size = 2.0;
                center += Vec2::splat(0.5);
            }
            let cell_color = grid.cell(xi, yi).expect("in map").color;
            let mut color = if cell_color.a < 10 {
                lerp3(BACKGROUND, [1.0; 3], amount * 0.7)
            } else {
                let [r, g, b, a] = cell_color.to_f32();
                lerp3(BACKGROUND, [r, g, b], a)
            };
            // Depth below the walls' base, in world units.
            let mut depth = (amount * cell * 2.0 + 1.0) * FLOOR_DEPTH_SCALE;
            if rng.random_bool(0.05) {
                depth = rng.random_range(4.0..16.0) * FLOOR_DEPTH_SCALE;
                let alpha = rng.random_range(0.5..0.7);
                color = lerp3(BACKGROUND, color, alpha);
            }
            let half = (size - amount * size / 4.0) / 2.0;
            let mut tilt = Vec3::new(
                PI / 16.0 * (0.5 - rng.random::<f32>()),
                PI / 16.0 * (0.5 - rng.random::<f32>()),
                PI / 32.0 * (0.5 - rng.random::<f32>()),
            );
            if !big && d < 2.0 {
                // Lean towards the nearest wall.
                let gx = grid.distance_to_wall(xi + 1, yi) - grid.distance_to_wall(xi - 1, yi);
                let gy = grid.distance_to_wall(xi, yi + 1) - grid.distance_to_wall(xi, yi - 1);
                let g = Vec2::new(gx, -gy).normalize_or_zero() * PI / 8.0;
                tilt.x += g.y;
                tilt.y -= g.x;
            }
            let rot = Quat::from_euler(EulerRot::XYZ, tilt.x, tilt.y, tilt.z);
            let world_center = Vec3::new(center.x * cell, -center.y * cell, -depth);
            let corners_local = [
                Vec3::new(-half, -half, 0.0),
                Vec3::new(half, -half, 0.0),
                Vec3::new(half, half, 0.0),
                Vec3::new(-half, half, 0.0),
            ];
            let mut corners = corners_local.map(|c| world_center + rot * (c * cell));
            let top = corners.iter().map(|c| c.z).fold(f32::MIN, f32::max);
            if top > FLOOR_TOP {
                for c in &mut corners {
                    c.z -= top - FLOOR_TOP;
                }
            }
            mb.quad(corners, [color; 4]);
        }
    }
    mb.build()
}

fn build_walls(grid: &MapGrid, light: &LightMap, cell: f32) -> Mesh {
    let mut mb = MeshBuilder::default();
    let black = [0.0; 3];
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let Some(c) = grid.cell(x, y).copied().filter(|c| c.is_wall()) else {
                continue;
            };
            let h = c.height;
            let [r, g, b, _] = c.color.to_f32();
            let base = [r, g, b];
            // Corners of the cell, map coordinates (y down), clockwise from top-left.
            let corner = |dx: f32, dy: f32| Vec2::new(x as f32 + dx, y as f32 + dy);
            let cells = [
                corner(0.0, 0.0),
                corner(1.0, 0.0),
                corner(1.0, 1.0),
                corner(0.0, 1.0),
            ];
            let lit: [[f32; 3]; 4] = cells.map(|p| wall_light(base, light.sample(p.x, p.y)));
            let to3 = |p: Vec2, z: f32| Vec3::new(p.x * cell, -p.y * cell, z);

            // Top, counter-clockwise in the world (y up): TL, BL, BR, TR,
            // split along the diagonal that follows the light so its
            // gradient reads smoothly (`Primitive3d.GetIndices`, 2015):
            // TR–BL where the light grows the same way along x and y,
            // else TL–BR.
            let [gx, gy] = light.gradient(x as f32 + 0.5, y as f32 + 0.5);
            let order = if gx.signum() == gy.signum() {
                [3, 2, 1, 0]
            } else {
                [0, 3, 2, 1]
            };
            mb.wall_quad(order.map(|i| to3(cells[i], h)), order.map(|i| lit[i]));

            // Faces: up (y-1), right, down, left, with the two top corners
            // of each face going round the cell.
            let faces = [
                ((0, -1), 0, 1),
                ((1, 0), 1, 2),
                ((0, 1), 2, 3),
                ((-1, 0), 3, 0),
            ];
            let fade_top = WALL_FADE_START + (WALL_FADE_END - WALL_FADE_START) * h / 8.0;
            for ((dx, dy), a, b) in faces {
                // TilesMap._height includes the triangular corner fill.
                // A hidden face against an equal-height wedge can bleed
                // through its roof along the shared edge in WebGL.
                let nh = grid.edge(x + dx, y + dy)
                    .map_or_else(|| grid.height_at(x + dx, y + dy), |edge| edge.height);
                if nh >= h {
                    continue;
                }
                // XNA's Primitive3d keeps each face's gradient anchored
                // at the floor. The neighbouring wall hides the lower
                // part; restarting the fade at its top creates a seam.
                let bottom = 0.0;
                let (pa, pb) = (cells[a], cells[b]);
                mb.wall_quad(
                    [to3(pb, bottom), to3(pa, bottom), to3(pa, h), to3(pb, h)],
                    [
                        lerp3(black, lit[b], WALL_FADE_START),
                        lerp3(black, lit[a], WALL_FADE_START),
                        lerp3(black, lit[a], fade_top.min(1.0)),
                        lerp3(black, lit[b], fade_top.min(1.0)),
                    ],
                );
            }
        }
    }
    // Corner wedges: a triangle of wall in the floor cell, its diagonal
    // face towards the open corner.
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let Some(edge) = grid.edge(x, y) else {
                continue;
            };
            let h = edge.height;
            let [r, g, b, _] = edge.color.to_f32();
            let corners = [
                Vec2::new(x as f32, y as f32),
                Vec2::new(x as f32 + 1.0, y as f32),
                Vec2::new(x as f32 + 1.0, y as f32 + 1.0),
                Vec2::new(x as f32, y as f32 + 1.0),
            ];
            let lit: [[f32; 3]; 4] = corners.map(|p| wall_light([r, g, b], light.sample(p.x, p.y)));
            let to3 = |p: Vec2, z: f32| Vec3::new(p.x * cell, -p.y * cell, z);
            // The three filled corners, going round the cell (clockwise in the map).
            let a = (edge.open + 1) % 4;
            let bb = (edge.open + 2) % 4;
            let c = (edge.open + 3) % 4;
            // Top: clockwise in the map is counter-clockwise in the world.
            let tri = [
                to3(corners[a], h),
                to3(corners[c], h),
                to3(corners[bb], h),
                to3(corners[bb], h),
            ];
            mb.wall_quad(tri, [lit[a], lit[c], lit[bb], lit[bb]]);
            // The diagonal face from c to a, facing the open corner.
            let fade_top = (WALL_FADE_START + (WALL_FADE_END - WALL_FADE_START) * h / 8.0).min(1.0);
            mb.wall_quad(
                [
                    to3(corners[a], 0.0),
                    to3(corners[c], 0.0),
                    to3(corners[c], h),
                    to3(corners[a], h),
                ],
                [
                    lerp3(black, lit[a], WALL_FADE_START),
                    lerp3(black, lit[c], WALL_FADE_START),
                    lerp3(black, lit[c], fade_top),
                    lerp3(black, lit[a], fade_top),
                ],
            );
        }
    }
    mb.build()
}
