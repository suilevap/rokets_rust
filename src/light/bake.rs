//! The static light map: the map's lamps baked into a soft, low-resolution
//! grid of colours at load time.
//!
//! Intent (from `LightManager`): each lamp adds a round spot (the `flash2`
//! falloff) of its colour; walls stop it; the edges of shadows are softened
//! by adding the light twice, turned by a small angle around the lamp
//! (`MergeLayersWithAngle`, the penumbra); the sum starts from the map's
//! ambient light and saturates at white; the whole map is then blurred.
//! The result multiplies the floor and lights the walls' vertices.
//!
//! It is kept on the CPU: gameplay asks how dark a spot is
//! (`HighlightInDark`), and the car's shadow (later) needs the lamps.

use crate::content::map::{LightData, MapData};
use crate::map::grid::MapGrid;

/// Texels per map cell.
pub const TEXELS_PER_CELL: usize = 4;

/// The `flash2` sprite's alpha from its centre (0) to its radius (1).
const FALLOFF: [f32; 16] = [
    251.0, 246.0, 237.0, 224.0, 207.0, 187.0, 164.0, 137.0, 106.0, 82.0, 65.0, 48.0, 30.0, 13.0,
    0.0, 0.0,
];

/// `RenderSettings.PenumbraMergeAngle`.
const PENUMBRA_ANGLE: f32 = 0.02;

fn falloff(t: f32) -> f32 {
    if t >= 1.0 {
        return 0.0;
    }
    let f = t * (FALLOFF.len() - 1) as f32;
    let i = f.floor() as usize;
    let a = FALLOFF[i];
    let b = FALLOFF[(i + 1).min(FALLOFF.len() - 1)];
    (a + (b - a) * (f - i as f32)) / 255.0
}

/// A lamp, in map cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub color: [f32; 3],
    pub use_for_dynamic_shadow: bool,
}

impl Lamp {
    pub fn from_data(l: &LightData) -> Self {
        // `color *= A/255` with A then set to 255.
        let [r, g, b, a] = l.color.to_f32();
        Lamp {
            x: l.position.x,
            y: l.position.y,
            radius: l.r,
            color: [r * a, g * a, b * a],
            use_for_dynamic_shadow: l.use_for_dynamic_shadow,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LightMap {
    pub width: usize,
    pub height: usize,
    /// Linear 0..1 RGB, row-major, y down.
    texels: Vec<[f32; 3]>,
    pub lamps: Vec<Lamp>,
}

impl LightMap {
    pub fn bake(map: &MapData, grid: &MapGrid) -> LightMap {
        let width = grid.width * TEXELS_PER_CELL;
        let height = grid.height * TEXELS_PER_CELL;
        let [ar, ag, ab, aa] = map.ambient_light.to_f32();
        let ambient = [ar * aa, ag * aa, ab * aa];
        let mut texels = vec![ambient; width * height];
        let lamps: Vec<Lamp> = map.lights.iter().map(Lamp::from_data).collect();

        for lamp in &lamps {
            add_lamp(&mut texels, width, height, grid, lamp);
        }
        for t in texels.iter_mut() {
            for c in t.iter_mut() {
                *c = c.min(1.0);
            }
        }
        let texels = blur(&texels, width, height, 2);
        LightMap {
            width,
            height,
            texels,
            lamps,
        }
    }

    /// Light at a point in map cells (bilinear; black outside the map).
    pub fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let fx = x * TEXELS_PER_CELL as f32 - 0.5;
        let fy = y * TEXELS_PER_CELL as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let get = |x: i32, y: i32| -> [f32; 3] {
            let x = x.clamp(0, self.width as i32 - 1) as usize;
            let y = y.clamp(0, self.height as i32 - 1) as usize;
            self.texels[y * self.width + x]
        };
        let (x0, y0) = (x0 as i32, y0 as i32);
        let (a, b, c, d) = (get(x0, y0), get(x0 + 1, y0), get(x0, y0 + 1), get(x0 + 1, y0 + 1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bottom = c[i] + (d[i] - c[i]) * tx;
            top + (bottom - top) * ty
        })
    }

    /// RGBA8 sRGB-encoded bytes, for a texture.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.texels
            .iter()
            .flat_map(|c| {
                [
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                    255,
                ]
            })
            .collect()
    }
}

fn add_lamp(texels: &mut [[f32; 3]], width: usize, height: usize, grid: &MapGrid, lamp: &Lamp) {
    let t = TEXELS_PER_CELL as f32;
    let r = lamp.radius * t;
    let (cx, cy) = (lamp.x * t, lamp.y * t);
    let x0 = ((cx - r).floor().max(0.0)) as usize;
    let y0 = ((cy - r).floor().max(0.0)) as usize;
    let x1 = ((cx + r).ceil() as usize).min(width);
    let y1 = ((cy + r).ceil() as usize).min(height);
    for ty in y0..y1 {
        for tx in x0..x1 {
            let (px, py) = ((tx as f32 + 0.5) / t, (ty as f32 + 0.5) / t);
            let d = ((px - lamp.x).powi(2) + (py - lamp.y).powi(2)).sqrt();
            let k = falloff(d / lamp.radius);
            if k <= 0.0 {
                continue;
            }
            // Two copies turned by ±angle around the lamp, half each.
            let mut seen = 0.0;
            for sign in [-1.0f32, 1.0] {
                let (s, c) = (sign * PENUMBRA_ANGLE).sin_cos();
                let (dx, dy) = (px - lamp.x, py - lamp.y);
                let (rx, ry) = (lamp.x + dx * c - dy * s, lamp.y + dx * s + dy * c);
                if lit(grid, lamp, rx, ry) {
                    seen += 0.5;
                }
            }
            if seen <= 0.0 {
                continue;
            }
            let texel = &mut texels[ty * width + tx];
            for (t, c) in texel.iter_mut().zip(lamp.color) {
                *t += c * k * seen;
            }
        }
    }
}

/// Whether light from the lamp reaches the point. The wall cell a ray
/// stops in is lit too, so walls catch light on their faces.
fn lit(grid: &MapGrid, lamp: &Lamp, x: f32, y: f32) -> bool {
    let (dx, dy) = (x - lamp.x, y - lamp.y);
    let len = (dx * dx + dy * dy).sqrt();
    let steps = (len * 4.0).ceil().max(1.0) as i32;
    let target = (x.floor() as i32, y.floor() as i32);
    for i in 0..steps {
        let s = i as f32 / steps as f32;
        let (sx, sy) = (lamp.x + dx * s, lamp.y + dy * s);
        let cell = (sx.floor() as i32, sy.floor() as i32);
        if cell != target && grid.is_wall(cell.0, cell.1) {
            return false;
        }
    }
    true
}

/// Box blur, `passes` times: soft, like the C# mip-pyramid blur.
fn blur(src: &[[f32; 3]], w: usize, h: usize, passes: usize) -> Vec<[f32; 3]> {
    let mut a = src.to_vec();
    let mut b = vec![[0.0; 3]; a.len()];
    for _ in 0..passes {
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0; 3];
                let mut n = 0.0;
                for yy in y.saturating_sub(1)..(y + 2).min(h) {
                    for xx in x.saturating_sub(1)..(x + 2).min(w) {
                        let c = a[yy * w + xx];
                        sum[0] += c[0];
                        sum[1] += c[1];
                        sum[2] += c[2];
                        n += 1.0;
                    }
                }
                b[y * w + x] = [sum[0] / n, sum[1] / n, sum[2] / n];
            }
        }
        std::mem::swap(&mut a, &mut b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with_lamp() -> MapData {
        MapData::parse(
            r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
            <AmbientLight>FF000000</AmbientLight>
            <Lights><Item><Position>2.5 2.5</Position><Color>FFFFFFFF</Color><R>6</R></Item></Lights>
            <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary>
            <CharMap>
**********
*........*
*........*
*......*.*
*......*.*
**********
</CharMap></Asset></XnaContent>"#,
        )
        .unwrap()
    }

    #[test]
    fn lamp_lights_and_walls_shadow() {
        let map = map_with_lamp();
        let grid = MapGrid::build(&map, None);
        let light = LightMap::bake(&map, &grid);
        let near = light.sample(2.5, 2.5)[0];
        let far = light.sample(6.5, 2.5)[0];
        let behind_wall = light.sample(8.5, 4.5)[0];
        assert!(near > 0.8, "near {near}");
        assert!(far < near && far > 0.0, "far {far}");
        assert!(behind_wall < 0.1, "shadow {behind_wall}");
    }
}
