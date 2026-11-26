//! The lock marker: a ring on the car the player's car aims at.
//!
//! A red ring (`UI/BigCircle`, the touch gadget's circle), a bit
//! wider than the target and turning slowly, drawn above everything in the
//! world. A locked target gets a bright one; the soft target, picked for
//! the player while nothing is locked, a faint one, so it can be seen where
//! the gun is going to point. One marker entity does for the player's car; it
//! lives with the map. The magnet's job has its own green ring, on the thing
//! to grab or at the point to drop it (fainter while it waits for the car to
//! come in reach); the same ring closes on the player's own car while a
//! boost holds the accelerator's button.

use bevy::prelude::*;
use avian2d::prelude::{Collider, Position};

use super::Assist;
use super::magnet::{Job, Phase};
use crate::car::CarRadius;
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// The ring's picture.
pub const RING: &str = "UI/BigCircle";
/// Opacity of the lock's ring and of the soft target's.
const LOCK_ALPHA: f32 = 0.9;
const SOFT_ALPHA: f32 = 0.3;
/// The ring's diameter in the target's diameters.
const SIZE: f32 = 1.8;
/// Turns a second.
const TURNS: f32 = 0.25;

/// The textures the marker needs, for the map's sprite list.
pub fn textures() -> [&'static str; 1] {
    [RING]
}

/// The magnet job's ring colour (green, against the lock's red) and its
/// opacity waiting for the car to come in reach and when the button is in
/// the assist's hand.
const CARGO_COLOR: [f32; 4] = [0.2, 1.0, 0.3, 0.0];
const CARGO_WAITING_ALPHA: f32 = 0.45;
const CARGO_ALPHA: f32 = 0.9;
/// The ring at a drop point, in units across.
const DROP_DIAMETER: f32 = 1.4;
/// The ring on a grab target, in the target's sizes, and its size when the
/// target has none.
const CARGO_SIZE: f32 = 1.5;
const DEFAULT_CARGO: f32 = 0.5;

#[derive(Component)]
struct LockMarker;

/// The ring of the magnet's job: on what is to be grabbed, or at the point
/// the cargo is to be dropped.
#[derive(Component)]
struct CargoMarker;

pub struct AssistViewPlugin;

impl Plugin for AssistViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (show_marker, show_cargo_marker).run_if(in_state(GameState::Playing)));
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_marker(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<&Assist, With<LocalPlayer>>,
    cars: Query<(&Position, &CarRadius)>,
    mut markers: Query<(Entity, &mut Transform, &mut Visibility, &mut SpriteTint), With<LockMarker>>,
) {
    let aimed = players.iter().find_map(|a| {
        let (target, alpha) = a.lock.map(|e| (e, LOCK_ALPHA)).or(a.soft.map(|e| (e, SOFT_ALPHA)))?;
        cars.get(target).ok().map(|(at, radius)| (at.0, radius.0, alpha))
    });
    let Some((_, mut transform, mut visibility, mut tint)) = markers.iter_mut().next() else {
        // Not there yet: make it, hidden, once something is aimed at.
        if aimed.is_some()
            && let Some(info) = map.sprites.get(RING)
        {
            debug!("assist: lock marker made");
            let tint = SpriteTint { color: [1.0, 0.1, 0.1, 0.0], lit: false };
            commands.spawn((
                Name::new("Lock marker"),
                MapEntity,
                LockMarker,
                Visibility::Hidden,
                sprite_bundle(info, 1.0, Layer::Hint, tint, &quad, &mut materials),
            ));
        }
        return;
    };
    match aimed {
        Some((at, radius, alpha)) => {
            debug!("assist: marker at {at:?} radius {radius} alpha {alpha}");
            let diameter = radius * 2.0 * SIZE;
            transform.translation = at.extend(Layer::Hint.z());
            transform.rotation = Quat::from_rotation_z(-time.elapsed_secs() * TURNS * std::f32::consts::TAU);
            transform.scale = Vec3::new(diameter, diameter, 1.0);
            tint.color[3] = alpha;
            visibility.set_if_neq(Visibility::Inherited);
        }
        None => {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Where the magnet's job is shown: the middle and diameter of the ring, and
/// its opacity.
fn cargo_ring(
    car: Entity,
    assist: &Assist,
    things: &Query<(&Position, Option<&Collider>, Option<&CarRadius>)>,
) -> Option<(Vec2, f32, f32)> {
    // A boost in hand: the ring on the player's own car.
    if assist.boost.is_some() {
        let (at, _, radius) = things.get(car).ok()?;
        return Some((at.0, radius.map_or(DEFAULT_CARGO, |r| r.0) * 2.0 * CARGO_SIZE, CARGO_ALPHA));
    }
    let handling = assist.cargo?;
    let alpha = if handling.phase == Phase::Waiting { CARGO_WAITING_ALPHA } else { CARGO_ALPHA };
    match handling.job {
        Job::Drop(at) => Some((at, DROP_DIAMETER, alpha)),
        Job::Grab(thing) => {
            let (at, collider, car) = things.get(thing).ok()?;
            let radius = car.map(|c| c.0).or_else(|| collider.map(|c| c.shape_scaled().compute_local_aabb().half_extents().max_element())).unwrap_or(DEFAULT_CARGO);
            Some((at.0, radius * 2.0 * CARGO_SIZE, alpha))
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_cargo_marker(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &Assist), With<LocalPlayer>>,
    things: Query<(&Position, Option<&Collider>, Option<&CarRadius>)>,
    mut markers: Query<(&mut Transform, &mut Visibility, &mut SpriteTint), With<CargoMarker>>,
) {
    let ring = players.iter().find_map(|(car, a)| cargo_ring(car, a, &things));
    let Some((mut transform, mut visibility, mut tint)) = markers.iter_mut().next() else {
        if ring.is_some()
            && let Some(info) = map.sprites.get(RING)
        {
            debug!("assist: cargo marker made");
            let tint = SpriteTint { color: CARGO_COLOR, lit: false };
            commands.spawn((
                Name::new("Cargo marker"),
                MapEntity,
                CargoMarker,
                Visibility::Hidden,
                sprite_bundle(info, 1.0, Layer::Hint, tint, &quad, &mut materials),
            ));
        }
        return;
    };
    match ring {
        Some((at, diameter, alpha)) => {
            transform.translation = at.extend(Layer::Hint.z());
            transform.rotation = Quat::from_rotation_z(time.elapsed_secs() * TURNS * std::f32::consts::TAU);
            transform.scale = Vec3::new(diameter, diameter, 1.0);
            tint.color[3] = alpha;
            visibility.set_if_neq(Visibility::Inherited);
        }
        None => {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}
