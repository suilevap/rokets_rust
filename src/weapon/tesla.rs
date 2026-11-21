//! The Tesla (`TeslaWeapon`, 2013-2015): a coil that strikes every enemy
//! around it at once.
//!
//! C# origin: `TeslaWeapon` (`TryShoot`, `OnShoot`, `Draw` and the dotted
//! circle's `Renderable`), `WeaponOptions.TeslaDefaultOptions` and
//! `DamageConst.Tesla` / `TeslaRange` (the numbers in [`super::model`]),
//! and `ParticleSystemManager.CreateTeslaTrailBetween` /
//! `CreateTeslaTrailStatic` / `CreateTeslaTrailStaticBetween` (the
//! lightning). `git log` of the C# file tells the story: the weapon came
//! with the base weapon's fire distance (June 2013) and struck by itself
//! every update; in October 2013 it began to strike only when its trigger
//! is pulled, together with the `AlwaysShoot` control that pulls it (the
//! Tesla turrets of GarbageCollector, Swarm, LetItSnow, …); then its
//! options (2014), the red dotted circle of its reach ("Tesla dangerous
//! range draw", October 2014) and its two spinning discs (June 2015).
//!
//! - While the trigger is down (either kind of fire, `FireType.Shoot`;
//!   [`super::fire_weapons`]) the coil is armed and crackles now and then
//!   ([`TeslaCrackle`]); once a reload (0.25 s, the first a random part of
//!   it) and if the player's energy pays (0.1), it strikes
//!   ([`TeslaStrike`]).
//! - A strike ([`tesla_strikes`]) hits every car (`ControlableEntities`)
//!   alive, of another team and closer than the range (8) to the coil: 8
//!   damage from the line between the cars' middles, and a push of 5 along
//!   it at a random point of the struck car's square (its radius each way)
//!   if it is a dynamic body; a bolt joins coil and car ([`TeslaBolt`]).
//!   There is no friendly fire, no wall in the way and no limit of
//!   targets, and boxes are left alone (only cars are struck).
//! - The crackle: one time in twenty a step the trigger is down, either
//!   (one time in four) a flash at the coil and a bolt wandering from it
//!   out to the range, or a single spark shooting off along a random line
//!   from somewhere between 2 and 6 units out (a quarter of them back
//!   inwards).
//! - A bolt is a chain of still cyan links (2 units each) from one end
//!   towards the other, each turned up to a quarter turn off the straight
//!   line, ending in the Railgun's flash; a strike draws one each way.
//! - Two discs spin on the coil over the car (`Entities\TeslaGenInner`
//!   once a second one way, `TeslaGenOuter` once every two seconds the
//!   other way, on the game's clock), and once the coil has been armed a
//!   red dotted circle the size of its reach pulses under the cars (a
//!   fifth to three tenths opaque, once a second) until it dies.
//!
//! Not ported:
//! - The view checks (`Camera.IsInView`) that skip the crackle, the bolts
//!   and the circle off screen: the particle pool takes them all, and the
//!   circle is drawn wherever it is.
//! - The bolts' walks are cut after [`MAX_LINKS`] links; C#'s loops had no
//!   bound (a random walk that wanders off for long would have run on).
//! - The circle's height: C#'s `CarBase + 0.01` sits just under the car's
//!   base; it is drawn with the saw's layer ([`Layer::Hammer`]), the
//!   nearest.
//! - The strike's coil is the car's physics position and the mount
//!   (`PiuPosition` used the drawn `FakePosition`); the crackle and bolts
//!   start there too. The circle follows the drawn car.
//! - An [`Inactive`](crate::actions::Inactive) car does not fire (as the
//!   Plasma gun's); C# skipped its update the same way. A deactivated car
//!   is still struck, as in C# (it stays among the `ControlableEntities`);
//!   it is not pushed, its body being out of the physics.
//! - `CreateTesla2` and `_teslaStyle2` (the commented-out glow of the
//!   first version), and the commented-out `TryShoot` in `Update`.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use super::model::{self, Kind};
use super::{ViewRng, Weapon, WeaponSystems, fire_weapons};
use crate::actions::Inactive;
use crate::car::CarRadius;
use crate::car::control::Team;
use crate::explosion::{Health, Hit};
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::particles::styles::{RAILGUN_SPLASH, TESLA_LINK_LENGTH, TESLA_TRAIL, TESLA_TRAIL_LENGTH, TESLA_TRAIL_STATIC};
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// The disc that spins once a second (`_teslaGeneratorInner`).
const INNER: &str = "Entities\\TeslaGenInner";
/// The disc that spins the other way once in two (`_teslaGeneratorOuter`).
const OUTER: &str = "Entities\\TeslaGenOuter";
/// The reach's circle (`_dottedCircle`): its radius is half the texture.
const CIRCLE: &str = "UI\\DottedCircle";

/// What the Tesla draws on and around the car, to load with the map.
pub const TEXTURES: [&str; 3] = [INNER, OUTER, CIRCLE];

/// `RandomEvent(0.05f)`: a step's chance of a crackle.
pub const CRACKLE_CHANCE: f32 = 0.05;
/// `RandomEvent(0.25f)`: of a crackle, the chance of a flash and a bolt
/// out to the range rather than a spark.
pub const CRACKLE_BOLT_CHANCE: f32 = 0.25;
/// `CreateTeslaTrailBetween`'s `RandomEvent(0.25f)`: a spark flies back.
pub const SPARK_BACK_CHANCE: f32 = 0.25;
/// `MathHelper.PI / 2`: how far a bolt's link turns off its way.
pub const BOLT_SPREAD: f32 = FRAC_PI_2;
/// The most links a bolt is walked for (C# had no bound).
pub const MAX_LINKS: usize = 64;

/// The circle's opacity: `0.1 × Triangle_Period500 + 0.2`.
const CIRCLE_ALPHA: (f32, f32) = (0.2, 0.1);
/// `MathHelper.PI / 4f`, C#'s turn of the circle (ours the other way).
const CIRCLE_TURN: f32 = -FRAC_PI_4;
/// Seconds a turn of the discs (`Sawtooth_Period1000`, `GetSawtoothPhase(2000)`).
const INNER_PERIOD: f32 = 1.0;
const OUTER_PERIOD: f32 = 2.0;

/// The coil strikes (`TeslaWeapon.OnShoot`): sent by the weapon's trigger
/// ([`super::fire_weapons`]) from `car`, its coil at `at`.
#[derive(Message, Debug, Clone, Copy)]
pub struct TeslaStrike {
    pub car: Entity,
    pub at: Vec2,
}

/// The trigger was pulled (`TeslaWeapon.TryShoot`): for the crackle, a
/// view's matter only.
#[derive(Message, Debug, Clone, Copy)]
pub struct TeslaCrackle {
    pub at: Vec2,
    pub range: f32,
}

/// A strike's bolt from the coil to a struck car, for the view.
#[derive(Message, Debug, Clone, Copy)]
pub struct TeslaBolt {
    pub from: Vec2,
    pub to: Vec2,
}

pub struct TeslaPlugin;

impl Plugin for TeslaPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TeslaStrike>()
            .add_message::<TeslaCrackle>()
            .add_message::<TeslaBolt>()
            .add_systems(
                FixedUpdate,
                tesla_strikes
                    .in_set(WeaponSystems)
                    .after(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (spawn_tesla_views, place_tesla_views, draw_lightning)
                    .chain()
                    .after(WeaponSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// `TeslaWeapon.OnShoot`: every car alive, of another team, within reach
/// of the coil is hurt and pushed, with a bolt each. The cars are taken
/// in the order of their map ids (`ControlableEntities`' order), so the
/// random points of the pushes fall the same each run.
#[allow(clippy::type_complexity)]
pub fn tesla_strikes(
    mut strikes: MessageReader<TeslaStrike>,
    owners: Query<(&Weapon, &Position, &Team)>,
    cars: Query<(Entity, &MapId, &Position, &CarRadius, &Team, &Health, Has<Inactive>)>,
    mut bodies: Query<(&RigidBody, Forces)>,
    mut hits: MessageWriter<Hit>,
    mut bolts: MessageWriter<TeslaBolt>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(43));
    for strike in strikes.read() {
        let Ok((gun, car_at, team)) = owners.get(strike.car) else {
            continue;
        };
        if gun.kind != Kind::Tesla {
            continue;
        }
        let options = gun.options;
        let mut struck: Vec<_> = cars
            .iter()
            .filter(|(_, _, at, _, other, hp, _)| {
                hp.hp > 0 && other.0 != team.0 && model::tesla_reaches(strike.at, at.0, &options)
            })
            .collect();
        struck.sort_by_key(|(_, id, ..)| id.0);
        for (target, _, at, radius, _, _, inactive) in struck {
            // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
            debug!("tesla strike on {target:?} at {:.3} {:.3}", at.0.x, -at.0.y);
            hits.write(Hit {
                target,
                damage: options.damage,
                from: (at.0 - car_at.0).to_angle(),
                by: Some(strike.car),
            });
            let point = at.0 + Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * radius.0;
            if !inactive
                && let Ok((body, mut forces)) = bodies.get_mut(target)
                && body.is_dynamic()
            {
                forces.apply_linear_impulse_at_point(model::tesla_push(car_at.0, at.0, &options), point);
            }
            bolts.write(TeslaBolt { from: strike.at, to: at.0 });
        }
    }
}

/// One still link of a bolt: its middle and its world angle.
pub type Link = (Vec2, f32);

/// `CreateTeslaTrailStaticBetween`: a bolt from `from` towards `to`, link
/// by link, each aimed at `to` and turned within ± `spread` (`random`
/// gives 0..1), until within a link of it. Returns the links, and where
/// and along which angle the walk ended (for the flash).
pub fn bolt_between(from: Vec2, to: Vec2, spread: f32, mut random: impl FnMut() -> f32) -> (Vec<Link>, Vec2, f32) {
    let mut at = from;
    let mut angle = (to - from).to_angle();
    let mut links = Vec::new();
    while at.distance_squared(to) >= TESLA_LINK_LENGTH * TESLA_LINK_LENGTH && links.len() < MAX_LINKS {
        angle = (to - at).to_angle() + (random() * 2.0 - 1.0) * spread;
        let along = Vec2::from_angle(angle) * TESLA_LINK_LENGTH / 2.0;
        at += along;
        links.push((at, angle));
        at += along;
    }
    (links, at, angle)
}

/// `CreateTeslaTrailStatic`: a bolt wandering out of `from`, first along
/// `angle`, each next link turned within ± `spread` of the last, until it
/// is `reach` away. Returns the links, and where and along which angle the
/// walk ended.
pub fn bolt_out(from: Vec2, mut angle: f32, spread: f32, reach: f32, mut random: impl FnMut() -> f32) -> (Vec<Link>, Vec2, f32) {
    let mut at = from;
    let mut links = Vec::new();
    while from.distance_squared(at) < reach * reach && links.len() < MAX_LINKS {
        let along = Vec2::from_angle(angle) * TESLA_LINK_LENGTH / 2.0;
        at += along;
        links.push((at, angle));
        at += along;
        angle += (random() * 2.0 - 1.0) * spread;
    }
    (links, at, angle)
}

/// `CreateTeslaTrailBetween(center, center + delta × range, dir)`: where a
/// spark of the crackle starts on a line of `length` out of `from` along
/// `angle`: a random point (`random` 0..1) from half a spark to half a
/// spark short of the end; none on a line no longer than a spark.
pub fn spark_on_line(from: Vec2, angle: f32, length: f32, random: f32) -> Option<Vec2> {
    let w = TESLA_TRAIL_LENGTH;
    (length > w).then(|| from + Vec2::from_angle(angle) * (random * (length - w) + w / 2.0))
}

fn add_bolt(particles: &mut Particles, (links, end, angle): (Vec<Link>, Vec2, f32)) {
    for (at, a) in links {
        particles.try_add(&TESLA_TRAIL_STATIC, at, 0.0, a);
    }
    // `CreateRailgunSplash(pos, dir0)`.
    particles.try_add(&RAILGUN_SPLASH, end, angle, 0.0);
}

/// The crackle while the trigger is down (`TeslaWeapon.TryShoot`) and the
/// strikes' bolts, a frame at a time, with the views' random numbers.
fn draw_lightning(
    mut crackles: MessageReader<TeslaCrackle>,
    mut bolts: MessageReader<TeslaBolt>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<ViewRng>,
) {
    let rng = &mut rng.0;
    for crackle in crackles.read() {
        if rng.random::<f32>() >= CRACKLE_CHANCE {
            continue;
        }
        let center = crackle.at;
        if rng.random::<f32>() < CRACKLE_BOLT_CHANCE {
            particles.try_add(&RAILGUN_SPLASH, center, 0.0, 0.0);
            let start = rng.random::<f32>() * TAU;
            add_bolt(&mut particles, bolt_out(center, start, BOLT_SPREAD, crackle.range, || rng.random()));
        } else {
            // `NextVector2Fake` then `SafeNormalize`: a direction out of
            // a square (more often along a diagonal), none for nought.
            let delta = Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0).normalize_or_zero();
            if delta == Vec2::ZERO {
                continue;
            }
            let mut angle = delta.to_angle();
            if let Some(at) = spark_on_line(center, angle, crackle.range, rng.random()) {
                if rng.random::<f32>() < SPARK_BACK_CHANCE {
                    angle += std::f32::consts::PI;
                }
                particles.try_add(&TESLA_TRAIL, at, angle, angle);
            }
        }
    }
    for bolt in bolts.read() {
        add_bolt(&mut particles, bolt_between(bolt.from, bolt.to, BOLT_SPREAD, || rng.random()));
        add_bolt(&mut particles, bolt_between(bolt.to, bolt.from, BOLT_SPREAD, || rng.random()));
    }
}

/// The discs on a Tesla's car: their pivot, a child of the car.
#[derive(Component)]
struct TeslaDiscs;

/// One disc: the outer one turns the other way, slower.
#[derive(Component)]
struct Disc {
    outer: bool,
}

/// The circle of a Tesla's reach, on its own (it does not turn with the
/// car).
#[derive(Component)]
struct ReachCircle {
    owner: Entity,
}

/// The discs on the car and the circle beside it, for every Tesla that
/// has none yet.
#[allow(clippy::type_complexity)]
fn spawn_tesla_views(
    mut commands: Commands,
    guns: Query<(Entity, &Weapon, Option<&Children>)>,
    discs: Query<(), With<TeslaDiscs>>,
    circles: Query<&ReachCircle>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lit = SpriteTint { color: [1.0; 4], lit: true };
    for (car, gun, children) in &guns {
        if gun.kind != Kind::Tesla {
            continue;
        }
        if !children.is_some_and(|c| c.iter().any(|e| discs.contains(e)))
            && let (Some(inner), Some(outer)) = (map.sprites.get(INNER), map.sprites.get(OUTER))
        {
            let pivot = commands
                .spawn((Name::new("Tesla discs"), TeslaDiscs, Transform::default(), Visibility::default()))
                .with_children(|p| {
                    for (info, outer) in [(inner, false), (outer, true)] {
                        p.spawn((Disc { outer }, sprite_bundle(info, 1.0, Layer::Weapon, lit, &quad, &mut materials)));
                    }
                })
                .id();
            commands.entity(car).add_child(pivot);
        }
        if !circles.iter().any(|c| c.owner == car)
            && let Some(info) = map.sprites.get(CIRCLE)
        {
            let tint = SpriteTint { color: [1.0, 0.0, 0.0, CIRCLE_ALPHA.0], lit: true };
            commands.spawn((
                Name::new("Tesla reach"),
                MapEntity,
                ReachCircle { owner: car },
                Visibility::Hidden,
                sprite_bundle(info, 1.0, Layer::Hammer, tint, &quad, &mut materials),
            ));
        }
    }
}

/// `TeslaWeapon.Draw` and the circle's `Renderable`: the discs spin on
/// the coil; the circle, once armed, lies round the coil, as wide as the
/// reach, pulsing; it goes with the gun (the car's death).
#[allow(clippy::type_complexity)]
fn place_tesla_views(
    mut commands: Commands,
    time: Res<Time>,
    guns: Query<(&Weapon, &Transform), (Without<TeslaDiscs>, Without<Disc>, Without<ReachCircle>)>,
    mut pivots: Query<(&ChildOf, &mut Transform), (With<TeslaDiscs>, Without<Disc>, Without<ReachCircle>)>,
    mut discs: Query<(&Disc, &mut Transform), (Without<TeslaDiscs>, Without<ReachCircle>)>,
    mut circles: Query<(Entity, &ReachCircle, &mut Transform, &mut Visibility, &mut SpriteTint), (Without<TeslaDiscs>, Without<Disc>)>,
) {
    let t = time.elapsed_secs();
    for (parent, mut pivot) in &mut pivots {
        if let Ok((gun, _)) = guns.get(parent.parent()) {
            pivot.translation = gun.local.extend(0.0);
        }
    }
    // `entityDir + Sawtooth × 2π` and `entityDir + (1 − Sawtooth) × 2π`,
    // C#'s clockwise negated; the car's turn comes from the parent.
    let inner = -(t % INNER_PERIOD) / INNER_PERIOD * TAU;
    let outer = (t % OUTER_PERIOD) / OUTER_PERIOD * TAU;
    for (disc, mut transform) in &mut discs {
        transform.rotation = Quat::from_rotation_z(if disc.outer { outer } else { inner });
    }
    for (entity, circle, mut transform, mut visibility, mut tint) in &mut circles {
        let Some((gun, car)) = guns.get(circle.owner).ok().filter(|(g, _)| g.kind == Kind::Tesla) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !gun.tesla.armed {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let at = car.translation.truncate() + (car.rotation * gun.mount.extend(0.0)).truncate();
        let diameter = gun.options.range * 2.0;
        transform.translation = at.extend(Layer::Hammer.z());
        transform.rotation = Quat::from_rotation_z(CIRCLE_TURN);
        transform.scale = Vec3::new(diameter, diameter, 1.0);
        tint.color[3] = CIRCLE_ALPHA.0 + CIRCLE_ALPHA.1 * super::view::triangle(t);
        visibility.set_if_neq(Visibility::Inherited);
    }
}
