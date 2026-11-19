//! Which lamp throws a moving object's shadow, spot by spot.
//!
//! C# origin: `LightManager.FillStaticLightPos` and `GetStaticLightInfo`
//! (2012), with `MapManager.LineCollide`. Every lamp marked
//! `UseForDynamicShadow` claimed the spots it could see, on a grid four
//! light texels across, unless a lamp seen from there was closer. The
//! object then took its shadow from the one lamp of its spot, rather than
//! one per lamp: cheap, and it reads as "the light that matters here".
//! Built once with the map; asking is a lookup.
//!
//! As in C#, a spot is tested from its corner, the box around each lamp's
//! circle is filled (beyond the radius the shadow has no length, see
//! `render/shadow`), and a spot within 0.75 units of a lamp that was
//! already claimed by another is claimed by none. (The C# check reads
//! `l2 / size * size < 0.75²`, which is just `l2 < 0.75²`; overlapping
//! lights were meant there, the effect is kept.)

use crate::light::bake::{Lamp, LightFrame};
use crate::map::grid::MapGrid;

/// Spots per light texel side (`StaticMapLightsPosScale = Scale * 4`).
const TEXELS_PER_SPOT: f32 = 4.0;
/// 0.75² world units².
const TOO_CLOSE2: f32 = 0.75 * 0.75;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Spot {
    Free,
    Lamp(usize),
    /// `LightSpot.Empty`: no lamp, and no later lamp may take it.
    Nobody,
}

/// The shadow-casting lamp of each spot of the map.
#[derive(Clone, Debug)]
pub struct ShadowLamps {
    /// Spot size, world units.
    step: f32,
    /// The map's size, world units.
    room: [f32; 2],
    columns: usize,
    spots: Vec<Spot>,
}

impl ShadowLamps {
    /// `lamps` in the order the map lists them, as `LightMap::lamps`.
    pub fn build(grid: &MapGrid, frame: &LightFrame, lamps: &[Lamp]) -> ShadowLamps {
        let step = frame.scale * TEXELS_PER_SPOT;
        let room = frame.room;
        let columns = (room[0] / step) as usize + 1;
        let rows = (room[1] / step) as usize + 1;
        let mut spots = vec![Spot::Free; columns * rows];
        let cell = frame.cell;
        for (i, lamp) in lamps.iter().enumerate().filter(|(_, l)| l.use_for_dynamic_shadow) {
            let at = [lamp.x * cell, lamp.y * cell];
            let r = lamp.radius * cell;
            // `DataMap.GetRect`: corners clamped to the room, size truncated.
            let lo = [(at[0] - r).clamp(0.0, room[0]), (at[1] - r).clamp(0.0, room[1])];
            let hi = [(at[0] + r).clamp(0.0, room[0]), (at[1] + r).clamp(0.0, room[1])];
            let (x0, y0) = ((lo[0] / step) as usize, (lo[1] / step) as usize);
            let (w, h) = (((hi[0] - lo[0]) / step) as usize, ((hi[1] - lo[1]) / step) as usize);
            for y in y0..(y0 + h).min(rows) {
                for x in x0..(x0 + w).min(columns) {
                    let spot = &mut spots[y * columns + x];
                    let p = [x as f32 * step, y as f32 * step];
                    let d2 = |a: [f32; 2]| (p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2);
                    match *spot {
                        Spot::Nobody => {}
                        Spot::Free => {
                            if !grid.line_collides(cell, at, p) {
                                *spot = Spot::Lamp(i);
                            }
                        }
                        Spot::Lamp(other) => {
                            if grid.line_collides(cell, at, p) {
                                continue;
                            }
                            let o = &lamps[other];
                            if d2(at) < TOO_CLOSE2 {
                                *spot = Spot::Nobody;
                            } else if d2(at) < d2([o.x * cell, o.y * cell]) {
                                *spot = Spot::Lamp(i);
                            }
                        }
                    }
                }
            }
        }
        ShadowLamps {
            step,
            room,
            columns,
            spots,
        }
    }

    /// The lamp (an index into the map's lamps) whose shadow an object at
    /// `(x, y)` in map cells casts, if any.
    pub fn lamp_at(&self, x: f32, y: f32, cell: f32) -> Option<usize> {
        let (x, y) = (x * cell, y * cell);
        if x < 0.0 || y < 0.0 || x >= self.room[0] || y >= self.room[1] {
            return None;
        }
        let i = (y / self.step) as usize * self.columns + (x / self.step) as usize;
        match self.spots.get(i) {
            Some(Spot::Lamp(lamp)) => Some(*lamp),
            _ => None,
        }
    }
}
