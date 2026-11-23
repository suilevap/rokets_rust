//! `TriggerActionByEntitiesInArea`: run actions for each map entity that
//! stands in an area of the map.
//!
//! C# origin: `ActionRunner.TriggerActionByEntitiesInAreaRun` (February
//! 2014, "trigger action by entities in area"; "don't trigger action by
//! entities without custom id" followed in April, for Escape), with
//! `EntitiesManager.GetEntitiesByArea`. Escape uses it to switch whole
//! rooms on and off (`DeactivateEntity(0)` / `ActivateEntity(0)` for
//! everything in the starting hall or the last doors' room) and to blow
//! up every bomb in a corridor (`DestroyEntity(0)`); AiTest and
//! `Part_LaserQuest1` have one too.
//!
//! Branches of the C# method, all ported:
//! - the entities are those of the world at the moment the action runs
//!   (`CurrentWorld.Entities`, a snapshot with `ToArray`): what the actions
//!   do to the entities does not change who is visited, so an entity an
//!   earlier one destroyed is still visited, and one an action creates is
//!   not. The snapshot is the ids of the entities with a [`MapId`] now;
//! - an entity is in the area when the cell its middle is in
//!   (`Position / CellSize`, each axis cut to a whole number toward zero,
//!   `(int)`) lies inside `Area` (`Rectangle.Contains`: the left and top
//!   edges in, the right and bottom ones out). Its middle is the body's
//!   [`Position`], not its drawn `Transform`, which `Update` eases between
//!   fixed steps, so a run does not depend on the frame rate; an entity
//!   with no body is where its `Transform` puts it. Inactive entities
//!   (`DeactivateEntity`) are still in the world and so visited, wherever
//!   they were left;
//! - an entity without a map id (`CustomId == 0`) is skipped; any other id
//!   counts, a clone's own (not its source's);
//! - for each such entity, in turn, every action of `Actions` runs
//!   (`Triggers.RunAction(name, id)`) with the entity's map id as the
//!   actor, so an `ItemId` of 0 in them means that entity. The result of
//!   an action is ignored; the action itself always succeeds (a void
//!   `Register` is always true in C#), so its `NextAction` follows.
//!
//! The actions run at once, in the middle of this one, as in C#, so the
//! second entity's actions see what the first's did. Where C# gave an
//! order the entities were created in, this takes the map id's, which is
//! the same for any run; it only matters when the actions of two entities
//! affect each other. It is not ported as creation order because the port
//! does not keep one.

use bevy::prelude::*;
use avian2d::prelude::Position;

use super::{AddMapAction, Run, run};
use crate::content::actions::defs::TriggerActionByEntitiesInArea;
use crate::content::xna::XRect;
use crate::game::{LoadedMap, MapId};
use crate::units::CellSize;

pub struct AreaActionPlugin;

impl Plugin for AreaActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action_fn::<TriggerActionByEntitiesInArea>(trigger_by_area);
    }
}

/// `EntitiesManager.GetEntitiesByArea`'s test: whether a point of the world
/// (y up) lies in `area`, which is in map cells (y down).
pub fn in_area(area: XRect, cell: CellSize, at: Vec2) -> bool {
    let (x, y) = cell.to_cells(at);
    // `(int)`: toward zero, as `as` does.
    area.contains((x as i32) as f32, (y as i32) as f32)
}

fn trigger_by_area(world: &mut World, run_: Run<TriggerActionByEntitiesInArea>) -> bool {
    let cell = world.resource::<LoadedMap>().cell;
    let area = run_.data.area;
    let mut ids: Vec<i64> = world
        .query::<(&MapId, Option<&Position>, Option<&Transform>)>()
        .iter(world)
        .filter(|(id, _, _)| id.0 != 0)
        .filter(|(_, position, transform)| {
            position
                .map(|p| p.0)
                .or(transform.map(|t| t.translation.truncate()))
                .is_some_and(|at| in_area(area, cell, at))
        })
        .map(|(id, _, _)| id.0)
        .collect();
    ids.sort_unstable();
    for id in ids {
        for name in &run_.data.actions {
            run(world, name, Some(id), run_.depth + 1);
        }
    }
    true
}
