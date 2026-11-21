//! Actions a map item runs for itself (`TriggerManager.OnNewEntity`,
//! `OnEntityDestroy`): `ActionOnCreate` when it appears (at map start or
//! as a clone, looked up by its source), `ActionOnDestroy` when the game
//! destroys it (`DestroyEntity`, a bomb going off, a simple car's wreck
//! removed two seconds after it died). Leaving the map does not count.
//!
//! The player's car runs its `ActionOnCreate` at map start only: coming
//! back at a respawn point it is the same car in C#. Its `ActionOnDestroy`
//! does not run, as it is wrecked rather than destroyed (`game_mode`);
//! the other `Players` cars run theirs as they are wrecked (`car::death`).

use bevy::prelude::*;

use super::{CloneOf, RunAction, source_id};
use crate::game::{GameState, LoadedMap, MapId};

/// A car brought back at a respawn point: not new.
#[derive(Component, Debug)]
pub struct Respawned;

/// Run when the game destroys this entity.
#[derive(Component, Debug, Clone)]
pub struct OnDestroy(pub String);

pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, on_create.before(super::ActionSystems).run_if(in_state(GameState::Playing)));
    }
}

/// The map's `ActionOnCreate` / `ActionOnDestroy` of an id.
fn actions_of(map: &LoadedMap, id: i64) -> (Option<String>, Option<String>) {
    let named = |a: &Option<String>| a.clone().filter(|a| !a.trim().is_empty());
    if let Some(e) = map.data.entities.iter().find(|e| e.id == id) {
        return (named(&e.action_on_create), named(&e.action_on_destroy));
    }
    if let Some(p) = map.data.players.items.iter().find(|p| p.id == id) {
        return (named(&p.action_on_create), named(&p.action_on_destroy));
    }
    if let Some(c) = map.data.simple_cars.items.iter().find(|c| c.id == id) {
        return (named(&c.action_on_create), named(&c.action_on_destroy));
    }
    (None, None)
}

#[allow(clippy::type_complexity)]
fn on_create(
    mut commands: Commands,
    map: Res<LoadedMap>,
    new: Query<(Entity, &MapId, Option<&CloneOf>, Has<Respawned>), Added<MapId>>,
    mut run: MessageWriter<RunAction>,
) {
    for (e, id, clone, respawned) in &new {
        let (create, destroy) = actions_of(&map, source_id(id, clone));
        if let Some(destroy) = destroy {
            commands.entity(e).insert(OnDestroy(destroy));
        }
        if let Some(name) = create.filter(|_| !respawned) {
            run.write(RunAction { name, actor: Some(id.0) });
        }
    }
}

/// `OnEntityDestroy`: the action of an entity the game destroys, if any.
pub fn destroy_action(on_destroy: Option<&OnDestroy>, id: Option<&MapId>) -> Option<RunAction> {
    Some(RunAction {
        name: on_destroy?.0.clone(),
        actor: id.map(|id| id.0),
    })
}
