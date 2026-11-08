//! Draw order of the world, in one place.
//!
//! How the C# game did it (`Renderer.Draw`, `DepthConst`): the map's
//! geometry (floor tiles, walls) was drawn first with depth writes; then
//! all world sprites were drawn in one `SpriteBatch` sorted
//! `BackToFront` by their `DepthConst` value, depth-tested against the
//! geometry but not writing depth. So sprites never fought each other:
//! their order was the table below, whatever the camera did.
//!
//! Here the same rules hold:
//! - **Geometry** (floor, walls) is opaque and writes depth. Floor tiles
//!   stay below [`FLOOR_TOP`], under every sprite layer.
//! - **Sprites** are alpha-blended quads at their layer's height, so walls
//!   still hide them. Between layers the order is fixed by a large
//!   `depth_bias` per rank: Bevy sorts transparent meshes by camera
//!   distance plus `depth_bias`, and the bias outweighs any tilt of the
//!   camera. (Its effect on the depth test against walls is ~1e-5 units.)
//!   Within one layer, nearer meshes draw last.
//!
//! Heights are the C# `DepthConst` values with the sign flipped (there z
//! grew away from the camera; here it grows towards it), in world units.

/// Highest point any floor tile may reach.
pub const FLOOR_TOP: f32 = -0.15;

/// `depth_bias` between consecutive ranks, in sort-distance units (world
/// units); larger than any camera-tilt difference across the screen.
const BIAS_PER_RANK: f32 = 100.0;

/// World sprite layers, bottom to top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// `Tile` entities on the floor: finish markings, signs, grass
    /// (`DepthConst.Tile` = 0.1 below the ground plane).
    GroundDecor,
    /// The car's chassis (`DepthConst.CarBase`).
    CarBase,
    /// Front wheels (`CarBase + WheelDelta`).
    CarWheel,
    /// The cabin (`CarBase + BodyDelta`).
    CarBody,
    /// Gears (`DepthConst.Coin`): above the cars.
    Coin,
}

impl Layer {
    pub const ALL: [Layer; 5] = [
        Layer::GroundDecor,
        Layer::CarBase,
        Layer::CarWheel,
        Layer::CarBody,
        Layer::Coin,
    ];

    /// Height above the ground plane, world units.
    pub fn z(self) -> f32 {
        // `toFixLightIssuesNewarWorlScale` in DepthConst.
        const S: f32 = 1.0 / 16.0;
        const CAR_BASE: f32 = 16.0 / 32.0 * S;
        match self {
            Layer::GroundDecor => -0.1,
            Layer::CarBase => CAR_BASE,
            Layer::CarWheel => CAR_BASE + 0.05 / 32.0 * S,
            Layer::CarBody => CAR_BASE + 1.0 / 32.0 * S,
            Layer::Coin => 16.0 / 32.0 - 0.1 * S,
        }
    }

    /// The material `depth_bias` that puts this layer above lower ones.
    pub fn depth_bias(self) -> f32 {
        self as u8 as f32 * BIAS_PER_RANK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_are_ordered_and_above_the_floor() {
        for pair in Layer::ALL.windows(2) {
            let (lower, upper) = (pair[0], pair[1]);
            assert!(lower < upper);
            assert!(lower.z() < upper.z(), "{lower:?} must sit below {upper:?}");
            assert!(lower.depth_bias() < upper.depth_bias());
        }
        assert!(Layer::ALL.iter().all(|l| l.z() > FLOOR_TOP));
    }
}
