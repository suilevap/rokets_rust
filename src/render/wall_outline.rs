//! The black outline of the walls: lines along the edges where a wall's
//! height changes, which make the blocks read as blocks from above.
//!
//! C# origin: `Primitive3d.AddBorders`, called for every wall cell and
//! corner wedge from `TilesMap.AddTiles2`. For each side of a wall cell
//! whose neighbour has another height (lower or higher; a wedge counts as
//! a wall of its own height, as `TilesMap._height` had it), a line along
//! the side's top edge, lifted a hair (`StaticLinesOffset`); where the side
//! reaches the floor, one along its foot too (`DrawLowBorders`); down each
//! outer corner where both sides meeting there drop, an upright one as
//! long as the shorter drop (`LinePrimitive.InitVertical`; C# drew it at
//! two of the four corners, its screenshots show all); for a wedge, the
//! top and foot of its slanted face.
//!
//! The lines are flat strips of a fixed width in the world
//! (`LinePrimitive`, `StaticLinesWidth`: 1.5 px each side of the edge), so
//! on screen they grow with the resolution and pixel density, as C# scaled
//! them by `DpiSizeMultiplier`; one-pixel lines vanish on dense screens and
//! along slanted edges.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::map::grid::MapGrid;
use crate::units::PX_PER_UNIT;

/// How far lines float off the edge they follow, up and out of the wall
/// (C#: `StaticLinesOffset`, up only). Lying exactly on the edge, a line
/// shares the depth of the faces meeting there and loses to them.
const LIFT: f32 = 0.5 / PX_PER_UNIT;
/// Half the width of a line, world units (`StaticLinesWidthBasic`).
const HALF_WIDTH: f32 = 1.5 / PX_PER_UNIT;
/// Heights closer than this are the same (`ConvertUnits.SimUnitRation`,
/// one pixel).
const SAME: f32 = 1.0 / PX_PER_UNIT;

/// A line along an edge: its ends in map cells, its height, and which way
/// (in map cells) is out of the wall.
struct Line {
    a: Vec2,
    b: Vec2,
    z: f32,
    out: Vec2,
    /// A foot line lies wholly on the floor in front of the face; a top
    /// line straddles the edge.
    foot: bool,
}

/// A vertical edge at a wall's outer corner: where, in map cells, its top
/// and bottom heights, and which way (in map cells) is out of the corner.
struct Upright {
    at: Vec2,
    top: f32,
    bottom: f32,
    out: Vec2,
}

/// The outline of the map's walls as black strips, in world units.
pub fn build_wall_outline(grid: &MapGrid, cell: f32) -> Mesh {
    let mut lines: Vec<Line> = Vec::new();
    let mut uprights: Vec<Upright> = Vec::new();
    // What a side of a wall cell faces: a wedge stands as tall as it is.
    let neighbour_height = |x: i32, y: i32| {
        grid.edge(x, y)
            .map_or_else(|| grid.height_at(x, y), |e| e.height)
    };
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let Some(c) = grid.cell(x, y).filter(|c| c.is_wall()) else {
                continue;
            };
            let h = c.height;
            // Corners in map cells, clockwise from the top-left; side i runs
            // from corner i to corner i + 1 (up, right, down, left).
            let corner = |dx: f32, dy: f32| Vec2::new(x as f32 + dx, y as f32 + dy);
            let corners = [
                corner(0.0, 0.0),
                corner(1.0, 0.0),
                corner(1.0, 1.0),
                corner(0.0, 1.0),
            ];
            let sides = [(0, -1), (1, 0), (0, 1), (-1, 0)];
            // How far each side drops to its neighbour.
            let mut drops = [0.0f32; 4];
            for (i, (dx, dy)) in sides.into_iter().enumerate() {
                // Past the map's edge there are no faces to outline.
                if grid.cell(x + dx, y + dy).is_none() {
                    continue;
                }
                let nh = neighbour_height(x + dx, y + dy);
                drops[i] = h - nh;
                if (h - nh).abs() <= SAME {
                    continue;
                }
                let (a, b) = (corners[i], corners[(i + 1) % 4]);
                let out = Vec2::new(dx as f32, dy as f32);
                lines.push(Line {
                    a,
                    b,
                    z: h,
                    out,
                    foot: false,
                });
                if nh <= SAME {
                    lines.push(Line {
                        a,
                        b,
                        z: 0.0,
                        out,
                        foot: true,
                    });
                }
            }
            // `LinePrimitive.InitVertical`: down an outer corner where both
            // sides meeting there drop, as far as the shorter drop. Corner
            // i ends side i - 1 and starts side i.
            for i in 0..4 {
                let prev = (i + 3) % 4;
                let drop = drops[prev].min(drops[i]);
                if drop > SAME {
                    let (a, b) = (sides[prev], sides[i]);
                    uprights.push(Upright {
                        at: corners[i],
                        top: h,
                        bottom: h - drop,
                        out: Vec2::new((a.0 + b.0) as f32, (a.1 + b.1) as f32),
                    });
                }
            }
        }
    }
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let Some(edge) = grid.edge(x, y) else {
                continue;
            };
            let corner = |dx: f32, dy: f32| Vec2::new(x as f32 + dx, y as f32 + dy);
            let corners = [
                corner(0.0, 0.0),
                corner(1.0, 0.0),
                corner(1.0, 1.0),
                corner(0.0, 1.0),
            ];
            let (a, b) = (corners[(edge.open + 1) % 4], corners[(edge.open + 3) % 4]);
            // Out of the slanted face, towards the open corner.
            let out = (corners[edge.open] - (a + b) / 2.0).normalize();
            lines.push(Line {
                a,
                b,
                z: edge.height,
                out,
                foot: false,
            });
            lines.push(Line {
                a,
                b,
                z: 0.0,
                out,
                foot: true,
            });
        }
    }

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(lines.len() * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(lines.len() * 6);
    for line in &lines {
        // To the world (y up), then a strip across the edge, lengthened by
        // its width at both ends so strips meet at corners.
        let world = |p: Vec2| Vec2::new(p.x * cell, -p.y * cell);
        let (a, b) = (world(line.a), world(line.b));
        let out = Vec2::new(line.out.x, -line.out.y).normalize();
        let along = (b - a).normalize();
        let (near, far, reach) = if line.foot {
            (LIFT, LIFT + 2.0 * HALF_WIDTH, 2.0 * HALF_WIDTH)
        } else {
            (-HALF_WIDTH, HALF_WIDTH, HALF_WIDTH)
        };
        let (a, b) = (a - along * reach, b + along * reach);
        let z = line.z + LIFT;
        let base = positions.len() as u32;
        for p in [a + out * near, a + out * far, b + out * far, b + out * near] {
            positions.push([p.x, p.y, z]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    for upright in &uprights {
        // A strip standing across the corner's diagonal, nudged out of it.
        let out = Vec2::new(upright.out.x, -upright.out.y).normalize();
        let across = out.perp() * HALF_WIDTH;
        let at = Vec2::new(upright.at.x * cell, -upright.at.y * cell) + out * LIFT;
        let (top, bottom) = (upright.top + LIFT, upright.bottom);
        let base = positions.len() as u32;
        for (p, z) in [
            (at - across, top),
            (at + across, top),
            (at + across, bottom),
            (at - across, bottom),
        ] {
            positions.push([p.x, p.y, z]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
