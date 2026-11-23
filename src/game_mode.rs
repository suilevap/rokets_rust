//! What happens when the player's car is destroyed: the single-player
//! game mode (`GameModeSinglePlayer`, `Car.OnKilled`,
//! `ControlableEntity.CreateDestroyedEntity`).
//!
//! The car turns into its wreck (`Destroyed1` or `Destroyed2`): it keeps
//! its speed and spin but nobody drives or brakes it any more, so a blast
//! throws it far. It bursts (`CreateCarParts`: black smoke and fiery
//! streaks). Two seconds later (`RespawnTime / 2`):
//!
//! - until an action sets a respawn point, the level starts again
//!   (`RestartOnDeath`);
//! - with one, a life goes (three a level; with none left the level starts
//!   again), the point's `ActionOnRespawn` runs, and two seconds after
//!   that the wreck goes up in smoke and the car is back at the point,
//!   whole, the camera on it alone (`OnRespawn`, `Car.Respawn`).
//!
//! The point is set by `SetRespawnPoint` (a place and heading) or
//! `SaveAction` (`Save()`: where the car is now, with an action to run on
//! respawn).
//!
//! Other cars' deaths are `car::death`'s.
//!
//! Not ported: the respawn flashes (`CreateRespawn`), the lives shown on
//! screen (`ShowEnergy`; the top line shows them).

use std::f32::consts::PI;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use crate::actions::{AddMapAction, MapIds, Respawned, Run, RunAction};
use crate::car::smoke::SmokePoints;
use crate::car::spawn_local_car;
use crate::content::actions::defs::{SaveAction, SetRespawnPoint};
use crate::controls::{LocalPlayer, NoInput, PlayerIntent};
use crate::explosion::{Destroyed, ExplosionSystems};
use crate::game::{ChangeMap, GameState, LoadedMap, MapId};
use crate::render::camera::{CameraTarget, Unfocused};
use crate::particles::styles::{CRASH_SMOKE, WRECK_FIRE, WRECK_SPARK};
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::map_degrees_to_world;

/// `GameModeSinglePlayer.RespawnTime / 2`: seconds from the wreck to the
/// restart, and from there to the car coming back.
const RESTART_AFTER: f32 = 2.0;
/// `_maxLives`.
const LIVES: i32 = 3;

/// Where the player's car comes back (`LastRespawnPoint`), and its lives.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Respawn {
    /// World position and heading; `None`: death restarts the level.
    pub point: Option<(Vec2, f32)>,
    /// Run for the car as it comes back.
    pub action: Option<String>,
    pub lives: i32,
}

impl Default for Respawn {
    fn default() -> Self {
        Respawn {
            point: None,
            action: None,
            lives: LIVES,
        }
    }
}

impl Respawn {
    /// A death: whether the car comes back (a life spent) or the level
    /// starts again (`OnRespawn`, `UpdateLives`).
    pub fn on_death(&mut self) -> bool {
        if self.point.is_none() {
            return false;
        }
        self.lives -= 1;
        self.lives >= 0
    }
}

/// The sprites a car turns into when destroyed, and their scale.
#[derive(Component, Debug, Clone)]
pub struct Wreckable {
    pub sprites: [String; 2],
    pub scale: f32,
    /// Where the wreck is drawn (the car's own sprite's layer).
    pub layer: Layer,
}

/// A destroyed car, until the level restarts or the car comes back.
#[derive(Component, Debug)]
pub struct Wreck {
    restart_in: f32,
    /// The car's map id, for its respawn action.
    id: Option<i64>,
    /// Counting down to the car coming back (after the restart's time).
    back_in: Option<f32>,
}

#[derive(Resource)]
struct WreckRng(SmallRng);

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WreckRng(rand::SeedableRng::seed_from_u64(5)))
            .init_resource::<Respawn>()
            .add_map_action::<SetRespawnPoint, _>(set_respawn_point)
            .add_map_action::<SaveAction, _>(save)
            .add_systems(OnEnter(GameState::Playing), |mut r: ResMut<Respawn>| *r = Respawn::default())
            .add_systems(
                FixedUpdate,
                (wreck_cars, restart_after_wreck)
                    .chain()
                    .after(ExplosionSystems)
                    .before(crate::actions::ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn wreck_cars(
    mut commands: Commands,
    mut destroyed: MessageReader<Destroyed>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<WreckRng>,
    cars: Query<(&Wreckable, &Transform, Option<&Children>, Option<&MapId>), (With<LocalPlayer>, Without<Wreck>)>,
) {
    let rng = &mut rng.0;
    for Destroyed(entity) in destroyed.read() {
        let Ok((wreckable, transform, children, id)) = cars.get(*entity) else {
            continue;
        };
        let mut car = commands.entity(*entity);
        // The wreck keeps no map id: a new car takes it.
        car.remove::<(LocalPlayer, PlayerIntent, SmokePoints, MapId, crate::weapon::Weapon, crate::weapon::PlayerEnergy, crate::gadget::accelerator::Accelerator)>()
            .insert(Wreck {
                restart_in: RESTART_AFTER,
                id: id.map(|id| id.0),
                back_in: None,
            });
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        let name = &wreckable.sprites[rng.random_range(0..2)];
        if let Some(info) = map.sprites.get(name) {
            let lit = SpriteTint {
                color: [1.0; 4],
                lit: true,
            };
            let sprite = sprite_bundle(info, wreckable.scale, wreckable.layer, lit, &quad, &mut materials);
            commands.entity(*entity).with_child(sprite);
        }
        burst(&mut particles, rng, transform.translation.truncate());
    }
}

/// `CreateCarParts` without the flying parts (a frame strip, not ported):
/// `CreateSmokeCrash(pos, 3)` and `CreateBoom`.
pub(crate) fn burst(particles: &mut Particles, rng: &mut SmallRng, at: Vec2) {
    for _ in 0..3 {
        // Within a unit of the wreck either way.
        let offset = Vec2::new(rng.random::<f32>() - 0.5, rng.random::<f32>() - 0.5) * 2.0;
        let (dir, view) = (rng.random::<f32>() * 2.0 * PI, rng.random::<f32>() * 2.0 * PI);
        particles.try_add(&CRASH_SMOKE, at + offset, dir, view);
    }
    // Up to two units out (64 px), flying on outwards.
    for style in [&WRECK_FIRE, &WRECK_SPARK] {
        let dir = rng.random::<f32>() * 2.0 * PI;
        let out = Vec2::from_angle(dir) * rng.random::<f32>() * 2.0;
        particles.try_add(style, at + out, dir, dir);
    }
}

/// `SetRespawnPoint`: a place and heading to come back to.
fn set_respawn_point(In(run): In<Run<SetRespawnPoint>>, map: Res<LoadedMap>, mut respawn: ResMut<Respawn>) -> bool {
    let d = &run.data;
    respawn.point = Some((map.world(d.position.x, d.position.y), map_degrees_to_world(d.direction_deg)));
    respawn.action = None;
    true
}

/// `SaveAction`, `Save()`: come back where the car is now.
fn save(
    In(run): In<Run<SaveAction>>,
    ids: MapIds,
    places: Query<&Transform>,
    mut respawn: ResMut<Respawn>,
) -> bool {
    let Some(t) = ids.entity(run.target(run.data.item_id)).and_then(|e| places.get(e).ok()) else {
        return false;
    };
    respawn.point = Some((t.translation.truncate(), t.rotation.to_euler(EulerRot::ZYX).0));
    respawn.action = run.data.action_on_respawn.clone().filter(|a| !a.trim().is_empty());
    true
}

#[allow(clippy::too_many_arguments)]
fn restart_after_wreck(
    mut commands: Commands,
    time: Res<Time>,
    mut wrecks: Query<(Entity, &mut Wreck, &Transform, Has<NoInput>)>,
    mut respawn: ResMut<Respawn>,
    (map, quad): (Res<LoadedMap>, Res<UnitQuad>),
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<WreckRng>,
    targets: Query<Entity, With<CameraTarget>>,
    mut change: MessageWriter<ChangeMap>,
    mut run: MessageWriter<RunAction>,
) {
    let dt = time.delta_secs();
    for (e, mut wreck, transform, no_input) in &mut wrecks {
        if let Some(back_in) = wreck.back_in.as_mut() {
            *back_in -= dt;
            if *back_in <= 0.0 {
                let Some((at, heading)) = respawn.point else {
                    continue;
                };
                // `CreateSmokeCrash(_corpse.Position, 1)`.
                let (d, v) = (rng.0.random::<f32>() * 2.0 * PI, rng.0.random::<f32>() * 2.0 * PI);
                particles.try_add(&CRASH_SMOKE, transform.translation.truncate(), d, v);
                commands.entity(e).despawn();
                // The camera on the car alone (`OnRespawn`).
                for t in targets.iter().filter(|&t| t != e) {
                    commands.entity(t).try_insert(Unfocused);
                }
                if let Some(car) = spawn_local_car(&mut commands, &map, &quad, &mut materials, Some((at, Some(heading)))) {
                    // It comes back driven as it was (C# kept its control).
                    commands.entity(car).insert(Respawned);
                    if !no_input {
                        commands.entity(car).remove::<NoInput>();
                    }
                }
            }
            continue;
        }
        wreck.restart_in -= dt;
        if wreck.restart_in > 0.0 {
            continue;
        }
        wreck.restart_in = f32::INFINITY;
        if respawn.on_death() {
            info!("{} lives left", respawn.lives);
            wreck.back_in = Some(RESTART_AFTER);
            if let Some(action) = respawn.action.clone() {
                run.write(RunAction { name: action, actor: wreck.id });
            }
        } else {
            change.write(ChangeMap::Restart);
        }
    }
}
