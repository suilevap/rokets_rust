//! Health, damage and explosions.
//!
//! C# origin: `WeaponHelper.CreateExplosion` with Farseer's `Explosion`,
//! `EntitiesManager.DamageEntity`, `ControlableEntity.TakeDamage`, `Bomb`
//! and `DamagableBox` (2012–2014). Systems meet in messages:
//!
//! - [`Explosion`] at a point: bodies in range are thrown away from it and
//!   whatever can be hurt there gets a [`Hit`]; walls shelter what is
//!   behind them. It flashes and smokes (`CreateBigBoom`).
//! - [`Hit`]: a [`Health`] loses points (after its [`Shield`] has taken
//!   what it can), is told of it by [`Damaged`] (sounds, health bar) and
//!   sends [`Destroyed`] when it runs out (what then happens to a car is
//!   the game mode's, for the player's, and `car::death`'s business); a
//!   car held for the player's devices ([`LocalDevice`]) takes no blow
//!   while the camera does not see it (`TakeDamage`: `LocalDevice &&
//!   !IsInView`, C#'s view of the last frame drawn); an
//!   [`Explosive`] that blows up when hit lights its fuse.
//! - An [`Explosive`] blows up on touch (bombs) or a moment after a hit
//!   (red boxes, which so set each other off).
//!
//! A blast with an owner spares the owner's team (`CreateExplosion`:
//! `Control.Team != ownerTeam`); one with none (bombs, red boxes) hurts
//! everything it reaches. The throw is not spared: it pushes every body.
//!
//! The throw is simpler than Farseer's: it cast rays over each body's
//! outline; here a body gets one push from the centre, as strong as all
//! those rays together would give, sized by how wide it looks from the
//! blast and fading to nothing at the edge of the range.

use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use crate::actions::{OnDestroy, RunAction, destroy_action};
use crate::car::CarRadius;
use crate::car::control::{LocalDevice, Team, spares};
use crate::car::shield::Shield;
use crate::game::{GameState, LoadedMap, MapId};
use crate::particles::styles::{BOMB_BEEP, BOOM_FIRE, BOOM_FLASH, BOOM_RING, BOOM_SMOKE, ring_rate};
use crate::render::camera::MainCamera;
use crate::render::particles::Particles;
use crate::render::sprite::SpriteTint;
use crate::weapon::aim_help::in_view;

/// Hit points (`ControlableEntity.Health`, `MaxHealth`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub hp: i32,
    pub max: i32,
}

impl Health {
    pub fn full(max: i32) -> Health {
        Health { hp: max, max }
    }
}

/// Blows up: on touch, or a moment after it is hit.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Explosive {
    pub blast: Blast,
    pub trigger: Trigger,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    /// World units (`DamageConst.*ExplosionRange`).
    pub range: f32,
    /// Farseer's `maxForce`.
    pub force: f32,
    pub damage: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trigger {
    /// When a car or box touches it (`Bomb.OnCollide`).
    Touch,
    /// This many seconds after a hit, red meanwhile (`DamagableBox`).
    Hit { fuse: f32 },
}

/// Pings to warn, every `period` seconds (`Bomb`: `CreateBip2`, each
/// bomb its own period between 1 and 2 s).
#[derive(Component, Debug)]
pub struct Beeps {
    period: f32,
    next: f32,
}

impl Beeps {
    pub fn every(period: f32) -> Beeps {
        Beeps { period, next: period }
    }
}

/// A blast at a point, in world units.
#[derive(Message, Debug, Clone, Copy)]
pub struct Explosion {
    pub at: Vec2,
    pub blast: Blast,
    /// Whose it is: its team is spared (`CreateExplosion`'s `owner`).
    pub owner: Option<Entity>,
}

/// Damage to an entity, from a direction (radians, towards it).
#[derive(Message, Debug, Clone, Copy)]
pub struct Hit {
    pub target: Entity,
    pub damage: i32,
    pub from: f32,
    /// Who dealt it, when it was someone's blow (`entityDamager`: the
    /// shooter, the saw's car, a blast's owner): the AI hits back at it.
    pub by: Option<Entity>,
}

/// A car with health took a blow it was alive for (`TakeDamage`): `taken`
/// is what got past its shield.
#[derive(Message, Debug, Clone, Copy)]
pub struct Damaged {
    pub target: Entity,
    /// The blow as it came, before the shield (`totalDamage`).
    pub damage: i32,
    pub taken: i32,
    /// It was the last of its health.
    pub killed: bool,
    /// Its shield is `Immortal` (`SetImmortal`): no health bar.
    pub immortal: bool,
    pub from: f32,
    pub by: Option<Entity>,
}

/// An entity's health ran out.
#[derive(Message, Debug, Clone, Copy)]
pub struct Destroyed(pub Entity);

/// A lit fuse.
#[derive(Component, Debug)]
struct Fuse(f32);

/// `Color.OrangeRed`: a box about to blow.
const LIT: [f32; 3] = [1.0, 69.0 / 255.0, 0.0];

#[derive(Resource)]
struct BoomRng(SmallRng);

/// The explosion systems (in `FixedUpdate`), for others to run after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExplosionSystems;

pub struct ExplosionPlugin;

impl Plugin for ExplosionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Explosion>()
            .add_message::<Hit>()
            .add_message::<Damaged>()
            .add_message::<Destroyed>()
            .insert_resource(BoomRng(rand::SeedableRng::seed_from_u64(11)))
            // In the fixed step, after the weapons' hits of the step.
            .configure_sets(FixedUpdate, ExplosionSystems.after(crate::weapon::WeaponSystems))
            .add_systems(
                FixedUpdate,
                (blow_up_on_touch, burn_fuses, explode, take_hits, beep)
                    .chain()
                    .in_set(ExplosionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// The impulse a blast at `at` gives a body of radius `radius` centred at
/// `center` (Farseer's `Explosion.Activate`, summed over its rays): the
/// angle the body covers seen from the blast × `force` × 180/π, fading
/// linearly from its near side to the edge of the range. Zero out of range.
pub fn blast_impulse(at: Vec2, blast: &Blast, center: Vec2, radius: f32) -> Vec2 {
    let to = center - at;
    let d = to.length();
    // A blast on a body's very middle (the accelerator's, on its car)
    // pushes it equally all round: Farseer's rays from inside cancel.
    if d < 1e-3 {
        return Vec2::ZERO;
    }
    let near = (d - radius).max(0.0);
    if near >= blast.range {
        return Vec2::ZERO;
    }
    let covered = if d > radius { 2.0 * (radius / d).atan() } else { PI };
    let strength = covered * blast.force * 180.0 / PI * (1.0 - near / blast.range);
    to.normalize_or(Vec2::X) * strength
}

fn blow_up_on_touch(
    mut commands: Commands,
    mut started: MessageReader<CollisionStart>,
    explosives: Query<(&Explosive, &Transform, Option<&OnDestroy>, Option<&MapId>)>,
    bodies: Query<&RigidBody, Without<Sensor>>,
    mut explosions: MessageWriter<Explosion>,
    mut run: MessageWriter<RunAction>,
) {
    for event in started.read() {
        for (this, other) in [(event.collider1, event.body2), (event.collider2, event.body1)] {
            let (Ok((explosive, t, on_destroy, id)), Some(other)) = (explosives.get(this), other) else {
                continue;
            };
            if explosive.trigger == Trigger::Touch
                && bodies.get(other) == Ok(&RigidBody::Dynamic)
                && let Ok(mut e) = commands.get_entity(this)
            {
                e.despawn();
                run.write_batch(destroy_action(on_destroy, id));
                explosions.write(Explosion {
                    at: t.translation.truncate(),
                    blast: explosive.blast,
                    owner: None,
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn burn_fuses(
    mut commands: Commands,
    time: Res<Time>,
    mut fuses: Query<(Entity, &mut Fuse, &Explosive, &Transform, Option<&OnDestroy>, Option<&MapId>)>,
    mut explosions: MessageWriter<Explosion>,
    mut run: MessageWriter<RunAction>,
) {
    for (entity, mut fuse, explosive, t, on_destroy, id) in &mut fuses {
        fuse.0 -= time.delta_secs();
        if fuse.0 <= 0.0 {
            commands.entity(entity).despawn();
            run.write_batch(destroy_action(on_destroy, id));
            explosions.write(Explosion {
                at: t.translation.truncate(),
                blast: explosive.blast,
                owner: None,
            });
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn explode(
    mut explosions: MessageReader<Explosion>,
    map: Res<LoadedMap>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<BoomRng>,
    mut bodies: Query<(&RigidBody, &ColliderAabb, &Position, Forces), Without<Sensor>>,
    targets: Query<(Entity, &Transform, Option<&ColliderAabb>, Option<&Team>), Or<(With<Health>, With<Explosive>, With<crate::weapon::rocket::Rocket>)>>,
    teams: Query<&Team>,
    mut hits: MessageWriter<Hit>,
) {
    for ex in explosions.read() {
        let (at, blast) = (ex.at, ex.blast);
        // A dead owner is no owner (C#'s `Control.Team` of a removed car
        // is not asked either: the explosion keeps the car it came from).
        let owner_team = ex.owner.and_then(|o| teams.get(o).ok()).copied();
        boom(&map, &mut particles, &mut rng.0, at, blast.range);
        let sheltered = |p: Vec2| {
            let (a, b) = (map.cell.to_cells(at), map.cell.to_cells(p));
            map.grid.line_hits_wall(a, b)
        };
        for (body, aabb, position, mut forces) in &mut bodies {
            if *body != RigidBody::Dynamic || sheltered(position.0) {
                continue;
            }
            let radius = aabb.size().max_element() / 2.0;
            let impulse = blast_impulse(at, &blast, position.0, radius);
            if impulse != Vec2::ZERO {
                forces.apply_linear_impulse(impulse);
            }
        }
        for (entity, t, aabb, team) in &targets {
            let p = t.translation.truncate();
            let radius = aabb.map_or(0.0, |a| a.size().max_element() / 2.0);
            if p.distance(at) < blast.range + radius && !sheltered(p) && !spares(owner_team, team.copied()) {
                hits.write(Hit {
                    target: entity,
                    damage: blast.damage,
                    from: (p - at).to_angle(),
                    by: ex.owner,
                });
            }
        }
    }
}

/// `ParticleSystemManager.CreateBigBoom`.
fn boom(map: &LoadedMap, particles: &mut Particles, rng: &mut SmallRng, at: Vec2, range: f32) {
    if let Some(circle) = map.sprites.get(BOOM_RING.texture) {
        particles.add_scaled(&BOOM_RING, at, 0.0, 0.0, ring_rate(range, circle.size.x));
    }
    particles.try_add(&BOOM_FLASH, at, 0.0, 0.0);
    particles.try_add(&BOOM_FIRE, at, 0.0, rng.random::<f32>() * 2.0 * PI);
    let jitter = Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * 0.5;
    let (d1, d2) = (rng.random::<f32>() * 2.0 * PI, rng.random::<f32>() * 2.0 * PI);
    particles.try_add(&BOOM_SMOKE, at + jitter, d1, d2);
}

#[allow(clippy::too_many_arguments)]
fn take_hits(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut health: Query<(&mut Health, Option<&mut Shield>)>,
    held: Query<(&GlobalTransform, Option<&CarRadius>), With<LocalDevice>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    explosives: Query<(&Explosive, &Children), Without<Fuse>>,
    mut tints: Query<&mut SpriteTint>,
    mut damaged: MessageWriter<Damaged>,
    mut destroyed: MessageWriter<Destroyed>,
) {
    let camera = cameras.single().ok();
    for hit in hits.read() {
        if let (Ok((at, radius)), Some((camera, view))) = (held.get(hit.target), camera)
            && !in_view(camera, view, at.translation().truncate(), radius.map_or(0.0, |r| r.0))
        {
            continue;
        }
        if let Ok((mut h, shield)) = health.get_mut(hit.target)
            && h.hp > 0
        {
            // The shield takes what it can first (`Shield.Spend`).
            let immortal = shield.as_ref().is_some_and(|s| s.is_immortal());
            let taken = shield.map_or(hit.damage, |mut s| s.spend(hit.damage));
            h.hp -= taken;
            let killed = h.hp <= 0;
            damaged.write(Damaged {
                target: hit.target,
                damage: hit.damage,
                taken,
                killed,
                immortal,
                from: hit.from,
                by: hit.by,
            });
            if killed {
                destroyed.write(Destroyed(hit.target));
            }
        }
        if let Ok((explosive, children)) = explosives.get(hit.target)
            && let Trigger::Hit { fuse } = explosive.trigger
        {
            commands.entity(hit.target).insert(Fuse(fuse));
            for child in children.iter() {
                if let Ok(mut tint) = tints.get_mut(child) {
                    let [r, g, b] = LIT;
                    tint.color = [tint.color[0] * r, tint.color[1] * g, tint.color[2] * b, tint.color[3]];
                }
            }
        }
    }
}

fn beep(
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut beepers: Query<(&mut Beeps, &Transform)>,
) {
    for (mut beeps, t) in &mut beepers {
        beeps.next -= time.delta_secs();
        if beeps.next <= 0.0 {
            beeps.next += beeps.period;
            particles.try_add(&BOMB_BEEP, t.translation.truncate(), 0.0, 0.0);
        }
    }
}
