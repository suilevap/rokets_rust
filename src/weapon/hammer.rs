//! The Hammer's saw at work (`HammerWeapon`, 2013–2015).
//!
//! C# origin: `HammerWeapon.OnEntityCollide` (a blow), `Update` (whether it
//! works, and the second hurt it keeps doing to enemies it still
//! touches); the numbers and the tests of reach are in [`super::model`].
//!
//! - A blow: when a contact starts between the car and a body that is no
//!   sensor, and the saw's centre is within the other's radius plus its own
//!   on both axes ([`Hammer::reaches`](super::model::Hammer::reaches)), the
//!   saw hurts what can be hurt (bombs, red boxes and cars) by the car's
//!   speed, 4 to 8 ([`model::hammer_damage`]), shoves any body by 48 from
//!   the saw's centre at a random point within its radius, and throws four
//!   sparks off the saw. A car it hit also shoves the owner back, by half,
//!   at the same point, and is tracked.
//! - Tracking: every 0.33 s the saw hurts again, for the least damage, the
//!   cars it has hit and still touches (and sparks twice), and forgets the
//!   dead and the ones that have moved away.
//! - The saw works while the car is alive and someone drives it (not
//!   `AINone`: [`NoInput`]); otherwise it does nothing and is drawn still.
//!   A car with an AI control works its saw (it turns and hurts what it
//!   touches) although the AI does not drive it yet.
//! - It spares its own team (`isFriend`): a car of the same team is not
//!   hurt, tracked or shoved, only sparked on.
//!
//! The contact begins are Avian's [`CollisionStart`]: Farseer called
//! `OnCollision` as a contact began too. The car's speed is the one before
//! the step ([`LastVelocity`]), as Farseer called it before resolving. Only
//! map entities count (`PhysicEntity`: not the walls, which are no entities).
//!
//! Not ported: see the weapon module.

use std::f32::consts::{FRAC_PI_2, TAU};

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use super::model::{self, HAMMER_BLOW_SPARKS, HAMMER_TRACK_SPARKS, Kind};
use super::{Hurt, Weapon, WeaponRng};
use crate::car::control::{Team, spares};
use crate::car::impact::LastVelocity;
use crate::controls::NoInput;
use crate::explosion::{Health, Hit};
use crate::game::MapId;
use crate::particles::styles::SPARKS;
use crate::render::particles::Particles;

/// Where the saw's centre is now, world (`PiuPosition`).
fn saw_at(gun: &Weapon, position: &Position, rotation: &Rotation) -> Vec2 {
    position.0 + Vec2::from_angle(rotation.as_radians()).rotate(gun.mount)
}

/// The saw's own turn: it works when its car is alive (`_entity.Alive`) and
/// driven (`!(Control is AINone)`).
fn works(health: Option<&Health>, off: bool) -> bool {
    !off && health.is_none_or(|h| h.hp > 0)
}

/// `Entity.Radius`: half the longer side of the entity's unturned box.
fn radius_of(collider: &Collider) -> f32 {
    collider.aabb(Vec2::ZERO, 0.0, 0.0).size().max_element() / 2.0
}

/// `CreateCollisionSparks(pos, dir, n)`: `n` sparks, each from a point
/// moved a little from the last, flying within a quarter turn of `dir`,
/// either of two curls.
fn collision_sparks(particles: &mut Particles, rng: &mut SmallRng, mut at: Vec2, dir: f32, n: usize) {
    for _ in 0..n {
        at += Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * 0.5;
        let fly = dir + (rng.random::<f32>() * 2.0 - 1.0) * FRAC_PI_2;
        let style = &SPARKS[rng.random_range(0..2)];
        particles.try_add(style, at, fly, fly);
    }
}

/// `HammerWeapon.Update`: whether the saw works, and its tracking hurts.
#[allow(clippy::type_complexity)]
pub(super) fn run_saws(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    mut saws: Query<(&mut Weapon, &Position, &Rotation, Option<&Health>, Has<NoInput>)>,
    enemies: Query<(&Position, &Collider, Option<&Health>), With<MapId>>,
    mut rng: ResMut<WeaponRng>,
    mut particles: ResMut<Particles>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    let rng = &mut rng.0;
    for (mut gun, position, rotation, health, off) in &mut saws {
        if gun.kind != Kind::Hammer {
            continue;
        }
        gun.working = works(health, off);
        gun.tracking.tick(dt);
        let at = saw_at(&gun, position, rotation);
        let hammer = gun.hammer;
        let damage = gun.options.damage;
        let working = gun.working;
        let hurt_again = gun.tracking.due(working, |enemy| {
            enemies.get(enemy).is_ok_and(|(p, c, h)| {
                h.is_none_or(|h| h.hp > 0) && hammer.reaches(at, p.0, radius_of(c))
            })
        });
        for enemy in hurt_again {
            hits.write(Hit {
                target: enemy,
                damage,
                from: rng.random::<f32>() * TAU,
            });
            let dir = rng.random::<f32>() * TAU;
            collision_sparks(&mut particles, rng, at, dir, HAMMER_TRACK_SPARKS);
        }
    }
}

/// `HammerWeapon.OnEntityCollide`: a contact starts between the saw's car
/// and something.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn hammer_blows(
    mut started: MessageReader<CollisionStart>,
    mut saws: Query<(&mut Weapon, &Position, &Rotation, &LastVelocity, &RigidBody)>,
    others: Query<(&Position, &Collider, Has<Sensor>, Option<&Team>), With<MapId>>,
    teams: Query<&Team>,
    hurt: Query<(), Hurt>,
    mut bodies: Query<(&RigidBody, Forces), Without<Weapon>>,
    mut owners: Query<Forces, With<Weapon>>,
    mut rng: ResMut<WeaponRng>,
    mut particles: ResMut<Particles>,
    mut hits: MessageWriter<Hit>,
) {
    let rng = &mut rng.0;
    for event in started.read() {
        for (car, other) in [(event.body1, event.body2), (event.body2, event.body1)] {
            let (Some(car), Some(other)) = (car, other) else {
                continue;
            };
            let Ok((mut gun, position, rotation, last, body)) = saws.get_mut(car) else {
                continue;
            };
            let Ok((other_at, collider, sensor, other_team)) = others.get(other) else {
                continue;
            };
            if gun.kind != Kind::Hammer || !gun.working || sensor {
                continue;
            }
            let saw = saw_at(&gun, position, rotation);
            let radius = radius_of(collider);
            if !gun.hammer.reaches(saw, other_at.0, radius) {
                continue;
            }
            let delta = other_at.0 - saw;
            let brain = other_team.is_some();
            // `isFriend`: a car of the saw's own team (the saw's car has a
            // team; a box has none).
            let friend = spares(teams.get(car).ok().copied(), other_team.copied());
            // `RUST_LOG=tilerider::weapon=debug`.
            debug!("hammer blow on {other:?} at {:.2} {:.2}, speed {:.2}", saw.x, -saw.y, last.0.length());
            // Whatever can be hurt (`NetworkPhysicEntity`) is, but a
            // friend: a car is tracked and hurt again.
            if !friend && hurt.contains(other) {
                let damage = model::hammer_damage(&gun.options, last.0.length_squared(), body.is_static());
                if brain {
                    gun.tracking.add(other);
                }
                hits.write(Hit {
                    target: other,
                    damage,
                    from: delta.to_angle(),
                });
            }
            // The shove (`SafeNormalize`: nothing when the centres meet),
            // at a point within the other's radius; not on a friend, but the
            // sparks fly (`CreateCollisionSparks` is outside the `if`).
            if friend {
                let dir = rng.random::<f32>() * TAU;
                collision_sparks(&mut particles, rng, saw, dir, HAMMER_BLOW_SPARKS);
                continue;
            }
            let push = delta.normalize_or_zero() * gun.options.impulse;
            let point = other_at.0 + Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * radius;
            if let Ok((body, mut forces)) = bodies.get_mut(other)
                && body.is_dynamic()
            {
                forces.apply_linear_impulse_at_point(push, point);
            }
            if brain
                && body.is_dynamic()
                && let Ok(mut forces) = owners.get_mut(car)
            {
                forces.apply_linear_impulse_at_point(-push * 0.5, point);
            }
            let dir = rng.random::<f32>() * TAU;
            collision_sparks(&mut particles, rng, saw, dir, HAMMER_BLOW_SPARKS);
        }
    }
}
