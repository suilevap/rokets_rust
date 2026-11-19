//! Cutting the free cells of a map into rectangles, for the navigation graph.
//!
//! C# origin: `DataMap<T>.ExtractAllRectangles`, `Flood`, `CheckRect` and
//! `FillRect` (`Helper/DataMap.cs`), and XNA's `Rectangle` (the few methods
//! the navigation used: `Intersects`, `Intersect`, `Center`, `Inflate`).
//!
//! `ExtractAllRectangles` visits the usable cells from the highest priority
//! down (ties in row order, as LINQ's stable `OrderByDescending` left them)
//! and from each cell not yet taken grows a rectangle: `Flood` tries to widen
//! it a column to the right, a row down, a column to the left and a row up,
//! in that order, until none fits. The priority is the distance field, so the
//! rectangles start in the middle of the open floor and the biggest come
//! first.
//!
//! `DataMap` kept one extra column and row (`(int)(size / cell) + 1`) holding
//! the default value; the navigation's predicate (`MapImage.GetPixelSafe(x,
//! y).A != 0`) is false there, so they never take part and are left out here:
//! anything outside the grid is not free.
//!
//! Not ported: the other `DataMap` members (`GetRect`, `CheckRegion`,
//! `DrawStatic`, …), which the navigation does not use.

/// XNA's `Rectangle`: `x`, `y` the top-left corner, in whatever units the
/// caller counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    /// `Right`.
    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    /// `Bottom`.
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// `Center`: the integer middle (half a size rounds down).
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// `Intersects`: they share some area (touching edges do not count).
    pub fn intersects(&self, other: &IRect) -> bool {
        other.x < self.right() && self.x < other.right() && other.y < self.bottom() && self.y < other.bottom()
    }

    /// `Rectangle.Intersect`: the shared area, or an empty rectangle at the
    /// origin when there is none.
    pub fn intersection(&self, other: &IRect) -> IRect {
        if !self.intersects(other) {
            return IRect::default();
        }
        let right = self.right().min(other.right());
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let bottom = self.bottom().min(other.bottom());
        IRect::new(left, top, right - left, bottom - top)
    }

    /// `Inflate(dx, dy)`: grows by `dx` on each side horizontally and `dy`
    /// vertically.
    pub fn inflate(&mut self, dx: i32, dy: i32) {
        self.x -= dx;
        self.y -= dy;
        self.w += dx * 2;
        self.h += dy * 2;
    }
}

/// `ExtractAllRectangles`: rectangles covering the cells of a `width` ×
/// `height` grid for which `usable(x, y, priority)` holds, once each, the
/// first from the cell with the highest `priority` (row-major `priority`,
/// `width` cells to a row).
pub fn extract_all_rectangles(
    width: usize,
    height: usize,
    priority: &[f32],
    usable: impl Fn(i32, i32, f32) -> bool,
) -> Vec<IRect> {
    let mut free = vec![false; width * height];
    let mut order: Vec<usize> = Vec::new();
    for (i, &p) in priority.iter().enumerate().take(width * height) {
        if usable((i % width) as i32, (i / width) as i32, p) {
            free[i] = true;
            order.push(i);
        }
    }
    // `OrderByDescending` is stable: equal priorities stay in cell order.
    order.sort_by(|&a, &b| priority[b].total_cmp(&priority[a]));
    let mut grid = Free { cells: &mut free, width: width as i32, height: height as i32 };
    let mut out = Vec::new();
    for i in order {
        let (x, y) = ((i % width) as i32, (i / width) as i32);
        if grid.get(x, y) {
            let rect = grid.flood(x, y);
            grid.fill(rect);
            out.push(rect);
        }
    }
    out
}

/// The cells still free to be taken (`usefull`).
struct Free<'a> {
    cells: &'a mut [bool],
    width: i32,
    height: i32,
}

impl Free<'_> {
    fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height && self.cells[(y * self.width + x) as usize]
    }

    /// `Flood`: widens a rectangle from one cell, a step at a time, in the
    /// order right, down, left, up, while the next row or column is all free.
    fn flood(&self, i: i32, j: i32) -> IRect {
        let mut rect = IRect::new(i, j, 0, 0);
        loop {
            let mut grew = false;
            if self.check(rect, 1, 0) {
                rect.w += 1;
                grew = true;
            }
            if self.check(rect, 0, 1) {
                rect.h += 1;
                grew = true;
            }
            if self.check(rect, -1, 0) {
                rect.x -= 1;
                rect.w += 1;
                grew = true;
            }
            if self.check(rect, 0, -1) {
                rect.y -= 1;
                rect.h += 1;
                grew = true;
            }
            if !grew {
                return rect;
            }
        }
    }

    /// `CheckRect`: whether every cell of the rectangle and of the row or
    /// column one step `(dx, dy)` beyond it is free (an empty rectangle has no
    /// cells, so a step to the right or down from it always passes; the cell
    /// the rectangle starts from is free anyway).
    fn check(&self, rect: IRect, dx: i32, dy: i32) -> bool {
        let (dx0, dy0) = (dx.min(0), dy.min(0));
        let (dx1, dy1) = (dx.max(0), dy.max(0));
        for y in rect.y + dy0..rect.bottom() + dy1 {
            for x in rect.x + dx0..rect.right() + dx1 {
                if !self.get(x, y) {
                    return false;
                }
            }
        }
        true
    }

    /// `FillRect(rect, false)`.
    fn fill(&mut self, rect: IRect) {
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                if x >= 0 && y >= 0 && x < self.width && y < self.height {
                    self.cells[(y * self.width + x) as usize] = false;
                }
            }
        }
    }
}
