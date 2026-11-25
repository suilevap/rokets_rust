//! The rocket launchers' rockets (`RocketLauncherWeapon`, `BaseRoket`,
//! `SmallRoket`, `GoalRoket`, `HeavyRoket`, 2010-2016): shots that fly,
//! speed up, home in on a moving enemy and blow up where they end.
//!
//! C# origin: `RocketLauncherWeapon.OnShoot` (fired from
//! [`super::fire_weapons`]), `EntitiesManager.CreateRoket` (the numbers of
//! a launch), `BaseRoket` (the body, its timers, `OnCollide`, `Destroy`),
//! `GoalRoket` (the homing), `SmallRoket` (the trail), `HeavyRoket` (the
//! smoke), `DamageConst` (`SmallRocket`, `HeavyRocket`,
//! `RocketExplosionRange`), `WeaponHelper.CreateExplosion` (the blast, our
//! [`crate::explosion`]), `ParticleSystemManager.CreatePiu` and
//! `CreateSmoke`, and the numbers in [`super::model`]
//! (`RocketLauncherDefaultOptions`, `RocketLauncher360DefaultOptions`).
//! `git log` of the C# files: `BaseRoket` came first (2010) as a force-driven
//! body that blew up on touch, `GoalRoket` gave it a goal, `HeavyRoket`
//! made the chasing one and `SmallRoket` the straight one with its trail;
//! the launcher fires only on accurate pulls (`_fireOnlyAccurate`, as the
//! Railgun) and its shot is not immediate (`IsImmidiate = false`), so the
//! attacking AI fires it in the slow gun's rhythm.
//!
//! - A shot (an accurate pulse, once a reload: 1.2 s; 0.1 of the player's
//!   energy, or 0.5 for the 360 launcher) kicks the gun back 10 pixels,
//!   blows two black puffs out sideways at the muzzle, plays `RocketLaunch`
//!   at the gun and sends a [`Rocket`] from the muzzle along the barrel.
//!   `RocketlauncherDirect` sends `SmallRoket`s (blast 16), the two
//!   `RocketlauncherChasing*` `HeavyRoket`s (blast 20, homing).
//! - A rocket is a body 1 by 0.27 units (`Weapons\roket` at scale 0.5, a
//!   rectangle of density 1: a mass of 0.27) with no friction that
//!   flies through what bullets fly through (sensors and low boxes:
//!   `PhysicBodyCategory.WeaponCollidesWith` leaves out `LowWall` and
//!   `Bomb`) and its own shooter (`Fix.IgnoreCollisionWith`). It is born
//!   at the speed it really flew at in C#: the top speed ([`MAX_SPEED`], 47
//!   units a second) along the barrel plus a tenth of the shooter's velocity
//!   (`InitImpulse`), cut to the top speed ([`launch_velocity`]); it points
//!   where that points. C# gave it an impulse of 64 (237 units a second on
//!   its mass), which Farseer's `MaxTranslation` cut to 2 units a step and
//!   `NormalizeSpeed` to 47 at the end of its first update.
//! - For its first 0.1 s it only coasts ([`ACTIVATION`]). Then it is
//!   active: every step a force of 32 along its force direction
//!   ([`STEP_FORCE`], 118 units a second² on its mass, against a damping of
//!   0.95: [`integrate`]) keeps it at its top speed, and its turning is
//!   locked. A `SmallRoket`'s force direction is where it was fired; a
//!   homing rocket points it at its goal ([`steer`]), the nearest car of
//!   another team that is moving (speed over 0.3 units a second; it never
//!   chases a turret), chosen when it becomes active and again every
//!   second ([`nearest_goal`]), and turns to face where it flies. The
//!   turn it can make is what 118 units a second² does to a speed of 47:
//!   about 2.5 radians a second, a wide arc.
//! - After 2 s active it blows up where it is ([`LIFETIME`]; `DeactivationTime`
//!   of 8 s was "not used?").
//! - What it touches (its rectangle swept along the step, [`fly`]) stops
//!   it dead, and the next step it blows up there (`SheduleDestroy`): an
//!   [`Explosion`] of range 7, force 0.5 and its damage, with the shooter
//!   as its owner, so the shooter's team is spared (`CreateExplosion`:
//!   [`BLAST_RANGE`], [`BLAST_FORCE`]). The blast's flash, smoke, sound
//!   and the camera's shake are [`crate::explosion`]'s. A blast (or any
//!   other [`Hit`]) that reaches a rocket sets it off the same way
//!   (`TakeDamage` is a `SheduleDestroy`): rockets in a blast's range chain.
//! - It is drawn as its sprite, with a big soft yellow-white flash
//!   (`flash` at 4 to 6 times its size, half its brightness, a new size
//!   and colour every frame) added over it. A `SmallRoket` leaves two
//!   fading lines of its flight, yellow (64 points) and red (16), shaking
//!   a little; the lines shrink back into the point where it blew up. A
//!   `HeavyRoket` puffs a white smoke every update ([`ROCKET_SMOKE`]).
//!
//! Differences from C#:
//! - The launch is the speed the rocket really flew at (see above), and
//!   [`integrate`] has no cap on a step's translation (Farseer's
//!   `MaxTranslation`, 2 units): the rocket never goes faster than
//!   [`MAX_SPEED`], so the cap only ever cut the launch. The rocket is
//!   about 1.2 units (a fiftieth of a second of flight) behind where C#'s
//!   first step put it, and flies the same line at the same speed after.
//!
//! Not ported:
//! - Its body is not a physics body: the sweep ([`fly`]) finds what it
//!   touches within the step it reaches it, where Farseer's contact began
//!   the step after the overlap (continuous collision made it stop a
//!   hair from the surface in both), and nothing pushes it (it did not
//!   push what it touched: `Speed` was zeroed before the contact was
//!   solved) or bounces it (`RestitutionCoefficient`).
//! - Two rockets do not touch each other (`WeaponCollidesWith` includes
//!   `Weapon`: they blew up on touch, both); a blast sets off those it
//!   reaches. Bullets' rays do not hit a rocket either (`SimulateBullet`
//!   found its fixture and hurt it: it blew up), nor does anything the
//!   sweep does not touch (the Hammer's saw, the Zombie).
//! - The step in which a rocket is hit, `Speed = Vector2.Zero` and
//!   `SheduleDestroy` are the same here; the blast is where the rocket
//!   stopped (C#'s `FakePosition`, the draw opinion a frame behind).
//! - The network: `EntityInfoMessage`, `PositionSynced` (a rocket is drawn
//!   from its first frame), `NetworkPhysicEntity`'s position syncs.
//! - `Camera.IsInView` before the launcher's puffs and the smoke (the
//!   particle pool takes them all); `RandomHelper.NextVector2Fake`'s exact
//!   range for the trail's shake (a quarter of a unit across); the trail's
//!   random height; the line renderer's one-pixel lines (a ribbon of
//!   [`TRAIL_WIDTH`] here) and its `Renderable` removed three seconds after
//!   the rocket (the trail is gone by then).
//! - A rocket with no one to chase keeps its last force direction (as
//!   C#); a goal that was killed (C# removed it from the list the step it
//!   died, replacing it with a wreck) is let go of the same way.
//! - A rocket that leaves the map flies on to its 2 s end (as C#: a map
//!   is walled).
//! - `GetPlasmagun(level)`-like levels: the launchers have none.

use std::collections::VecDeque;

use avian2d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rand::Rng;

use super::{PassedByBullets, ViewRng, Weapon, WeaponSystems, fire_weapons};
use crate::car::control::{Team, spares};
use crate::explosion::{Blast, Explosion, Health, Hit};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::{LAUNCH_SMOKE, ROCKET_SMOKE};
use crate::render::layers::Layer;
use crate::render::particles::{FlashBoost, Particles};
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::px;

/// `Weapons\roket`'s size in C# pixels, at the entity's scale of 0.5.
const SPRITE_SIZE: Vec2 = Vec2::new(65.0, 17.0);
const SPRITE_SCALE: f32 = 0.5;
pub const SPRITE: &str = "Weapons\\roket";

/// The rocket's rectangle, world units (`CreateRoket`: `Scale` 0.5).
pub const SIZE: Vec2 = Vec2::new(SPRITE_SIZE.x * SPRITE_SCALE / 32.0, SPRITE_SIZE.y * SPRITE_SCALE / 32.0);
/// A rectangle of density 1 (`PhysicsManager._defaultDensity`).
pub const MASS: f32 = SIZE.x * SIZE.y;
/// `PhysicBody.LinearDamping = 0.95 * PhysicsManager.Friction`.
pub const DAMPING: f32 = 0.95;

/// `CreateRoket`: `InitImpulse = push * 64 + 0.1f * ent.Speed`: the share of
/// the shooter's velocity a rocket is born with.
pub const OWNER_SPEED_SHARE: f32 = 0.1;
/// `ActivationTime`: it only coasts this long (100 ms).
pub const ACTIVATION: f32 = 0.1;
/// `StepForce`.
pub const STEP_FORCE: f32 = 32.0;
/// `MaxSpeed`, units a second.
pub const MAX_SPEED: f32 = 47.0;
/// `BaseRoket._deactivationTimer`: 2 s of being active, then it blows up
/// (`OnDeactivate`).
pub const LIFETIME: f32 = 2.0;
/// `GoalRoket._newGoalTime`: it looks for a goal again every second.
pub const RETARGET: f32 = 1.0;
/// `GoalRoket.UpdateGoal`: only a car faster than this is chased
/// (`Speed.LengthSquared() > 0.1`, squared).
pub const GOAL_SPEED_SQ: f32 = 0.1;

/// `DamageConst.RocketExplosionRange` and `CreateExplosion`'s force.
pub const BLAST_RANGE: f32 = 7.0;
pub const BLAST_FORCE: f32 = 0.5;

/// A fraction of a second the timers' strict comparisons leave over
/// (`PeriodicUpdate` compared ticks: six steps were just under 0.1 s).
const SPARE: f32 = 1e-6;

/// The one-pixel lines of `LineView`, as a ribbon: a little over a pixel
/// at the game's usual zoom.
pub const TRAIL_WIDTH: f32 = 0.05;
/// `SmallRoket`: a yellow line of 64 points and a red one of 16
/// (`Color.Yellow * 0.8`, `Color.Red * 0.75`).
const TRAILS: [(usize, [f32; 3], f32); 2] = [(64, [1.0, 1.0, 0.0], 0.8), (16, [1.0, 0.0, 0.0], 0.75)];
/// The shake of the trail's newest point, units (`ToSimUnits(4)`).
const TRAIL_SHAKE: f32 = 4.0 / 32.0;
/// C#'s game updates a second: the views' puffs and trail points follow it,
/// not the frame rate.
const UPDATES_PER_SECOND: f32 = 60.0;

/// A rocket in flight (`BaseRoket`).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rocket {
    /// Where it is now and where it was a step ago (the view eases
    /// between them).
    pub at: Vec2,
    pub previous: Vec2,
    /// Units a second.
    pub velocity: Vec2,
    /// The body's angle (`Direction`); a homing rocket turns it to its
    /// flight once active.
    pub direction: f32,
    /// Where the force pushes (`_forceDirection`).
    pub force_direction: Vec2,
    /// The force applied this step, taken by the next one (Farseer clears
    /// forces after a step).
    pub force: Vec2,
    /// Who fired it: it flies through its shooter and is the blast's owner.
    pub owner: Entity,
    /// The shooter's team (`Control.Team`), whom the homing spares.
    pub team: Option<Team>,
    /// The blast's damage (`SmallRocket`, `HeavyRocket`).
    pub damage: i32,
    /// `GoalRoket`, not `SmallRoket`.
    pub homing: bool,
    /// Seconds since it was fired.
    pub age: f32,
    /// `isActive`: past its coasting.
    pub active: bool,
    pub goal: Option<Entity>,
    /// Seconds since a goal was last chosen (`_newGoalTime`).
    pub since_goal: f32,
    /// It touched something or was hit: it blows up on the next step
    /// (`SheduleDestroy`).
    pub struck: bool,
}

impl Rocket {
    /// A rocket fired from `at` along world angle `direction` by `owner`
    /// moving at `owner_velocity` (`CreateRoket`, `BaseRoket`'s
    /// constructor): born at its launch velocity, moving where it points.
    pub fn new(at: Vec2, direction: f32, owner: Entity, team: Option<Team>, owner_velocity: Vec2, damage: i32, homing: bool) -> Rocket {
        let velocity = launch_velocity(direction, owner_velocity);
        Rocket {
            at,
            previous: at,
            velocity,
            direction: velocity.to_angle(),
            force_direction: velocity.normalize_or_zero(),
            force: Vec2::ZERO,
            owner,
            team,
            damage,
            homing,
            age: 0.0,
            active: false,
            goal: None,
            since_goal: 0.0,
            struck: false,
        }
    }
}

/// `InitImpulse` as the rocket used it: the top speed along `direction` and
/// a tenth of the shooter's velocity, cut to the top speed.
pub fn launch_velocity(direction: f32, owner_velocity: Vec2) -> Vec2 {
    limit_speed(Vec2::from_angle(direction) * MAX_SPEED + owner_velocity * OWNER_SPEED_SHARE)
}

/// What a physics step does to a rocket's velocity (Farseer's
/// `Island.Solve`): the force's push and the damping.
pub fn integrate(velocity: Vec2, force: Vec2, dt: f32) -> Vec2 {
    (velocity + force / MASS * dt) * (1.0 - dt * DAMPING).clamp(0.0, 1.0)
}

/// `NormalizeSpeed`: no faster than [`MAX_SPEED`].
pub fn limit_speed(velocity: Vec2) -> Vec2 {
    if velocity.length_squared() > MAX_SPEED * MAX_SPEED { velocity * (MAX_SPEED / velocity.length()) } else { velocity }
}

/// `GoalRoket.Update`: the force direction towards the goal. The same
/// point leaves no direction (`SafeNormalize`).
pub fn steer(at: Vec2, goal: Vec2) -> Vec2 {
    (goal - at).normalize_or_zero()
}

/// A car the homing may choose (`ControlableEntities`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub entity: Entity,
    pub at: Vec2,
    pub velocity: Vec2,
    pub team: Option<Team>,
}

/// `GoalRoket.UpdateGoal`: the nearest car that is not the shooter, is of
/// another team and is moving (`GetNearest` by squared distance from the
/// rocket).
pub fn nearest_goal(at: Vec2, owner: Entity, team: Option<Team>, cars: impl IntoIterator<Item = Candidate>) -> Option<Entity> {
    cars.into_iter()
        .filter(|c| c.entity != owner && !spares(team, c.team) && c.velocity.length_squared() > GOAL_SPEED_SQ)
        .min_by(|a, b| {
            a.at.distance_squared(at)
                .total_cmp(&b.at.distance_squared(at))
                .then(a.entity.cmp(&b.entity))
        })
        .map(|c| c.entity)
}

/// How a rocket's step ends ([`fly`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    /// Nothing in the way: it is here now.
    Flies(Vec2),
    /// It touched `entity` with its middle at `at`.
    Strikes { entity: Entity, at: Vec2 },
}

/// One step of a rocket's flight: its rectangle, turned along `direction`,
/// swept from `from` by `velocity × dt`, missing its `owner` and what
/// `passes` (what weapons fly through). Something it already overlaps is
/// struck at once.
pub fn fly(
    spatial: &SpatialQuery,
    passes: &dyn Fn(Entity) -> bool,
    owner: Entity,
    from: Vec2,
    direction: f32,
    velocity: Vec2,
    dt: f32,
) -> Flight {
    let step = velocity * dt;
    let Ok(along) = Dir2::new(step) else {
        return Flight::Flies(from);
    };
    let filter = SpatialQueryFilter::from_excluded_entities([owner]);
    let config = ShapeCastConfig::from_max_distance(step.length());
    let shape = Collider::rectangle(SIZE.x, SIZE.y);
    match spatial.cast_shape_predicate(&shape, from, direction, along, &config, &filter, &|e| !passes(e)) {
        Some(hit) => Flight::Strikes {
            entity: hit.entity,
            at: from + along * hit.distance,
        },
        None => Flight::Flies(from + step),
    }
}

/// A launcher fired a rocket from `at` along `direction`: for the puffs
/// (`CreatePiu`).
#[derive(Message, Debug, Clone, Copy)]
pub struct RocketLaunched {
    pub at: Vec2,
    pub direction: f32,
}

/// What a fired rocket is spawned with: the rocket, and what makes it a
/// thing of the map that can be drawn.
pub fn rocket_bundle(rocket: Rocket) -> impl Bundle {
    (
        Name::new("Rocket"),
        MapEntity,
        Transform::from_translation(rocket.at.extend(0.0)).with_rotation(Quat::from_rotation_z(rocket.direction)),
        Visibility::default(),
        rocket,
    )
}

/// The rockets: their flight in the fixed step, their sprite, flash,
/// smoke and trails in the frame.
pub struct RocketPlugin;

impl Plugin for RocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RocketLaunched>()
            .add_systems(FixedFirst, snap_rockets)
            .add_systems(
                FixedUpdate,
                (rockets_hit, fly_rockets)
                    .chain()
                    .in_set(WeaponSystems)
                    // Physics came first in C#'s frame and the weapons'
                    // `Update` after: a rocket fired now moves next step.
                    .before(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (spawn_rocket_views, place_rocket_views, launch_puffs, rocket_smoke, trail_rockets).run_if(in_state(GameState::Playing)),
            );
    }
}

/// The fixed step sees a rocket where it is, not where the last frame
/// eased it ([`place_rocket_views`]): the blast finds its targets by their
/// `Transform`, and a frame's easing depends on the frame rate. (Avian's
/// `TransformInterpolation` does the same for the cars.)
fn snap_rockets(mut rockets: Query<(&Rocket, &mut Transform)>) {
    for (rocket, mut t) in &mut rockets {
        t.translation = rocket.at.extend(0.0);
        t.rotation = Quat::from_rotation_z(rocket.direction);
    }
}

/// `BaseRoket.TakeDamage`: any blow sets it off, next step.
pub fn rockets_hit(mut hits: MessageReader<Hit>, mut rockets: Query<&mut Rocket>) {
    for hit in hits.read() {
        if let Ok(mut rocket) = rockets.get_mut(hit.target) {
            rocket.struck = true;
            rocket.velocity = Vec2::ZERO;
        }
    }
}

/// `BaseRoket.Update`, `GoalRoket.Update` and the physics step before them:
/// every rocket blows up if it is struck, else moves a step, then thinks.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn fly_rockets(
    mut commands: Commands,
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    spatial: SpatialQuery,
    skipped: Query<(), PassedByBullets>,
    cars: Query<(Entity, &Position, &Health, Option<&Team>, Option<&LinearVelocity>)>,
    mut rockets: Query<(Entity, &mut Rocket)>,
    mut explosions: MessageWriter<Explosion>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    let passes = |e| skipped.contains(e);
    let goals = |rocket: &Rocket| {
        nearest_goal(
            rocket.at,
            rocket.owner,
            rocket.team,
            cars.iter().filter(|(.., health, _, _)| health.hp > 0).map(|(entity, position, _, team, velocity)| Candidate {
                entity,
                at: position.0,
                velocity: velocity.map_or(Vec2::ZERO, |v| v.0),
                team: team.copied(),
            }),
        )
    };
    for (entity, mut rocket) in &mut rockets {
        let rocket = &mut *rocket;
        if rocket.struck {
            blow_up(&mut commands, &mut explosions, entity, rocket);
            continue;
        }
        // The physics step: the force of the last update, the damping,
        // the move.
        rocket.previous = rocket.at;
        rocket.velocity = integrate(rocket.velocity, rocket.force, dt);
        rocket.force = Vec2::ZERO;
        match fly(&spatial, &passes, rocket.owner, rocket.at, rocket.direction, rocket.velocity, dt) {
            Flight::Flies(to) => rocket.at = to,
            Flight::Strikes { entity: target, at } => {
                // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
                debug!("rocket hit {target:?} at {:.3} {:.3}", at.x, -at.y);
                rocket.at = at;
                // `OnCollide`: it stops, and goes next step (`SheduleDestroy`).
                rocket.velocity = Vec2::ZERO;
                rocket.struck = true;
                continue;
            }
        }
        // The update.
        rocket.age += dt;
        if !rocket.active {
            if rocket.age - ACTIVATION > SPARE {
                rocket.active = true;
                if rocket.homing {
                    // `OnActivate`: a goal, and the next one in a second.
                    rocket.goal = goals(rocket);
                    rocket.since_goal = 0.0;
                }
            }
        } else {
            if rocket.homing {
                rocket.since_goal += dt;
                if rocket.since_goal - RETARGET > SPARE {
                    rocket.goal = goals(rocket);
                    rocket.since_goal = 0.0;
                }
                // A goal that is gone (dead, removed) is let go of.
                match rocket.goal.map(|g| cars.get(g)) {
                    Some(Ok((_, position, health, ..))) if health.hp > 0 => rocket.force_direction = steer(rocket.at, position.0),
                    _ => rocket.goal = None,
                }
                rocket.direction = rocket.velocity.to_angle();
            }
            rocket.force = rocket.force_direction * STEP_FORCE;
            if rocket.age - LIFETIME > SPARE {
                // `OnDeactivate`: `Destroy()`, at once.
                blow_up(&mut commands, &mut explosions, entity, rocket);
                continue;
            }
        }
        rocket.velocity = limit_speed(rocket.velocity);
    }
}

/// `BaseRoket.Destroy`: the rocket is gone, and its blast is where it was.
fn blow_up(commands: &mut Commands, explosions: &mut MessageWriter<Explosion>, entity: Entity, rocket: &Rocket) {
    explosions.write(Explosion {
        at: rocket.at,
        blast: Blast {
            range: BLAST_RANGE,
            force: BLAST_FORCE,
            damage: rocket.damage,
        },
        owner: Some(rocket.owner),
    });
    commands.entity(entity).despawn();
}

/// A rocket's drawing: the clock of its smoke.
#[derive(Component, Default)]
struct RocketView {
    /// Seconds towards the next update's puff.
    smoke: f32,
}

/// The flash added over the rocket, a child of it (`BaseRoket.DrawFlash`).
#[derive(Component)]
struct Flash;

#[allow(clippy::too_many_arguments)]
fn spawn_rocket_views(
    mut commands: Commands,
    rockets: Query<(Entity, &Rocket), Added<Rocket>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (mut images, mut premultiplied): (ResMut<Assets<Image>>, ResMut<crate::render::premultiply::Premultiplied>),
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(sprite), Some(flash)) = (map.sprites.get(SPRITE), map.sprites.get("Particles\\flash")) else {
        return;
    };
    let lit = SpriteTint {
        color: [1.0; 4],
        lit: true,
    };
    for (entity, rocket) in &rockets {
        let body = sprite_bundle(sprite, SPRITE_SCALE, Layer::AirParticles, lit, &quad, &mut materials);
        // Added as XNA added a premultiplied texture (`premultiply`).
        let glow = materials.add(StandardMaterial {
            base_color_texture: Some(premultiplied.of(&flash.image, &mut images)),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            depth_bias: Layer::Glow.depth_bias(),
            ..default()
        });
        commands
            .entity(entity)
            .insert(RocketView::default())
            .with_child(body)
            .with_child((
                Flash,
                Mesh3d(quad.0.clone()),
                MeshMaterial3d(glow),
                Transform::from_xyz(0.0, 0.0, Layer::Glow.z()),
            ));
        // `SmallRoket`'s two lines, which outlive the rocket.
        if !rocket.homing {
            for (points, color, alpha) in TRAILS {
                let material = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    depth_bias: Layer::AirParticles.depth_bias(),
                    ..default()
                });
                commands.spawn((
                    Name::new("Rocket trail"),
                    MapEntity,
                    Trail {
                        rocket: entity,
                        points: VecDeque::from(vec![rocket.at; points]),
                        color,
                        alpha,
                        clock: 0.0,
                        head: rocket.at,
                        gone: 0,
                    },
                    Mesh3d(meshes.add(ribbon(&[], Vec2::ZERO, color, alpha))),
                    MeshMaterial3d(material),
                    Transform::from_translation(rocket.at.extend(Layer::AirParticles.z())),
                    Visibility::default(),
                    SpriteTint {
                        color: [1.0; 4],
                        lit: true,
                    },
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        }
    }
}

/// Eases each rocket between its last two steps, turned along its body,
/// and gives its flash a new size and colour (`DrawFlash`: 4 to 6 times
/// the texture, yellow to white, half bright).
#[allow(clippy::type_complexity)]
fn place_rocket_views(
    fixed: Res<Time<Fixed>>,
    flash_boost: Res<FlashBoost>,
    mut rng: ResMut<ViewRng>,
    map: Res<LoadedMap>,
    mut rockets: Query<(&Rocket, &mut Transform, &Children)>,
    mut flashes: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), (With<Flash>, Without<Rocket>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let k = fixed.overstep_fraction();
    let size = map.sprites.get("Particles\\flash").map_or(Vec2::splat(64.0), |s| s.size);
    for (rocket, mut t, children) in &mut rockets {
        t.translation = rocket.previous.lerp(rocket.at, k).extend(0.0);
        t.rotation = Quat::from_rotation_z(rocket.direction);
        for child in children.iter() {
            let Ok((mut flash, material)) = flashes.get_mut(child) else {
                continue;
            };
            // Turned to the camera (`Camera.RotationZ`): against the rocket's
            // own turn.
            let scale = 4.0 + rng.0.random::<f32>() * 2.0;
            *flash = Transform::from_xyz(0.0, 0.0, Layer::Glow.z())
                .with_rotation(Quat::from_rotation_z(-rocket.direction))
                .with_scale((size * px(1.0) * scale).extend(1.0));
            let white = 0.5 * rng.0.random::<f32>();
            if let Some(mut m) = materials.get_mut(&material.0) {
                // `Color.Lerp(Yellow, White, white) * 0.5`: premultiplied, so
                // the colour and the alpha are both halved.
                let c = Color::srgb(0.5, 0.5, 0.5 * white).to_linear();
                m.base_color = Color::linear_rgba(c.red * flash_boost.0, c.green * flash_boost.0, c.blue * flash_boost.0, 0.5);
            }
        }
    }
}

/// `CreatePiu`: two black puffs flung sideways from the muzzle.
fn launch_puffs(mut launched: MessageReader<RocketLaunched>, mut particles: ResMut<Particles>) {
    for shot in launched.read() {
        for side in [-1.0, 1.0] {
            let along = shot.direction + side * std::f32::consts::FRAC_PI_2;
            particles.try_add(&LAUNCH_SMOKE, shot.at, along, shot.direction);
        }
    }
}

/// `HeavyRoket.Update`: a white puff, a random way round, every game
/// update.
fn rocket_smoke(
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    mut rng: ResMut<ViewRng>,
    mut particles: ResMut<Particles>,
    mut rockets: Query<(&Rocket, &mut RocketView)>,
) {
    let k = fixed.overstep_fraction();
    for (rocket, mut view) in &mut rockets {
        if !rocket.homing {
            continue;
        }
        view.smoke += time.delta_secs() * UPDATES_PER_SECOND;
        // At most a few at once after a long frame.
        for _ in 0..(view.smoke as usize).min(4) {
            let (a, b) = (rng.0.random::<f32>() * std::f32::consts::TAU, rng.0.random::<f32>() * std::f32::consts::TAU);
            particles.try_add(&ROCKET_SMOKE, rocket.previous.lerp(rocket.at, k), a, b);
        }
        view.smoke = view.smoke.fract();
    }
}

/// One of a `SmallRoket`'s lines (`LineView`): the newest point first.
#[derive(Component)]
struct Trail {
    rocket: Entity,
    points: VecDeque<Vec2>,
    color: [f32; 3],
    alpha: f32,
    /// Game updates towards the next point.
    clock: f32,
    /// Where the rocket was last.
    head: Vec2,
    /// Points pushed since the rocket was gone: when they fill the line,
    /// it has shrunk into one point.
    gone: usize,
}

/// A ribbon along `points` from `head`, its colour fading linearly to
/// nothing along the line (`_colors`: `MainColor * (1 - i / n)`), at
/// `head`'s place.
fn ribbon(points: &[Vec2], head: Vec2, color: [f32; 3], alpha: f32) -> Mesh {
    let n = points.len().max(1) as f32;
    let (mut positions, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for (i, pair) in points.windows(2).enumerate() {
        let (a, b) = (pair[0] - head, pair[1] - head);
        let Some(side) = (b - a).try_normalize().map(|d| d.perp() * (TRAIL_WIDTH / 2.0)) else {
            continue;
        };
        let base = positions.len() as u32;
        let fade = |k: usize| {
            let c = Color::srgb(color[0], color[1], color[2]).to_linear();
            [c.red, c.green, c.blue, alpha * (1.0 - k as f32 / n)]
        };
        positions.extend([a - side, a + side, b + side, b - side].map(|p| p.extend(0.0).to_array()));
        colors.extend([fade(i), fade(i), fade(i + 1), fade(i + 1)]);
        indices.extend([0, 1, 2, 0, 2, 3].map(|k| base + k));
    }
    // Never an empty mesh: a line with no length is a quad with no size.
    if positions.is_empty() {
        positions = vec![[0.0; 3]; 4];
        colors = vec![[0.0; 4]; 4];
        indices = vec![0, 1, 2, 0, 2, 3];
    }
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_indices(Indices::U32(indices))
}

/// `LineView.Draw` once an update: a new newest point (the rocket's, a
/// little shaken while it lives), the oldest dropped; then the line is drawn.
/// When the rocket is gone the points go on being pushed in the same place,
/// so the line shrinks into it.
#[allow(clippy::too_many_arguments)]
fn trail_rockets(
    mut commands: Commands,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    mut rng: ResMut<ViewRng>,
    mut meshes: ResMut<Assets<Mesh>>,
    rockets: Query<&Rocket>,
    mut trails: Query<(Entity, &mut Trail, &mut Transform, &Mesh3d)>,
) {
    let k = fixed.overstep_fraction();
    for (entity, mut trail, mut transform, mesh) in &mut trails {
        let alive = rockets.get(trail.rocket).ok();
        if let Some(rocket) = alive {
            trail.head = rocket.previous.lerp(rocket.at, k);
        }
        trail.clock += time.delta_secs() * UPDATES_PER_SECOND;
        for _ in 0..(trail.clock as usize).min(4) {
            let shake = if alive.is_some() {
                Vec2::new(rng.0.random::<f32>() * 2.0 - 1.0, rng.0.random::<f32>() * 2.0 - 1.0) * TRAIL_SHAKE
            } else {
                trail.gone += 1;
                Vec2::ZERO
            };
            let point = trail.head + shake;
            trail.points.pop_back();
            trail.points.push_front(point);
        }
        trail.clock = trail.clock.fract();
        if alive.is_none() && trail.gone >= trail.points.len() {
            commands.entity(entity).despawn();
            continue;
        }
        let points: Vec<Vec2> = trail.points.iter().copied().collect();
        transform.translation = trail.head.extend(Layer::AirParticles.z());
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            *m = ribbon(&points, trail.head, trail.color, trail.alpha);
        }
    }
}

/// The launcher's shot: a rocket from `muzzle` (`RocketLauncherWeapon.OnShoot`).
pub fn launch(weapon: &Weapon, car: Entity, team: Option<Team>, velocity: Vec2) -> Rocket {
    Rocket::new(weapon.muzzle(), weapon.direction, car, team, velocity, weapon.options.damage, weapon.options.homing)
}
