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
//! Order and height are separate. The order comes from the C#
//! `DepthConst` table (which is what the sprite batch sorted by). The
//! height is only how far above the ground plane a sprite lies: every
//! sprite layer lies within a hundredth of a unit of it, so nothing is
//! lifted towards the camera and drawn larger or shifted in perspective;
//! the small steps only keep them above the floor and below the walls in
//! the depth test.

/// Highest point any floor tile may reach.
pub const FLOOR_TOP: f32 = -0.15;

/// Height of the light layer (`DepthConst.Light`): above every floor
/// tile, below every sprite and wall, so it darkens the floor and the
/// outland under it and nothing else.
pub const LIGHT_Z: f32 = FLOOR_TOP / 2.0;

/// The light layer's `depth_bias`: sorted before every sprite layer, so it
/// never multiplies a sprite drawn before it.
pub const LIGHT_DEPTH_BIAS: f32 = -BIAS_PER_RANK;

/// The additive light's `depth_bias`: sorted after every sprite layer, as
/// C# drew it last (`IgnoreDepthWorldAdditive`), over everything.
pub const ADDITIVE_DEPTH_BIAS: f32 = 100.0 * BIAS_PER_RANK;
/// Fullscreen lens/vignette, after world light and before the HUD.
pub const SCREEN_EFFECT_DEPTH_BIAS: f32 = ADDITIVE_DEPTH_BIAS + BIAS_PER_RANK;

/// Height between consecutive layers: tiny, only for the depth test.
const Z_STEP: f32 = 0.002;

/// `depth_bias` between consecutive ranks, in sort-distance units (world
/// units); larger than any camera-tilt difference across the screen.
const BIAS_PER_RANK: f32 = 100.0;

/// World sprite layers, bottom to top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// `Tile` entities on the floor: finish markings, signs, grass
    /// (`DepthConst.Tile`).
    GroundDecor,
    /// Accelerator pads (`DepthConst.Accelerator`, just under the ground).
    Accelerator,
    /// Contact and directional shadows, above floor decoration, under bodies.
    Shadow,
    /// Boxes (`DepthConst.CustomEntity`, the car's height).
    Prop,
    /// The car's chassis (`DepthConst.CarBase`).
    CarBase,
    /// Front wheels (`CarBase + WheelDelta`).
    CarWheel,
    /// The cabin (`CarBase + BodyDelta`).
    CarBody,
    /// Gears (`DepthConst.Coin`): drawn over the cars.
    Coin,
}

impl Layer {
    pub const ALL: [Layer; 8] = [
        Layer::GroundDecor,
        Layer::Accelerator,
        Layer::Shadow,
        Layer::Prop,
        Layer::CarBase,
        Layer::CarWheel,
        Layer::CarBody,
        Layer::Coin,
    ];

    /// Height above the ground plane, world units: all near zero (see the
    /// module doc); the order comes from [`Layer::depth_bias`].
    pub fn z(self) -> f32 {
        self as u8 as f32 * Z_STEP
    }

    /// The material `depth_bias` that puts this layer above lower ones.
    pub fn depth_bias(self) -> f32 {
        self as u8 as f32 * BIAS_PER_RANK
    }
}
