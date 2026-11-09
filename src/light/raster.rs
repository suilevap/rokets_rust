//! A small software stand-in for the render-target drawing the C# light
//! code was built from: sprites drawn with a linear sampler, triangles
//! with vertex colours, alpha and additive blending into an 8-bit target.
//!
//! C# origin: `LightManager.DrawOnLightMap` and the XNA states it used.
//! The light map was made by drawing on the GPU and read back; doing the
//! same few operations on the CPU gives the same picture without a GPU
//! round trip, works the same on the web, and keeps the light testable.
//!
//! Colours are premultiplied RGBA in 0..1, as XNA's were: a texture from
//! the content pipeline is premultiplied, and a tint multiplies every
//! channel. Like an 8-bit target, every blend saturates at 1.

/// The x range where a horizontal line at `y` crosses a triangle, if it does.
fn row_span(p: [[f32; 2]; 3], y: f32) -> Option<(f32, f32)> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for i in 0..3 {
        let (a, b) = (p[i], p[(i + 1) % 3]);
        let (ymin, ymax) = (a[1].min(b[1]), a[1].max(b[1]));
        if y < ymin || y > ymax {
            continue;
        }
        let x = if b[1] == a[1] {
            lo = lo.min(a[0].min(b[0]));
            a[0].max(b[0])
        } else {
            a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        };
        lo = lo.min(x);
        hi = hi.max(x);
    }
    (lo <= hi).then_some((lo, hi))
}

/// Premultiplied RGBA, 0..1.
pub type Rgba = [f32; 4];

pub const BLACK: Rgba = [0.0, 0.0, 0.0, 1.0];
pub const WHITE: Rgba = [1.0, 1.0, 1.0, 1.0];

/// How a drawn colour meets the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// `BlendState.AlphaBlend`: `src + dst × (1 − src.a)`.
    Alpha,
    /// `BlendState.Additive`: `src × src.a + dst`.
    Additive,
}

/// An axis-aligned rectangle in texels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
}

/// Where a source rectangle lands: `dest = position + R(rotation) ×
/// ((s − source corner − origin) × scale)`, as `SpriteBatch.Draw` places it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: [f32; 2],
    pub origin: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
}

impl Placement {
    /// Stretched over a destination rectangle.
    pub fn onto(src: Rect, dest: Rect) -> Placement {
        Placement {
            position: [dest.x, dest.y],
            origin: [0.0, 0.0],
            rotation: 0.0,
            scale: [dest.w / src.w, dest.h / src.h],
        }
    }
}

/// A target or a texture.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub texels: Vec<Rgba>,
}

impl Canvas {
    pub fn new(width: usize, height: usize, fill: Rgba) -> Canvas {
        Canvas {
            width,
            height,
            texels: vec![fill; width * height],
        }
    }

    /// A texture from straight-alpha RGBA bytes, premultiplied as the
    /// content pipeline did.
    pub fn from_rgba8(width: usize, height: usize, bytes: &[u8]) -> Canvas {
        let texels = bytes
            .chunks_exact(4)
            .map(|p| {
                let a = p[3] as f32 / 255.0;
                [
                    p[0] as f32 / 255.0 * a,
                    p[1] as f32 / 255.0 * a,
                    p[2] as f32 / 255.0 * a,
                    a,
                ]
            })
            .collect();
        Canvas {
            width,
            height,
            texels,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    pub fn get(&self, x: usize, y: usize) -> Rgba {
        self.texels[y * self.width + x]
    }

    pub fn fill(&mut self, color: Rgba) {
        self.texels.fill(color);
    }

    /// Bilinear sample at a point in texels (texel centres at +0.5),
    /// clamped to the edges (`SamplerState.LinearClamp`).
    pub fn sample(&self, u: f32, v: f32) -> Rgba {
        let (fx, fy) = (u - 0.5, v - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let (w, h) = (self.width as i64, self.height as i64);
        let (a, b, c, d) = if xi >= 0 && yi >= 0 && xi + 1 < w && yi + 1 < h {
            // All four texels inside: no clamping.
            let i = (yi * w + xi) as usize;
            let row = self.width;
            (
                self.texels[i],
                self.texels[i + 1],
                self.texels[i + row],
                self.texels[i + row + 1],
            )
        } else {
            let at = |x: i64, y: i64| {
                let x = x.clamp(0, w - 1) as usize;
                let y = y.clamp(0, h - 1) as usize;
                self.get(x, y)
            };
            (
                at(xi, yi),
                at(xi + 1, yi),
                at(xi, yi + 1),
                at(xi + 1, yi + 1),
            )
        };
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bottom = c[i] + (d[i] - c[i]) * tx;
            top + (bottom - top) * ty
        })
    }

    fn blend(&mut self, x: usize, y: usize, src: Rgba, blend: Blend) {
        let dst = &mut self.texels[y * self.width + x];
        // Skip what changes nothing: clear over anything, and black added
        // where the target is already opaque.
        let clear_rgb = src[0] == 0.0 && src[1] == 0.0 && src[2] == 0.0;
        match blend {
            Blend::Alpha if clear_rgb && src[3] == 0.0 => return,
            Blend::Additive if clear_rgb && (src[3] == 0.0 || dst[3] >= 1.0) => return,
            _ => {}
        }
        for i in 0..4 {
            let v = match blend {
                Blend::Alpha => src[i] + dst[i] * (1.0 - src[3]),
                Blend::Additive => src[i] * src[3] + dst[i],
            };
            dst[i] = v.clamp(0.0, 1.0);
        }
    }

    /// Draws `src_rect` of `src` placed by `place`, tinted, blended. Only
    /// texels whose centres fall inside the placed rectangle change.
    pub fn draw(
        &mut self,
        src: &Canvas,
        src_rect: Rect,
        place: Placement,
        tint: Rgba,
        blend: Blend,
    ) {
        let (sin, cos) = place.rotation.sin_cos();
        let to_dest = |sx: f32, sy: f32| {
            let lx = (sx - place.origin[0]) * place.scale[0];
            let ly = (sy - place.origin[1]) * place.scale[1];
            [
                place.position[0] + lx * cos - ly * sin,
                place.position[1] + lx * sin + ly * cos,
            ]
        };
        let corners = [
            to_dest(0.0, 0.0),
            to_dest(src_rect.w, 0.0),
            to_dest(src_rect.w, src_rect.h),
            to_dest(0.0, src_rect.h),
        ];
        let Some((x0, y0, x1, y1)) = self.bounds(&corners) else {
            return;
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (
                    x as f32 + 0.5 - place.position[0],
                    y as f32 + 0.5 - place.position[1],
                );
                // Back to the source: undo the rotation, then the scale.
                let lx = (px * cos + py * sin) / place.scale[0] + place.origin[0];
                let ly = (-px * sin + py * cos) / place.scale[1] + place.origin[1];
                if lx < 0.0 || ly < 0.0 || lx >= src_rect.w || ly >= src_rect.h {
                    continue;
                }
                let s = src.sample(src_rect.x + lx, src_rect.y + ly);
                let c = std::array::from_fn(|i| s[i] * tint[i]);
                self.blend(x, y, c, blend);
            }
        }
    }

    /// A triangle with vertex colours; texels whose centres lie inside it
    /// (either winding) get the interpolated colour.
    pub fn triangle(&mut self, p: [[f32; 2]; 3], c: [Rgba; 3], blend: Blend) {
        let Some((x0, y0, x1, y1)) = self.bounds(&p) else {
            return;
        };
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-12 {
            return;
        }
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            // Where this row's centre line crosses the triangle, give or
            // take a texel; only that span is tested.
            let Some((sx0, sx1)) = row_span(p, py) else {
                continue;
            };
            let sx0 = ((sx0 - 1.0).floor().max(x0 as f32)) as usize;
            let sx1 = ((sx1 + 1.0).ceil().min(x1 as f32)) as usize;
            for x in sx0..sx1 {
                let px = x as f32 + 0.5;
                let w = |a: [f32; 2], b: [f32; 2]| {
                    ((b[0] - a[0]) * (py - a[1]) - (px - a[0]) * (b[1] - a[1])) / area
                };
                let (w0, w1, w2) = (w(p[1], p[2]), w(p[2], p[0]), w(p[0], p[1]));
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let color = std::array::from_fn(|i| c[0][i] * w0 + c[1][i] * w1 + c[2][i] * w2);
                self.blend(x, y, color, blend);
            }
        }
    }

    /// Texel range covering the points, clipped to the canvas.
    fn bounds(&self, points: &[[f32; 2]]) -> Option<(usize, usize, usize, usize)> {
        let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        if !(min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite()) {
            return None;
        }
        let x0 = (min_x - 0.5).floor().max(0.0) as usize;
        let y0 = (min_y - 0.5).floor().max(0.0) as usize;
        let x1 = ((max_x + 0.5).ceil().max(0.0) as usize).min(self.width);
        let y1 = ((max_y + 0.5).ceil().max(0.0) as usize).min(self.height);
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }

    /// `LightManager.BlurSurface`: the target drawn at a half, a quarter
    /// and an eighth of its size side by side, then the three stretched
    /// back over it, a third each. `clear` fills the unused helper area,
    /// which the linear sampler reaches at the copies' edges.
    pub fn blur(&mut self, clear: Rgba) {
        const COPIES: usize = 3;
        let (w, h) = (self.width, self.height);
        let mut helper = Canvas::new(w, h / 2, clear);
        let mut rect = (0usize, 0usize, w / 2, h / 2);
        let mut copies = Vec::with_capacity(COPIES);
        for _ in 0..COPIES {
            let r = Rect::new(rect.0 as f32, rect.1 as f32, rect.2 as f32, rect.3 as f32);
            copies.push(r);
            helper.draw(
                self,
                self.rect(),
                Placement::onto(self.rect(), r),
                WHITE,
                Blend::Alpha,
            );
            rect.0 += rect.2;
            rect.2 /= 2;
            rect.3 /= 2;
        }
        self.fill(BLACK);
        // `Color.White` with alpha 255 / 3, drawn additively.
        let third = [1.0, 1.0, 1.0, (255 / COPIES) as f32 / 255.0];
        let full = self.rect();
        for r in copies.iter().rev() {
            if r.w >= 1.0 && r.h >= 1.0 {
                self.draw(
                    &helper,
                    *r,
                    Placement::onto(*r, full),
                    third,
                    Blend::Additive,
                );
            }
        }
    }
}
