//! The effects, as `ParticleSystemManager` defined them, converted to our
//! units: speeds from the C# game's pixels per second to world units
//! (written `px / 32`), colours from bytes, turn rates negated (C# angles
//! turn the other way, see `units`). Heights stay C#'s `DepthConst` values.
//!
//! Only the effects something spawns are here; add the others with the
//! code that spawns them.

use bevy::math::Vec2;

use super::{State, Style};

/// `DepthConst.CarBase`: the car's chassis.
pub const CAR_BASE: f32 = -16.0 / 32.0 / 16.0;
/// `DepthConst.SmokeFlyTrailEnd`.
const SMOKE_FLY_TRAIL_END: f32 = -24.0 / 32.0 / 16.0;

const TRANSPARENT: [f32; 4] = [0.0; 4];

/// `Color.Lerp(Color.White, Color.Black, k)`: an opaque grey.
const fn grey(k: f32) -> [f32; 4] {
    let v = 1.0 - k;
    [v, v, v, 1.0]
}

/// `_smokeTrailStyleVer2`: a soft dark streak left on the ground behind a
/// slipping wheel, along the way it slides.
pub static WHEEL_TRAIL: Style = Style {
    begin: State {
        speed: 0.0,
        scale: Vec2::new(16.0, 3.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: grey(0.75),
        height: 0.0,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::new(24.0, 3.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: 0.0,
    },
    lifetime: 1.0,
    texture: "Gradient4Pixel",
};

/// `_smokeCarTrailStyle`: an exhaust puff shot backwards from the car,
/// growing and slowing as it rises.
pub static EXHAUST: Style = Style {
    begin: State {
        speed: 128.0 / 32.0,
        scale: Vec2::ZERO,
        view_rotation: 0.0,
        rotation: 0.0,
        color: grey(0.85),
        height: CAR_BASE,
    },
    end: State {
        speed: 24.0 / 32.0,
        scale: Vec2::new(1.4, 1.4),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_FLY_TRAIL_END,
    },
    lifetime: 0.5,
    texture: "Particles\\Smoke",
};

/// Every texture a style here draws with, to load with the map.
pub const TEXTURES: [&str; 2] = [WHEEL_TRAIL.texture, EXHAUST.texture];
