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

/// `_accelerationReadyEffect`: a white ring closing onto the accelerator's
/// car when its energy is whole again.
pub static ACCELERATION_READY: Style = Style {
    begin: still(4.0, 0.0, color(1.0, 1.0, 1.0), OVER_BOMB),
    end: still(0.5, 0.0, TRANSPARENT, OVER_BOMB),
    lifetime: 0.2,
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

/// `_zombieCircleStartEffect`: the flash of the Zombie's wave, a white ring
/// swelling by half and fading in a quarter second (start it with
/// [`crate::weapon::model::zombie_circle_rate`]).
pub static ZOMBIE_CIRCLE_START: Style = Style {
    begin: still(1.0, 0.0, color(1.0, 1.0, 1.0), OVER_BOMB),
    end: still(1.5, 0.0, TRANSPARENT, OVER_BOMB),
    lifetime: 0.25,
    texture: "UI\\BigCircle",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_zombieCircleEffect`: the dark ring the wave leaves, fading in three
/// quarters of a second, as big as it.
pub static ZOMBIE_CIRCLE: Style = Style {
    begin: still(1.0, 0.0, color(0.0, 0.0, 0.0), OVER_BOMB),
    end: still(1.0, 0.0, TRANSPARENT, OVER_BOMB),
    lifetime: 0.75,
    texture: "UI\\BigCircle",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_stunEffect`: the stun's star flying in as it spins up, from four
/// times its size to its own, in a quarter second, when a car is stunned.
pub static STUN_FLASH: Style = Style {
    begin: still(4.0, -std::f32::consts::PI, TRANSPARENT, SMOKE_END - 0.01),
    end: still(1.0, -8.0 * std::f32::consts::PI, [1.0; 4], SMOKE_END - 0.01),
    lifetime: 0.25,
    texture: "Particles\\Stun",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

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

/// `_plasmaEndStyle`: where a plasma bullet ends, a white flash growing
/// from nothing to twice its size as it fades, in 0.2 s.
pub static PLASMA_END: Style = Style {
    begin: still(0.0, 0.0, color(1.0, 1.0, 1.0), SMOKE_START),
    end: still(2.0, 0.0, TRANSPARENT, SMOKE_START),
    lifetime: 0.2,
    texture: "Particles\\flash",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `DepthConst.SmokePiuStart` and `SmokePiuEnd`.
const SMOKE_PIU: f32 = -24.0 / 32.0 / 16.0;

/// `_smokePiuStyle`: the two black puffs a rocket launcher blows out
/// sideways at its muzzle (`CreatePiu`), shrinking as they thin in 2 s.
pub static LAUNCH_SMOKE: Style = Style {
    begin: State {
        speed: 16.0 / 32.0,
        scale: Vec2::splat(2.0),
        view_rotation: -0.5,
        rotation: 0.0,
        color: color(0.0, 0.0, 0.0),
        height: SMOKE_PIU,
    },
    end: State {
        speed: 16.0 / 32.0,
        scale: Vec2::splat(0.5),
        view_rotation: -0.05,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_PIU,
    },
    lifetime: 2.0,
    texture: "Particles\\Smoke",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_smokeStyle`: the white puff a homing rocket leaves every update,
/// growing from a tenth of the texture to two and a half and thinning
/// over four seconds.
pub static ROCKET_SMOKE: Style = Style {
    begin: State {
        speed: 4.0 / 32.0,
        scale: Vec2::splat(0.25),
        view_rotation: -0.5,
        rotation: 0.0,
        color: color(1.0, 1.0, 1.0),
        height: SMOKE_START,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::splat(2.5),
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

/// `DelayWeaponDecorator._chargingStyle`: a red ring closing on the muzzle
/// of a charging Laser, from nothing (three times the texture's size) to
/// a tenth of it as it reddens, for the 0.9 s the charge takes
/// (`weapon::model::LASER_CHARGE`).
pub static LASER_CHARGE: Style = Style {
    begin: still(3.0, 0.0, TRANSPARENT, SPARK),
    end: still(0.1, 0.0, color(1.0, 0.0, 0.0), SPARK),
    lifetime: 0.9,
    texture: "Particles\\Circle",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `Color.Cyan`.
const CYAN: [f32; 4] = color(0.0, 1.0, 1.0);

/// `_teslaTrailStyle`: a cyan spark of the Tesla's lightning shooting out
/// along a random line at 512 pixels a second and stopping, short and
/// thin to start, then shorter and wider as it fades, in 0.2 s.
pub static TESLA_TRAIL: Style = Style {
    begin: State {
        speed: 512.0 / 32.0,
        scale: Vec2::new(2.0, 0.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: CYAN,
        height: SMOKE_FLY_TRAIL_START,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::new(0.75, 2.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_FLY_TRAIL_START,
    },
    lifetime: 0.2,
    texture: "Particles\\TeslaTrail",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `TeslaTrailMaxLength` (`GetMaxWidth`): the texture's 64 pixels × 2.
pub const TESLA_TRAIL_LENGTH: f32 = 64.0 / 32.0 * 2.0;

/// `_teslaTrailStaticStyle`: one still link of a bolt, cyan, widening
/// from half to 2.75 times the texture's height as it fades in 0.2 s.
pub static TESLA_TRAIL_STATIC: Style = Style {
    begin: State {
        speed: 0.0,
        scale: Vec2::new(1.0, 0.5),
        view_rotation: 0.0,
        rotation: 0.0,
        color: CYAN,
        height: SMOKE_FLY_TRAIL_START,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::new(1.0, 2.75),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SMOKE_FLY_TRAIL_START,
    },
    lifetime: 0.2,
    texture: "Particles\\TeslaTrail",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `TeslaTrailStaticLength` (`GetMaxWidth`): a link is the texture's 64
/// pixels long.
pub const TESLA_LINK_LENGTH: f32 = 64.0 / 32.0;

/// `DepthConst.SpeedTrailParticleStart` and `…End` (-4): high enough to be
/// drawn over the cars.
const SPEED_TRAIL_PARTICLE: f32 = -4.0 / 32.0 / 16.0;

/// `_newZombieFriendStyle`: the ring a zombie's outline makes when a car
/// joins the player's horde (`CreateParticleEffect`, `NewZombieFriend`),
/// flaring from 0.65 to 12 times the border sprite's size and fading in
/// 0.3 s. It stays still; the sprite is turned the way the car faces.
pub static NEW_ZOMBIE_FRIEND: Style = Style {
    begin: still(0.65, 0.0, color(1.0, 1.0, 1.0), SPEED_TRAIL_PARTICLE),
    end: still(12.0, 0.0, TRANSPARENT, SPEED_TRAIL_PARTICLE),
    lifetime: 0.3,
    texture: "Cars\\Zombie\\Border",
    additive: true,
    random_frame: false,
    origin: CENTRE,
};

/// `_snowStyle`: a streak of wind-blown snow (`SnowWind` maps,
/// `WindSnowEffect`), growing from nothing to 128 × 24 times the three-pixel
/// sprite and quickening as it fades.
pub static WIND_SNOW: Style = Style {
    begin: State {
        speed: 256.0 / 32.0,
        scale: Vec2::ZERO,
        view_rotation: 0.0,
        rotation: 0.0,
        color: [0.5; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 512.0 / 32.0,
        scale: Vec2::new(128.0, 24.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 1.0,
    texture: "Particles\\3Pixel",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_snowStyleWithRotate`: a swirl of snow dust, a random frame of the
/// `SnowEffect` sprite, that curves as it flies and straightens.
pub static WIND_SNOW_TURNING: Style = Style {
    begin: State {
        speed: 256.0 / 32.0 * 2.0,
        scale: Vec2::ZERO,
        view_rotation: -1.4,
        rotation: -1.4,
        color: [0.5; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 512.0 / 32.0 * 4.0,
        scale: Vec2::new(32.0, 16.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 1.0,
    texture: "Particles\\SnowEffect",
    additive: false,
    random_frame: true,
    origin: CENTRE,
};

/// The screen snow's styles (`SnowEffect`, `CreateSnowUI`) are in the
/// screen's pixels, not world units: a speed is pixels per second and a
/// scale is times the sprite's size, both before the interface's scale.
///
/// `_snowStyleUI`: a long streak, curving one way.
pub static SCREEN_SNOW_STREAK: Style = Style {
    begin: State {
        speed: 256.0,
        scale: Vec2::ZERO,
        view_rotation: -1.4,
        rotation: -1.4,
        color: [0.5; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 512.0,
        scale: Vec2::new(64.0, 12.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 1.0,
    texture: "Particles\\3Pixel",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// `_snowStyleUI2`: a soft flake (a random frame of `SnowEffect`), faster
/// than the streak and curving the other way.
pub static SCREEN_SNOW_FLAKE: Style = Style {
    begin: State {
        speed: 256.0 * 2.0,
        scale: Vec2::ZERO,
        view_rotation: 1.4,
        rotation: 1.4,
        color: [0.5; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 512.0 * 2.0,
        scale: Vec2::new(16.0, 12.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 1.0,
    texture: "Particles\\SnowEffect",
    additive: false,
    random_frame: true,
    origin: CENTRE,
};

/// `_snowStyleUI3`: a speck of dirt on the glass (a random frame of
/// `GlassDirt`) that stays where it is two seconds and fades.
pub static SCREEN_SNOW_SPECK: Style = Style {
    begin: State {
        speed: 0.0,
        scale: Vec2::splat(4.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: [0.4; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 0.0,
        scale: Vec2::splat(4.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 2.0,
    texture: "Particles\\GlassDirt",
    additive: false,
    random_frame: true,
    origin: CENTRE,
};

/// `_snowUIStyles`: what `CreateSnowUI` picks from, at random: two streaks,
/// eight flakes and a speck in eleven.
pub static SCREEN_SNOW: [&Style; 11] = [
    &SCREEN_SNOW_STREAK,
    &SCREEN_SNOW_STREAK,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_FLAKE,
    &SCREEN_SNOW_SPECK,
];

/// `_speedTrailStyle`: a pale streak flying across the screen against the
/// way a car at its top speed goes (`CreateSpeedTrails`, the interface's
/// pool), growing from nothing to 196 × 16 times the three-pixel sprite
/// while it fades, at 1024 pixels a second. Scale and speed are the
/// canvas's, as the screen snow's.
pub static SPEED_LINE: Style = Style {
    begin: State {
        speed: 1024.0,
        scale: Vec2::ZERO,
        view_rotation: 0.0,
        rotation: 0.0,
        color: [0.5; 4],
        height: SPEED_TRAIL_PARTICLE,
    },
    end: State {
        speed: 1024.0,
        scale: Vec2::new(196.0, 16.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: TRANSPARENT,
        height: SPEED_TRAIL_PARTICLE,
    },
    lifetime: 1.0,
    texture: "Particles\\3Pixel",
    additive: false,
    random_frame: false,
    origin: CENTRE,
};

/// Every texture a style here draws with, to load with the map.
pub const TEXTURES: [&str; 21] = [
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
    ZOMBIE_CIRCLE.texture,
    STUN_FLASH.texture,
    ZOMBIE_CIRCLE_START.texture,
    PLASMA_END.texture,
    TESLA_TRAIL.texture,
    NEW_ZOMBIE_FRIEND.texture,
];
