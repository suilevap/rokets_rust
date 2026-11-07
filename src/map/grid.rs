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
    /// Distance from each cell's centre to the nearest wall cell, in cells.
    distance: Vec<f32>,
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
            distance: Vec::new(),
        };
        grid.distance = grid.distance_field();
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
        if self.is_wall(x, y) {
            return false;
        }
        let opaque = |x: i32, y: i32| {
            let x = x.clamp(0, self.width as i32 - 1);
            let y = y.clamp(0, self.height as i32 - 1);
            self.cells[y as usize * self.width + x as usize].color.a > 0
        };
        opaque(x, y) || (opaque(x - 1, y) && opaque(x + 1, y)) || (opaque(x, y - 1) && opaque(x, y + 1))
    }

    pub fn height_at(&self, x: i32, y: i32) -> f32 {
        self.cell(x, y).map_or(0.0, |c| c.height)
    }

    /// Distance from the centre of cell (x, y) to the nearest wall, in cells.
    pub fn distance_to_wall(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0.0;
        }
        self.distance[y as usize * self.width + x as usize]
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

    /// Brute-force distance transform; maps are at most a few hundred cells a side.
    fn distance_field(&self) -> Vec<f32> {
        let walls: Vec<(i32, i32)> = (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y)))
            .filter(|&(x, y)| self.cells[y * self.width + x].is_wall())
            .map(|(x, y)| (x as i32, y as i32))
            .collect();
        let mut out = vec![0.0; self.width * self.height];
        const LIMIT: i32 = 8;
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                if self.is_wall(x, y) {
                    continue;
                }
                // Search a window first; most cells have a wall nearby.
                let mut best = f32::MAX;
                for wy in (y - LIMIT).max(-1)..=(y + LIMIT).min(self.height as i32) {
                    for wx in (x - LIMIT).max(-1)..=(x + LIMIT).min(self.width as i32) {
                        if self.is_wall(wx, wy) {
                            best = best.min(cell_distance(x, y, wx, wy));
                        }
                    }
                }
                if best == f32::MAX {
                    best = walls
                        .iter()
                        .map(|&(wx, wy)| cell_distance(x, y, wx, wy))
                        .fold(f32::MAX, f32::min)
                        .min(LIMIT as f32 * 4.0);
                }
                out[y as usize * self.width + x as usize] = best;
            }
        }
        out
    }
}

/// Distance from the centre of cell a to the nearest point of cell b.
fn cell_distance(ax: i32, ay: i32, bx: i32, by: i32) -> f32 {
    let dx = ((ax - bx).abs() as f32 - 0.5).max(0.0);
    let dy = ((ay - by).abs() as f32 - 0.5).max(0.0);
    (dx * dx + dy * dy).sqrt()
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
mod tests {
    use super::*;

    fn char_map(dict: &str, rows: &str) -> MapData {
        let xml = format!(
            r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
            <ImageWallTreshold>96</ImageWallTreshold>
            <CharMapDictionary>{dict}</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
        );
        MapData::parse(&xml).unwrap()
    }

    #[test]
    fn char_map_builds_walls_and_floor() {
        let map = char_map("*:#FFFF0000;.:#100000A0", "\n****\n*..*\n****\n");
        let grid = MapGrid::build(&map, None);
        assert_eq!((grid.width, grid.height), (4, 3));
        assert!(grid.is_wall(0, 0));
        assert!(!grid.is_wall(1, 1));
        assert!(grid.is_wall(-1, 1), "outside the map is solid");
        assert_eq!(grid.distance_to_wall(1, 1), 0.5);
        assert!(grid.is_floor(1, 1));
        assert!(!grid.is_floor(0, 0));
        let covered: usize = grid.wall_rects().iter().map(|r| r.2 * r.3).sum();
        assert_eq!(covered, 10);
    }

    #[test]
    fn heights_come_in_steps() {
        // alpha 255 with threshold 96: 160 above → 6 steps → 193 px.
        assert_eq!(wall_height(255, 96), 193.0 / crate::units::PX_PER_UNIT);
        assert_eq!(wall_height(95, 96), 0.0);
        assert_eq!(wall_height(96, 96), 33.0 / crate::units::PX_PER_UNIT);
    }
}
