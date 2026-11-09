//! The black outline of the walls: thin lines along the edges where a
//! wall's height changes, which make the blocks read as blocks from above.
//!
//! C# origin: `Primitive3d.AddBorders`, called for every wall cell and
//! corner wedge from `TilesMap.AddTiles2`. For each side of a wall cell
//! whose neighbour has another height (lower or higher), a line along the
//! side's top edge, lifted a hair (`StaticLinesOffset`); where the side
//! reaches the floor, one along its foot too (`DrawLowBorders`). They were
//! one-pixel lines (`Drawer.Lines.AddStaticLine`), as here: a line-list
//! mesh, which draws one pixel wide on every backend, the web included.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::map::grid::MapGrid;
use crate::units::PX_PER_UNIT;

/// How far lines float off the edge they follow, up and out of the wall
/// (C#: `StaticLinesOffset`, up only). Lying exactly on the edge, a line
/// shares the depth of the faces meeting there and loses to them.
const LIFT: f32 = 0.5 / PX_PER_UNIT;
/// Heights closer than this are the same (`ConvertUnits.SimUnitRation`,
/// one pixel).
const SAME: f32 = 1.0 / PX_PER_UNIT;

/// The outline of the map's walls as a line list, in world units.
pub fn build_wall_outline(grid: &MapGrid, cell: f32) -> Mesh {
    let mut lines: Vec<[Vec3; 2]> = Vec::new();
    let at = |p: Vec2, z: f32| Vec3::new(p.x * cell, -p.y * cell, z);
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
            for (i, (dx, dy)) in sides.into_iter().enumerate() {
                // Past the map's edge there are no faces to outline.
                if grid.cell(x + dx, y + dy).is_none() {
                    continue;
                }
                let nh = grid.height_at(x + dx, y + dy);
                if (h - nh).abs() <= SAME {
                    continue;
                }
                let out = Vec2::new(dx as f32, dy as f32) * LIFT / cell;
                let (a, b) = (corners[i] + out, corners[(i + 1) % 4] + out);
                lines.push([at(a, h + LIFT), at(b, h + LIFT)]);
                if nh <= SAME {
                    lines.push([at(a, LIFT), at(b, LIFT)]);
                }
            }
        }
    }
    // Corner wedges: the top and the foot of the slanted face.
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
            let out = (corners[edge.open] - (a + b) / 2.0).normalize() * LIFT / cell;
            let (a, b) = (a + out, b + out);
            lines.push([at(a, edge.height + LIFT), at(b, edge.height + LIFT)]);
            lines.push([at(a, LIFT), at(b, LIFT)]);
        }
    }
    let positions: Vec<[f32; 3]> = lines.iter().flat_map(|l| l.map(|p| p.to_array())).collect();
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let indices = (0..positions.len() as u32).collect();
    Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}
