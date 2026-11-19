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

/// The texture's middle on the particle's point.
const CENTRE: Vec2 = Vec2::splat(0.5);

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
    additive: false,
    random_frame: false,
    origin: CENTRE,
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
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `DepthConst.SmokeStart`.
const SMOKE_START: f32 = -19.1 / 32.0 / 16.0;
/// `DepthConst.SmokeEnd`.
const SMOKE_END: f32 = -24.0 / 32.0 / 16.0;
/// `DepthConst.Bomb - 0.01`: just over a bomb.
const OVER_BOMB: f32 = -0.01;

const fn color(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}

const fn still(scale: f32, view_rotation: f32, color: [f32; 4], height: f32) -> State {
    State {
        speed: 0.0,
        scale: Vec2::splat(scale),
        view_rotation,
        rotation: 0.0,
        color,
        height,
    }
}

/// `_bip2Style`: a bomb's red ping, once a second or so.
pub static BOMB_BEEP: Style = Style {
    begin: still(0.5, 0.0, color(1.0, 0.0, 0.0), OVER_BOMB),
    end: still(2.0, 0.0, TRANSPARENT, OVER_BOMB),
    lifetime: 0.5,
    texture: "Particles\\Circle",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_boomCircleEffect`: the blast's ring, sized to its range
/// ([`ring_rate`]).
pub static BOOM_RING: Style = Style {
    begin: still(0.5, 0.0, color(1.0, 1.0, 1.0), OVER_BOMB),
    end: still(4.0, 0.0, TRANSPARENT, OVER_BOMB),
    lifetime: 0.5,
    texture: "Particles\\Circle",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_boomStyleCircle`: a big yellow flash shrinking as it turns.
pub static BOOM_FLASH: Style = Style {
    begin: still(12.5, -1.0, color(1.0, 1.0, 0.0), SMOKE_START + 2.0 / 32.0),
    end: still(1.0, 1.0, TRANSPARENT, SMOKE_START),
    lifetime: 0.75,
    texture: "Particles\\flash2",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_boomStyleCircle2`: the fireball.
pub static BOOM_FIRE: Style = Style {
    begin: still(0.5, 0.0, color(1.0, 1.0, 1.0), SMOKE_START + 2.0 / 32.0),
    end: still(2.0, 0.0, TRANSPARENT, SMOKE_START),
    lifetime: 0.8,
    texture: "Particles\\Explosion",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_smokeFastStyle`: black smoke left by a blast.
pub static BOOM_SMOKE: Style = Style {
    begin: State {
        speed: 4.0 / 32.0,
        scale: Vec2::splat(2.25),
        view_rotation: -0.5,
        rotation: 0.0,
        color: color(0.0, 0.0, 0.0),
        height: SMOKE_START,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::splat(4.5),
        view_rotation: -0.05,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_END,
    },
    lifetime: 1.0,
    texture: "Particles\\Smoke",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_damagedSmokeStyle`: grey smoke a wrecked simple car puffs for its
/// last two seconds, growing and thinning over four.
pub static DAMAGED_SMOKE: Style = Style {
    begin: State {
        speed: 4.0 / 32.0,
        scale: Vec2::splat(1.25),
        view_rotation: -0.5,
        rotation: 0.0,
        // `Color.Gray`.
        color: color(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0),
        height: SMOKE_START,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::splat(4.5),
        view_rotation: -0.05,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_END,
    },
    lifetime: 4.0,
    texture: "Particles\\Smoke",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_smokeCrashedStyle`: thick black smoke over a wreck, shrinking as it
/// thins over six seconds.
pub static CRASH_SMOKE: Style = Style {
    begin: State {
        speed: 4.0 / 32.0,
        scale: Vec2::splat(3.0),
        view_rotation: -0.3,
        rotation: 0.0,
        color: color(0.0, 0.0, 0.0),
        height: SMOKE_START,
    },
    end: State {
        speed: 4.0 / 32.0,
        scale: Vec2::splat(1.0),
        view_rotation: -0.05,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_END,
    },
    lifetime: 6.0,
    texture: "Particles\\Smoke",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_boomStyle`: a fiery streak flying out of a wreck.
pub static WRECK_FIRE: Style = Style {
    begin: State {
        speed: 64.0 * 1.5 / 32.0,
        scale: Vec2::new(1.0, 0.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: color(1.0, 1.0, 0.0),
        height: SMOKE_START,
    },
    end: State {
        speed: 16.0 * 1.5 / 32.0,
        scale: Vec2::new(2.0, 0.25),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_START - 2.0 / 32.0,
    },
    lifetime: 0.75,
    texture: "Particles\\firetrail",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_boomStyle2`: a thinner, faster streak.
pub static WRECK_SPARK: Style = Style {
    begin: State {
        speed: 32.0 * 1.5 / 32.0,
        scale: Vec2::new(0.25, 0.05),
        view_rotation: 0.0,
        rotation: 0.0,
        color: color(1.0, 1.0, 0.0),
        height: SMOKE_START + 1.0 / 32.0,
    },
    end: State {
        speed: 64.0 * 1.5 / 32.0,
        scale: Vec2::new(1.0, 0.25),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_START + 1.0 / 32.0,
    },
    lifetime: 0.8,
    texture: "Particles\\firetrail",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `DepthConst.Spark`.
const SPARK: f32 = -24.0 / 32.0 / 16.0;

/// A spark off a hit (`_collisionSpark`, `_collisionSpark2`): a white
/// streak shot out fast, slowing and thinning to nothing in 0.4 s while
/// it curls round one way (`curl` radians a second, the two styles turn
/// opposite ways).
const fn spark_state(curl: f32, speed: f32, width: f32) -> State {
    State {
        speed,
        scale: Vec2::new(1.0, width),
        view_rotation: curl,
        rotation: curl,
        color: color(1.0, 1.0, 1.0),
        height: SPARK,
    }
}

const fn spark(curl: f32) -> Style {
    Style {
        begin: spark_state(curl, 512.0 / 32.0, 2.0),
        end: spark_state(curl, 128.0 / 32.0, 0.0),
        lifetime: 0.4,
        texture: "Particles\\SmallLine",
        additive: true,
        random_frame: false,
        origin: CENTRE,
    }
}

/// The two ways sparks curl (C#'s ±0.75π, negated for our angles).
pub static SPARKS: [Style; 2] = [spark(-0.75 * std::f32::consts::PI), spark(0.75 * std::f32::consts::PI)];

/// `_cloneStyle`: a soft white flash where a clone appears.
pub static CLONE_FLASH: Style = Style {
    begin: still(6.0, 0.0, [1.0; 4], SMOKE_END),
    end: still(5.0, 0.0, TRANSPARENT, SMOKE_END),
    lifetime: 0.5,
    texture: "Particles\\SoftBrush",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `DepthConst.Teleport + 0.01`: on the floor.
const TELEPORT: f32 = (0.2 + 0.01) / 16.0;

/// `_teleportStyle`: a streak drawn into a teleport, off towards where it
/// leads.
pub static TELEPORT_STREAK: Style = Style {
    begin: State {
        speed: 4.0 / 32.0,
        scale: Vec2::new(1.5, 4.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: [1.0; 4],
        height: TELEPORT,
    },
    end: State {
        speed: 16.0 / 32.0,
        scale: Vec2::new(3.0, 3.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: TELEPORT,
    },
    lifetime: 0.5,
    texture: "Particles\\SnowEffect",
    additive: true,
    random_frame: true,
    origin: CENTRE,
};

/// `_teleportStyle2`: a glow at the middle, turning as it shrinks into it.
pub static TELEPORT_SWIRL: Style = Style {
    begin: still(1.5, 0.0, TRANSPARENT, TELEPORT),
    end: still(0.0, -3.0, [1.0; 4], TELEPORT),
    lifetime: 0.5,
    texture: "Particles\\Explosion",
    additive: true,
    random_frame: true,
    origin: CENTRE,
};

/// `GetScaleForEnd`: the size rate that makes [`BOOM_RING`] end about
/// `range` world units across its radius, for a `Circle` texture
/// `texture_width` pixels wide.
pub fn ring_rate(range: f32, texture_width: f32) -> f32 {
    let growth = BOOM_RING.end.scale.x - BOOM_RING.begin.scale.x;
    range / (texture_width / 2.0 / 32.0) - growth
}

/// `Color.Yellow`.
const YELLOW: [f32; 4] = color(1.0, 1.0, 0.0);

/// `_bulletSplashStyle`: the Minigun's muzzle flash, a yellow streak out
/// of the barrel, its texture held at 80% of its length.
pub static BULLET_SPLASH: Style = Style {
    begin: State {
        speed: 512.0 / 32.0,
        scale: Vec2::new(0.25 * 2.5, 2.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: YELLOW,
        height: SPARK,
    },
    end: State {
        speed: 512.0 / 32.0,
        scale: Vec2::new(0.5 * 2.5, 0.25 * 2.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPARK,
    },
    lifetime: 0.1,
    texture: "Particles\\minigunTrail",
    additive: true,
    random_frame: false,
    origin: Vec2::new(0.8, 0.5),
};

/// `_bulletTrailStyle`: a bullet in flight, a yellow streak racing along
/// its path and shrinking in 0.2 s.
pub static BULLET_TRAIL: Style = Style {
    begin: State {
        speed: 1512.0 / 32.0,
        scale: Vec2::new(2.0, 1.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: YELLOW,
        height: SPARK,
    },
    end: State {
        speed: 512.0 / 32.0,
        scale: Vec2::new(0.25, 0.25),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPARK,
    },
    lifetime: 0.2,
    texture: "Particles\\minigunTrail",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `BulletTrailMaxLength`: the trail's longest, world units (the
/// texture's 127 pixels at its largest width, 2).
pub const BULLET_TRAIL_LENGTH: f32 = 127.0 * 2.0 / 32.0;

/// `_sparkConstStyle`: a yellow flash where a bullet strikes, widening
/// as it fades in a quarter of a second.
pub static BULLET_SPARK: Style = Style {
    begin: State {
        speed: 0.0,
        scale: Vec2::new(3.0, 3.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: YELLOW,
        height: SPARK,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::new(6.0, 3.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPARK,
    },
    lifetime: 0.25,
    texture: "Particles\\Spark",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_damageObjStyle2`: a yellow glow on something a bullet hurts,
/// shrinking to nothing as it turns white.
pub static DAMAGE_GLOW: Style = Style {
    begin: still(16.0, 0.0, YELLOW, SMOKE_START),
    end: still(0.0, 0.0, [1.0; 4], SMOKE_END),
    lifetime: 0.5,
    texture: "Gradient4PixelUniform",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `DepthConst.SmokeFlyTrailStart`: over the cars.
const SMOKE_FLY_TRAIL_START: f32 = -10.0 / 32.0 / 16.0;

/// `_railgunTrailStyle1` and `2`: a white spark of the Railgun's beam that
/// turns yellow and shrinks away in half a second, drifting off and
/// spinning (the two spin opposite ways; C#'s +2 and -2 rad/s, negated).
const fn railgun_trail(spin: f32) -> Style {
    Style {
        begin: State {
            speed: 0.0,
            scale: Vec2::new(2.0, 1.0),
            view_rotation: 0.0,
            rotation: 0.0,
            color: [1.0; 4],
            height: SMOKE_FLY_TRAIL_START,
        },
        end: State {
            speed: 1.5,
            scale: Vec2::new(0.0, 1.0),
            view_rotation: spin,
            rotation: 0.0,
            color: YELLOW,
            height: SMOKE_FLY_TRAIL_START,
        },
        lifetime: 0.5,
        texture: "Particles\\SmallLine",
        additive: true,
        random_frame: false,
        origin: CENTRE,
    }
}

pub static RAILGUN_TRAIL: [Style; 2] = [railgun_trail(-2.0), railgun_trail(2.0)];

/// A trail spark's spacing along the beam: the texture's 16 pixels × 2.5.
pub const RAILGUN_TRAIL_SPACING: f32 = 16.0 / 32.0 * 2.5;

/// `_railgunSplash`: a white flash at the muzzle and where the beam ends,
/// turning yellow and shrinking away in 0.2 s.
pub static RAILGUN_SPLASH: Style = Style {
    begin: State {
        speed: 0.0,
        scale: Vec2::new(2.0, 2.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: [1.0; 4],
        height: SMOKE_FLY_TRAIL_START,
    },
    end: State {
        speed: 1.5,
        scale: Vec2::new(0.0, 0.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: YELLOW,
        height: SMOKE_FLY_TRAIL_START,
    },
    lifetime: 0.2,
    texture: "Particles\\flash2",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// Every texture a style here draws with, to load with the map.
pub const TEXTURES: [&str; 15] = [
    WHEEL_TRAIL.texture,
    EXHAUST.texture,
    BOMB_BEEP.texture,
    BOOM_FLASH.texture,
    BOOM_FIRE.texture,
    BOOM_SMOKE.texture,
    WRECK_FIRE.texture,
    SPARKS[0].texture,
    TELEPORT_STREAK.texture,
    CLONE_FLASH.texture,
    BULLET_TRAIL.texture,
    BULLET_SPARK.texture,
    DAMAGE_GLOW.texture,
    RAILGUN_TRAIL[0].texture,
    RAILGUN_SPLASH.texture,
];
