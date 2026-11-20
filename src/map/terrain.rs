//! What the AI asks of the floor plan: how much room there is around a
//! point, whether a wall is in the way, and picks of random places.
//!
//! C# origin: `MapManager` (2013–2016): `DistanceField`,
//! `GetDistanceFieldValue`, `GetGradientDistancefieldValue`, `IsPointFree`,
//! `GetNearestFreePoint(pos, count)`, `LineCollide`, `FreePoints`
//! (`GetFreePoints`), `GetRandomFreePoint`, `GetRandomFreeVisiblePoint`,
//! `GetRandomFreeNonVisiblePoint`; `RandomHelper.NextVector2Fake`,
//! `IEnumerableHelper.Random(predicate, estimatedCount)`.
//!
//! The field is the one [`MapGrid::chamfer_distance`] makes (C#'s
//! `CreateDistanceField`), in world units: positive and at least a cell
//! where the floor is open, negative in a wall. C# kept its points with y
//! down; here they are world points, y up, and only the sign of y differs
//! where a rule is not symmetric (a sign is flipped once, in [`Terrain::at`]).
//!
//! Every pick takes its random numbers from a `rand` function (0 up to 1)
//! the caller gives, so each car draws from its own stream, and the picks
//! are tested apart from the app (`tests/terrain.rs`).
//!
//! Not ported: fake walls (`_fakeWalls`, which `FreePoint` counts as floor:
//! the port has none yet); `RandomHelper`'s *fake* random numbers (a table of
//! 256 numbers drawn in turn, shared by every car, used so the picks would be
//! cheap: each car's own stream is used, the same distribution); the other
//! `MapManager` pickers (`GetRandomFreePoint()`, `GetRandomPoint`,
//! `GetNearestFreePoint(pos)` and `(pos, count, rand)`,
//! `GetFarestFreePoint`: no tree of the port calls them yet).

use bevy::math::Vec2;

use super::grid::MapGrid;
use crate::weapon::model::closer_by_chebyshev;

/// `MapManager.FreeThreshold`: `ToSimUnits(32)`, how much room a point needs
/// to count as free, world units.
pub const FREE_THRESHOLD: f32 = 1.0;
/// `GetFreePoints(CellSize * 6)`: free points are this many cells apart.
const FREE_POINT_SPACING_CELLS: f32 = 6.0;
/// `GetRandomFreeVisiblePoint` and `GetRandomFreeNonVisiblePoint` try this
/// many random spots before they take a free point.
const TRIES: u32 = 10;
/// `GetRandomFreePoint`: `Random(predicate, 10)` takes one of the first ten
/// matches.
const ESTIMATED_COUNT: f32 = 10.0;

/// The floor plan, for the picks and questions of the AI.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// World units a cell is.
    cell: f32,
    width: usize,
    height: usize,
    /// `DistanceField`, world units, row-major, map rows down.
    distance: Vec<f32>,
    /// `FreePoints`, world points: the middles of the roomiest cells, none
    /// within six cells of another, roomiest first.
    free_points: Vec<Vec2>,
}

impl Terrain {
    /// The floor plan of a map (`MapManager.CreateDistanceField` and
    /// `GetFreePoints`); `cell` is a map cell in world units.
    pub fn from_grid(grid: &MapGrid, cell: f32) -> Terrain {
        let distance: Vec<f32> = grid.chamfer_distance().iter().map(|d| d * cell).collect();
        let (width, height) = (grid.width, grid.height);
        // `GetFreePoints`: the cells drawn (alpha not 0), from the roomiest;
        // C# ordered its column-major field by `OrderByDescending`, which is
        // stable, so equally roomy cells go column by column.
        let mut cells: Vec<(usize, usize, f32)> = (0..width)
            .flat_map(|x| (0..height).map(move |y| (x, y)))
            .filter(|&(x, y)| grid.cell(x as i32, y as i32).is_some_and(|c| c.color.a != 0))
            .map(|(x, y)| (x, y, distance[y * width + x]))
            .collect();
        cells.sort_by(|a, b| b.2.total_cmp(&a.2));
        let spacing = cell * FREE_POINT_SPACING_CELLS;
        let mut free_points: Vec<Vec2> = Vec::new();
        for (x, y, value) in cells {
            let at = Vec2::new((x as f32 + 0.5) * cell, -(y as f32 + 0.5) * cell);
            if value > cell
                && free_points.iter().all(|p| p.distance_squared(at) > spacing * spacing)
                && !grid.is_wall(x as i32, y as i32)
            {
                free_points.push(at);
            }
        }
        Terrain { cell, width, height, distance, free_points }
    }

    /// `FreePoints`.
    pub fn free_points(&self) -> &[Vec2] {
        &self.free_points
    }

    /// `GetDistanceFieldValue`: how far the nearest wall is (negative inside
    /// one); 0 at and past the map's outer ring.
    pub fn distance_at(&self, p: Vec2) -> f32 {
        let (x, y) = self.at(p);
        if x <= 0 || y <= 0 || x >= self.width as i32 - 2 || y >= self.height as i32 - 2 {
            return 0.0;
        }
        self.distance[y as usize * self.width + x as usize]
    }

    /// The cell of a world point, as C# cut it (`(int)(x / CellSize)`, map
    /// rows down; a cut towards zero).
    fn at(&self, p: Vec2) -> (i32, i32) {
        ((p.x / self.cell) as i32, (-p.y / self.cell) as i32)
    }

    /// `IsPointFree`: room enough for a car.
    pub fn is_point_free(&self, p: Vec2) -> bool {
        self.distance_at(p) > FREE_THRESHOLD
    }

    /// `GetGradientDistancefieldValue`: which way the room grows, from the
    /// cells on either side of `p`'s (their difference over two cells, so
    /// about a unit vector in the open and away from the nearest wall); zero
    /// at and past the map's outer ring. C# took the cells below as `y + 1`;
    /// the world's y is up, so the y difference is turned over here.
    pub fn gradient_at(&self, p: Vec2) -> Vec2 {
        let (x, y) = self.at(p);
        if x <= 0 || y <= 0 || x >= self.width as i32 - 2 || y >= self.height as i32 - 2 {
            return Vec2::ZERO;
        }
        let d = |x: i32, y: i32| self.distance[y as usize * self.width + x as usize];
        let along_rows = (d(x + 1, y) - d(x - 1, y)) / (self.cell * 2.0);
        let down = (d(x, y + 1) - d(x, y - 1)) / (self.cell * 2.0);
        Vec2::new(along_rows, -down)
    }

    /// `GetNearestFreePoint(pos, count)`: of the first `count` free points
    /// (the roomiest), the nearest to `pos`; the origin ("no point") when
    /// the map has none.
    pub fn nearest_free_point(&self, pos: Vec2, count: usize) -> Vec2 {
        // `OrderBy` is stable: the first of equals.
        let mut best: Option<(f32, Vec2)> = None;
        for &p in self.free_points.iter().take(count) {
            let d = p.distance_squared(pos);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, p));
            }
        }
        best.map_or(Vec2::ZERO, |b| b.1)
    }

    /// `MapManager.LineCollide`: walks from `a` towards `b` a unit at a time
    /// along the longer axis, `b` itself not asked, and hits where the field
    /// is negative. The same as [`MapGrid::line_collides`].
    pub fn line_collides(&self, a: Vec2, b: Vec2) -> bool {
        if a == b {
            return false;
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let d = dx.abs().max(dy.abs());
        let (sx, sy) = (dx / d, dy / d);
        let mut i = 0.0;
        while i < d {
            if self.distance_at(Vec2::new(a.x + sx * i, a.y + sy * i)) < 0.0 {
                return true;
            }
            i += 1.0;
        }
        false
    }

    /// `GetRandomFreePoint(pos, maxDistance)`: of the free points within
    /// `max_distance` of `pos` on both axes, one of the first ten (the
    /// last of them when fewer); the origin when there is none, which is how
    /// C# says "no point".
    pub fn random_free_point(&self, pos: Vec2, max_distance: f32, rand: &mut dyn FnMut() -> f32) -> Vec2 {
        // `RandomHelper.Instance.Next(estimatedCount)`.
        let index = ((rand() * ESTIMATED_COUNT) as usize).min(ESTIMATED_COUNT as usize - 1);
        let mut result = Vec2::ZERO;
        let mut count = 0;
        for &p in &self.free_points {
            if closer_by_chebyshev(p, pos, max_distance) {
                count += 1;
                result = p;
                if count == index {
                    break;
                }
            }
        }
        result
    }

    /// `GetRandomFreeVisiblePoint(pos, minDistance, maxDistance)`: a spot a
    /// random way off (between `min_distance` and `max_distance`, any
    /// direction) that is either inside the room's square around `pos`
    /// (`|dx|` and `|dy|` within the distance field there) or in line of
    /// sight of it; after ten tries, a free point near. The spot itself is
    /// not asked to be free (`LineCollide` leaves its end out).
    pub fn random_free_visible_point(
        &self,
        pos: Vec2,
        min_distance: f32,
        max_distance: f32,
        rand: &mut dyn FnMut() -> f32,
    ) -> Vec2 {
        let room = self.distance_at(pos);
        for _ in 0..TRIES {
            let delta = random_vector(min_distance, max_distance, rand);
            let spot = pos + delta;
            if (delta.x.abs() <= room && delta.y.abs() <= room) || !self.line_collides(pos, spot) {
                return spot;
            }
        }
        self.random_free_point(pos, max_distance, rand)
    }

    /// `GetRandomFreeNonVisiblePoint(pos, from, maxDistance)`: a free spot
    /// within `max_distance` of `pos` (up to that far away from `from`'s side,
    /// and across by as much to either side) that is out of `max_distance`
    /// of `from` or hidden from it by a wall; after ten tries, a free point
    /// near.
    pub fn random_free_non_visible_point(
        &self,
        pos: Vec2,
        from: Vec2,
        max_distance: f32,
        rand: &mut dyn FnMut() -> f32,
    ) -> Vec2 {
        // Away from `from`, and across (`RotateLeft90`, a turn to the right
        // on a y-up plane).
        let away = (pos - from).normalize_or_zero();
        let across = Vec2::new(away.y, -away.x);
        for _ in 0..TRIES {
            let along = rand() * max_distance;
            let side = (-1.0 + 2.0 * rand()) * max_distance;
            let spot = pos + away * along + across * side;
            if self.is_point_free(spot)
                && (!closer_by_chebyshev(spot, from, max_distance) || self.line_collides(from, spot))
            {
                return spot;
            }
        }
        self.random_free_point(pos, max_distance, rand)
    }
}

/// `RandomHelper.NextVector2Fake(min, max)`: a direction picked in the square
/// of side 2 and made a unit, its length between `min` and `max`.
fn random_vector(min: f32, max: f32, rand: &mut dyn FnMut() -> f32) -> Vec2 {
    let direction = Vec2::new(rand() * 2.0 - 1.0, rand() * 2.0 - 1.0).normalize_or_zero();
    direction * (min + (max - min) * rand())
}
