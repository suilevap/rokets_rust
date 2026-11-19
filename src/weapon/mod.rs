//! The car's gun: the Minigun and the Railgun so far.
//!
//! C# origin: `BaseWeapon`, `BaseComplexWeapon`, `MinigunWeapon`,
//! `RailgunWeapon`, `WeaponHelper.SimulateMinigunBullet` /
//! `SimulateRailgunBullet` / `SimulateBullet`, `WeaponFactory`,
//! `EnergyComponent` and the fire input of the `PlayerCarControl*`
//! classes (2010–2016). The numbers and the timing are in [`model`]; this
//! is where they meet the world:
//!
//! - A car from `Cars.xml` with a `Weapon` gets a [`Weapon`] at its
//!   `WeaponPositon`, the player preset when the player drives it, and
//!   the player's car an energy store ([`PlayerEnergy`]).
//! - Every step: the aim turns towards [`PlayerIntent::aim_dir`] (the
//!   mouse, the arrows, the right stick, a drag on the fire zone), the
//!   Minigun's trigger is pulled while [`PlayerIntent::fire`] is held, and
//!   each bullet is a ray from the gun's mount ([`strike`]): it pushes what
//!   it hits at the point, off the line by up to 45°, damages what can be
//!   hurt ([`Hit`]), and flashes ([`shoot`]). The gun kicks back 2 pixels
//!   a bullet.
//! - The Railgun fires only on [`PlayerIntent::fire_accurate`]
//!   (`_fireOnlyAccurate`, `FireType.ShootAccurate`): one beam a reload
//!   (1.5 s) that costs 0.6 of the energy, a ray of 25 damage and a big
//!   push that bounces once off what it hits ([`shoot_beam`],
//!   `SimulateRailgunBullet`; [`model::reflect`]), the bounce doing half
//!   the damage, and kicks the gun back 10 pixels.
//! - The gun rides the swaying cabin (`DrawCabina` updated it a second
//!   time with the cabin's offset, which also doubled the speed of its
//!   eases; [`Aim::step`] runs twice for cars with a cabin, as there).
//! - While the player aims, a flickering green line shows where the gun
//!   points, to the first thing in the way, up to its range (`view`); the
//!   Railgun's bounces where its beam will. White and pulsing while the
//!   energy is too low to fire; the Railgun's, wide and fading, while it
//!   reloads.
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
//!
//! Railgun, not ported: damage not sparing one's own team (no teams); the team check and `IsStatic` of the aim
//! line (enemies' guns).

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
use crate::particles::styles::{
    BULLET_SPARK, BULLET_SPLASH, BULLET_TRAIL, BULLET_TRAIL_LENGTH, DAMAGE_GLOW, RAILGUN_SPLASH, RAILGUN_TRAIL,
    RAILGUN_TRAIL_SPACING, SPARKS,
};
use crate::render::particles::Particles;
use crate::sound::{CarSounds, LoopCue, PlaySound};
use crate::units::PX_PER_UNIT;
use model::{AimLook, Aim, Energy, Kind, Minigun, Options, Railgun, Shot};

pub use view::{AIM_LINE_TEXTURES, energy_bar_alpha};

/// A gun's sprite and the pixel it turns about (the view's `Origin`).
#[derive(Clone, Copy, Debug)]
pub struct GunSprite {
    pub name: &'static str,
    /// C# pixels.
    pub size: Vec2,
    pub origin: Vec2,
}

/// `MinigunWeapon`: `Weapons\Minigun`, turned about pixel (9, 5).
pub const MINIGUN_SPRITE: GunSprite = GunSprite {
    name: "Weapons\\Minigun",
    size: Vec2::new(40.0, 11.0),
    origin: Vec2::new(9.0, 5.0),
};
/// `RailgunWeapon`: `Weapons\Railgun`, turned about pixel (6, 6).
pub const RAILGUN_SPRITE: GunSprite = GunSprite {
    name: "Weapons\\Railgun",
    size: Vec2::new(42.0, 13.0),
    origin: Vec2::new(6.0, 6.0),
};

/// The sprite of a kind of gun.
pub fn sprite_of(kind: Kind) -> GunSprite {
    match kind {
        Kind::Minigun => MINIGUN_SPRITE,
        Kind::Railgun => RAILGUN_SPRITE,
    }
}

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
    /// The firing rhythm of a [`Kind::Minigun`] and of a [`Kind::Railgun`]
    /// (the other is never used).
    pub minigun: Minigun,
    pub railgun: Railgun,
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
            length: {
                let sprite = sprite_of(kind);
                (sprite.size.x - sprite.origin.x) / PX_PER_UNIT
            },
            aim: Aim::new(options.max_angle, local),
            minigun: Minigun::new(&options, rng.random()),
            railgun: Railgun::new(&options, rng.random()),
            local: mount,
            local_angle: 0.0,
            at: Vec2::ZERO,
            direction: 0.0,
            aiming: false,
        }
    }

    /// How the aim line looks now (`UpdateLaser`); asking the energy sets
    /// its last-asked mark, as in C#.
    pub fn aim_look(&self, energy: Option<&mut Energy>) -> AimLook {
        match self.kind {
            // Low while it cannot fire and is not firing.
            Kind::Minigun => {
                if !self.minigun.can_pull(energy) && !self.minigun.active {
                    AimLook::Low
                } else {
                    AimLook::Normal
                }
            }
            Kind::Railgun => self.railgun.look(&self.options, energy),
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
    &'a mut PlayerIntent,
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
    for (car, mut gun, mut intent, position, rotation, sway, mut energy) in &mut guns {
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
        // The pulse is taken (`Control.Fire &= ~FireType.Shoot`), changing
        // the intent only when there is one.
        let accurate = intent.fire_accurate;
        if accurate {
            intent.fire_accurate = false;
        }
        match gun.kind {
            Kind::Minigun => {
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
            }
            Kind::Railgun => {
                let fired = accurate && gun.railgun.pull(&options, energy.as_mut().map(|e| &mut e.0));
                gun.railgun.step(dt);
                if fired {
                    let bullet = Bullet {
                        from: gun.at,
                        muzzle: gun.muzzle(),
                        direction: gun.direction,
                        impulse: options.impulse,
                        damage: options.damage,
                    };
                    let sound_at = shoot_beam(bullet, options.reflections, car, &spatial, &skipped, &mut bodies, &hurt, rng, &mut particles, &mut hits);
                    // Set, not added to: the Railgun kicks the same each time.
                    gun.aim.recoil.current = model::RAILGUN_RECOIL;
                    play.write(PlaySound {
                        name: "Railgun",
                        at: Some(sound_at),
                        pitch: 0.0,
                        volume: 1.0,
                    });
                }
            }
        }
        let cabin = sway.map_or((Vec2::ZERO, 0.0), |s| s.pose(angle));
        gun.aim.step(dt);
        if sway.is_some() {
            gun.aim.step(dt);
        }
        gun.place(position.0, angle, cabin);
    }
}

/// One bullet (`SimulateMinigunBullet`, `SimulateRailgunBullet`).
#[derive(Clone, Copy, Debug)]
struct Bullet {
    /// The ray starts at the gun's mount, the trail at its muzzle.
    from: Vec2,
    muzzle: Vec2,
    direction: f32,
    impulse: f32,
    damage: i32,
}

/// Where a bullet ended and what it did there.
#[derive(Clone, Copy, Debug)]
struct Strike {
    point: Vec2,
    /// The surface's normal, pointing out of it; zero when it hit nothing
    /// (or started inside).
    normal: Vec2,
    /// It hit something that can be hurt (and so shows its own damage).
    wounded: bool,
}

/// `SimulateBullet`: a ray from `from` along `direction`: it pushes what
/// it hits at the point along its line, swung off it by up to 45°, and
/// hurts what can be hurt.
#[allow(clippy::too_many_arguments)]
fn strike(
    from: Vec2,
    direction: f32,
    impulse: f32,
    damage: i32,
    shooter: Entity,
    spatial: &SpatialQuery,
    skipped: &Query<(), PassedByBullets>,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
) -> Strike {
    let dir = Vec2::from_angle(direction);
    let filter = SpatialQueryFilter::from_excluded_entities([shooter]);
    let hit = Dir2::new(dir)
        .ok()
        .and_then(|d| spatial.cast_ray_predicate(from, d, BULLET_REACH, true, &filter, &|e| !skipped.contains(e)));
    let point = hit.map_or(from + dir * BULLET_REACH, |h| from + dir * h.distance);
    // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
    debug!("shot impulse {:.2} at {:.3} {:.3} hit {:?}", impulse, point.x, -point.y, hit.map(|h| h.entity));
    let mut wounded = false;
    if let Some(hit) = hit {
        // Pushed along the bullet's line, swung off it by up to 45°.
        if let Ok((body, mut forces)) = bodies.get_mut(hit.entity)
            && body.is_dynamic()
        {
            let along = point - from;
            let amount = (along + along.perp() * (rng.random::<f32>() * 2.0 - 1.0)).normalize_or_zero();
            forces.apply_linear_impulse_at_point(amount * impulse, point);
        }
        if let Ok(health) = hurt.get(hit.entity) {
            wounded = true;
            if health.is_some_and(|h| h.hp > 0) {
                particles.add_scaled(&DAMAGE_GLOW, point, 0.0, rng.random::<f32>() * std::f32::consts::TAU, 1.0);
            }
            hits.write(Hit {
                target: hit.entity,
                damage,
                from: direction,
            });
        }
    }
    Strike {
        point,
        normal: hit.map_or(Vec2::ZERO, |h| h.normal),
        wounded,
    }
}

/// `SimulateMinigunBullet`: [`strike`], then its flash, its trail and
/// where it struck.
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
    let hit = strike(bullet.from, bullet.direction, bullet.impulse, bullet.damage, shooter, spatial, skipped, bodies, hurt, rng, particles, hits);
    let point = hit.point;
    particles.try_add(&BULLET_SPLASH, bullet.muzzle, bullet.direction, bullet.direction + std::f32::consts::PI);
    trail(particles, rng, bullet.muzzle, point, bullet.direction);
    // What can be hurt shows its own damage; the rest sparks and rings.
    if !hit.wounded {
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

/// How far off the surface a bounced beam starts (`normal * 0.1f`).
const BOUNCE_OFFSET: f32 = 0.1;

/// `SimulateRailgunBullet`: [`strike`], then up to `reflections` times
/// bounced off what it hit (not at a head-on blow, [`model::reflect`]),
/// each bounce half as damaging and the same push, with a trail along each
/// leg and a flash at the muzzle and at every end. Bounces off whatever it
/// hits, things that can be hurt too; the aim line stops at those.
///
/// Returns where the shot is heard: the gun, or where it last bounced
/// (C# passed `_worldPosition` by reference as the start of each leg and
/// played the sound there after).
#[allow(clippy::too_many_arguments)]
fn shoot_beam(
    bullet: Bullet,
    reflections: u32,
    shooter: Entity,
    spatial: &SpatialQuery,
    skipped: &Query<(), PassedByBullets>,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
) -> Vec2 {
    let mut heard_at = bullet.from;
    let mut direction = bullet.direction;
    let mut hit = strike(bullet.from, direction, bullet.impulse, bullet.damage, shooter, spatial, skipped, bodies, hurt, rng, particles, hits);
    beam_trail(particles, rng, bullet.muzzle, hit.point, direction);
    particles.try_add(&RAILGUN_SPLASH, bullet.muzzle, direction, 0.0);
    particles.try_add(&RAILGUN_SPLASH, hit.point, direction, 0.0);
    for _ in 0..reflections {
        let Some(bounced) = model::reflect(direction, hit.normal) else {
            break;
        };
        direction = bounced;
        let start = hit.point + hit.normal * BOUNCE_OFFSET;
        heard_at = start;
        let damage = model::reflected_damage(bullet.damage);
        hit = strike(start, direction, bullet.impulse, damage, shooter, spatial, skipped, bodies, hurt, rng, particles, hits);
        beam_trail(particles, rng, start, hit.point, direction);
        particles.try_add(&RAILGUN_SPLASH, hit.point, direction, 0.0);
    }
    heard_at
}

/// `CreateRailgunTrailBetween`: a spark every 1.25 units along the path,
/// each drifting off within 45° of the beam, one of two spins.
fn beam_trail(particles: &mut Particles, rng: &mut SmallRng, from: Vec2, to: Vec2, direction: f32) {
    let d = from.distance(to);
    if d <= 0.0 {
        return;
    }
    let along = (to - from) / d;
    let mut i = 0.0;
    while i < d / RAILGUN_TRAIL_SPACING {
        let drift = direction + (rng.random::<f32>() * 2.0 - 1.0) * std::f32::consts::FRAC_PI_4;
        let style = &RAILGUN_TRAIL[rng.random_range(0..2)];
        particles.try_add(style, from + along * RAILGUN_TRAIL_SPACING * i, drift, direction);
        i += 1.0;
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
        let cue = gun.filter(|g| g.kind == Kind::Minigun && g.minigun.active).map(|_| LoopCue {
            name: "Minigun",
            pitch: 0.0,
            volume: 1.0,
        });
        if sounds.gun != cue {
            sounds.gun = cue;
        }
    }
}
