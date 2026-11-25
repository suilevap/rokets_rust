//! The "collected" ring of `SetWeapon`: a white ring with a gun (or a
//! magnet) in it in the middle of the screen, when the player's car is
//! handed one without `ChangeSilently` (`CreateWeaponCollectedUI`,
//! `CreateMagnetCollectedUI`).
//!
//! C# origin: `ParticleSystemManager._weaponCollectUIStyle` and `…2`,
//! `_magnetCollectUIStyle` and `…2` in the UI particle pool (drawn over
//! everything, `DepthConst.SuccessUI`), at `Drawer.ViewSize / 2`.
//!
//! Two rings of the same sprite, tilted by the same angle (a fake random
//! up to ±π/8, here the views' own generator): a steady one at 0.8 of the
//! sprite that fades out over 3 s, and one that swells from 0.8 to 1.5 and
//! fades in half a second. The sizes grow with the screen as C#'s UI did
//! (`UISizeMultiplier`, from 800 × 480).
//!
//! Not ported: the slow-motion scaling of the particles' time (no slow
//! motion yet).

use std::f32::consts::FRAC_PI_8;

use bevy::prelude::*;
use rand::Rng;

use super::ViewRng;
use super::swap::WeaponCollected;
use super::view::ui_scale;
use crate::game::{GameState, LoadedMap, MapEntity};

/// The ring of a gun (`UI/BigCircleGun`).
pub const GUN_RING: &str = "UI/BigCircleGun";
/// The ring of the magnet (`UI/BigCircleGadget`).
pub const GADGET_RING: &str = "UI/BigCircleGadget";

/// One ring, in its life.
#[derive(Component, Debug)]
struct Ring {
    age: f32,
    life: f32,
    /// The scale at the start and at the end (of the sprite's own size,
    /// times the screen's).
    scale: (f32, f32),
}

pub struct CollectedPlugin;

impl Plugin for CollectedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_rings, animate_rings).chain().run_if(in_state(GameState::Playing)));
    }
}

fn spawn_rings(
    mut commands: Commands,
    mut collected: MessageReader<WeaponCollected>,
    map: Res<LoadedMap>,
    windows: Query<&Window>,
    mut rng: ResMut<ViewRng>,
) {
    let k = windows.iter().next().map_or(1.0, ui_scale);
    for WeaponCollected { gadget } in collected.read() {
        let Some(info) = map.sprites.get(if *gadget { GADGET_RING } else { GUN_RING }) else {
            continue;
        };
        let tilt = (rng.0.random::<f32>() * 2.0 - 1.0) * FRAC_PI_8;
        for (life, scale) in [(3.0, (0.8, 0.8)), (0.5, (0.8, 1.5))] {
            let size = info.size * k;
            commands.spawn((
                Name::new("Collected ring"),
                MapEntity,
                Ring { age: 0.0, life, scale },
                ImageNode::new(info.image.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect { left: Val::Px(-size.x / 2.0), top: Val::Px(-size.y / 2.0), ..default() },
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    ..default()
                },
                UiTransform { scale: Vec2::splat(scale.0), rotation: Rot2::radians(tilt), ..UiTransform::IDENTITY },
                Pickable::IGNORE,
            ));
        }
    }
}

fn animate_rings(mut commands: Commands, time: Res<Time>, mut rings: Query<(Entity, &mut Ring, &mut ImageNode, &mut UiTransform)>) {
    for (entity, mut ring, mut image, mut transform) in &mut rings {
        ring.age += time.delta_secs();
        let t = ring.age / ring.life;
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        image.color = Color::srgba(1.0, 1.0, 1.0, 1.0 - t);
        transform.scale = Vec2::splat(ring.scale.0 + (ring.scale.1 - ring.scale.0) * t);
    }
}
