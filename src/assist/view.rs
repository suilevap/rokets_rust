//! The lock marker: a ring on the car the player's car aims at.
//!
//! A red ring (`UI/BigCircle`, the touch gadget's circle), a bit
//! wider than the target and turning slowly, drawn above everything in the
//! world. A locked target gets a bright one; the soft target, picked for
//! the player while nothing is locked, a faint one, so it can be seen where
//! the gun is going to point. One marker entity does for the player's car; it
//! lives with the map.

use bevy::prelude::*;
use avian2d::prelude::Position;

use super::Assist;
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

#[derive(Component)]
struct LockMarker;

pub struct AssistViewPlugin;

impl Plugin for AssistViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_marker.run_if(in_state(GameState::Playing)));
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
