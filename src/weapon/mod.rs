//! The car's gun: the Minigun, the Railgun, the Hammer, the Zombie's and
//! the Plasma gun so far.
//!
//! C# origin: `BaseWeapon`, `BaseComplexWeapon`, `MinigunWeapon`,
//! `RailgunWeapon`, `HammerWeapon`, `PlasmaGun`, `WeaponHelper.SimulateMinigunBullet` /
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
//! - The Hammer is a saw on the car's nose (`HammerWeapon`, [`hammer`]): it
//!   has no gun to aim and no trigger to pull; whatever the car runs into
//!   with it hurts and is shoved. Only the player's fire keys do anything,
//!   and that is to spend 0.1 of the energy a press (`BaseWeapon.TryShoot`
//!   with an empty `OnShoot`). It spares its own team.
//! - The Zombie's weapon ([`zombie`]) has no barrel and nothing to aim: it
//!   drags the first enemy within its reach towards itself (a force of 36),
//!   bites it for 2 every half second it stays near, and its trigger sends
//!   out a wave that stuns every enemy within twice the reach for two
//!   seconds ([`crate::stun`]), a wave a reload (2 s for the player, 10 s
//!   for the enemies) that costs the player's whole energy store.
//! - The Plasma gun ([`plasma`]) fires while the trigger is held, once a
//!   reload (0.5 s, or 1 s for `PlasmaGunSlow`), 0.1 of the energy a shot:
//!   a bullet that flies (red at 32 units a second, or yellow at 16) and
//!   hurts (8) and pushes (18) the first thing it touches. Its shot is not
//!   immediate ([`Kind::is_immediate`]): the attacking AI fires it in the
//!   slow gun's rhythm.
//! - The gun rides the swaying cabin (`DrawCabina` updated it a second
//!   time with the cabin's offset, which also doubled the speed of its
//!   eases; [`Aim::step`] runs twice for cars with a cabin, as there).
//! - While the player aims, a flickering green line shows where the gun
//!   points, to the first thing in the way, up to its range (`view`); the
//!   Railgun's bounces where its beam will. White and pulsing while the
//!   energy is too low to fire; the Railgun's, wide and fading, while it
//!   reloads.
//!
//! - A bullet or beam does not hurt a car of the shooter's own team
//!   (`SimulateBullet`'s `ignoreDamage`): it still pushes it, but no damage
//!   is done or shown, and nothing sparks off it (it is a car).
//!
//! - Enemies' guns (a simple car's, the non-player preset) fire when their
//!   control pulls the trigger ([`crate::ai`] writes `fire` and
//!   `fire_accurate` as the player's devices do): the Minigun's bursts of
//!   ten and its 1.4 s reload, the Railgun's pulse once a 1.5 s reload,
//!   without an energy store (`EnergyComponent` spends nothing for a car
//!   the player does not drive: no [`PlayerEnergy`], so `None` spends
//!   nothing). An enemy aims with [`Aim::aim`](model::Aim::aim) limited to
//!   the gun's `MaxAngle` (the player's turns all round and faster), and
//!   [`Weapon::in_fire_range`] says whether it points at a target
//!   (`TargetInFireRange`). [`max_angle`] narrows a static car's `MaxAngle`
//!   where walls are near (`AdjustWeaponMaxAngle`).
//! - The player's aim help ([`aim_help`]): the aim leans towards the enemy
//!   it points near (`UpdateAimMagnet`).
//!
//! Not ported: the energy the score multiplier gives
//! (`OnScoreManagerAddMult`, no score yet); easy mode's cheaper bullets;
//! `Unaim` (unused); the slow-motion extras along a bullet's trail
//! (`CreateBulletTrailBetween` with `TimeScale < 0.9`) and the sounds'
//! slow-motion pitch (no slow motion yet); the view checks that skip
//! effects off screen (the particle pool takes them all). A ray that
//! starts inside a box hits it at once; Farseer's ray passed out of it.
//!
//! Aim line, not ported: the team check and `IsStatic` of
//! `LaserVisiblityUpdate` (a static or enemy gun shows a red line in C#, a
//! warning of where it points; only the player's green line is drawn here),
//! and the Railgun's bounces drawn for the player only (`ReflectionCount`).
//!
//! Hammer, not ported: `GetHammer(level)`'s damage
//! by level (the enemy spawn director); the view check on the
//! sparks (the particle pool takes them all); the saw staying on a dead car
//! (the wreck takes it and the car's drawn parts away, as with the other
//! guns); the saw's hold on `IsStatic` cars is ported
//! ([`model::hammer_damage`]) but only a dynamic car exists for now.

pub mod aim_help;
pub mod beam;
pub mod hammer;
pub mod max_angle;
pub mod model;
pub mod plasma;
mod view;
pub mod zombie;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::car::cabin::{CabinSway, sway_cabins};
use crate::car::control::{Team, spares};
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
use model::{AimLook, Aim, Energy, Hammer, Kind, Minigun, Options, Plasma, Railgun, Shot, Tracking, Zombie};

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

/// `PlasmaGun`: `Weapons\Plasmagun`, turned about pixel (9, 5).
pub const PLASMA_SPRITE: GunSprite = GunSprite {
    name: "Weapons\\Plasmagun",
    size: Vec2::new(44.0, 11.0),
    origin: Vec2::new(9.0, 5.0),
};

/// `HammerWeapon`: `Weapons\Saw`, round and turned about its middle.
pub const HAMMER_SPRITE: GunSprite = GunSprite {
    name: "Weapons\\Saw",
    size: Vec2::new(128.0, 128.0),
    origin: Vec2::new(64.0, 64.0),
};

/// The sprite of a kind of gun; the Zombie's has none (`ZombieWeapon.Draw`
/// only draws its pull).
pub fn sprite_of(kind: Kind) -> Option<GunSprite> {
    match kind {
        Kind::Minigun => Some(MINIGUN_SPRITE),
        Kind::Railgun => Some(RAILGUN_SPRITE),
        Kind::Hammer => Some(HAMMER_SPRITE),
        Kind::Zombie => None,
        Kind::Plasma => Some(PLASMA_SPRITE),
    }
}

/// Whether the gun shows an aim line (`BaseComplexWeapon`; the Hammer and
/// the Zombie's are plain `BaseWeapon`s).
pub fn has_aim_line(kind: Kind) -> bool {
    kind.is_complex()
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
    /// The saw of a [`Kind::Hammer`]: its size and reach, and the cars it
    /// keeps hurting.
    pub hammer: Hammer,
    pub tracking: Tracking<Entity>,
    /// The Zombie's reload and its target ([`Kind::Zombie`]).
    pub zombie: Zombie<Entity>,
    /// The reload of a [`Kind::Plasma`].
    pub plasma: Plasma,
    /// The saw is turning and hurts (`_isWorking`): its car is alive and
    /// someone drives it.
    pub working: bool,
    /// The pivot in the car's frame and the barrel's angle from straight
    /// ahead, as last placed (`_worldPosition`, `_worldDirection`).
    pub local: Vec2,
    pub local_angle: f32,
    /// The pivot and the barrel in the world, as last placed.
    pub at: Vec2,
    pub direction: f32,
    /// Aimed this step: the aim line shows (`EnableWeaponLineDraw`).
    pub aiming: bool,
    /// The player's aim help ([`aim_help`]).
    pub magnet: aim_help::AimMagnet,
}

impl Weapon {
    /// The gun a `Cars.xml` weapon type names, at `mount` (car frame), on a
    /// car of `car` size (the Hammer sits by it, not by `WeaponPositon`).
    pub fn new(kind: Kind, options: Options, mount: Vec2, car: Vec2, local: bool, rng: &mut impl Rng) -> Weapon {
        // The Hammer's sprite is the only one its size depends on; the
        // Zombie has none, and no barrel.
        let sprite = sprite_of(kind).unwrap_or(HAMMER_SPRITE);
        let hammer = Hammer::new(car, sprite.size.x / sprite.size.y);
        let mount = if kind == Kind::Hammer { hammer.mount } else { mount };
        Weapon {
            kind,
            options,
            mount,
            length: sprite_of(kind).map_or(0.0, |s| (s.size.x - s.origin.x) / PX_PER_UNIT),
            aim: Aim::new(options.max_angle, local),
            minigun: Minigun::new(&options, rng.random()),
            railgun: Railgun::new(&options, rng.random()),
            hammer,
            tracking: Tracking::default(),
            zombie: Zombie::new(&options, rng.random()),
            plasma: Plasma::new(&options, rng.random()),
            working: false,
            local: mount,
            local_angle: 0.0,
            at: Vec2::ZERO,
            direction: 0.0,
            aiming: false,
            magnet: aim_help::AimMagnet::default(),
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
            Kind::Plasma => self.plasma.look(&self.options, energy),
            // No aim line; never asked.
            Kind::Hammer | Kind::Zombie => AimLook::Normal,
        }
    }

    /// `CanShoot` of a gun with no energy store (the AI's): the trigger
    /// would take now. The Hammer has no reload (`_reload` null).
    pub fn can_shoot(&self) -> bool {
        match self.kind {
            Kind::Minigun => self.minigun.can_pull(None),
            Kind::Railgun => self.railgun.can_fire(&self.options, None),
            Kind::Hammer => true,
            Kind::Zombie => self.zombie.reloaded(),
            Kind::Plasma => self.plasma.can_fire(&self.options, None),
        }
    }

    /// The muzzle in the world (`PiuPosition`).
    pub fn muzzle(&self) -> Vec2 {
        self.at + Vec2::from_angle(self.direction) * self.length
    }

    /// `TargetInFireRange`: whether a target (middle and radius) is where
    /// the gun points now and within its range. The Hammer is a plain
    /// `BaseWeapon`: its `PiuPosition` is its mount, not a muzzle.
    pub fn in_fire_range(&self, target: Vec2, radius: f32) -> bool {
        self.fire_range().contains(target, radius)
    }

    /// The numbers `TargetInFireRange` reads, as they are now.
    pub fn fire_range(&self) -> model::FireRange {
        let from = if self.kind == Kind::Hammer { self.at } else { self.muzzle() };
        model::FireRange { from, direction: self.direction, range: self.options.range, dispersion: self.options.dispersion }
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

/// The weapons' gameplay draws, in the fixed steps only.
#[derive(Resource)]
struct WeaponRng(SmallRng);

/// What only the views draw (the energy bar's jolt, the zombie's line), a
/// frame at a time: apart from [`WeaponRng`], or the frame rate would
/// change the fights.
#[derive(Resource)]
struct ViewRng(SmallRng);

/// The weapon systems, for others to order against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WeaponSystems;

pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WeaponRng(SmallRng::seed_from_u64(11)))
            .insert_resource(ViewRng(SmallRng::seed_from_u64(12)))
            .add_systems(
                FixedUpdate,
                fire_weapons
                    .in_set(WeaponSystems)
                    .after(sway_cabins)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                FixedUpdate,
                aim_help::update_aim_magnets
                    .in_set(WeaponSystems)
                    .before(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                FixedUpdate,
                hammer::run_saws
                    .in_set(WeaponSystems)
                    .after(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            // The contacts of the last physics step, before the cars drive
            // and `remember_velocity` forgets the speed going into them.
            .add_systems(
                FixedUpdate,
                hammer::hammer_blows
                    .before(crate::car::drive_cars)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, gun_sounds.run_if(in_state(GameState::Playing)))
            .add_plugins((view::WeaponViewPlugin, max_angle::MaxAnglePlugin, zombie::ZombiePlugin, plasma::PlasmaPlugin));
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
    Has<crate::actions::Inactive>,
);

/// `ControlableEntity.ControlHandle` and `HandleControlEvent`, then the
/// weapon's `Update`, every step the game is not paused.
#[allow(clippy::too_many_arguments)]
fn fire_weapons(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    spatial: SpatialQuery,
    mut guns: Query<Gun>,
    places: Query<&Position>,
    skipped: Query<(), PassedByBullets>,
    mut bodies: Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: Query<Option<&Health>, Hurt>,
    teams: Query<&Team>,
    mut rng: ResMut<WeaponRng>,
    mut particles: ResMut<Particles>,
    mut hits: MessageWriter<Hit>,
    mut play: MessageWriter<PlaySound>,
    mut waves: MessageWriter<zombie::ZombieWave>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    let rng = &mut rng.0;
    let (passes, can_hurt) = (|e| skipped.contains(e), |e| hurt.contains(e));
    let targets = Targets {
        passes: &passes,
        hurt: &can_hurt,
    };
    for (car, mut gun, mut intent, position, rotation, sway, mut energy, inactive) in &mut guns {
        let angle = rotation.as_radians();
        let gun = &mut *gun;
        if let Some(e) = energy.as_mut() {
            e.0.step(dt);
        }
        // The Hammer and the Zombie do not aim (`Aim` is empty).
        gun.aiming = gun.kind.is_complex() && intent.aim_dir != Vec2::ZERO;
        if gun.aiming {
            // The aim help leans the aim towards its candidate (`Aim`).
            gun.magnet.source = intent.aim_dir;
            let aim = match gun.magnet.candidate.and_then(|c| places.get(c).ok()) {
                Some(candidate) => aim_help::pulled(intent.aim_dir, gun.at, candidate.0),
                None => intent.aim_dir,
            };
            gun.aim.aim(aim.to_angle(), angle);
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
                        shoot(bullet, car, &spatial, targets, &mut bodies, &hurt, &teams, rng, &mut particles, &mut hits, &mut play);
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
                    let sound_at = shoot_beam(bullet, options.reflections, car, &spatial, targets, &mut bodies, &hurt, &teams, rng, &mut particles, &mut hits);
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
            // `PlasmaGun.OnShoot`, by `BaseComplexWeapon.TryShoot` while the
            // trigger is held (`ShootMultiple`). Not from an inactive car
            // (`ControlableEntity.Update` is skipped for it; its trigger may
            // still be down from before).
            Kind::Plasma => {
                let fired = !inactive && intent.fire && gun.plasma.pull(&options, energy.as_mut().map(|e| &mut e.0));
                gun.plasma.step(dt);
                if fired {
                    // Set, not added to.
                    gun.aim.recoil.current = model::plasma_recoil(rng.random());
                    let muzzle = gun.muzzle();
                    let team = teams.get(car).ok().copied();
                    for direction in model::plasma_directions(gun.direction, &options, || rng.random()) {
                        // In C#'s axes and degrees, as `/trace`.
                        debug!("plasma shot from {:.3} {:.3} at {:.1}", muzzle.x, -muzzle.y, -direction.to_degrees());
                        commands.spawn(plasma::bullet_bundle(plasma::PlasmaBullet::new(muzzle, direction, car, team, &options)));
                    }
                    play.write(PlaySound {
                        name: "PlasmaShot",
                        at: Some(muzzle),
                        pitch: (rng.random::<f32>() * 2.0 - 1.0) * 0.2,
                        volume: 1.0,
                    });
                }
            }
            // `FireType.Shoot` of either kind reaches `TryShoot`, which spends
            // the energy and then does nothing (`OnShoot` is empty), once a
            // step as the key stays down.
            Kind::Hammer => {
                if (intent.fire || accurate)
                    && let Some(e) = energy.as_mut()
                {
                    e.0.try_spend(options.energy);
                }
            }
            // The same `TryShoot`, a reload and all; its `OnShoot` is the
            // wave ([`zombie::zombie_waves`]).
            Kind::Zombie => {
                gun.zombie.tick(dt);
                if (intent.fire || accurate) && gun.zombie.pull(&options, energy.as_mut().map(|e| &mut e.0)) {
                    waves.write(zombie::ZombieWave { car });
                }
            }
        }
        // The saw does not ride the cabin (its `Update` ignored `deltaPos`).
        let cabin = sway.filter(|_| gun.kind != Kind::Hammer).map_or((Vec2::ZERO, 0.0), |s| s.pose(angle));
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

/// What bullets fly through, and what they can hurt, as [`beam::trace`]
/// asks.
#[derive(Clone, Copy)]
struct Targets<'a> {
    passes: &'a dyn Fn(Entity) -> bool,
    hurt: &'a dyn Fn(Entity) -> bool,
}

impl Targets<'_> {
    /// One leg per bounce, each its full reach, bouncing off anything
    /// (`SimulateBullet`, `SimulateRailgunBullet`).
    fn bullet(&self, spatial: &SpatialQuery, shooter: Entity, bullet: &Bullet, bounces: u32) -> Vec<beam::Leg> {
        let rules = beam::BeamRules {
            reach: beam::Reach::EachLeg(BULLET_REACH),
            bounces,
            stop_at_hurt: false,
        };
        beam::trace(spatial, shooter, self.passes, self.hurt, bullet.muzzle, bullet.from, bullet.direction, rules)
    }
}

/// `SimulateBullet`'s blow where a leg ended: it pushes what it hit at the
/// point along its line, swung off it by up to 45°, and hurts what can be
/// hurt; true then (it shows its own damage rather than sparks).
#[allow(clippy::too_many_arguments)]
fn strike(
    leg: &beam::Leg,
    impulse: f32,
    damage: i32,
    shooter: Entity,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    teams: &Query<&Team>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
) -> bool {
    let point = leg.end;
    // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
    debug!("shot impulse {:.2} at {:.3} {:.3} hit {:?}", impulse, point.x, -point.y, leg.hit.map(|h| h.0));
    let Some((entity, _)) = leg.hit else {
        return false;
    };
    if let Ok((body, mut forces)) = bodies.get_mut(entity)
        && body.is_dynamic()
    {
        let along = point - leg.cast_from;
        let amount = (along + along.perp() * (rng.random::<f32>() * 2.0 - 1.0)).normalize_or_zero();
        forces.apply_linear_impulse_at_point(amount * impulse, point);
    }
    let Ok(health) = hurt.get(entity) else {
        return false;
    };
    // `ignoreDamage`: the shooter's own team is pushed but not hurt, and
    // it is a car all the same (no sparks).
    if spares(teams.get(shooter).ok().copied(), teams.get(entity).ok().copied()) {
        return true;
    }
    if health.is_some_and(|h| h.hp > 0) {
        particles.add_scaled(&DAMAGE_GLOW, point, 0.0, rng.random::<f32>() * std::f32::consts::TAU, 1.0);
    }
    hits.write(Hit {
        target: entity,
        damage,
        from: leg.direction,
        by: Some(shooter),
    });
    true
}

/// `SimulateMinigunBullet`: one leg, its blow, then its flash, its trail
/// and where it struck.
#[allow(clippy::too_many_arguments)]
fn shoot(
    bullet: Bullet,
    shooter: Entity,
    spatial: &SpatialQuery,
    targets: Targets,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    teams: &Query<&Team>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
    play: &mut MessageWriter<PlaySound>,
) {
    let legs = targets.bullet(spatial, shooter, &bullet, 0);
    let leg = &legs[0];
    let wounded = strike(leg, bullet.impulse, bullet.damage, shooter, bodies, hurt, teams, rng, particles, hits);
    let point = leg.end;
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

/// `SimulateRailgunBullet`: a beam of up to `reflections` bounces
/// ([`beam::trace`]: off whatever it hits, things that can be hurt too;
/// the aim line stops at those), each bounce half as damaging and the same
/// push, with a trail along each leg (a bounced one from where its ray
/// starts) and a flash at the muzzle and at every end.
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
    targets: Targets,
    bodies: &mut Query<(&RigidBody, Forces), Without<Weapon>>,
    hurt: &Query<Option<&Health>, Hurt>,
    teams: &Query<&Team>,
    rng: &mut SmallRng,
    particles: &mut Particles,
    hits: &mut MessageWriter<Hit>,
) -> Vec2 {
    let legs = targets.bullet(spatial, shooter, &bullet, reflections);
    particles.try_add(&RAILGUN_SPLASH, bullet.muzzle, bullet.direction, 0.0);
    for (i, leg) in legs.iter().enumerate() {
        let damage = if i == 0 { bullet.damage } else { model::reflected_damage(bullet.damage) };
        strike(leg, bullet.impulse, damage, shooter, bodies, hurt, teams, rng, particles, hits);
        let from = if i == 0 { leg.start } else { leg.cast_from };
        beam_trail(particles, rng, from, leg.end, leg.direction);
        particles.try_add(&RAILGUN_SPLASH, leg.end, leg.direction, 0.0);
    }
    legs.last().filter(|_| legs.len() > 1).map_or(bullet.from, |l| l.cast_from)
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
