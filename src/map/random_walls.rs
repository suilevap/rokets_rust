//! `WallWithProbability`: parts of the map that are painted over, or not,
//! when the map is made.
//!
//! C# origin: `MapImageModifierManager.Modify` (September 2014, "Add action
//! to add walls with probability"; the map generator's `ObjectsGenerator`
//! and the `Part_*` pieces of random maps use it for doors and gaps, Escort2
//! has one). `MapManager.LoadMapFromPixelTextureAction` called it on the
//! map's image right after reading it and before anything is built from it
//! (the fake-wall mask, the wall colliders, the light, the navigation), so
//! here it is applied to the pixels of [`MapGrid::build`](super::grid::MapGrid::build)
//! before the cells are made, and everything built from the grid sees the
//! result. The actions themselves are never run (`RunOnMapStart` is
//! false and C# registered no handler for the type): they are data.
//!
//! What it does, as C# did:
//!
//! - The actions are taken in the map's order and grouped by `WallGroup`,
//!   the groups in the order their first member appears.
//! - A group with no name (null, empty or only spaces) is not a group: each
//!   of its actions draws its own number and, when it is below its
//!   `Probability`, paints its `Area` with its `Color` (`Image.SetRectangle`:
//!   every pixel of the rectangle is *replaced*, alpha included, so an
//!   alpha of 0 digs a hole where there was a wall and an opaque colour
//!   builds one).
//! - A named group draws one number for all, against the `Probability` of its
//!   first member (C#: "get first? or min/max/avg?"), and paints all
//!   its areas in order, or none.
//! - A number is in `[0, 1)`, so a `Probability` of 1 or more always paints
//!   and 0 or less never does. The draw is made even then, so the numbers
//!   the later actions get do not depend on the probabilities before them.
//!
//! Randomness. C# used `new Random()`, seeded by the clock, so the
//! walls were different on every start. Here the draws come from a small
//! generator seeded from the map's name ([`seed_of`]): the same map has the
//! same walls every time it is loaded, and a test can say which. Escort2's
//! only action has a `Probability` of 1, so it does not matter there.
//!
//! Not ported: the `Rectangle` is not trusted to lie within the image. C#
//! wrote `_data[x + y * width]` for every pixel of it, so a rectangle
//! over the right edge painted the start of the next row, and one below
//! or above the image threw. Here the part outside the image is left out
//! (no map has one). Also not ported: the actions of the map's
//! `Parts` (the generator's pieces are copied into a generated map with
//! their `Area` moved, `ApplySpecificMergeOffset`; the port builds no
//! generated maps).

use crate::content::actions::MapActions;
use crate::content::actions::defs::WallWithProbability;
use crate::content::xna::XColor;

/// Paints the map's `WallWithProbability` areas over `pixels` (row-major,
/// `width` by `height`, y down), drawing with `rng`. Returns how many
/// areas it painted.
pub fn apply(actions: &MapActions, pixels: &mut [XColor], width: usize, height: usize, rng: &mut UnitRng) -> usize {
    let walls: Vec<WallWithProbability> = actions
        .0
        .iter()
        .filter(|a| a.kind == "WallWithProbability")
        .filter_map(|a| WallWithProbability::read_fields(&a.node).ok())
        .collect();

    // `GroupBy(x => x.WallGroup)`: keys in order of first appearance.
    let mut groups: Vec<(Option<&str>, Vec<&WallWithProbability>)> = Vec::new();
    for w in &walls {
        let key = w.wall_group.as_deref();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(w),
            None => groups.push((key, vec![w])),
        }
    }

    let mut painted = 0;
    for (key, members) in groups {
        let named = key.is_some_and(|k| !k.trim().is_empty());
        if named {
            if rng.next() < members[0].probability as f64 {
                for w in members {
                    paint(pixels, width, height, w);
                    painted += 1;
                }
            }
        } else {
            for w in members {
                if rng.next() < w.probability as f64 {
                    paint(pixels, width, height, w);
                    painted += 1;
                }
            }
        }
    }
    painted
}

/// `Image.SetRectangle`, clipped to the image.
fn paint(pixels: &mut [XColor], width: usize, height: usize, w: &WallWithProbability) {
    let r = w.area;
    let (x0, x1) = (r.x.max(0) as usize, (r.x + r.w).clamp(0, width as i32) as usize);
    let (y0, y1) = (r.y.max(0) as usize, (r.y + r.h).clamp(0, height as i32) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            pixels[y * width + x] = w.color;
        }
    }
}

/// The seed for a map: a hash of its name, so the same map gets the same
/// walls (and not the same draws as the other things seeded from its name).
pub fn seed_of(map_name: &str) -> u64 {
    map_name
        .bytes()
        .fold(0x84222325cbf29ce4u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// A small generator of numbers in `[0, 1)` (`Random.NextDouble`), the same
/// for the same seed.
#[derive(Clone, Debug)]
pub struct UnitRng(u64);

impl UnitRng {
    pub fn new(seed: u64) -> Self {
        UnitRng(seed | 1)
    }

    /// The next number, `0 <= n < 1`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        // xorshift64*; its top 53 bits make the mantissa.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
}
