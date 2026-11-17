//! World units and the conversion from map coordinates.
//!
//! One world unit is the C# game's "sim unit": 32 of its pixels
//! (`ConvertUnits`), so the car's forces, speeds and masses keep their C#
//! values. A map cell is `MapData.Scale / 32` units (2 for a 64-pixel map).
//!
//! Map coordinates have y pointing down; the world has y up, as in Bevy.
//! Convert only at this boundary.
//!
//! Map directions (`DirectionDeg`) turn counter-clockwise on screen: 0° is
//! right and 90° is up. The C# game used `-DirectionDeg` as its angle in
//! y-down coordinates everywhere (`MapManager`, `ActionRunner`), which is
//! `+DirectionDeg` with y up.
//!
//! # Reading C# coordinates
//!
//! The C# game's axes are not ours. Two of the three point the other way,
//! so any position, angle or rotation copied from C# without translating
//! it comes out flipped:
//!
//! | | x | y | z |
//! |---|---|---|---|
//! | C# (`Vector2`/`Vector3`, `DepthConst`) | right | **down** | depth: **away** from the camera (floor > 0, sprites < 0) |
//! | here (Bevy) | right | up | **towards** the camera (walls > 0, floor < 0) |
//!
//! Ours is C#'s turned half a turn about x: `(x, y, z) → (x, −y, −z)`. It
//! is a rotation, not a mirror, so handedness and triangle winding keep,
//! but every sign that involves y or z flips:
//!
//! - **Points and vectors** (positions, speeds, distance field gradients,
//!   `v.GetAngle()` inputs): negate y (and z). [`CellSize::to_world`],
//!   [`from_cs`].
//! - **Angles in the plane** (`Direction`, `GetAngle()`, sprite rotation):
//!   a turn about z, so negated.
//! - **3D turns** (`Matrix.CreateRotationX/Y/Z`, `Primitive.Rotate`): about
//!   x keeps its sign, about y and z negate. [`from_cs_rotation`].
//! - **Matrix order**: XNA multiplies row vectors, so `A * B` applies `A`
//!   first; Bevy's `B * A` applies `A` first.
//! - **Depth order** (`DepthConst`, `SpriteSortMode.BackToFront`): larger
//!   is further back, drawn first. Only `render/layers.rs` maps it.
//!
//! Formulas symmetric in a sign (`|sin|`, random ± jitter) survive either
//! way, which is how such a slip can hide; check against the C# game.

use bevy::math::{Quat, Vec2, Vec3};

/// The C# game's pixels per world unit.
pub const PX_PER_UNIT: f32 = 32.0;

/// Size of one map cell in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellSize(pub f32);

impl CellSize {
    pub fn from_map_scale(scale_px: f32) -> Self {
        CellSize(scale_px / PX_PER_UNIT)
    }

    /// A point in map cells (y down) to the world (y up).
    pub fn to_world(self, x: f32, y: f32) -> Vec2 {
        Vec2::new(x * self.0, -y * self.0)
    }

    /// A world point back to map cells (y down).
    pub fn to_cells(self, p: Vec2) -> (f32, f32) {
        (p.x / self.0, -p.y / self.0)
    }
}

/// A map direction in degrees (0° right, 90° up) to a world angle in radians.
pub fn map_degrees_to_world(deg: f32) -> f32 {
    deg.to_radians()
}

/// A length in the C# game's pixels to world units.
pub fn px(v: f32) -> f32 {
    v / PX_PER_UNIT
}

/// A C# point or vector (y down, z away from the camera) in ours.
pub fn from_cs(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.y, -v.z)
}

/// The turn C# built as `CreateRotationX(x) * CreateRotationY(y) *
/// CreateRotationZ(z)` (x first: XNA's row vectors), in ours.
pub fn from_cs_rotation(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_rotation_z(-z) * Quat::from_rotation_y(-y) * Quat::from_rotation_x(x)
}
