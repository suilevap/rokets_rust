//! A beam's path: a ray from a gun, bounced off what it hits.
//!
//! C# traced the same path twice, by two sets of rules: the Railgun's
//! bullet (`WeaponHelper.SimulateRailgunBullet`) and the aim line
//! (`LaserRenderer.Render` with `ReflectionCount`), which the Laser weapon
//! also used to hurt what its beam touched (`LaserWeapon.Update` read the
//! renderer's `CollideFixtures`, `CollidePositions`, `CollideNormals`).
//! Here [`trace`] is that path once. The aim line and the Laser
//! ([`super::laser::beam_legs`]: `Reach::Total` of 256 units) trace by one
//! set of rules, the Railgun's bullet by another, and [`BeamRules`] are
//! the differences:
//!
//! - [`Reach`]: the bullet's every leg flies its full reach; the aim line's
//!   legs share one range, measured from where each is drawn.
//! - `stop_at_hurt`: the aim line ends at what can be hurt
//!   (`NetworkPhysicEntity`); the bullet bounces off it too.
//!
//! Both bounce by [`model::reflect`] (not head-on, not without a surface)
//! and start the next leg [`BOUNCE_OFFSET`] off the surface.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::model;

/// How far off the surface a bounced beam starts (`normal * 0.1f`).
pub const BOUNCE_OFFSET: f32 = 0.1;

/// How far a beam reaches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reach {
    /// Each leg this far (`SimulateBullet`'s 1024 pixels a ray).
    EachLeg(f32),
    /// All legs together this far, each counted from where it is drawn
    /// (`LaserRenderer`: `length -= l`).
    Total(f32),
}

/// How a beam bounces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamRules {
    pub reach: Reach,
    /// Times it may bounce (`MaxReflections`, `ReflectionCount`).
    pub bounces: u32,
    /// It ends at what can be hurt rather than bouncing off it.
    pub stop_at_hurt: bool,
}

/// One straight leg of a beam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    /// Where it is drawn from: the muzzle, then the previous leg's end.
    pub start: Vec2,
    /// Where its ray starts: the gun's mount, then off the surface it
    /// bounced off.
    pub cast_from: Vec2,
    pub end: Vec2,
    /// World radians.
    pub direction: f32,
    /// What it ended on, and that surface's normal (zero when the ray
    /// started inside it).
    pub hit: Option<(Entity, Vec2)>,
}

/// The beam from a gun whose ray starts at `cast_from` (its mount) and is
/// drawn from `start` (its muzzle), along `direction`, missing `shooter`
/// and what `passes` (what bullets fly through); `hurt` tells what can be
/// hurt.
#[allow(clippy::too_many_arguments)]
pub fn trace(
    spatial: &SpatialQuery,
    shooter: Entity,
    passes: &dyn Fn(Entity) -> bool,
    hurt: &dyn Fn(Entity) -> bool,
    start: Vec2,
    cast_from: Vec2,
    direction: f32,
    rules: BeamRules,
) -> Vec<Leg> {
    let filter = SpatialQueryFilter::from_excluded_entities([shooter]);
    let (mut start, mut cast_from, mut direction) = (start, cast_from, direction);
    let mut left = match rules.reach {
        Reach::EachLeg(r) | Reach::Total(r) => r,
    };
    let mut legs = Vec::new();
    for i in 0..=rules.bounces {
        let reach = match rules.reach {
            Reach::EachLeg(r) => r,
            Reach::Total(_) => left,
        };
        let dir = Vec2::from_angle(direction);
        let hit = Dir2::new(dir)
            .ok()
            .and_then(|d| spatial.cast_ray_predicate(cast_from, d, reach, true, &filter, &|e| !passes(e)));
        let end = cast_from + dir * hit.map_or(reach, |h| h.distance);
        legs.push(Leg {
            start,
            cast_from,
            end,
            direction,
            hit: hit.map(|h| (h.entity, h.normal)),
        });
        if let Reach::Total(_) = rules.reach {
            left -= start.distance(end);
        }
        let Some(hit) = hit.filter(|_| i < rules.bounces && left > 0.0) else {
            break;
        };
        if rules.stop_at_hurt && hurt(hit.entity) {
            break;
        }
        let Some(bounced) = model::reflect(direction, hit.normal) else {
            break;
        };
        direction = bounced;
        start = end;
        cast_from = end + hit.normal * BOUNCE_OFFSET;
    }
    legs
}
