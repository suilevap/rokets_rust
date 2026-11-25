//! Float images and the few operations the light bake needs: a lamp's
//! falloff stretched, turned and added in a colour, shadows multiplied in,
//! a separable blur, a box downscale.
//!
//! C# origin: `LightManager.DrawOnLightMap`, `BlurSurface` and the XNA
//! render-target states they drew with. The C# light was drawn on 8-bit
//! targets in sRGB numbers, premultiplied, saturating at 1 on every blend,
//! and blurred by stretching three downscaled copies back over the target.
//! Here the light is plain float arithmetic: lamps add, shadows multiply,
//! nothing saturates until the bake maps the sum to a multiplier once at
//! the end (`light/bake`), and the blur is a Gaussian, separable.
//!
//! No Bevy: everything here is unit-testable and runs at load, on the CPU.

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

/// An image of `N` float channels per texel: one for a falloff, a shadow
/// or a reach ([`Mask`]), three for light ([`Rgb`]). Values are plain
/// numbers, unclamped: what they mean is up to the bake.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane<const N: usize> {
    pub width: usize,
    pub height: usize,
    pub texels: Vec<[f32; N]>,
}

/// One value per texel.
pub type Mask = Plane<1>;
/// Light: a red, green and blue amount per texel.
pub type Rgb = Plane<3>;

impl Mask {
    /// The alpha of straight RGBA bytes, 0..1: what a white sprite like
    /// the lamp's `flash2` is.
    pub fn from_alpha(width: usize, height: usize, rgba: &[u8]) -> Mask {
        Plane {
            width,
            height,
            texels: rgba.chunks_exact(4).map(|p| [p[3] as f32 / 255.0]).collect(),
        }
    }

    /// Multiplies by `1 − alpha` over a triangle, `alpha` interpolated from
    /// its corners: a shadow of that opacity. Texels whose centres lie
    /// inside it change, either winding.
    pub fn shade(&mut self, p: [[f32; 2]; 3], alpha: [f32; 3]) {
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
                let a = alpha[0] * w0 + alpha[1] * w1 + alpha[2] * w2;
                self.texels[y * self.width + x][0] *= 1.0 - a;
            }
        }
    }
}

impl<const N: usize> Plane<N> {
    pub fn new(width: usize, height: usize, fill: [f32; N]) -> Plane<N> {
        Plane {
            width,
            height,
            texels: vec![fill; width * height],
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    pub fn get(&self, x: usize, y: usize) -> [f32; N] {
        self.texels[y * self.width + x]
    }

    /// Every channel of every texel through `f`.
    pub fn map(&mut self, f: impl Fn(f32) -> f32) {
        for t in &mut self.texels {
            *t = t.map(&f);
        }
    }

    /// Bilinear sample at a point in texels (texel centres at +0.5),
    /// clamped to the edges.
    pub fn sample(&self, u: f32, v: f32) -> [f32; N] {
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

    /// Adds `src_rect` of `src` placed by `place`, times `weight` per
    /// channel. Only texels whose centres fall inside the placed rectangle
    /// change; the source is sampled bilinearly.
    pub fn add_placed(&mut self, src: &Mask, src_rect: Rect, place: Placement, weight: [f32; N]) {
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
                let s = src.sample(src_rect.x + lx, src_rect.y + ly)[0];
                let dst = &mut self.texels[y * self.width + x];
                for i in 0..N {
                    dst[i] += s * weight[i];
                }
            }
        }
    }

    /// Texel range covering the points, clipped to the image.
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

    /// A Gaussian blur of standard deviation `sigma` texels, one pass along
    /// each axis, the edge texels extended outwards. A flat image stays
    /// flat and the total stays (up to the edges' extension).
    pub fn blur(&mut self, sigma: f32) {
        self.blur_padded(sigma, None);
    }

    /// As [`Plane::blur`], with `outside` as what lies beyond the edges
    /// instead of the edge texels extended.
    pub fn blur_padded(&mut self, sigma: f32, outside: Option<[f32; N]>) {
        if sigma <= 0.0 {
            return;
        }
        let radius = (sigma * 3.0).ceil() as i64;
        let mut kernel: Vec<f32> = (-radius..=radius)
            .map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp())
            .collect();
        let sum: f32 = kernel.iter().sum();
        kernel.iter_mut().for_each(|k| *k /= sum);

        let (w, h) = (self.width, self.height);
        let mut tmp = vec![[0.0; N]; w * h];
        // Along x, then along y; `at` clamps to the edge.
        for y in 0..h {
            for x in 0..w {
                tmp[y * w + x] = Self::weigh(&kernel, radius, |i| {
                    let xi = x as i64 + i;
                    match outside {
                        Some(o) if xi < 0 || xi >= w as i64 => o,
                        _ => self.texels[y * w + xi.clamp(0, w as i64 - 1) as usize],
                    }
                });
            }
        }
        for y in 0..h {
            for x in 0..w {
                self.texels[y * w + x] = Self::weigh(&kernel, radius, |i| {
                    let yi = y as i64 + i;
                    match outside {
                        Some(o) if yi < 0 || yi >= h as i64 => o,
                        _ => tmp[yi.clamp(0, h as i64 - 1) as usize * w + x],
                    }
                });
            }
        }
    }

    fn weigh(kernel: &[f32], radius: i64, at: impl Fn(i64) -> [f32; N]) -> [f32; N] {
        let mut sum = [0.0; N];
        for (k, i) in kernel.iter().zip(-radius..=radius) {
            let t = at(i);
            for c in 0..N {
                sum[c] += t[c] * k;
            }
        }
        sum
    }

    /// The mean of each `factor`×`factor` block, for an image `1/factor`
    /// the size (a remainder at the right and bottom is dropped).
    pub fn downscale(&self, factor: usize) -> Plane<N> {
        let (w, h) = (self.width / factor, self.height / factor);
        let mut out = Plane::new(w, h, [0.0; N]);
        let share = 1.0 / (factor * factor) as f32;
        for y in 0..h * factor {
            for x in 0..w * factor {
                let t = self.get(x, y);
                let dst = &mut out.texels[(y / factor) * w + x / factor];
                for c in 0..N {
                    dst[c] += t[c] * share;
                }
            }
        }
        out
    }

    /// `src_rect` of this image as one of `width`×`height` texels, each the
    /// mean of the area it covers (sampled in a grid finer than the source).
    pub fn resample(&self, src_rect: Rect, width: usize, height: usize) -> Plane<N> {
        let mut out = Plane::new(width, height, [0.0; N]);
        let (sw, sh) = (src_rect.w / width as f32, src_rect.h / height as f32);
        // At least one sample per source texel across each new texel.
        let (nx, ny) = (sw.ceil().max(1.0) as usize, sh.ceil().max(1.0) as usize);
        let share = 1.0 / (nx * ny) as f32;
        for y in 0..height {
            for x in 0..width {
                let dst = &mut out.texels[y * width + x];
                for j in 0..ny {
                    for i in 0..nx {
                        let u = src_rect.x + (x as f32 + (i as f32 + 0.5) / nx as f32) * sw;
                        let v = src_rect.y + (y as f32 + (j as f32 + 0.5) / ny as f32) * sh;
                        let s = self.sample(u, v);
                        for c in 0..N {
                            dst[c] += s[c] * share;
                        }
                    }
                }
            }
        }
        out
    }
}
