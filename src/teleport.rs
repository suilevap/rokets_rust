//! Teleports (`Entities/Teleport.cs`, made by `CreateTeleport`).
//!
//! A teleport is a static sensor that moves whatever drives into it (one
//! of its `Subscribers`, or any body when it lists none) to its target,
//! keeping its speed, and runs its `Action` for it. What arrives is
//! hidden for half a second; contacts that start meanwhile do not count,
//! so it does not bounce back through a teleport at the target
//! (`GenerateBackTeleport` makes one, id × 10 + 1). The camera jumps
//! along when the player's car goes far.
//!
//! On the floor it is a dark pulsing hole with streaks drawn into it,
//! off towards where it leads.
//!
//! Not ported: the screen flashes (`CreateCameraFlash`, the streak across
//! the screen, the burst on arrival), navigation links.

use std::f32::consts::TAU;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::actions::{AddMapAction, CloneOf, Run, RunAction, source_id};
use crate::content::actions::defs::CreateTeleport;
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::particles::styles::{TELEPORT_STREAK, TELEPORT_SWIRL};
use crate::render::camera::MainCamera;
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::sound::PlaySound;

const HOLE: &str = "Particles\\flash2";
/// How long what arrives stays hidden.
const HIDDEN_FOR: f32 = 0.5;
/// Streaks a second while in sight (`RandomEvent(0.25f)` a frame at 60).
const STREAKS_PER_SEC: f32 = 15.0;
/// How near the view's centre a teleport makes streaks.
const IN_SIGHT: f32 = 24.0;
/// A jump longer than this moves the camera at once (about a view's
/// width; C# compared with `Drawer.ViewWidth`).
const CAMERA_JUMP: f32 = 20.0;

#[derive(Component, Debug)]
pub struct Teleport {
    target: Vec2,
    action: Option<String>,
    /// Who it takes; `None`: anything.
    subscribers: Option<Vec<i64>>,
    radius: f32,
    hole: Entity,
    /// The hole's size at rest.
    hole_size: Vec3,
}

/// Just arrived: hidden, and not taken by teleports, for a moment.
#[derive(Component, Debug)]
pub struct Arriving(f32);

pub struct TeleportPlugin;

impl Plugin for TeleportPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CreateTeleport, _>(create_teleport)
            .add_action_sprites::<CreateTeleport>(&[HOLE])
            .add_systems(
                FixedUpdate,
                (take, arrive)
                    .chain()
                    .after(crate::explosion::ExplosionSystems)
                    .before(crate::actions::ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, (pulse, streaks).run_if(in_state(GameState::Playing)));
    }
}

fn create_teleport(
    In(run): In<Run<CreateTeleport>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> bool {
    let d = run.data;
    let Some(hole) = map.sprites.get(HOLE) else {
        return false;
    };
    let (from, to) = (map.world(d.position.x, d.position.y), map.world(d.target_position.x, d.target_position.y));
    let mut ends = vec![(d.id, from, to)];
    if d.generate_back_teleport {
        ends.push((d.id * 10 + 1, to, from));
    }
    let size = hole.size * crate::units::px(1.0) * d.scale;
    for (id, at, target) in ends {
        let tint = SpriteTint {
            color: [0.0, 0.0, 0.0, 1.0],
            lit: false,
        };
        let sprite = sprite_bundle(hole, d.scale, Layer::GroundDecor, tint, &quad, &mut materials);
        let hole_size = sprite.2.scale;
        let view = commands.spawn(sprite).id();
        commands
            .spawn((
                Name::new(format!("Teleport {id}")),
                MapEntity,
                MapId(id),
                Transform::from_translation(at.extend(0.0)),
                Visibility::default(),
                RigidBody::Static,
                Collider::rectangle(size.x, size.y),
                Sensor,
                CollisionEventsEnabled,
                Teleport {
                    target,
                    action: d.action.clone().filter(|a| !a.trim().is_empty()),
                    subscribers: d.subscribers.clone(),
                    radius: size.max_element() / 2.0,
                    hole: view,
                    hole_size,
                },
            ))
            .add_child(view);
    }
    true
}

/// `OnCollide` and `TeleportActivate`.
#[allow(clippy::type_complexity)]
fn take(
    mut commands: Commands,
    mut started: MessageReader<CollisionStart>,
    teleports: Query<&Teleport>,
    mut bodies: Query<(&RigidBody, &mut Position, Option<&MapId>, Has<LocalPlayer>, Option<&CloneOf>), Without<Arriving>>,
    mut camera: Query<&mut MainCamera>,
    mut run: MessageWriter<RunAction>,
    mut sound: MessageWriter<PlaySound>,
) {
    for event in started.read() {
        for (sensor, other) in [(event.collider1, event.body2), (event.collider2, event.body1)] {
            let (Ok(t), Some(other)) = (teleports.get(sensor), other) else {
                continue;
            };
            let Ok((RigidBody::Dynamic, mut pos, map_id, player, clone)) = bodies.get_mut(other) else {
                continue;
            };
            let id = map_id.map(|id| id.0);
            if let Some(subs) = &t.subscribers
                && !map_id.is_some_and(|i| subs.contains(&source_id(i, clone)))
            {
                continue;
            }
            sound.write(PlaySound {
                name: "Teleport",
                at: Some(pos.0),
                pitch: 0.0,
                volume: 1.0,
            });
            if let Some(action) = &t.action {
                run.write(RunAction {
                    name: action.clone(),
                    actor: id,
                });
            }
            let jump = t.target - pos.0;
            if player
                && jump.length() > CAMERA_JUMP
                && let Ok(mut cam) = camera.single_mut()
            {
                cam.ground += jump;
                cam.focus += jump;
            }
            pos.0 = t.target;
            commands.entity(other).insert((Arriving(HIDDEN_FOR), Visibility::Hidden));
        }
    }
}

fn arrive(mut commands: Commands, time: Res<Time>, mut arriving: Query<(Entity, &mut Arriving)>) {
    for (e, mut a) in &mut arriving {
        a.0 -= time.delta_secs();
        if a.0 <= 0.0 {
            commands.entity(e).remove::<Arriving>().insert(Visibility::Inherited);
        }
    }
}

/// `Teleport.Draw`: the hole breathes, 0.8 to 1.6 times its size twice a
/// second (`PhaseHelper.Triangle_Period500`).
fn pulse(time: Res<Time>, teleports: Query<&Teleport>, mut views: Query<&mut Transform>) {
    let phase = (time.elapsed_secs() / 0.5).fract();
    let triangle = 1.0 - (phase * 2.0 - 1.0).abs();
    for t in &teleports {
        if let Ok(mut transform) = views.get_mut(t.hole) {
            transform.scale = t.hole_size * (0.8 + 0.8 * triangle);
        }
    }
}

/// `Teleport.Update` / `TeleportEffect`: half the time a glow turning
/// into the middle, half a streak from somewhere on it towards the target.
fn streaks(
    time: Res<Time>,
    teleports: Query<(&Teleport, &Transform)>,
    camera: Query<&MainCamera>,
    mut particles: ResMut<Particles>,
    mut rng: Local<Option<SmallRng>>,
) {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(29));
    let Ok(cam) = camera.single() else {
        return;
    };
    for (t, transform) in &teleports {
        let at = transform.translation.truncate();
        if at.distance(cam.focus) > IN_SIGHT || rng.random::<f32>() > STREAKS_PER_SEC * time.delta_secs() {
            continue;
        }
        if rng.random::<bool>() {
            let dir = rng.random::<f32>() * TAU;
            particles.try_add(&TELEPORT_SWIRL, at, dir, dir);
        } else {
            let off = Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * t.radius;
            let dir = (t.target - at).to_angle();
            particles.try_add(&TELEPORT_STREAK, at + off, dir, dir);
        }
    }
}
