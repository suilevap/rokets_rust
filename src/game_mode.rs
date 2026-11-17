//! What happens when the player's car is destroyed: the single-player
//! game mode (`GameModeSinglePlayer`, `Car.OnKilled`,
//! `ControlableEntity.CreateDestroyedEntity`).
//!
//! The car turns into its wreck (`Destroyed1` or `Destroyed2`): it keeps
//! its speed and spin but nobody drives or brakes it any more, so a blast
//! throws it far. It bursts (`CreateCarParts`: black smoke and fiery
//! streaks), and two seconds later (`RespawnTime / 2`) the level starts
//! again.
//!
//! C# restarted the level unless an action had set a respawn point
//! (`RestartOnDeath`); then the car came back there, three lives a
//! level. Those actions are not ported yet, so this is always a restart.

use std::f32::consts::PI;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use crate::car::smoke::SmokePoints;
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::explosion::Destroyed;
use crate::game::{ChangeMap, GameState, LoadedMap};
use crate::particles::styles::{CRASH_SMOKE, WRECK_FIRE, WRECK_SPARK};
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// `GameModeSinglePlayer.RespawnTime / 2`: seconds from the wreck to the
/// restart.
const RESTART_AFTER: f32 = 2.0;

/// The sprites a car turns into when destroyed, and their scale.
#[derive(Component, Debug, Clone)]
pub struct Wreckable {
    pub sprites: [String; 2],
    pub scale: f32,
}

/// A destroyed car, until the level restarts.
#[derive(Component, Debug)]
pub struct Wreck {
    restart_in: f32,
}

#[derive(Resource)]
struct WreckRng(SmallRng);

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WreckRng(rand::SeedableRng::seed_from_u64(5)))
            .add_systems(
                Update,
                (wreck_cars, restart_after_wreck).run_if(in_state(GameState::Playing)),
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
    cars: Query<(&Wreckable, &Transform, Option<&Children>), (With<LocalPlayer>, Without<Wreck>)>,
) {
    let rng = &mut rng.0;
    for Destroyed(entity) in destroyed.read() {
        let Ok((wreckable, transform, children)) = cars.get(*entity) else {
            continue;
        };
        let mut car = commands.entity(*entity);
        car.remove::<(LocalPlayer, PlayerIntent, SmokePoints)>()
            .insert(Wreck {
                restart_in: RESTART_AFTER,
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
            let sprite = sprite_bundle(info, wreckable.scale, Layer::CarBase, lit, &quad, &mut materials);
            commands.entity(*entity).with_child(sprite);
        }
        burst(&mut particles, rng, transform.translation.truncate());
    }
}

/// `CreateCarParts` without the flying parts (a frame strip, not ported):
/// `CreateSmokeCrash(pos, 3)` and `CreateBoom`.
fn burst(particles: &mut Particles, rng: &mut SmallRng, at: Vec2) {
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

fn restart_after_wreck(time: Res<Time>, mut wrecks: Query<&mut Wreck>, mut change: MessageWriter<ChangeMap>) {
    for mut wreck in &mut wrecks {
        wreck.restart_in -= time.delta_secs();
        if wreck.restart_in <= 0.0 {
            wreck.restart_in = f32::INFINITY;
            change.write(ChangeMap::Restart);
        }
    }
}
