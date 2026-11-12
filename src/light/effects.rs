//! Extra light data from XNA's `DirtyLensEffectAdvanced` and `DynamicShadow`.
//! Prepared at map load; no Bevy app or GPU readback is involved.

use super::bake::Lamp;
use super::raster::{BLACK, Blend, Canvas, Placement, Rect};
use crate::map::grid::MapGrid;

/// Shadowless light circles for the screen's dirty glass. XNA used one texel
/// per four world units, halving until both dimensions fit within 512.
pub fn lens_light(lamps: &[Lamp], room: [f32; 2], cell: f32, flash: &Canvas) -> Canvas {
    let (mut w, mut h) = ((room[0] / 4.0) as usize, (room[1] / 4.0) as usize);
    while w > 512 || h > 512 {
        w /= 2;
        h /= 2;
    }
    let (w, h) = (w.max(1), h.max(1));
    let mut result = Canvas::new(w, h, BLACK);
    let scale = [w as f32 / room[0].max(1.0), h as f32 / room[1].max(1.0)];
    for lamp in lamps {
        let r = (lamp.radius * cell).clamp(4.0, 16.0) * 0.8;
        let dest = Rect::new(
            (lamp.x * cell - r) * scale[0],
            (lamp.y * cell - r) * scale[1],
            2.0 * r * scale[0],
            2.0 * r * scale[1],
        );
        result.draw(
            flash,
            flash.rect(),
            Placement::onto(flash.rect(), dest),
            [lamp.color[0], lamp.color[1], lamp.color[2], 1.0],
            Blend::Additive,
        );
    }
    result
}

/// The visible shadow-casting lamp at each floor cell. Multiple strong lamps
/// suppress a directional shadow; weak overlaps use the nearest visible lamp.
/// An ambient contact shadow remains regardless of this lookup.
#[derive(Clone, Debug)]
pub struct ShadowLights {
    width: usize,
    height: usize,
    lights: Vec<Option<usize>>,
}

impl ShadowLights {
    pub fn bake(grid: &MapGrid, lamps: &[Lamp]) -> Self {
        let mut lights = Vec::with_capacity(grid.width * grid.height);
        for y in 0..grid.height {
            for x in 0..grid.width {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let mut best = None;
                let mut nearest = f32::INFINITY;
                let mut strong = 0;
                for (i, lamp) in lamps.iter().enumerate() {
                    let d2 = (p[0] - lamp.x).powi(2) + (p[1] - lamp.y).powi(2);
                    if !lamp.use_for_dynamic_shadow
                        || lamp.radius <= 0.0
                        || d2 >= lamp.radius * lamp.radius
                        || blocked(grid, [lamp.x, lamp.y], p)
                    {
                        continue;
                    }
                    if d2 < (lamp.radius * 0.75).powi(2) {
                        strong += 1;
                    }
                    if d2 < nearest {
                        nearest = d2;
                        best = Some(i);
                    }
                }
                lights.push(if strong > 1 { None } else { best });
            }
        }
        Self {
            width: grid.width,
            height: grid.height,
            lights,
        }
    }

    pub fn at(&self, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= self.width as f32
            || y >= self.height as f32
        {
            return None;
        }
        self.lights[y as usize * self.width + x as usize]
    }
}

fn blocked(grid: &MapGrid, a: [f32; 2], b: [f32; 2]) -> bool {
    let delta = [b[0] - a[0], b[1] - a[1]];
    let steps = (delta[0].abs().max(delta[1].abs()) * 4.0).ceil().max(1.0) as usize;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = (a[0] + delta[0] * t, a[1] + delta[1] * t);
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        if grid.height_at(ix, iy) > 0.0 {
            return true;
        }
        if let Some(edge) = grid.edge(ix, iy) {
            let (u, v) = (x - x.floor(), y - y.floor());
            let filled = match edge.open {
                0 => u + v >= 1.0,
                1 => v >= u,
                2 => u + v <= 1.0,
                _ => v <= u,
            };
            if filled {
                return true;
            }
        }
    }
    false
}

/// Reversible, half-second hand-over, including disappearance into darkness.
#[derive(Default, Debug)]
pub struct ShadowBlend {
    weights: Vec<(usize, f32)>,
}

impl ShadowBlend {
    /// Returns whether another frame is needed even if the object is stationary.
    pub fn update(&mut self, target: Option<usize>, dt: f32) -> bool {
        if let Some(i) = target
            && !self.weights.iter().any(|(id, _)| *id == i)
        {
            self.weights.push((i, 0.0));
        }
        let step = dt.max(0.0) * 2.0;
        for (id, weight) in &mut self.weights {
            let goal = if Some(*id) == target { 1.0 } else { 0.0 };
            *weight += (goal - *weight).clamp(-step, step);
        }
        self.weights.retain(|(_, weight)| *weight > 0.0);
        self.weights
            .iter()
            .any(|(id, weight)| Some(*id) != target || *weight < 1.0)
    }

    pub fn weights(&self) -> &[(usize, f32)] {
        &self.weights
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowPose {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct ShadowQuad {
    pub positions: [[f32; 2]; 4],
    pub alpha: f32,
}

/// A gradient sprite, anchored a quarter along its length (XNA origin 1,2
/// on a 4x4 texture). Its far end stops at `front`, not beyond the wall limit.
fn quad(pose: ShadowPose, direction: f32, front: f32, width: f32, alpha: f32) -> ShadowQuad {
    let (s, c) = direction.sin_cos();
    let positions = [
        [-front / 3.0, -width / 2.0],
        [front, -width / 2.0],
        [front, width / 2.0],
        [-front / 3.0, width / 2.0],
    ]
    .map(|[x, y]| {
        [
            pose.position[0] + x * c - y * s,
            pose.position[1] + x * s + y * c,
        ]
    });
    ShadowQuad { positions, alpha }
}

/// One gradient far from a lamp, two at ± the object's angular half-width
/// when closer, fading out smoothly from 20 to 45 degrees. Width is the
/// oriented rectangle's projection across the light ray.
pub fn directional_shadow(
    pose: ShadowPose,
    lamp: &Lamp,
    cell: f32,
    wall_distance: f32,
    weight: f32,
) -> Vec<ShadowQuad> {
    let delta = [
        pose.position[0] - lamp.x * cell,
        pose.position[1] + lamp.y * cell,
    ];
    let distance = delta[0].hypot(delta[1]);
    let radius = lamp.radius * cell;
    if distance <= 1e-5 || radius <= 0.0 || weight <= 0.0 {
        return Vec::new();
    }
    let front = (radius - distance)
        .min(pose.size[0].max(pose.size[1]) * 2.0)
        .min(wall_distance.max(0.0) + cell);
    if front <= 0.0 {
        return Vec::new();
    }
    let direction = delta[1].atan2(delta[0]);
    let angle = direction - pose.angle;
    let width = (pose.size[0] * angle.sin().abs() + pose.size[1] * angle.cos().abs()) * 0.95;
    let half_angle = width / distance * 0.5;
    let opacity = (1.0 - (distance / radius - 0.5).abs() * 2.0).clamp(0.0, 1.0) * weight;
    if half_angle <= 2.0_f32.to_radians() {
        vec![quad(pose, direction, front, width * 1.75, opacity)]
    } else {
        let fade = ((45.0_f32.to_radians() - half_angle) / 25.0_f32.to_radians()).clamp(0.0, 1.0);
        if fade <= 0.0 {
            return Vec::new();
        }
        [-half_angle, half_angle]
            .map(|a| {
                quad(
                    pose,
                    direction + a,
                    front,
                    width * 1.75,
                    opacity * 0.5 * fade,
                )
            })
            .to_vec()
    }
}
