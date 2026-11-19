//! The map as a grid of cells, built from the map image and/or `CharMap`.
//!
//! One image pixel is one cell. Its alpha says what the cell is: below the
//! map's `ImageWallTreshold` it is floor (alpha then blends its colour over
//! the background), at or above it is a wall, and how far above sets the
//! wall's height. Rows go down the map (y down), as in the XML.

use crate::content::map::MapData;
use crate::content::xna::XColor;

/// Height of one wall step in the C# game's pixels.
const WALL_STEP_PX: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// Straight (not premultiplied) colour from the image.
    pub color: XColor,
    /// Wall height in world units (0 for floor).
    pub height: f32,
}

impl Cell {
    pub fn is_wall(&self) -> bool {
        self.height > 0.0
    }
}

#[derive(Clone, Debug)]
pub struct MapGrid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
    /// Corner wedges (`MapManager.Edges`): per cell, the corner left open.
    edges: Vec<Option<Edge>>,
}

/// A triangular wall wedge in a floor cell between two walls that meet
/// at a right angle, which rounds the inside corner off with a diagonal
/// (`MapManager.GetEdgeIndex`). `open` is the corner of the cell that stays
/// floor: 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left (map
/// coordinates, y down); the wedge fills the other three corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub open: usize,
    pub height: f32,
    pub color: XColor,
}

impl MapGrid {
    /// Builds the grid from RGBA pixels (row-major, y down) and the map's
    /// `CharMap`, which is drawn over the image as in `MapInfo.BuildTextureFromCharMap`.
    pub fn build(map: &MapData, image: Option<(usize, usize, &[u8])>) -> MapGrid {
        let char_rows = map
            .char_map
            .as_deref()
            .map(split_char_map_rows)
            .unwrap_or_default();
        let char_w = char_rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let (img_w, img_h) = image.map(|(w, h, _)| (w, h)).unwrap_or((0, 0));
        let width = img_w.max(char_w);
        let height = img_h.max(char_rows.len());

        let mut pixels = vec![XColor::rgba(0, 0, 0, 0); width * height];
        if let Some((w, h, data)) = image {
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) * 4;
                    pixels[y * width + x] =
                        XColor::rgba(data[i], data[i + 1], data[i + 2], data[i + 3]);
                }
            }
        }
        let dict = parse_char_map_dictionary(map.char_map_dictionary.as_deref().unwrap_or(""));
        let mut rng = StableRng::new(stable_string_hash(&map.name));
        for (y, row) in char_rows.iter().enumerate() {
            for (x, key) in row.chars().enumerate() {
                let Some(colors) = dict.iter().find(|(k, _)| *k == key).map(|(_, c)| c) else {
                    continue;
                };
                let chosen = if colors.len() == 1 {
                    colors[0]
                } else {
                    colors[rng.next_below(colors.len())]
                };
                if chosen.a > 0 {
                    pixels[y * width + x] = chosen;
                }
            }
        }

        let threshold = map.image_wall_treshold;
        let cells = pixels
            .into_iter()
            .map(|color| Cell {
                color,
                height: wall_height(color.a, threshold),
            })
            .collect();
        let mut grid = MapGrid {
            width,
            height,
            cells,
            edges: Vec::new(),
        };
        grid.edges = (0..grid.height as i32)
            .flat_map(|y| (0..grid.width as i32).map(move |x| (x, y)))
            .map(|(x, y)| grid.find_edge(x, y, threshold))
            .collect();
        grid
    }

    pub fn cell(&self, x: i32, y: i32) -> Option<&Cell> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        Some(&self.cells[y as usize * self.width + x as usize])
    }

    /// Walls outside the map count as solid.
    pub fn is_wall(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).is_none_or(Cell::is_wall)
    }

    /// Whether a floor tile is drawn here (`MapManager.IsFloor`): the cell
    /// is not transparent, or it sits between two that are not (so thin
    /// gaps in the image still get a floor). Fully transparent areas are
    /// outside the playable map.
    pub fn is_floor(&self, x: i32, y: i32) -> bool {
        !self.is_wall(x, y) && self.is_ground(x, y)
    }

    /// Part of the map's ground, walls included (`MapManager.IsFloor`): a
    /// drawn cell, or a gap between two; past the edge, the nearest cell.
    /// What is not ground is the void around the map.
    pub fn is_ground(&self, x: i32, y: i32) -> bool {
        let drawn = |x: i32, y: i32| {
            let x = x.clamp(0, self.width as i32 - 1);
            let y = y.clamp(0, self.height as i32 - 1);
            self.cells[y as usize * self.width + x as usize].color.a > 0
        };
        drawn(x, y) || (drawn(x - 1, y) && drawn(x + 1, y)) || (drawn(x, y - 1) && drawn(x, y + 1))
    }

    pub fn height_at(&self, x: i32, y: i32) -> f32 {
        self.cell(x, y).map_or(0.0, |c| c.height)
    }

    /// `MapManager.CreateDistanceField`, in cells: how far each cell is
    /// from the nearest wall cell (positive, at least 1) or from open
    /// ground (negative, at most -1), by a two-pass chamfer (1 straight,
    /// √2 diagonal). The outer ring is just ±1. Row-major, y down.
    pub fn chamfer_distance(&self) -> Vec<f32> {
        let (w, h) = (self.width, self.height);
        let mut d = vec![0.0f32; w * h];
        let free = |x: usize, y: usize| !self.cells[y * w + x].is_wall();
        let inner = |x: usize, y: usize| x > 0 && y > 0 && x + 1 < w && y + 1 < h;
        let s2 = std::f32::consts::SQRT_2;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                d[i] = if !inner(x, y) {
                    if free(x, y) { 1.0 } else { -1.0 }
                } else if free(x, y) {
                    let a = d[i - 1].min(d[i - w]) + 1.0;
                    let b = d[i - w - 1].min(d[i - w + 1]) + s2;
                    a.min(b).max(1.0)
                } else {
                    let a = d[i - 1].max(d[i - w]) - 1.0;
                    let b = d[i - w - 1].max(d[i - w + 1]) - s2;
                    a.max(b).min(-1.0)
                };
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                d[i] = if !inner(x, y) {
                    if free(x, y) { 1.0 } else { -1.0 }
                } else if free(x, y) {
                    let a = d[i + 1].min(d[i + w]) + 1.0;
                    let b = d[i + w - 1].min(d[i + w + 1]) + s2;
                    a.min(b).min(d[i]).max(1.0)
                } else {
                    let a = d[i + 1].max(d[i + w]) - 1.0;
                    let b = d[i + w - 1].max(d[i + w + 1]) - s2;
                    a.max(b).max(d[i]).min(-1.0)
                };
            }
        }
        d
    }

    /// The corner wedge in a cell, if any.
    pub fn edge(&self, x: i32, y: i32) -> Option<Edge> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.edges[y as usize * self.width + x as usize]
    }

    fn find_edge(&self, x: i32, y: i32, threshold: u8) -> Option<Edge> {
        let c = self.cell(x, y)?;
        if c.is_wall() || c.color.a == 0 {
            return None;
        }
        // Walls inside the map only: the map's border does not count here.
        let wall = |x: i32, y: i32| self.cell(x, y).is_some_and(Cell::is_wall);
        let (up, right, down, left) = (wall(x, y - 1), wall(x + 1, y), wall(x, y + 1), wall(x - 1, y));
        // (open corner, the wall cell whose colour it takes, the other wall)
        let (open, a, b) = match (up, right, down, left) {
            (false, true, true, false) => (0, (x + 1, y), (x, y + 1)),
            (false, false, true, true) => (1, (x, y + 1), (x - 1, y)),
            (true, false, false, true) => (2, (x - 1, y), (x, y - 1)),
            (true, true, false, false) => (3, (x, y - 1), (x + 1, y)),
            _ => return None,
        };
        // `GetEdgeColor`: the lower of the two walls.
        let (ca, cb) = (self.cell(a.0, a.1)?.color, self.cell(b.0, b.1)?.color);
        let color = if cb.a < ca.a { cb } else { ca };
        Some(Edge {
            open,
            height: wall_height(color.a, threshold),
            color,
        })
    }

    /// Wall cells merged into rectangles (x, y, w, h in cells), for colliders.
    pub fn wall_rects(&self) -> Vec<(usize, usize, usize, usize)> {
        let mut used = vec![false; self.width * self.height];
        let mut rects = Vec::new();
        let wall = |x: usize, y: usize| self.cells[y * self.width + x].is_wall();
        for y in 0..self.height {
            for x in 0..self.width {
                if used[y * self.width + x] || !wall(x, y) {
                    continue;
                }
                let mut w = 1;
                while x + w < self.width && wall(x + w, y) && !used[y * self.width + x + w] {
                    w += 1;
                }
                let mut h = 1;
                'grow: while y + h < self.height {
                    for xx in x..x + w {
                        if !wall(xx, y + h) || used[(y + h) * self.width + xx] {
                            break 'grow;
                        }
                    }
                    h += 1;
                }
                for yy in y..y + h {
                    for xx in x..x + w {
                        used[yy * self.width + xx] = true;
                    }
                }
                rects.push((x, y, w, h));
            }
        }
        rects
    }

    /// `MapManager.LineCollide`: walks from `a` towards `b` (world units, y
    /// down) a unit at a time along the longer axis, without `b` itself, and
    /// hits on a wall cell; the map's outer ring does not count.
    pub fn line_collides(&self, cell: f32, a: [f32; 2], b: [f32; 2]) -> bool {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let d = dx.abs().max(dy.abs());
        if d == 0.0 {
            return false;
        }
        let (sx, sy) = (dx / d, dy / d);
        let (w, h) = (self.width as i32, self.height as i32);
        let mut i = 0.0;
        while i < d {
            let (x, y) = (a[0] + sx * i, a[1] + sy * i);
            // C# truncates towards zero, so just outside the map reads as the ring.
            let (cx, cy) = ((x / cell) as i32, (y / cell) as i32);
            if cx > 0 && cy > 0 && cx < w - 2 && cy < h - 2 && self.is_wall(cx, cy) {
                return true;
            }
            i += 1.0;
        }
        false
    }

    /// Whether a ray from `from` (world units, y down) along `direction`
    /// (a unit vector, y down) meets a wall within `length`: a wall cell, a
    /// corner wedge, or the map's edge. Sampled every eighth of a cell, so a
    /// sliver of a corner thinner than that can slip through. Stands for
    /// `PhysicsManager.RayCollide` against the wall geometry alone
    /// (`AdjustWeaponMaxAngle` ignores every entity).
    pub fn ray_hits_wall(&self, cell: f32, from: [f32; 2], direction: [f32; 2], length: f32) -> bool {
        let step = cell / 8.0;
        let mut d = 0.0;
        while d <= length {
            let (x, y) = ((from[0] + direction[0] * d) / cell, (from[1] + direction[1] * d) / cell);
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            if self.is_wall(cx, cy) {
                return true;
            }
            if let Some(edge) = self.edge(cx, cy)
                && edge_solid(edge.open, x - cx as f32, y - cy as f32)
            {
                return true;
            }
            d += step;
        }
        false
    }

    /// True if the segment between two points (in cells) crosses a wall cell.
    pub fn line_hits_wall(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = (dx.abs().max(dy.abs()) * 4.0).ceil().max(1.0) as i32;
        (0..=steps).any(|i| {
            let t = i as f32 / steps as f32;
            let (x, y) = (from.0 + dx * t, from.1 + dy * t);
            self.is_wall(x.floor() as i32, y.floor() as i32)
        })
    }
}

/// Whether the point `(u, v)` of a cell (0..1, y down) is in the wedge of
/// an [`Edge`] whose `open` corner stays floor: the side of the diagonal
/// between the two corners beside it that the open one is not on.
fn edge_solid(open: usize, u: f32, v: f32) -> bool {
    let corner = |i: usize| [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)][i % 4];
    let (o, a, b) = (corner(open), corner(open + 1), corner(open + 3));
    let side = |p: (f32, f32)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    side((u, v)) * side(o) <= 0.0
}

/// `TilesMap.GetH`: heights come in steps of 32 pixels.
fn wall_height(alpha: u8, threshold: u8) -> f32 {
    if alpha < threshold {
        return 0.0;
    }
    let h = alpha as f32 - (threshold as f32 - 1.0);
    let px = ((h / WALL_STEP_PX) as i32 + 1) as f32 * WALL_STEP_PX + 1.0;
    px / crate::units::PX_PER_UNIT
}

fn split_char_map_rows(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let rows: Vec<&str> = normalized.split('\n').collect();
    let start = rows.iter().position(|r| !r.trim().is_empty());
    let end = rows.iter().rposition(|r| !r.trim().is_empty());
    match (start, end) {
        (Some(s), Some(e)) => rows[s..=e].iter().map(|r| r.to_string()).collect(),
        _ => Vec::new(),
    }
}

/// `*:#FFFF0000;.:#100000A0` — a key may be listed several times; one of
/// its colours is picked per cell.
fn parse_char_map_dictionary(text: &str) -> Vec<(char, Vec<XColor>)> {
    let mut dict: Vec<(char, Vec<XColor>)> = Vec::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let Some(colon) = entry.find(':') else {
            continue;
        };
        if colon == 0 || colon + 1 >= entry.len() {
            continue;
        }
        let Some(key) = entry[..colon].trim().chars().next() else {
            continue;
        };
        let Some(color) = XColor::parse_argb(&entry[colon + 1..]) else {
            continue;
        };
        match dict.iter_mut().find(|(k, _)| *k == key) {
            Some((_, colors)) => colors.push(color),
            None => dict.push((key, vec![color])),
        }
    }
    dict
}

/// A deterministic hash of the map name, so random choices repeat per map.
fn stable_string_hash(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

struct StableRng(u64);

impl StableRng {
    fn new(seed: u64) -> Self {
        StableRng(seed | 1)
    }
    fn next_below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
