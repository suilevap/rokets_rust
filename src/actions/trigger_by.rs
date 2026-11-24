//! `TriggerActionBy` and `TriggerActionByFirstSuccess`: run actions for
//! each of a list of entities, as if that entity had set them off; and
//! `SuccessSequence`, the actions in turn until one fails.
//!
//! C# origin: `ActionRunner.RunActionForItems` ("Add Action Exectuion for
//! list of items", August 2013) and `TriggerActionByFirstSuccessRun`
//! (March 2014, with the actions that return a result and
//! `SuccessSequence`, for Swarm). `TriggerActionBy` is in 14 maps (32
//! actions: BattleCity, DeadCity, Dungeon, Escort, Escort2, MyGalaxy, Race,
//! Race2, Revenge, Swarm, Targets, TheCity, TheLastStand, UnderPressure),
//! always a list of ids and a list of action names (`Activate…`,
//! `DestroyEntity(0)`, a path's steps); Targets has one list with a
//! variable's name among the numbers (`var_ReferencedCoin`).
//! `TriggerActionByFirstSuccess` is in Swarm only: it offers its
//! `TryToChangeToLocalControl` to the cars of a multiplayer game one after
//! another until one takes it, and restarts the map if none does.
//!
//! Branches of the C# methods, all ported unless named below:
//! - `ItemIds` are read as `GetInt(itemId, text)`: a whole number, or the
//!   name of a variable (global first, then the actor's; unset is 0), the
//!   same as `CheckIntVariable`'s `Value` ([`Variables::int`]). The list
//!   is walked in the order the map gives it;
//! - `TriggerActionBy`: for each id, in turn, every action of `Actions`
//!   runs at once with that id as the actor (so an `ItemId` of 0 in them
//!   means that entity). An action's result is ignored and the action
//!   itself always succeeds (a void `Register` is true in C#), so its
//!   `NextAction` follows. An id with no entity is still given to the
//!   actions, which then find nobody; an empty action name does nothing;
//! - `TriggerActionByFirstSuccess`: for each id, only if an entity of that
//!   id exists now, `Action` runs with it as the actor; the first to
//!   succeed (as `RunAction` says: a map action's result, or any of the
//!   actions of its name) stops the walk and the action succeeds. When
//!   none did, `ElseAction` runs once with the original actor, and the
//!   action fails (its own `ElseAction`'s result is not used), so its
//!   `NextAction` does not follow. An empty or missing `Action` or
//!   `ElseAction` is a failed run (`IsNullOrWhiteSpace`).
//!
//! Not ported: C#'s `GetEntityByCustomId` also finds an entity with no map
//! id for the id 0 (any bullet or particle owner, so id 0 "exists" nearly
//! always); here an id counts only when a map entity has it, so 0 never
//! does. No map lists a 0 in `TriggerActionByFirstSuccess`. A missing
//! `ItemIds` or `Actions` list (`null`, which would throw in C#) is an
//! empty one. Variables are whole numbers of 64 bits, not 32.
//!
//! The actions run in the middle of this one, as in C#, so the second
//! entity's actions see what the first's did.

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run, Variables, run};
use crate::content::actions::defs::{SuccessSequence, TriggerActionBy, TriggerActionByFirstSuccess};

pub struct TriggerByPlugin;

impl Plugin for TriggerByPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action_fn::<TriggerActionBy>(trigger_by)
            .add_map_action_fn::<TriggerActionByFirstSuccess>(first_success)
            .add_map_action_fn::<SuccessSequence>(success_sequence);
    }
}

/// `GetInt` of one entry of `ItemIds`, read when its turn comes.
fn id(world: &mut World, text: &str, actor: Option<i64>) -> i64 {
    world.resource_mut::<Variables>().int(actor, text)
}

fn trigger_by(world: &mut World, run_: Run<TriggerActionBy>) -> bool {
    for text in &run_.data.item_ids {
        let id = id(world, text, run_.actor);
        for name in &run_.data.actions {
            run(world, name, Some(id), run_.depth + 1);
        }
    }
    true
}

fn first_success(world: &mut World, run_: Run<TriggerActionByFirstSuccess>) -> bool {
    let data = &run_.data;
    let action = data.action.as_deref().unwrap_or_default();
    for text in &data.item_ids {
        let id = id(world, text, run_.actor);
        let exists = world.run_system_cached_with(exists, id).unwrap_or(false);
        if exists && run(world, action, Some(id), run_.depth + 1) {
            return true;
        }
    }
    run(world, data.else_action.as_deref().unwrap_or_default(), run_.actor, run_.depth + 1);
    false
}

/// `SuccessSequenceRun` (2014, with `TriggerActionByFirstSuccess`, for
/// Swarm): its `Actions` run in order, for the same actor, until one
/// fails; it succeeds when all did (none: success). Swarm's
/// `TryToChangeToLocalControl` is one, its `if(..)` checks first, so the
/// first car that passes them is the one taken.
fn success_sequence(world: &mut World, run_: Run<SuccessSequence>) -> bool {
    run_.data.actions.iter().all(|name| run(world, name, run_.actor, run_.depth + 1))
}

fn exists(In(id): In<i64>, ids: MapIds) -> bool {
    ids.entity(Some(id)).is_some()
}
