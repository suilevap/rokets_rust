//! The Shotgun (`ShotgunWeapon`, 2012-2014): five pellets in a spread and a
//! kick that shoves the car back.
//!
//! C# origin: `ShotgunWeapon` (`OnShoot`; its constructor's
//! `_fireOnlyAccurate` and `MaxAngle`), `WeaponOptions.ShotgunDefaultOptions`
//! and `DamageConst.Shotgun` / `ShotgunImpulse` (the numbers in
//! [`super::model`]), `WeaponHelper.SimulateMinigunBullet` /
//! `SimulateBullet` (each pellet), `WeaponFactory` (the one `Shotgun`
//! type) and `SoundManager.Shotgun`. The `Shotgun` type is the only one:
//! `Cars.xml` gives it to no car; the maps give it to an enemy of
//! `Swarm` (a deactivated buggy, woken by the map) and of `UnderPressure`.
//! `git log` of the C# file: a gun of its own with a damage and a push per
//! pellet and a dispersion of its own (2012), then its options in
//! `WeaponOptions` (the player's own preset, `SetLocalControlPreferences`,
//! was commented out), `_fireOnlyAccurate` and the kick on the car.
//!
//! - It fires only on an accurate pull ([`crate::controls::PlayerIntent::fire_accurate`]:
//!   `_fireOnlyAccurate`, as the Railgun and the rocket launchers), once a
//!   reload (2 s, the first a random part of it) and if the player's
//!   energy pays (0.1): [`super::fire_weapons`] takes the pull on the
//!   Railgun's rhythm ([`model::Railgun`]) and sends a [`ShotgunBlast`].
//!   It turns half a turn either way (`MaxAngle` π); the aim line shows
//!   its reload as the Railgun's does.
//! - A blast ([`shotgun_blasts`]) kicks the gun 30 pixels back
//!   ([`model::SHOTGUN_RECOIL`], set), shoves the car back along the barrel
//!   with an impulse of 15 ([`model::shotgun_kick`]; `FeedbackImpulse`,
//!   `ApplyLinearImpulse` on the car's body, nothing for a car that
//!   cannot move), then fires five pellets ([`model::shotgun_directions`]),
//!   each a ray from the gun's mount along its own angle, a random part of
//!   ±π/8 off the barrel (not evenly spread: they bunch and gap), the
//!   rays of the Minigun's bullet (`SimulateBullet`): a pellet pushes what
//!   it hits at the point, 6 along its line swung off by up to 45° (every
//!   pellet pushes the same; the Minigun's vary), hurts what can be hurt
//!   for 4 (so up to 20 a blast), spares a car of the shooter's team (it
//!   still pushes it) and flies through sensors and low boxes.
//! - The view ([`show_shots`], in `Update` with its own random numbers):
//!   per pellet a flash at the muzzle and a streak somewhere along the
//!   path (as the Minigun's), a spark and a one-in-three ricochet where it
//!   struck what cannot be hurt, the red damage flash where it hurt a
//!   living car; one `Shotgun` sound at the gun for the blast.
//!
//! Not ported:
//! - `SetLocalControlPreferences` / `UnSetLocalControlPreferences`' player
//!   preset (damage 7, push 12, half the dispersion): commented out in C#.
//! - The sound pool of two instances (`RegisterSound("Shotgun", 2)`): every
//!   blast sounds.
//! - A pellet that hits a car with a gun still pushes it (C#'s did); the
//!   Minigun's and the Railgun's bullets of this port do not push such
//!   cars, `fire_weapons` leaving them out of its bodies; this is not a
//!   difference from C#.
//! - The view checks that skip effects off screen (the particle pool takes
//!   them all), and the slow-motion extras of `CreateBulletTrailBetween`
//!   (no slow motion yet).
//! - `WeaponOptions.BaseDamageModify` levels: none exists for the Shotgun
//!   (only the spawn director's `GetMinigun`, `GetPlasmagun`, `GetHammer`,
//!   `GetZombieWeapon`).
//! - An [`Inactive`](crate::actions::Inactive) car does not fire (as the Plasma gun's); C# skipped
//!   its update the same way (the deactivated buggy of `Swarm`).

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::model::{self, Kind};
use super::{Bullet, Hurt, PassedByBullets, Targets, ViewRng, Weapon, WeaponSystems, fire_weapons, spark, trail};
use crate::car::control::{Team, spares};
use crate::explosion::{Health, Hit};
use crate::game::GameState;
use crate::particles::styles::{BULLET_SPLASH, DAMAGE_GLOW};
use crate::render::particles::Particles;
use crate::sound::PlaySound;

/// `PlaySoundAndCreateSpark`: a spark off what cannot be hurt rings when a
/// random number is above this (`NextFloatFake() > 0.65f`, about one time
/// in three).
pub const RICOCHET_ABOVE: f32 = 0.65;

/// The Shotgun fires (`ShotgunWeapon.OnShoot`): sent by the weapon's
/// trigger ([`super::fire_weapons`]) from `car`, its gun's mount at `from`
/// and muzzle at `muzzle`, barrel pointing `direction`.
#[derive(Message, Debug, Clone, Copy)]
pub struct ShotgunBlast {
    pub car: Entity,
    pub from: Vec2,
    pub muzzle: Vec2,
    pub direction: f32,
}

/// What a pellet came to, for the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landed {
    /// Open air, or something that cannot be hurt (a wall, a box): sparks.
    Nothing,
    /// A car of the shooter's team: pushed, not hurt, no sparks (it is a
    /// car).
    Spared,
    /// Something that can be hurt; `living` shows the damage flash (a wreck
    /// is hurt silently).
    Hurt { living: bool },
}

/// One pellet's path, for the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pellet {
    pub end: Vec2,
    pub direction: f32,
    pub landed: Landed,
}

/// A blast's pellets after they flew, for the view: where the gun is, its
/// muzzle, and each pellet.
#[derive(Message, Debug, Clone)]
pub struct ShotgunShot {
    pub at: Vec2,
    pub muzzle: Vec2,
    pub pellets: Vec<Pellet>,
}

pub struct ShotgunPlugin;

impl Plugin for ShotgunPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShotgunBlast>()
            .add_message::<ShotgunShot>()
            .add_systems(
                FixedUpdate,
                shotgun_blasts
                    .in_set(WeaponSystems)
                    .after(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, show_shots.run_if(in_state(GameState::Playing)));
    }
}

/// `ShotgunWeapon.OnShoot`: the kick, then the pellets. The random numbers
/// are the gameplay's own (seeded), drawn in the order the blasts come.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn shotgun_blasts(
    mut blasts: MessageReader<ShotgunBlast>,
    spatial: SpatialQuery,
    owners: Query<&Weapon>,
    skipped: Query<(), PassedByBullets>,
    hurt: Query<Option<&Health>, Hurt>,
    teams: Query<&Team>,
    mut bodies: Query<(&RigidBody, Forces)>,
    mut hits: MessageWriter<Hit>,
    mut shots: MessageWriter<ShotgunShot>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(44));
    let (passes, can_hurt) = (|e| skipped.contains(e), |e| hurt.contains(e));
    let targets = Targets {
        passes: &passes,
        hurt: &can_hurt,
    };
    for blast in blasts.read() {
        let Ok(gun) = owners.get(blast.car) else {
            continue;
        };
        if gun.kind != Kind::Shotgun {
            continue;
        }
        let options = gun.options;
        // `PhysicBody.ApplyLinearImpulse`: on the car's body, at its middle.
        if let Ok((body, mut forces)) = bodies.get_mut(blast.car)
            && body.is_dynamic()
        {
            forces.apply_linear_impulse(model::shotgun_kick(blast.direction, &options));
        }
        let mut pellets = Vec::with_capacity(options.bullets as usize);
        for direction in model::shotgun_directions(blast.direction, &options, || rng.random()) {
            let bullet = Bullet {
                from: blast.from,
                muzzle: blast.muzzle,
                direction,
                impulse: options.impulse,
                damage: options.damage,
            };
            let legs = targets.bullet(&spatial, blast.car, &bullet, 0);
            let leg = &legs[0];
            let landed = land(leg, &bullet, blast.car, &mut bodies, &hurt, &teams, rng, &mut hits);
            // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
            debug!(
                "shotgun pellet at {:.1} to {:.3} {:.3} hit {:?}",
                -direction.to_degrees(),
                leg.end.x,
                -leg.end.y,
                leg.hit.map(|h| h.0)
            );
            pellets.push(Pellet {
                end: leg.end,
                direction,
                landed,
            });
        }
        shots.write(ShotgunShot {
            at: blast.from,
            muzzle: blast.muzzle,
            pellets,
        });
    }
}

/// `SimulateBullet`'s blow where a pellet ended: it pushes what it hit at
/// the point along its line, swung off it by up to 45°, if that is a
/// dynamic body, and hurts what can be hurt unless it is a car of the
/// shooter's team.
#[allow(clippy::too_many_arguments)]
fn land(
    leg: &super::beam::Leg,
    bullet: &Bullet,
    shooter: Entity,
    bodies: &mut Query<(&RigidBody, Forces)>,
    hurt: &Query<Option<&Health>, Hurt>,
    teams: &Query<&Team>,
    rng: &mut SmallRng,
    hits: &mut MessageWriter<Hit>,
) -> Landed {
    let point = leg.end;
    let Some((entity, _)) = leg.hit else {
        return Landed::Nothing;
    };
    if bullet.impulse != 0.0
        && let Ok((body, mut forces)) = bodies.get_mut(entity)
        && body.is_dynamic()
    {
        let along = point - leg.cast_from;
        let amount = (along + along.perp() * (rng.random::<f32>() * 2.0 - 1.0)).normalize_or_zero();
        forces.apply_linear_impulse_at_point(amount * bullet.impulse, point);
    }
    let Ok(health) = hurt.get(entity) else {
        return Landed::Nothing;
    };
    // `ignoreDamage`: pushed but not hurt, and a car all the same.
    if spares(teams.get(shooter).ok().copied(), teams.get(entity).ok().copied()) {
        return Landed::Spared;
    }
    hits.write(Hit {
        target: entity,
        damage: bullet.damage,
        from: leg.direction,
        by: Some(shooter),
    });
    Landed::Hurt {
        living: health.is_some_and(|h| h.hp > 0),
    }
}

/// What a blast looks and sounds like (`SimulateMinigunBullet`'s effects
/// for every pellet, `SoundManager.PlayOnce(Shotgun, …)`), a frame at a
/// time with the views' own random numbers.
fn show_shots(
    mut shots: MessageReader<ShotgunShot>,
    mut particles: ResMut<Particles>,
    mut play: MessageWriter<PlaySound>,
    mut rng: ResMut<ViewRng>,
) {
    let rng = &mut rng.0;
    for shot in shots.read() {
        for pellet in &shot.pellets {
            let (dir, point) = (pellet.direction, pellet.end);
            particles.try_add(&BULLET_SPLASH, shot.muzzle, dir, dir + std::f32::consts::PI);
            trail(&mut particles, rng, shot.muzzle, point, dir);
            match pellet.landed {
                Landed::Nothing => {
                    spark(&mut particles, rng, point, dir);
                    if rng.random::<f32>() > RICOCHET_ABOVE {
                        play.write(PlaySound {
                            name: "Ricochet1",
                            at: Some(point),
                            pitch: (rng.random::<f32>() * 2.0 - 1.0) * 0.8,
                            volume: 0.3 + rng.random::<f32>() * 0.5,
                        });
                    }
                }
                Landed::Hurt { living: true } => {
                    particles.add_scaled(&DAMAGE_GLOW, point, 0.0, rng.random::<f32>() * std::f32::consts::TAU, 1.0);
                }
                Landed::Hurt { living: false } | Landed::Spared => {}
            }
        }
        play.write(PlaySound {
            name: "Shotgun",
            at: Some(shot.at),
            pitch: 0.0,
            volume: 1.0,
        });
    }
}
