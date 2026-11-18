//! The car's gun: the Minigun so far.
//!
//! C# origin: `BaseWeapon`, `BaseComplexWeapon`, `MinigunWeapon`,
//! `WeaponHelper.SimulateMinigunBullet` / `SimulateBullet`,
//! `WeaponFactory`, `EnergyComponent` and the fire input of the
//! `PlayerCarControl*` classes (2010–2016). The numbers and the timing
//! are in [`model`]; this is where they meet the world:
//!
//! - A car from `Cars.xml` with a `Weapon` gets a [`Weapon`] at its
//!   `WeaponPositon`, the player preset when the player drives it, and
//!   the player's car an energy store ([`PlayerEnergy`]).
//! - Every step: the aim turns towards [`PlayerIntent::aim_dir`] (the
//!   mouse, the arrows, the right stick, a drag on the fire zone), the
//!   trigger is pulled while [`PlayerIntent::fire`] is held, and each
//!   bullet is a ray from the gun's mount ([`shoot`]): it pushes what it
//!   hits at the point, off the line by up to 45°, damages what can be
//!   hurt ([`Hit`]), and flashes. The gun kicks back 2 pixels a bullet.
//! - The gun rides the swaying cabin (`DrawCabina` updated it a second
//!   time with the cabin's offset, which also doubled the speed of its
//!   eases; [`Aim::step`] runs twice for cars with a cabin, as there).
//! - While the player aims, a flickering green line shows where the gun
//!   points, to the first thing in the way, up to its range (`view`);
//!   white and pulsing while the energy is too low to fire.
//!
//! Not ported: enemies' guns (they come with the AI) and so teams and
//! the aim help that bends the aim towards the nearest enemy
//! (`UpdateAimMagnet`); the energy the score multiplier gives
//! (`OnScoreManagerAddMult`, no score yet); easy mode's cheaper bullets;
//! `Unaim` (unused); the slow-motion extras along a bullet's trail
//! (`CreateBulletTrailBetween` with `TimeScale < 0.9`) and the sounds'
//! slow-motion pitch (no slow motion yet); the view checks that skip
//! effects off screen (the particle pool takes them all). A ray that
//! starts inside a box hits it at once; Farseer's ray passed out of it.

pub mod model;
mod view;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::car::cabin::{CabinSway, sway_cabins};
use crate::controls::PlayerIntent;
use crate::explosion::{Explosive, Health, Hit};
use crate::game::GameState;
use crate::particles::styles::{BULLET_SPARK, BULLET_SPLASH, BULLET_TRAIL, BULLET_TRAIL_LENGTH, DAMAGE_GLOW, SPARKS};
use crate::render::particles::Particles;
use crate::sound::{CarSounds, LoopCue, PlaySound};
use crate::units::PX_PER_UNIT;
use model::{Aim, Energy, Kind, Minigun, Options, Shot};

pub use view::{AIM_LINE_TEXTURES, energy_bar_alpha};

/// The Minigun's sprite (`Weapons\Minigun`), turned about pixel (9, 5).
pub const MINIGUN_SPRITE: &str = "Weapons\\Minigun";
const MINIGUN_SIZE: Vec2 = Vec2::new(40.0, 11.0);
const MINIGUN_ORIGIN: Vec2 = Vec2::new(9.0, 5.0);

/// How far a bullet flies (`SimulateBullet`: 1024 pixels).
pub const BULLET_REACH: f32 = 1024.0 / PX_PER_UNIT;

/// A car's gun.
#[derive(Component, Debug, Clone)]
pub struct Weapon {
    pub kind: Kind,
    pub options: Options,
    /// Where it sits on the car, car frame (`WeaponRelativePosition`).
    pub mount: Vec2,
    /// From the pivot to the muzzle (`_length`: the sprite's width past
    /// its origin; the gun is drawn unscaled).
    pub length: f32,
    pub aim: Aim,
    pub minigun: Minigun,
    /// The pivot in the car's frame and the barrel's angle from straight
    /// ahead, as last placed (`_worldPosition`, `_worldDirection`).
    pub local: Vec2,
    pub local_angle: f32,
    /// The pivot and the barrel in the world, as last placed.
    pub at: Vec2,
    pub direction: f32,
    /// Aimed this step: the aim line shows (`EnableWeaponLineDraw`).
    pub aiming: bool,
}

impl Weapon {
    /// The gun a `Cars.xml` weapon type names, at `mount` (car frame).
    pub fn new(kind: Kind, options: Options, mount: Vec2, local: bool, rng: &mut impl Rng) -> Weapon {
        Weapon {
            kind,
            options,
            mount,
            length: (MINIGUN_SIZE.x - MINIGUN_ORIGIN.x) / PX_PER_UNIT,
            aim: Aim::new(options.max_angle, local),
            minigun: Minigun::new(&options, rng.random()),
            local: mount,
            local_angle: 0.0,
            at: Vec2::ZERO,
            direction: 0.0,
            aiming: false,
        }
    }

    /// The muzzle in the world (`PiuPosition`).
    pub fn muzzle(&self) -> Vec2 {
        self.at + Vec2::from_angle(self.direction) * self.length
    }

    /// Places the gun on a car at `car` facing `angle`, its cabin moved
    /// by `cabin` (offset and turn in the car's frame).
    fn place(&mut self, car: Vec2, angle: f32, cabin: (Vec2, f32)) {
        let (offset, turn) = cabin;
        let aim = self.aim.angle.current;
        let recoil = Vec2::from_angle(aim) * self.aim.recoil.current / PX_PER_UNIT;
        self.local = offset + Vec2::from_angle(turn).rotate(self.mount) - recoil;
        self.local_angle = aim;
        self.at = car + Vec2::from_angle(angle).rotate(self.local);
        self.direction = angle + aim;
    }
}

/// The player's energy store, on the player's car.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerEnergy(pub Energy);

/// Bullets pass through it (`IgnoreWeaponBox`: `PhysicBodyCategory.LowWall`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct IgnoresWeapons;

#[derive(Resource)]
struct WeaponRng(SmallRng);

/// The weapon systems, for others to order against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WeaponSystems;

pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WeaponRng(SmallRng::seed_from_u64(11)))
            .add_systems(
                FixedUpdate,
                fire_weapons
                    .in_set(WeaponSystems)
                    .after(sway_cabins)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, gun_sounds.run_if(in_state(GameState::Playing)))
            .add_plugins(view::WeaponViewPlugin);
    }
}

/// What bullets fly through: sensors (pads, bombs, teleports, buttons,
/// gears) and low boxes.
pub type PassedByBullets = Or<(With<Sensor>, With<IgnoresWeapons>)>;
/// What a bullet can hurt: whatever has health, and red boxes.
type Hurt = Or<(With<Health>, With<Explosive>)>;

type Gun<'a> = (
    Entity,
    &'a mut Weapon,
    &'a PlayerIntent,
    &'a Position,
    &'a Rotation,
    Option<&'a CabinSway>,
    Option<&'a mut PlayerEnergy>,
);

/// `ControlableEntity.ControlHandle` and `HandleControlEvent`, then the
/// weapon's `Update`, every step the game is not paused.
#[allow(clippy::too_many_arguments)]
fn fire_weapons(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    spatial: SpatialQuery,
    mut guns: Query<Gun>,
    skipped: Query<(), PassedByBullets>,
    mut bodies: Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: Query<Option<&Health>, Hurt>,
    mut rng: ResMut<WeaponRng>,
    mut particles: ResMut<Particles>,
    mut hits: MessageWriter<Hit>,
    mut play: MessageWriter<PlaySound>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    let rng = &mut rng.0;
    for (car, mut gun, intent, position, rotation, sway, mut energy) in &mut guns {
        let angle = rotation.as_radians();
        let gun = &mut *gun;
        if let Some(e) = energy.as_mut() {
            e.0.step(dt);
        }
        gun.aiming = intent.aim_dir != Vec2::ZERO;
        if gun.aiming {
            gun.aim.aim(intent.aim_dir.to_angle(), angle);
        }
        let options = gun.options;
        if intent.fire {
            gun.minigun.pull(&options, energy.as_mut().map(|e| &mut e.0));
        }
        let shot = gun.minigun.step(dt, &options, energy.as_mut().map(|e| &mut e.0), || rng.random());
        match shot {
            Some(Shot::Bullet { impulse_rate, spread }) => {
                let bullet = Bullet {
                    from: gun.at,
                    muzzle: gun.muzzle(),
                    direction: gun.direction + spread * options.dispersion,
                    impulse: options.impulse * impulse_rate,
                    damage: options.damage,
                };
                shoot(bullet, car, &spatial, &skipped, &mut bodies, &hurt, rng, &mut particles, &mut hits, &mut play);
                gun.aim.recoil.current += model::RECOIL;
            }
            Some(Shot::Misfire { pitch }) => {
                play.write(PlaySound {
                    name: "Missfire",
                    at: None,
                    pitch,
                    volume: 0.6,
                });
            }
            None => {}
        }
        let cabin = sway.map_or((Vec2::ZERO, 0.0), |s| s.pose(angle));
        gun.aim.step(dt);
        if sway.is_some() {
            gun.aim.step(dt);
        }
        gun.place(position.0, angle, cabin);
    }
}

/// One bullet (`SimulateMinigunBullet`).
#[derive(Clone, Copy, Debug)]
struct Bullet {
    /// The ray starts at the gun's mount, the trail at its muzzle.
    from: Vec2,
    muzzle: Vec2,
    direction: f32,
    impulse: f32,
    damage: i32,
}

/// `SimulateBullet`, then its flash, its trail and where it struck.
#[allow(clippy::too_many_arguments)]
fn shoot(
    bullet: Bullet,
    shooter: Entity,
    spatial: &SpatialQuery,
    skipped: &Query<(), PassedByBullets>,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
    play: &mut MessageWriter<PlaySound>,
) {
    let dir = Vec2::from_angle(bullet.direction);
    let filter = SpatialQueryFilter::from_excluded_entities([shooter]);
    let hit = Dir2::new(dir)
        .ok()
        .and_then(|d| spatial.cast_ray_predicate(bullet.from, d, BULLET_REACH, true, &filter, &|e| !skipped.contains(e)));
    let point = hit.map_or(bullet.from + dir * BULLET_REACH, |h| bullet.from + dir * h.distance);
    // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
    debug!("shot impulse {:.2} at {:.3} {:.3} hit {:?}", bullet.impulse, point.x, -point.y, hit.map(|h| h.entity));
    let mut wounded = false;
    if let Some(hit) = hit {
        // Pushed along the bullet's line, swung off it by up to 45°.
        if let Ok((body, mut forces)) = bodies.get_mut(hit.entity)
            && body.is_dynamic()
        {
            let along = point - bullet.from;
            let amount = (along + along.perp() * (rng.random::<f32>() * 2.0 - 1.0)).normalize_or_zero();
            forces.apply_linear_impulse_at_point(amount * bullet.impulse, point);
        }
        if let Ok(health) = hurt.get(hit.entity) {
            wounded = true;
            if health.is_some_and(|h| h.hp > 0) {
                particles.add_scaled(&DAMAGE_GLOW, point, 0.0, rng.random::<f32>() * std::f32::consts::TAU, 1.0);
            }
            hits.write(Hit {
                target: hit.entity,
                damage: bullet.damage,
                from: bullet.direction,
            });
        }
    }
    particles.try_add(&BULLET_SPLASH, bullet.muzzle, bullet.direction, bullet.direction + std::f32::consts::PI);
    trail(particles, rng, bullet.muzzle, point, bullet.direction);
    // What can be hurt shows its own damage; the rest sparks and rings.
    if !wounded {
        spark(particles, rng, point, bullet.direction);
        if rng.random::<f32>() > 0.65 {
            play.write(PlaySound {
                name: "Ricochet1",
                at: Some(point),
                pitch: (rng.random::<f32>() * 2.0 - 1.0) * 0.8,
                volume: 0.3 + rng.random::<f32>() * 0.5,
            });
        }
    }
}

/// `CreateBulletTrailBetween`: one streak somewhere along a path longer
/// than a streak.
fn trail(particles: &mut Particles, rng: &mut SmallRng, from: Vec2, to: Vec2, direction: f32) {
    let w = BULLET_TRAIL_LENGTH;
    let d = from.distance(to);
    if d > w {
        let along = rng.random::<f32>() * (d - w) + w / 2.0;
        particles.try_add(&BULLET_TRAIL, from + (to - from) / d * along, direction, direction);
    }
}

/// `CreateSpark` and `CreateCollisionSparks(…, 1)`: a flash facing back
/// up the bullet's path, and a spark flying back off the hit.
fn spark(particles: &mut Particles, rng: &mut SmallRng, at: Vec2, direction: f32) {
    let back = direction + std::f32::consts::PI;
    let flash = back + (rng.random::<f32>() * 2.0 - 1.0) * std::f32::consts::PI / 8.0;
    particles.try_add(&BULLET_SPARK, at, flash, flash);
    let jitter = Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * 0.5;
    let fly = back + (rng.random::<f32>() * 2.0 - 1.0) * std::f32::consts::FRAC_PI_2;
    let style = &SPARKS[rng.random_range(0..2)];
    particles.try_add(style, at + jitter, fly, fly);
}

/// The gun's sound: `Minigun` while it fires, paused between bursts.
fn gun_sounds(mut cars: Query<(&mut CarSounds, Option<&Weapon>)>) {
    for (mut sounds, gun) in &mut cars {
        let cue = gun.filter(|g| g.minigun.active).map(|_| LoopCue {
            name: "Minigun",
            pitch: 0.0,
            volume: 1.0,
        });
        if sounds.gun != cue {
            sounds.gun = cue;
        }
    }
}
