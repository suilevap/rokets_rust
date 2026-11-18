//! What the camera keeps in frame (`Camera2DOperatoAllInCadr`):
//! `AddEntitiesToFocus` (templates `FocusOnEntity`, `AddFocusOnEntity`),
//! `RemoveEntitiesFromFocus` and `InstantCameraMove`.
//!
//! The camera frames every entity with a [`CameraTarget`] that is not
//! [`Unfocused`]; the player's car starts as the only one. Adding an
//! entity gives it a target its size if it has none (no looking ahead:
//! that is the car's); `RemoveAllOthers` first leaves every other one
//! out. A car left out keeps its settings for when it comes back.
//!
//! Each also ends a camera flight (`CameraShowPath`): back to the targets.
//!
//! Not ported: the long screen flash of `InstantCameraMove`.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{AddEntitiesToFocus, InstantCameraMove, RemoveEntitiesFromFocus};
use crate::game::{LoadedMap, MapId};
use crate::render::camera::{CameraTarget, MainCamera, Unfocused};

pub struct FocusActionsPlugin;

impl Plugin for FocusActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddEntitiesToFocus, _>(add.pipe(back_to_targets))
            .add_map_action::<RemoveEntitiesFromFocus, _>(remove.pipe(back_to_targets))
            .add_map_action::<InstantCameraMove, _>(instant_move.pipe(back_to_targets));
    }
}

/// `GetDefaultOperator`: the camera on its targets again.
fn back_to_targets(In(ok): In<bool>, mut commands: Commands) -> bool {
    commands.run_system_cached(super::stop_camera_path);
    ok
}

fn add(
    In(run): In<Run<AddEntitiesToFocus>>,
    ids: MapIds,
    targets: Query<Entity, With<CameraTarget>>,
    sizes: Query<(Has<CameraTarget>, Option<&ColliderAabb>)>,
    mut commands: Commands,
) -> bool {
    if run.data.remove_all_others {
        for e in &targets {
            commands.entity(e).insert(Unfocused);
        }
    }
    for &id in &run.data.item_ids {
        let Some(e) = ids.entity(run.target(id)) else {
            continue;
        };
        let Ok((has_target, aabb)) = sizes.get(e) else {
            continue;
        };
        commands.entity(e).remove::<Unfocused>();
        if !has_target {
            let radius = aabb.map_or(1.0, |a| a.size().max_element() / 2.0);
            commands.entity(e).insert(CameraTarget { radius, max_speed: 0.0 });
        }
    }
    true
}

/// By map id as listed (C# did not stand in the actor for 0 here).
fn remove(
    In(run): In<Run<RemoveEntitiesFromFocus>>,
    targets: Query<(Entity, &MapId), With<CameraTarget>>,
    mut commands: Commands,
) -> bool {
    for (e, id) in &targets {
        if run.data.item_ids.contains(&id.0) {
            commands.entity(e).insert(Unfocused);
        }
    }
    true
}

/// `InstantMove`: the camera is there at once, then follows its targets
/// from there.
fn instant_move(In(run): In<Run<InstantCameraMove>>, map: Res<LoadedMap>, mut cameras: Query<&mut MainCamera>) -> bool {
    let at = map.world(run.data.position.x, run.data.position.y);
    for mut cam in &mut cameras {
        cam.ground = at;
        cam.focus = at;
    }
    true
}
