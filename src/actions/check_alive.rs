//! `CheckEntityAlive`: runs one action when an entity exists and another
//! when it does not (a condition on the map's entities, as
//! `CheckIntVariable` is one on its variables).
//!
//! C# origin: `ActionRunner.CheckEntityAliveRun`, added with the Targets
//! map in 2013 ("New action CheckEntityAlive") and unchanged since. Used
//! by Targets (is the target still up, then highlight the weapon),
//! TheLastStand (an achievement when the shield survived), Race2 (a
//! trigger picks a sequence by whether a marker entity is still there),
//! GarbageCollector (the end of the level depends on the cargo), and
//! WinnersRoom (a door opening also checks the coin).
//!
//! `ItemId` is a number or the name of a variable that holds one
//! (`GetInt`: a global variable, else the actor's); 0 or less means the
//! actor (`GetId`). An entity is alive when the map has an entity of that
//! id: C# looked it up in the world's entity list (`GetEntityByCustomId`),
//! where it stays from creation until it is destroyed. So a deactivated
//! entity (`Inactive`) is alive, a destroyed one (`DestroyEntity`, a blown
//! box, a simple car's wreck once removed) is not, and a clone has its
//! own id, not its source's.
//!
//! The action runs `ActionOnAlive` or else `ActionOnNotAlive`, for the
//! same actor (an empty or missing name runs nothing), and then, like
//! every action that does not return a result, always succeeds, so its
//! `NextAction` follows whichever branch ran, and whether that branch
//! succeeded does not matter (WinnersRoom's `NextAction` runs when the
//! coin is gone too).
//!
//! Not ported: an action with no `ItemId` (Escort's unused one has
//! `Null`): C# looked a null name up in a dictionary and threw; here it
//! logs and fails, running neither branch.

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run, run};
use crate::content::actions::defs::CheckEntityAlive;

pub struct CheckAlivePlugin;

impl Plugin for CheckAlivePlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action_fn::<CheckEntityAlive>(check_alive);
    }
}

fn check_alive(world: &mut World, run_: Run<CheckEntityAlive>) -> bool {
    let d = &run_.data;
    let Some(item) = d.item_id.as_deref() else {
        warn!("CheckEntityAlive has no ItemId");
        return false;
    };
    let id = world.resource_mut::<super::Variables>().int(run_.actor, item);
    let alive = world
        .run_system_cached_with(alive, run_.target(id))
        .unwrap_or(false);
    let branch = if alive { &d.action_on_alive } else { &d.action_on_not_alive };
    if let Some(name) = branch {
        run(world, name, run_.actor, run_.depth + 1);
    }
    true
}

fn alive(In(id): In<Option<i64>>, ids: MapIds) -> bool {
    ids.entity(id).is_some()
}
