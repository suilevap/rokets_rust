//! `ChangeSubscribers`: adds an entity to the ones that set a trigger off,
//! or takes it off (`ChangeSubscribers(Add|Remove)`).
//!
//! C# origin: `ActionRunner.ChangeSubscribersRun`, with
//! `TriggerManager.AddSubscriberToTrigger` / `RemoveSubscriberFromTrigger`,
//! added in July 2013 for the Race map ("Action to add/remove from
//! subscribers"): a lap counts only for a car that has passed the
//! checkpoint trigger first. The checkpoint's `ActivateLapPlus` adds the
//! car to the finish line's subscribers (which the map lists as `0`,
//! nobody), and the finish line's leave action `DeActivateLapPlus` takes it
//! off again; 2WeeksAgo takes the actor off trigger 3 the same way.
//!
//! `ItemId` is the entity (0 or less: the actor that set the action off)
//! and `TriggerId` the trigger, as in the map's `Triggers`. A trigger the
//! map lists no subscribers for is set off by the player's car alone
//! (`TriggersPlugin`), and the changes keep to C#'s rules for it:
//!
//! - Add to a trigger with a list: the entity's own id joins, if it is not
//!   in it yet.
//! - Add to the player's alone trigger: nothing when the entity is the
//!   player's car (it already sets it off); else the list becomes the
//!   player and the entity.
//! - Remove from a list: the first entry of the entity's id goes; one that
//!   is not in it is no change.
//! - Remove from the player's alone trigger: when the entity is the
//!   player's car the list becomes empty, so nobody sets it off; else no
//!   change.
//!
//! The action always succeeds, as C#'s (no result), so its `NextAction`
//! follows. Joining a list does not make the entity watched: as in C#,
//! only entities named in some trigger of the map at its start are
//! (`TriggerManager.AddPossibleSubscriber`), so a car that is not has no
//! effect on any trigger.
//!
//! Not ported: C# crashed (a null reference) when the entity or the
//! trigger does not exist, or when there is no player's car to compare
//! with; here the action logs and fails, changing nothing. A map that
//! lists `<Subscribers/>` empty (an empty list in C#, nobody) reads as
//! none listed (the player), as `TriggerData.subscribers` always did.

use bevy::prelude::*;

use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{ChangeSubscribers, ChangeSubscribersOperationType as Op};
use crate::controls::LocalPlayer;
use crate::game::MapId;
use crate::triggers::Triggers;

pub struct TriggerSubscribersPlugin;

impl Plugin for TriggerSubscribersPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<ChangeSubscribers, _>(change_subscribers);
    }
}

fn change_subscribers(
    In(run): In<Run<ChangeSubscribers>>,
    ids: MapIds,
    player: Query<&MapId, With<LocalPlayer>>,
    mut triggers: ResMut<Triggers>,
) -> bool {
    let d = &run.data;
    // The entity must exist (C#'s `ent.CustomId`), as its id is what is kept.
    let id = run.target(d.item_id);
    let (Some(ent), Some(main)) = (id.filter(|_| ids.entity(id).is_some()), player.iter().next()) else {
        warn!("ChangeSubscribers {}: no entity {id:?} or no player's car", d.trigger_id);
        return false;
    };
    let done = match d.operation {
        Op::Add => triggers.add_subscriber(d.trigger_id, ent, main.0),
        Op::Remove => triggers.remove_subscriber(d.trigger_id, ent, main.0),
    };
    if done {
        debug!("trigger {} is now set off by {:?}", d.trigger_id, triggers.subscribers_of(d.trigger_id).flatten());
    } else {
        warn!("ChangeSubscribers: no trigger {}", d.trigger_id);
    }
    done
}
