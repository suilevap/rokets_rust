//! The Plasma gun's bullets (`PlasmaGun`, `SimpleBullet`, 2014-2015): shots
//! that fly, rather than rays that arrive at once.
//!
//! C# origin: `PlasmaGun.OnShoot` (fired from [`super::fire_weapons`]),
//! `BulletManager.GetSimpleBullet` / `BulletFactory.GetBullet` (the bullet
//! from the pool, its speed, damage, colour and push), `SimpleBullet` (its
//! body, `OnCollide` and `Destroy`), `ParticleSystemManager.CreatePlasmaEnd`
//! and the numbers in [`super::model`] (`PlasmagunDefaultOptions`,
//! `PlasmagunSlowOptions`). `git log` of the C# files: the gun and its
//! pooled bullet came together (872d40c), then its sound, its numbers as
//! `WeaponOptions` (red and fast, yellow and slow), several bullets spread
//! evenly (d8bede9), and `IsImmidiate = false` so that the attacking AI
//! fires it in its own rhythm (b29e004, `SmartAttackTargetSlowWeapon`).
//!
//! - A shot (the trigger held, once a reload: 0.5 s for `PlasmaGun`, 1 s
//!   for `PlasmaGunSlow`; 0.1 of the player's energy) kicks the gun back 1
//!   to 5 pixels, plays `PlasmaShot` at the muzzle (a random pitch within a
//!   fifth of an octave) and sends a [`PlasmaBullet`] from the muzzle along
//!   the barrel, or along each of [`model::plasma_directions`].
//! - A bullet is a circle of radius half a unit (the 4-pixel
//!   `Gradient4Pixel` at scale 8, `UseCircleForGeometry`) that
//!   flies straight at its speed (32 or 16 units a second, no damping)
//!   through what bullets fly through (sensors and low boxes: Farseer's
//!   `WeaponCollidesWithExceptWeapon` leaves out `LowWall`, `Bomb` and the
//!   other bullets) and its own shooter. Each fixed step it sweeps its
//!   circle along the step ([`fly`]; C#'s body was `IsBullet`, swept too):
//!   the first thing it touches is pushed at the bullet's centre along its
//!   flight by the gun's impulse (18) if it is a dynamic body, hurt by its
//!   damage (8) if it can be hurt and is not a car of the shooter's team,
//!   and the bullet is gone, with a white flash where it ended
//!   ([`PlasmaEnd`], `_plasmaEndStyle`). A car of the shooter's team is
//!   pushed all the same, unhurt.
//! - It is drawn as a soft streak of its colour, 6 units by 1, added to
//!   what is under it (`Gradient4Pixel`, `DrawScale` (6, 1) × 8,
//!   `DefaultWorldAdditive`), its bright end ahead.
//!
//! Not ported:
//! - The flash where the bullet ends is where its sweep touched; C# put it
//!   a step back from where the bullet was when it was destroyed
//!   (`Position` less `Speed * UpdateTimeSec`), which, the destroy waiting for the next
//!   update, was about where it first overlapped: a fraction of a unit
//!   further in. Likewise the sweep finds the touch within the step the
//!   bullet reaches it; Farseer's sensor found the overlap at the start of
//!   the next step, so a hit lands up to a step (1/60 s) sooner here.
//! - Two things touched in the same step: C# hurt and pushed each
//!   (`OnCollide` ran per contact before the scheduled destroy); the sweep
//!   stops at the first.
//! - A bullet that leaves the map is dropped; C#'s flew on forever in its
//!   pool (`CanMoreThanMax`), out of sight.
//! - The view check on the flash (the particle pool takes them all), and
//!   the `SoundInstancePool` of two `PlasmaShot`s (every shot sounds).
//! - `GetPlasmagun(level)` (the spawn director's levels: more bullets and
//!   damage), as with the other guns' levels; the spread of several
//!   bullets is ported ([`model::plasma_directions`]) for it.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{Hurt, PassedByBullets, WeaponSystems};
use crate::car::control::{Team, spares};
use crate::explosion::Hit;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::PLASMA_END;
use crate::render::glow::{GlowMaterial, additive};
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::UnitQuad;
use crate::units::px;

/// `SimpleBullet`'s circle: `Gradient4Pixel`'s 4 pixels at scale 8 are
/// one unit (`4 × 8 / 32`) across.
pub const BULLET_RADIUS: f32 = 0.5;

/// The bullet's sprite (`BulletManager.CreateSimpleBullet`).
const BULLET_TEXTURE: &str = "Gradient4Pixel";
/// Its drawn scale: `Scale` 8 times `DrawScale` (6, 1).
const BULLET_DRAW_SCALE: Vec2 = Vec2::new(8.0 * 6.0, 8.0);

/// A Plasma gun's bullet in flight (`SimpleBullet`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PlasmaBullet {
    /// Where it is now and where it was a step ago (the view eases
    /// between them).
    pub at: Vec2,
    pub previous: Vec2,
    /// Units a second.
    pub velocity: Vec2,
    /// Who fired it: it flies through its shooter and is the blow's
    /// author (`Owner`).
    pub owner: Entity,
    /// The shooter's team when it fired (`_ownerTeam`).
    pub team: Option<Team>,
    pub damage: i32,
    pub impulse: f32,
    /// sRGB.
    pub color: [f32; 3],
}

/// What a fired bullet is spawned with: the bullet, and what makes it a
/// thing of the map that can be drawn.
pub fn bullet_bundle(bullet: PlasmaBullet) -> impl Bundle {
    (
        Name::new("Plasma bullet"),
        MapEntity,
        Transform::from_translation(bullet.at.extend(0.0)),
        Visibility::default(),
        bullet,
    )
}

impl PlasmaBullet {
    /// A bullet from the gun's `options` at `at`, flying along world angle
    /// `direction` (`BulletFactory.GetBullet`).
    pub fn new(at: Vec2, direction: f32, owner: Entity, team: Option<Team>, options: &super::model::Options) -> PlasmaBullet {
        PlasmaBullet {
            at,
            previous: at,
            velocity: Vec2::from_angle(direction) * options.bullet_speed,
            owner,
            team,
            damage: options.damage,
            impulse: options.impulse,
            color: options.bullet_color,
        }
    }
}

/// A bullet ended at `at`, flying along `direction` (`SimpleBullet.Destroy`):
/// for the flash.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlasmaEnd {
    pub at: Vec2,
    pub direction: f32,
}

/// How a bullet's step ends ([`fly`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    /// Nothing in the way: it is here now.
    Flies(Vec2),
    /// It touched `entity` with its centre at `at`.
    Strikes { entity: Entity, at: Vec2 },
}

/// One step of a bullet's flight: its circle swept from `from` by
/// `velocity × dt`, missing its `owner` and what `passes` (what bullets
/// fly through). Something it already overlaps is struck at once.
pub fn fly(
    spatial: &SpatialQuery,
    passes: &dyn Fn(Entity) -> bool,
    owner: Entity,
    from: Vec2,
    velocity: Vec2,
    dt: f32,
) -> Flight {
    let step = velocity * dt;
    let Ok(direction) = Dir2::new(step) else {
        return Flight::Flies(from);
    };
    let filter = SpatialQueryFilter::from_excluded_entities([owner]);
    let config = ShapeCastConfig::from_max_distance(step.length());
    let shape = Collider::circle(BULLET_RADIUS);
    match spatial.cast_shape_predicate(&shape, from, 0.0, direction, &config, &filter, &|e| !passes(e)) {
        Some(hit) => Flight::Strikes {
            entity: hit.entity,
            at: from + direction * hit.distance,
        },
        None => Flight::Flies(from + step),
    }
}

/// The Plasma gun's bullets: their flight in the fixed step, their sprite
/// and flash in the frame.
pub struct PlasmaPlugin;

impl Plugin for PlasmaPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlasmaEnd>()
            .add_systems(
                FixedUpdate,
                move_bullets
                    .in_set(WeaponSystems)
                    .after(super::fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (spawn_bullet_views, place_bullet_views, flash_bullet_ends).run_if(in_state(GameState::Playing)),
            );
    }
}

/// `SimpleBullet.OnCollide` and the physics that carried it: every bullet
/// flies a step, and what it touches is pushed and hurt and ends it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn move_bullets(
    mut commands: Commands,
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    spatial: SpatialQuery,
    map: Option<Res<LoadedMap>>,
    mut bullets: Query<(Entity, &mut PlasmaBullet)>,
    skipped: Query<(), PassedByBullets>,
    mut bodies: Query<(&RigidBody, Forces)>,
    hurt: Query<(), Hurt>,
    teams: Query<&Team>,
    mut hits: MessageWriter<Hit>,
    mut ends: MessageWriter<PlasmaEnd>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    let passes = |e| skipped.contains(e);
    // The map's rectangle, with a margin: a bullet past it is dropped.
    let bounds = map.map(|m| {
        let size = Vec2::new(m.grid.width as f32, m.grid.height as f32) * m.cell.0;
        Rect::new(0.0, -size.y, size.x, 0.0).inflate(BULLET_RADIUS * 4.0)
    });
    for (entity, mut bullet) in &mut bullets {
        bullet.previous = bullet.at;
        match fly(&spatial, &passes, bullet.owner, bullet.at, bullet.velocity, dt) {
            Flight::Flies(to) => {
                bullet.at = to;
                if bounds.is_some_and(|b| !b.contains(to)) {
                    commands.entity(entity).despawn();
                }
            }
            Flight::Strikes { entity: target, at } => {
                bullet.at = at;
                // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
                debug!("plasma hit {target:?} at {:.3} {:.3}", at.x, -at.y);
                if hurt.contains(target) && !spares(bullet.team, teams.get(target).ok().copied()) {
                    hits.write(Hit {
                        target,
                        damage: bullet.damage,
                        from: bullet.velocity.to_angle(),
                        by: Some(bullet.owner),
                    });
                }
                if let Ok((body, mut forces)) = bodies.get_mut(target)
                    && body.is_dynamic()
                {
                    forces.apply_linear_impulse_at_point(bullet.velocity.normalize_or_zero() * bullet.impulse, at);
                }
                ends.write(PlasmaEnd {
                    at,
                    direction: bullet.velocity.to_angle(),
                });
                commands.entity(entity).despawn();
            }
        }
    }
}

/// The bullet's streak (`SimpleBullet.Draw`), a child of the bullet.
#[derive(Component)]
struct BulletView;

/// The streak's material, one per colour (the map's texture with it).
#[derive(Default)]
struct BulletMaterials(Vec<([u32; 3], AssetId<Image>, Handle<GlowMaterial>)>);

#[allow(clippy::too_many_arguments)]
fn spawn_bullet_views(
    mut commands: Commands,
    bullets: Query<(Entity, &PlasmaBullet), Added<PlasmaBullet>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<GlowMaterial>>,
    mut cache: Local<BulletMaterials>,
) {
    let Some(info) = map.sprites.get(BULLET_TEXTURE) else {
        return;
    };
    for (entity, bullet) in &bullets {
        let key = bullet.color.map(f32::to_bits);
        let material = match cache.0.iter().find(|(k, image, _)| *k == key && *image == info.image.id()) {
            Some((_, _, m)) => m.clone(),
            None => {
                let [r, g, b] = bullet.color;
                let m = materials.add(additive(
                    StandardMaterial {
                        base_color: Color::srgb(r, g, b),
                        base_color_texture: Some(info.image.clone()),
                        ..default()
                    },
                    Layer::Glow.depth_bias(),
                ));
                cache.0.push((key, info.image.id(), m.clone()));
                m
            }
        };
        let size = info.size * px(1.0) * BULLET_DRAW_SCALE;
        commands.entity(entity).with_child((
            BulletView,
            Mesh3d(quad.0.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.0, Layer::Glow.z()).with_scale(size.extend(1.0)),
        ));
    }
}

/// Eases the streak between the bullet's last two steps, turned along its
/// flight, its bright end ahead (`Direction = dir + π`).
fn place_bullet_views(fixed: Res<Time<Fixed>>, mut bullets: Query<(&PlasmaBullet, &mut Transform)>) {
    let k = fixed.overstep_fraction();
    for (bullet, mut t) in &mut bullets {
        t.translation = bullet.previous.lerp(bullet.at, k).extend(0.0);
        t.rotation = Quat::from_rotation_z(bullet.velocity.to_angle() + std::f32::consts::PI);
    }
}

/// `CreatePlasmaEnd`: the flash, turned a quarter from the flight (C#'s
/// `dir + π/2`, its angles the other way round).
fn flash_bullet_ends(mut ends: MessageReader<PlasmaEnd>, mut particles: ResMut<Particles>) {
    for end in ends.read() {
        particles.try_add(&PLASMA_END, end.at, 0.0, end.direction - std::f32::consts::FRAC_PI_2);
    }
}
