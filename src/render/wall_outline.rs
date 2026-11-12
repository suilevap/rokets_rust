//! The walls' black outline (`Primitive3d.AddBorders`, `LinePrimitive` in
//! XNA): top edges at height changes, feet at ground level, and vertical
//! edges down convex corners as far as the shorter of the two drops.
//!
//! Squares and corner wedges contribute to the same contours, including
//! the map's outer boundary. Adjacent strips share their offset-line
//! intersection (a miter), rather than running past each other's ends.
//! The width is fixed in world units (`StaticLinesWidthBasic`), so it
//! scales with the view's resolution like XNA's `DpiSizeMultiplier`.
//! Uprights store their centre line and signed half-width in UV.x. The
//! outline material turns these single quads towards the camera on the
//! GPU, so they cannot turn edge-on. Horizontal strips keep their miters.

use std::collections::HashMap;

use bevy::prelude::*;

use super::lines::LineMeshBuilder;
use crate::map::grid::MapGrid;
use crate::units::PX_PER_UNIT;

/// XNA's `StaticLinesOffset`: lift tops, lower feet so their inner half
/// is hidden by the wall. No lateral nudge: it separates corner joins.
pub(super) const LIFT: f32 = 0.1 / PX_PER_UNIT;
/// Half the width of a line (`StaticLinesWidthBasic`), in world units.
const HALF_WIDTH: f32 = 1.5 / PX_PER_UNIT;
/// Heights within one pixel count as equal (`ConvertUnits.SimUnitRation`).
const SAME: f32 = 1.0 / PX_PER_UNIT;

/// Directed with the wall on the right in map coordinates (y down).
struct Line {
    a: Vec2,
    b: Vec2,
    z: f32,
    bottom: f32,
    foot: bool,
}

impl Line {
    fn direction(&self) -> Vec2 {
        (self.b - self.a).normalize()
    }

    fn out(&self) -> Vec2 {
        let d = self.direction();
        Vec2::new(d.y, -d.x)
    }

    fn key(&self, at: Vec2) -> (i32, i32, u32, bool) {
        // Endpoints are cell corners and heights come from the same
        // quantized grid; keep distinct contours at a height step apart.
        (at.x as i32, at.y as i32, self.z.to_bits(), self.foot)
    }
}

fn add_face(lines: &mut Vec<Line>, a: Vec2, b: Vec2, top: f32, bottom: f32) {
    if (top - bottom).abs() <= SAME {
        return;
    }
    lines.push(Line {
        a,
        b,
        z: top,
        bottom,
        foot: false,
    });
    if bottom <= SAME {
        lines.push(Line {
            a,
            b,
            z: 0.0,
            bottom: 0.0,
            foot: true,
        });
    }
}

/// Offset shared by the ends of two strips. The map's square/diagonal
/// edges turn by at most 135 degrees, so their miters stay bounded.
fn miter(a: Vec2, b: Vec2) -> Vec2 {
    (a + b) / (1.0 + a.dot(b))
}

/// The outline of the map's walls as one static mesh, in world units.
pub fn build_wall_outline(grid: &MapGrid, cell: f32) -> Mesh {
    let mut lines = Vec::new();
    // `TilesMap._height`: a wedge counts as a wall of its own height;
    // `GetWallH`: outside the image the neighbour's height is zero.
    let height = |x, y| {
        grid.edge(x, y)
            .map_or_else(|| grid.height_at(x, y), |e| e.height)
    };
    let sides = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let corners = [
                Vec2::new(x as f32, y as f32),
                Vec2::new(x as f32 + 1.0, y as f32),
                Vec2::new(x as f32 + 1.0, y as f32 + 1.0),
                Vec2::new(x as f32, y as f32 + 1.0),
            ];
            if let Some(c) = grid.cell(x, y).filter(|c| c.is_wall()) {
                for (i, (dx, dy)) in sides.into_iter().enumerate() {
                    add_face(
                        &mut lines,
                        corners[i],
                        corners[(i + 1) % 4],
                        c.height,
                        height(x + dx, y + dy),
                    );
                }
            } else if let Some(edge) = grid.edge(x, y) {
                // The three filled corners, clockwise. Unlike a square,
                // one face crosses the cell towards its open corner.
                let filled = [
                    (edge.open + 1) % 4,
                    (edge.open + 2) % 4,
                    (edge.open + 3) % 4,
                ];
                for i in 0..3 {
                    let (a, b) = (filled[i], filled[(i + 1) % 3]);
                    let bottom = if b == (a + 1) % 4 {
                        let (dx, dy) = sides[a];
                        height(x + dx, y + dy)
                    } else {
                        0.0
                    };
                    add_face(&mut lines, corners[a], corners[b], edge.height, bottom);
                }
            }
        }
    }

    let mut starts = HashMap::<_, Vec<usize>>::new();
    let mut ends = HashMap::<_, Vec<usize>>::new();
    for (i, line) in lines.iter().enumerate() {
        starts.entry(line.key(line.a)).or_default().push(i);
        ends.entry(line.key(line.b)).or_default().push(i);
    }
    // At diagonally touching blocks several contours can share a node.
    // Take the sharpest right turn to stay on the same block, rather than
    // joining across the touching point (or dividing by a zero miter).
    let turn = |from: &Line, to: &Line| {
        let (a, b) = (from.direction(), to.direction());
        a.perp_dot(b).atan2(a.dot(b))
    };
    let previous: Vec<_> = lines
        .iter()
        .map(|line| {
            ends.get(&line.key(line.a))
                .into_iter()
                .flatten()
                .copied()
                .filter(|&j| lines[j].direction().dot(line.direction()) > -0.99)
                .max_by(|&a, &b| turn(&lines[a], line).total_cmp(&turn(&lines[b], line)))
        })
        .collect();
    let next: Vec<_> = lines
        .iter()
        .map(|line| {
            starts
                .get(&line.key(line.b))
                .into_iter()
                .flatten()
                .copied()
                .filter(|&j| line.direction().dot(lines[j].direction()) > -0.99)
                .max_by(|&a, &b| turn(line, &lines[a]).total_cmp(&turn(line, &lines[b])))
        })
        .collect();

    let world = |p: Vec2| Vec2::new(p.x * cell, -p.y * cell);
    let offset = |v: Vec2| Vec2::new(v.x, -v.y) * HALF_WIDTH;
    let mut mesh = LineMeshBuilder::default();
    for (i, line) in lines.iter().enumerate() {
        let n = line.out();
        let a_offset = offset(previous[i].map_or(n, |j| miter(lines[j].out(), n)));
        let b_offset = offset(next[i].map_or(n, |j| miter(n, lines[j].out())));
        let (a, b) = (world(line.a), world(line.b));
        let z = line.z + if line.foot { -LIFT } else { LIFT };
        mesh.add_quad([
            (a - a_offset).extend(z),
            (a + a_offset).extend(z),
            (b + b_offset).extend(z),
            (b - b_offset).extend(z),
        ]);

        if let Some(j) = previous[i] {
            let prev = &lines[j];
            let bottom = prev.bottom.max(line.bottom);
            if !line.foot
                && line.z - bottom > SAME
                && prev.direction().perp_dot(line.direction()) > 0.01
            {
                // The material expands this centre line sideways in the
                // camera's direction. The ends meet the top/foot joins.
                mesh.add_line(a.extend(z), a.extend(bottom - LIFT), 2.0 * HALF_WIDTH);
            }
        }
    }
    mesh.build()
}
