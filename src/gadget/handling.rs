//! How a hand on the magnet's button grabs and drops: the rules both the
//! enemies' `GrabByChainGadget` / `DropByChainGadget` (`ai::nodes::cargo`)
//! and the Smart scheme's assist (`assist::magnet`) follow, as plain
//! functions of numbers, so the two cannot drift apart.
//!
//! C# origin: `BTActions/GrabByChainGadget.cs` and `DropByChainGadget.cs`,
//! whose constants and button conditions these are.

use bevy::math::Vec2;

use super::REACH;

/// `ChainGadget.ActiveDistance - 1`: how near the cargo or the drop point
/// must be to start.
pub const NEAR: f32 = REACH - 1.0;
/// `_durationMin`: the button is held this long at least, s.
pub const HOLD_MIN: f32 = 1.0;
/// `_durationMax`: the longest it is held, s.
pub const HOLD_MAX: f32 = 5.0;

/// `VectorExtension.CloserThan`.
pub fn closer_than(a: Vec2, b: Vec2, distance: f32) -> bool {
    a.distance_squared(b) < distance * distance
}

/// Whether the button is still held (true) or let go (false) while
/// grabbing, `held` seconds into it: held while the magnet has nothing it
/// would grab (`found` false) or the first second is not over.
pub fn grab_holds(found: bool, held: f32) -> bool {
    !found || held < HOLD_MIN
}

/// The same while dropping: held while the cargo is not yet over the point
/// (`on_target` false) or the first second is not over, but at most
/// [`HOLD_MAX`].
pub fn drop_holds(on_target: bool, held: f32) -> bool {
    (!on_target || held < HOLD_MIN) && held < HOLD_MAX
}
