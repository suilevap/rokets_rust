//! Integer variables that actions set and check (`ActionRunner`:
//! `ChangeIntVariable`, `CheckIntVariable`; templates `SetVar`,
//! `SetGlobalVar`, `SetVarEx`, `if`, `IfEx`).
//!
//! Each map item (by map id) has its own variables and there is one
//! global set; all start empty with the map. `Id` is built in: an item's
//! reads as its id until set (maps check it to tell who set a trigger
//! off, Race and 2WeeksAgo), the global set's as -1. Reading never
//! changes anything. An action's variables are its item's when it is not global:
//! `ItemId`, or the one that set it off when that is 0.
//!
//! `CheckIntVariable` compares a variable with `Value` (a number, or a
//! variable's name: global, else the actor's) and runs the action for
//! less, equal or greater; it succeeds when there was one to run, so it
//! works as a condition for `NextAction` (`if(x,>=,2)` runs `NA`).
//! `ElapsedActiveTime` (whole seconds of play) is a built-in global.
//!
//! Not ported: `PlayerLives` (no lives yet), the debug overlay of values.

use std::collections::HashMap;

use bevy::prelude::*;

use super::{AddMapAction, Run, run};
use crate::content::actions::defs::{ChangeIntVariable, ChangeIntVariableOperationType as Op, CheckIntVariable};
use crate::game::GameState;

/// Whose variables: the global set, or a map item's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    Global,
    Item(i64),
}

impl Owner {
    /// C#'s key: the item's id, -1 for the global set.
    fn id(self) -> i64 {
        match self {
            Owner::Global => -1,
            Owner::Item(id) => id,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct Variables {
    sets: HashMap<Owner, HashMap<String, i64>>,
    /// When the map started, for `ElapsedActiveTime`.
    started: f32,
}

impl Variables {
    /// A variable's value, or the built-in `Id` when it was not set.
    pub fn get(&self, owner: Owner, name: &str) -> Option<i64> {
        let stored = self.sets.get(&owner).and_then(|set| set.get(name).copied());
        stored.or_else(|| (name == "Id").then(|| owner.id()))
    }

    pub fn set(&mut self, owner: Owner, name: &str, value: i64) {
        self.values(owner).insert(name.to_string(), value);
    }

    /// An owner's set, to write to.
    fn values(&mut self, owner: Owner) -> &mut HashMap<String, i64> {
        self.sets.entry(owner).or_default()
    }

    /// `GetValues`: the set an action works on.
    fn owner(global: bool, item_id: i64, actor: Option<i64>) -> Option<Owner> {
        match (global, item_id) {
            (false, id) if id > 0 => Some(Owner::Item(id)),
            (false, 0) => actor.map(Owner::Item),
            _ => Some(Owner::Global),
        }
    }

    /// `GetInt`: a number, or a variable's value (global, else the actor's).
    pub(super) fn int(&self, actor: Option<i64>, text: &str) -> i64 {
        if let Ok(n) = text.trim().parse() {
            return n;
        }
        self.get(Owner::Global, text)
            .or_else(|| actor.and_then(|a| self.get(Owner::Item(a), text)))
            .unwrap_or(0)
    }
}

pub struct VariablesPlugin;

impl Plugin for VariablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Variables>()
            .add_map_action::<ChangeIntVariable, _>(change)
            .add_map_action_fn::<CheckIntVariable>(check)
            .add_systems(OnEnter(GameState::Playing), clear);
    }
}

fn clear(mut vars: ResMut<Variables>, time: Res<Time>) {
    *vars = Variables {
        started: time.elapsed_secs(),
        ..default()
    };
}

fn change(In(run): In<Run<ChangeIntVariable>>, mut vars: ResMut<Variables>) -> bool {
    let d = &run.data;
    let (Some(name), Some(owner)) = (&d.variable_name, Variables::owner(d.global, d.item_id, run.actor)) else {
        return true;
    };
    let values = vars.values(owner);
    let old = values.get(name).copied().or_else(|| (name == "Id").then(|| owner.id())).unwrap_or(0);
    let new = match d.operation {
        Op::Set => d.value,
        Op::Increase => old.wrapping_add(d.value),
        Op::Mul => old.wrapping_mul(d.value),
        Op::Div | Op::Mod if d.value == 0 => {
            warn!("{name}: division by 0");
            old
        }
        Op::Div => old / d.value,
        Op::Mod => old % d.value,
    };
    values.insert(name.clone(), new);
    true
}

fn check(world: &mut World, run_: Run<CheckIntVariable>) -> bool {
    let d = &run_.data;
    let name = d.variable_name.as_deref().unwrap_or_default();
    let now = world.resource::<Time>().elapsed_secs();
    let (value, target) = {
        let vars = world.resource::<Variables>();
        let builtin = (d.global && name == "ElapsedActiveTime").then_some((now - vars.started) as i64);
        let own = Variables::owner(d.global, d.item_id, run_.actor).and_then(|o| vars.get(o, name));
        if builtin.is_none() && own.is_none() {
            debug!("variable {name} not set");
        }
        let target = vars.int(run_.actor, d.value.as_deref().unwrap_or_default());
        (builtin.or(own).unwrap_or(0), target)
    };
    let action = match value.cmp(&target) {
        std::cmp::Ordering::Less => &d.action_on_less,
        std::cmp::Ordering::Equal => &d.action_on_equal,
        std::cmp::Ordering::Greater => &d.action_on_greater,
    };
    match action.as_deref().filter(|a| !a.trim().is_empty()) {
        Some(action) => {
            run(world, action, run_.actor, run_.depth + 1);
            true
        }
        None => false,
    }
}
