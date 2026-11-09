//! The static light: the map's lamps baked at load time into the light
//! layer (laid over the map, multiplying what lies under it) and the
//! walls' light (sampled per wall vertex).
//!
//! C# origin: `LightManager` (2012–2015; "Light: Reworked blur alg" 2014).
//! It drew on small render targets and read the result back; the same
//! steps here, on [`raster`](super::raster):
//!
//! 1. A target about 512 texels on its long side covers the map and 4
//!    cells around it, so each map gets the same softness on screen
//!    whatever its size. It starts at the map's ambient light, the border
//!    all white (`ClearStaticLightBuffer`, `AddBorder`).
//! 2. Each lamp: the `flash2` sprite at its radius on black, then a black
//!    quad behind every wall edge in reach, stretched away from the lamp
//!    (`RenderLight`, `AddShadow`); the result added twice in the lamp's
//!    colour at half strength, turned ±0.02 rad around the lamp, which
//!    softens shadow edges (`MergeLayersWithAngle`, the penumbra).
//! 3. The light layer is that, blurred (`BlurSurface`: copies at ½, ¼ and
//!    ⅛ size averaged), plus full light over the void around the map's
//!    ground (`MapManager.AddOutSideLight`).
//! 4. The walls' light is the same before the blur and the void, at a
//!    quarter of the size, blurred, and read per texel without blending
//!    (`PrepareLightMapImage`, `GetColor`).
//!
//! It is kept on the CPU: gameplay asks how dark a spot is
//! (`HighlightInDark`), and the car's shadow (later) needs the lamps.

use crate::content::map::{LightData, MapData};
use crate::light::raster::{BLACK, Blend, Canvas, Placement, Rgba, WHITE};
use crate::map::grid::MapGrid;

/// How far past the map's edge the light reaches, in cells
/// (`LightManager.BorderCells`).
pub const BORDER_CELLS: usize = 4;
/// Texels on the long side of the light target.
const TARGET_SIZE: f32 = 512.0;
/// `RenderSettings.PenumbraMergeAngle`.
const PENUMBRA_ANGLE: f32 = 0.02;
/// Closer than this (texels) a lamp counts as lying on a wall edge.
const ON_EDGE: f32 = 1e-3;
/// The walls' light is this many times smaller (`_lightMapImageLevel` 2).
const WALL_LEVEL: usize = 2;

/// A lamp, in map cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    /// sRGB 0..1, already scaled by its alpha.
    pub color: [f32; 3],
    pub use_for_dynamic_shadow: bool,
}

impl Lamp {
    pub fn from_data(l: &LightData) -> Self {
        // `color *= A / 255f` on bytes, truncating; then A = 255.
        let a = l.color.a as f32 / 255.0;
        let scaled = |c: u8| (c as f32 * a).floor() / 255.0;
        Lamp {
            x: l.position.x,
            y: l.position.y,
            radius: l.r,
            color: [scaled(l.color.r), scaled(l.color.g), scaled(l.color.b)],
            use_for_dynamic_shadow: l.use_for_dynamic_shadow,
        }
    }
}

/// Where the light target lies in the world, in C# units (y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightFrame {
    /// World units per texel (`LightManager.Scale`).
    pub scale: f32,
    /// The border around the map, world units (`_border`, whole units).
    pub border: f32,
    /// The map's size, world units (`World.RoomSize`, whole units).
    pub room: [f32; 2],
    /// Map cell size, world units.
    pub cell: f32,
}

impl LightFrame {
    pub fn new(map_width: usize, map_height: usize, cell: f32) -> LightFrame {
        let room = [
            (map_width as f32 * cell).trunc(),
            (map_height as f32 * cell).trunc(),
        ];
        let border = (BORDER_CELLS as f32 * cell).trunc();
        let scale = (room[0].max(room[1]) + border * 2.0) / TARGET_SIZE;
        LightFrame {
            scale,
            border,
            room,
            cell,
        }
    }

    /// The target's size in texels (`_fullArea`).
    pub fn size(&self) -> (usize, usize) {
        (
            ((self.room[0] + self.border * 2.0) / self.scale) as usize,
            ((self.room[1] + self.border * 2.0) / self.scale) as usize,
        )
    }

    /// A point in map cells to the target's texels.
    pub fn texel(&self, x: f32, y: f32) -> [f32; 2] {
        [
            (x * self.cell + self.border) / self.scale,
            (y * self.cell + self.border) / self.scale,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct LightMap {
    pub frame: LightFrame,
    /// The light layer (`FinalLightTarget`), over the map and its border.
    layer: Canvas,
    /// The walls' light (`_lightMapImage`).
    walls: Canvas,
    pub lamps: Vec<Lamp>,
}

impl LightMap {
    /// Bakes the map's lamps; `flash` is the lamp sprite (`Particles\flash2`),
    /// `objects` the outlines of fixed objects that cast shadows, in map
    /// cells (`map::templates::static_shadow_outline`).
    pub fn bake(
        map: &MapData,
        grid: &MapGrid,
        flash: &Canvas,
        objects: &[[[f32; 2]; 4]],
    ) -> LightMap {
        let frame = LightFrame::new(grid.width, grid.height, cell_size(map));
        let (w, h) = frame.size();
        let lamps: Vec<Lamp> = map.lights.iter().map(Lamp::from_data).collect();

        let mut lit = Canvas::new(w, h, ambient(map));
        add_border(&mut lit, frame.border / frame.scale);
        let blockers = Blockers {
            walls: wall_edges(grid, &frame),
            objects: object_edges(objects, &frame),
        };
        for lamp in &lamps {
            add_lamp(&mut lit, &frame, &blockers, flash, lamp);
        }

        let mut layer = lit.clone();
        layer.blur(WHITE);
        let void = void_image(grid);
        let full = layer.rect();
        layer.draw(
            &void,
            void.rect(),
            Placement::onto(void.rect(), full),
            WHITE,
            Blend::Additive,
        );

        let mut walls = Canvas::new(w >> WALL_LEVEL, h >> WALL_LEVEL, BLACK);
        let small = walls.rect();
        walls.draw(
            &lit,
            lit.rect(),
            Placement::onto(lit.rect(), small),
            WHITE,
            Blend::Additive,
        );
        walls.blur(WHITE);

        LightMap {
            frame,
            layer,
            walls,
            lamps,
        }
    }

    /// The light layer at a point in map cells (bilinear), as it lies over
    /// the map and its border: what the floor, the outland and sprites
    /// are multiplied by. Beyond the border there is no layer: white.
    pub fn sample_layer(&self, x: f32, y: f32) -> [f32; 3] {
        let [u, v] = self.layer_uv(x, y);
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return [1.0; 3];
        }
        let c = self
            .layer
            .sample(u * self.layer.width as f32, v * self.layer.height as f32);
        [c[0], c[1], c[2]]
    }

    /// A point in map cells to the layer's 0..1 coordinates: the layer is
    /// stretched over the map's rectangle and border, as C# drew it.
    fn layer_uv(&self, x: f32, y: f32) -> [f32; 2] {
        let f = &self.frame;
        [
            (x * f.cell + f.border) / (f.room[0] + 2.0 * f.border),
            (y * f.cell + f.border) / (f.room[1] + 2.0 * f.border),
        ]
    }

    /// The light layer's texels, row-major from its top-left corner:
    /// `(width, height, sRGB)`.
    pub fn layer_image(&self) -> (usize, usize, Vec<[f32; 3]>) {
        let texels = self
            .layer
            .texels
            .iter()
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        (self.layer.width, self.layer.height, texels)
    }

    /// The walls' light image: `(width, height, sRGB)`.
    pub fn walls_image(&self) -> (usize, usize, Vec<[f32; 3]>) {
        let texels = self
            .walls
            .texels
            .iter()
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        (self.walls.width, self.walls.height, texels)
    }

    /// The light layer's rectangle in map cells: `(left, top, width,
    /// height)`.
    pub fn layer_rect(&self) -> [f32; 4] {
        let f = &self.frame;
        let b = f.border / f.cell;
        [
            -b,
            -b,
            f.room[0] / f.cell + 2.0 * b,
            f.room[1] / f.cell + 2.0 * b,
        ]
    }

    /// Which way the walls' light grows at a point in map cells: the
    /// brightness of the next texel right and the next one down minus this
    /// one's, in whole bytes (`LightManager.GetGradient`; only the signs
    /// are used, to lay wall tops' triangles along the light).
    pub fn gradient(&self, x: f32, y: f32) -> [i32; 2] {
        let [u, v] = self.frame.texel(x, y);
        let k = (1usize << WALL_LEVEL) as f32;
        let (tx, ty) = ((u / k) as i64, (v / k) as i64);
        // `ColorHelper.AvgColor`: the mean of the bytes, black off the image.
        let brightness = |x: i64, y: i64| -> i32 {
            if x < 0 || y < 0 || x >= self.walls.width as i64 || y >= self.walls.height as i64 {
                return 0;
            }
            let c = self.walls.get(x as usize, y as usize);
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as i32;
            (byte(c[0]) + byte(c[1]) + byte(c[2])) / 3
        };
        let here = brightness(tx, ty);
        [brightness(tx + 1, ty) - here, brightness(tx, ty + 1) - here]
    }

    /// The walls' light at a point in map cells: the texel it falls in, no
    /// blending; black off the image (`LightManager.GetColor`).
    pub fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let [u, v] = self.frame.texel(x, y);
        let k = (1usize << WALL_LEVEL) as f32;
        // C# casts to int, which rounds towards zero.
        let (tx, ty) = ((u / k) as i64, (v / k) as i64);
        if tx < 0 || ty < 0 || tx >= self.walls.width as i64 || ty >= self.walls.height as i64 {
            return [0.0; 3];
        }
        let c = self.walls.get(tx as usize, ty as usize);
        [c[0], c[1], c[2]]
    }
}

/// Map cell size in world units (`MapManager.CellSize`: pixels / 32).
fn cell_size(map: &MapData) -> f32 {
    map.scale / crate::units::PX_PER_UNIT
}

/// `AmbientLight * (A / 255)` on bytes, opaque.
fn ambient(map: &MapData) -> Rgba {
    let c = map.ambient_light;
    let a = c.a as f32 / 255.0;
    let scaled = |v: u8| (v as f32 * a).floor() / 255.0;
    [scaled(c.r), scaled(c.g), scaled(c.b), 1.0]
}

/// `AddBorder`: white, added, everywhere within `size` texels of the edge.
fn add_border(target: &mut Canvas, size: f32) {
    let (w, h) = (target.width as f32, target.height as f32);
    let outer = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let inner = [
        [size, size],
        [w - size, size],
        [w - size, h - size],
        [size, h - size],
    ];
    for i in 0..4 {
        let k = (i + 1) % 4;
        target.triangle([outer[i], outer[k], inner[i]], [WHITE; 3], Blend::Additive);
        target.triangle([inner[i], outer[k], inner[k]], [WHITE; 3], Blend::Additive);
    }
}

/// What casts shadows, as edges in texels.
struct Blockers {
    /// Wall edges: hard shadows to the lamp's reach (`AddShadow`).
    walls: Vec<[[f32; 2]; 2]>,
    /// Fixed objects' edges: shadows fading out by the lamp's reach
    /// (`AddEntShadow`).
    objects: Vec<[[f32; 2]; 2]>,
}

/// Each outline's edges in texels, going round it.
fn object_edges(objects: &[[[f32; 2]; 4]], frame: &LightFrame) -> Vec<[[f32; 2]; 2]> {
    objects
        .iter()
        .flat_map(|o| {
            let t = o.map(|[x, y]| frame.texel(x, y));
            (0..4).map(move |i| [t[i], t[(i + 1) % 4]])
        })
        .collect()
}

/// The walls' outlines in texels: each merged wall rectangle, and the
/// diagonal of each corner wedge (`PhysicsManager.CreateWall`), as the
/// C# light got them from the physics shapes.
fn wall_edges(grid: &MapGrid, frame: &LightFrame) -> Vec<[[f32; 2]; 2]> {
    let t = |x: f32, y: f32| frame.texel(x, y);
    let mut edges = Vec::new();
    for (x, y, w, h) in grid.wall_rects() {
        let (x0, y0, x1, y1) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
        let c = [t(x0, y0), t(x1, y0), t(x1, y1), t(x0, y1)];
        for i in 0..4 {
            edges.push([c[i], c[(i + 1) % 4]]);
        }
    }
    for y in 0..grid.height as i32 {
        for x in 0..grid.width as i32 {
            let Some(edge) = grid.edge(x, y) else {
                continue;
            };
            let (fx, fy) = (x as f32, y as f32);
            let corners = [
                t(fx, fy),
                t(fx + 1.0, fy),
                t(fx + 1.0, fy + 1.0),
                t(fx, fy + 1.0),
            ];
            let (a, b) = (corners[(edge.open + 1) % 4], corners[(edge.open + 3) % 4]);
            // A two-point outline: both ways round, as the C# loop went.
            edges.push([a, b]);
            edges.push([b, a]);
        }
    }
    edges
}

/// `AddLight`: the lamp's spot and shadows on a scratch target, then added to
/// `lit` twice, turned by ± the penumbra angle, in its colour at half.
fn add_lamp(
    lit: &mut Canvas,
    frame: &LightFrame,
    blockers: &Blockers,
    flash: &Canvas,
    lamp: &Lamp,
) {
    let center = frame.texel(lamp.x, lamp.y);
    let size = lamp.radius * frame.cell / frame.scale;

    // C# drew on a target as big as the map, but past the spot it stays
    // black and adds nothing: only the square around the spot is drawn,
    // on a scratch target that size (the shadows' long quads clip to it).
    let reach = size + 2.0;
    let x0 = (center[0] - reach).floor().max(0.0);
    let y0 = (center[1] - reach).floor().max(0.0);
    let x1 = (center[0] + reach).ceil().min(lit.width as f32);
    let y1 = (center[1] + reach).ceil().min(lit.height as f32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let local = |p: [f32; 2]| [p[0] - x0, p[1] - y0];
    let spot = local(center);
    let mut scratch = Canvas::new((x1 - x0) as usize, (y1 - y0) as usize, BLACK);

    // `RenderLight`: the spot on black, then the shadows.
    let half = flash.width as f32 / 2.0;
    scratch.draw(
        flash,
        flash.rect(),
        Placement {
            position: spot,
            origin: [half, flash.height as f32 / 2.0],
            rotation: 0.0,
            scale: [size / half; 2],
        },
        WHITE,
        Blend::Alpha,
    );
    for [a, b] in &blockers.walls {
        let d = distance_to_segment(center, *a, *b);
        // A lamp on the edge's line casts no shadow from it (the quad has
        // no area); rounding must not turn it into a half-plane.
        if d < size && d > ON_EDGE {
            add_shadow(&mut scratch, spot, local(*a), local(*b), size, d);
        }
    }
    for [a, b] in &blockers.objects {
        if distance_to_segment(center, *a, *b) < size {
            add_object_shadow(&mut scratch, spot, local(*a), local(*b), size);
        }
    }

    // `MergeLayersWithAngle`: alpha 255 / 2, added.
    let tint = [
        lamp.color[0],
        lamp.color[1],
        lamp.color[2],
        (255 / 2) as f32 / 255.0,
    ];
    for angle in [-PENUMBRA_ANGLE, PENUMBRA_ANGLE] {
        let place = Placement {
            position: center,
            origin: spot,
            rotation: angle,
            scale: [1.0, 1.0],
        };
        lit.draw(&scratch, scratch.rect(), place, tint, Blend::Additive);
    }
}

/// `AddShadow`: a black quad from the edge away from the lamp, long
/// enough to leave the lamp's reach.
fn add_shadow(
    target: &mut Canvas,
    lamp: [f32; 2],
    v1: [f32; 2],
    v2: [f32; 2],
    size: f32,
    distance: f32,
) {
    let away = |v: [f32; 2]| {
        let (dx, dy) = (v[0] - lamp[0], v[1] - lamp[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 || distance <= 0.0 {
            return v;
        }
        let l = size / distance * len;
        [v[0] + dx / len * l, v[1] + dy / len * l]
    };
    let (v3, v4) = (away(v2), away(v1));
    for tri in [[v1, v2, v3], [v1, v3, v4]] {
        if front_facing(tri) {
            target.triangle(tri, [BLACK; 3], Blend::Alpha);
        }
    }
}

/// `AddEntShadow`: a quad from the edge away from the lamp to its reach,
/// black at the edge and clear at the far end.
fn add_object_shadow(target: &mut Canvas, lamp: [f32; 2], v1: [f32; 2], v2: [f32; 2], size: f32) {
    let away = |v: [f32; 2]| {
        let (dx, dy) = (v[0] - lamp[0], v[1] - lamp[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            return v;
        }
        let l = size - len;
        [v[0] + dx / len * l, v[1] + dy / len * l]
    };
    let (v3, v4) = (away(v2), away(v1));
    let clear = [0.0; 4];
    for (tri, colors) in [
        ([v3, v2, v1], [clear, BLACK, BLACK]),
        ([v4, v3, v1], [clear, clear, BLACK]),
    ] {
        if front_facing(tri) {
            target.triangle(tri, colors, Blend::Alpha);
        }
    }
}

/// Whether a shadow triangle survives the C# rasterizer, which culled
/// counter-clockwise ones (`RasterizerState.CullCounterClockwise`, left on
/// by the sprite batch). Outlines go clockwise on screen, so a wall edge's
/// quad (`AddShadow`) is kept only for edges facing the lamp, and the
/// walls stay dark inside; an object edge's quad (`AddEntShadow`, wound
/// the other way) only for edges facing away, so objects stay lit and
/// their shadow starts behind them.
fn front_facing(p: [[f32; 2]; 3]) -> bool {
    (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]) > 0.0
}

fn distance_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + abx * t - p[0], a[1] + aby * t - p[1]);
    (cx * cx + cy * cy).sqrt()
}

/// `MapManager.AddOutSideLight`: one texel per cell over the map and its
/// border, white off the map's ground.
fn void_image(grid: &MapGrid) -> Canvas {
    let b = BORDER_CELLS as i32;
    let (w, h) = (
        grid.width + 2 * BORDER_CELLS,
        grid.height + 2 * BORDER_CELLS,
    );
    let mut image = Canvas::new(w, h, BLACK);
    for y in 0..h {
        for x in 0..w {
            if !grid.is_ground(x as i32 - b, y as i32 - b) {
                image.texels[y * w + x] = WHITE;
            }
        }
    }
    image
}
