//! What becomes of a car that is not the player's when its health runs
//! out (`ControlableEntity.TakeDamage`, `OnKilled`, `Update`).
//!
//! - It bursts (`CreateCarParts`: smoke, a fireball and fiery streaks; the
//!   flying parts are not ported, as with the player's wreck) and sounds
//!   `Accident` (`sound`).
//! - It turns into its wreck (`Destroyed1` or `Destroyed2` of its sprites
//!   in the same place and scale); nobody drives it, its gun is gone and
//!   its health bar too; a static car turns dynamic, so a blast can throw
//!   it (`PhysicBody.IsStatic = false`).
//! - For two seconds it smokes (`CreateDamagedSmoke(pos, 0.02)` a frame),
//!   then it is removed and its `ActionOnDestroy` runs
//!   (`SimpleCarCorpseTime`, `Destroy`, `OnEntityDestroy`).
//!
//! An AI's `Car` (another of the map's `Players`) dies as `Car.OnKilled`
//! and `GameModeSinglePlayer.OnNewAvatarCreate` have it (no local device
//! and `RespawnAll` off: `SheduleDestroy`): it bursts and sounds as above,
//! and its wreck (`CreateDestroyedEntity`: a `Box` with the wreck's sprite,
//! the car's speed and spin) stays to the end of the level, a plain body
//! the magnet may grab, which smokes not (a box does not) and does not move
//! if the car could not (`SetRestrictions` all on a static car). The car
//! itself is gone at once: its map id is free and its `ActionOnDestroy`
//! runs (`Destroy`, `OnEntityDestroy`); nobody drives it, its gun, magnet,
//! sounds and health bar are gone, the camera lets it go.
//!
//! The player's car has its own (`game_mode`: restart, lives, respawn), as
//! long as the devices drive it or it is held for them (`LocalDevice`). A
//! car that was the player's and has a control of another kind (Revenge's
//! `RemovePlayerControl`: `NoneFriend`) dies as an AI's `Car` does
//! (`GameModeSinglePlayer` asked `LocalDevice`): its `ActionOnDestroy`
//! runs, which is how Revenge hands the player another car.
//!
//! Not ported: the red border that flashes round a dead enemy
//! (`HighlightDeadEnemy`) and the flying car parts a damaged or destroyed
//! car throws (`CreateCarPartOnDamage`, a frame strip); the wreck of an
//! AI's `Car` keeps the car's collider, mass and damping (C#'s `Box` took
//! the wreck sprite's size and its density's mass).

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::cabin::CabinSway;
use super::control::{AutoAimVisible, LocalDevice, Team};
use super::smoke::SmokePoints;
use super::Car;
use super::health_bar::HealthBar;
use super::shield::Shield;
use super::simple::SimpleDrive;
use crate::actions::{OnDestroy, RunAction, destroy_action};
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::explosion::{Destroyed, ExplosionSystems, Health};
use crate::game::{GameState, LoadedMap, MapId};
use crate::gadget::accelerator::Accelerator;
use crate::gadget::{Linkable, Magnet};
use crate::render::camera::CameraTarget;
use crate::sound::CarSounds;
use crate::game_mode::{Wreck, Wreckable, burst};
use crate::particles::styles::DAMAGED_SMOKE;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::weapon::{PlayerEnergy, Weapon};

/// `Settings.SimpleCarCorpseTime`: seconds from the wreck to its removal.
pub const CORPSE_TIME: f32 = 2.0;
/// The share of frames the wreck puffs smoke.
const SMOKE_DENSITY: f32 = 0.02;

/// A destroyed car, until it is removed.
#[derive(Component, Debug)]
pub struct Corpse {
    pub left: f32,
}

#[derive(Resource)]
struct DeathRng(SmallRng);

pub struct DeathPlugin;

impl Plugin for DeathPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DeathRng(SmallRng::seed_from_u64(13)))
            .add_systems(
                FixedUpdate,
                (kill_cars, remove_corpses, corpse_smoke)
                    .chain()
                    .after(ExplosionSystems)
                    .before(crate::actions::ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn kill_cars(
    mut commands: Commands,
    mut destroyed: MessageReader<Destroyed>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<DeathRng>,
    cars: Query<
        (&Wreckable, &Transform, Option<&Children>, &RigidBody, Has<Car>, Option<&OnDestroy>, Option<&MapId>),
        (With<Team>, Or<(Without<LocalPlayer>, Without<LocalDevice>)>, Without<Corpse>, Without<Wreck>),
    >,
    mut run: MessageWriter<RunAction>,
) {
    let rng = &mut rng.0;
    for Destroyed(entity) in destroyed.read() {
        let Ok((wreckable, transform, children, body, ai_car, on_destroy, id)) = cars.get(*entity) else {
            continue;
        };
        let mut car = commands.entity(*entity);
        // Nobody drives it, shoots, soaks or shows a bar any more; a blow
        // is no blow to it (no health).
        car.remove::<(Weapon, PlayerIntent, SimpleDrive, HealthBar, Health, Shield, LocalPlayer, PlayerEnergy)>();
        if ai_car {
            // The car is gone, its wreck a plain box that stays.
            crate::ai::set_control(&mut car, "", 0);
            car.remove::<(Car, CarSounds, SmokePoints, CabinSway, Magnet, Accelerator, CameraTarget, MapId)>()
                .remove::<(Team, AutoAimVisible, LocalDevice)>();
            // A static car's stays put: nothing to pull.
            if *body != RigidBody::Static {
                car.insert(Linkable);
            }
            run.write_batch(destroy_action(on_destroy, id));
        } else {
            car.insert(Corpse { left: CORPSE_TIME });
            if *body == RigidBody::Static {
                car.insert((RigidBody::Dynamic, TransformInterpolation));
            }
        }
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

/// `CreateDamagedSmoke`: a puff now and then, rising off the wreck.
fn corpse_smoke(corpses: Query<&Transform, With<Corpse>>, mut particles: ResMut<Particles>, mut rng: ResMut<DeathRng>) {
    let rng = &mut rng.0;
    for t in &corpses {
        if rng.random::<f32>() < SMOKE_DENSITY {
            // C#: `π/2 − π/4 + rnd × π/2` (y down: a downward half turn,
            // here the same way on the screen).
            let direction = -(std::f32::consts::FRAC_PI_4 + rng.random::<f32>() * std::f32::consts::FRAC_PI_2);
            let view = rng.random::<f32>() * std::f32::consts::TAU;
            particles.try_add(&DAMAGED_SMOKE, t.translation.truncate(), direction, view);
        }
    }
}

/// After two seconds the wreck goes (`Destroy`) and its map action runs.
#[allow(clippy::type_complexity)]
fn remove_corpses(
    mut commands: Commands,
    time: Res<Time>,
    mut corpses: Query<(Entity, &mut Corpse, Option<&OnDestroy>, Option<&MapId>)>,
    mut run: MessageWriter<RunAction>,
) {
    for (e, mut corpse, on_destroy, id) in &mut corpses {
        corpse.left -= time.delta_secs();
        if corpse.left <= 0.0 {
            commands.entity(e).despawn();
            run.write_batch(destroy_action(on_destroy, id));
        }
    }
}
