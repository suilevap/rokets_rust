//! Triggers: areas of the map that run actions when something enters or
//! leaves them (`TriggerManager`).
//!
//! As in C#, the triggers are painted into a map of cells in the order the
//! map lists them (a later one covers an earlier one where they overlap),
//! and each watched entity is in at most one trigger at a time. Watched
//! are the player's car and every entity some trigger lists as a
//! subscriber; any of them sets off any trigger. Entering an armed trigger
//! runs its actions for that entity and counts down its `Amount`; leaving
//! runs its `LeaveActions`. A trigger used up is wiped from the map (then
//! nothing is there, not the trigger it covered).
//!
//! The actions themselves are `actions`. [`LevelEnd`] shows "Level
//! complete" and goes to the next map in `MapList.xml` after a moment.
//! `ActivateTriggerActionData` (arm or disarm triggers) is registered here.

use bevy::prelude::*;

use crate::actions::{AddMapAction, Inactive, Run, RunAction};
use crate::content::actions::defs::ActivateTrigger;
use crate::controls::LocalPlayer;
use crate::game::{ChangeMap, GameState, LoadedMap, MapId};

/// Seconds between "Level complete" and the next map.
const LEVEL_END_DELAY: f32 = 2.0;

struct TriggerState {
    id: i64,
    area: crate::content::xna::XRect,
    remaining: i64,
    active: bool,
    enter: Vec<String>,
    leave: Vec<String>,
}

/// The map's triggers and the cells they cover.
#[derive(Resource, Default)]
pub struct Triggers {
    width: usize,
    height: usize,
    /// Per cell, the trigger there (an index into `list` + 1), 0 for none.
    cells: Vec<usize>,
    list: Vec<TriggerState>,
    /// Map ids of the entities the triggers watch, besides the player.
    subscribers: Vec<i64>,
}

impl Triggers {
    fn paint(&mut self, index: usize, value: usize) {
        let a = self.list[index].area;
        for y in a.y.max(0)..(a.y + a.h).min(self.height as i32) {
            for x in a.x.max(0)..(a.x + a.w).min(self.width as i32) {
                self.cells[y as usize * self.width + x as usize] = value;
            }
        }
    }

    /// The trigger at a point in map cells, if any.
    fn at(&self, x: f32, y: f32) -> Option<usize> {
        if x < 0.0 || y < 0.0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.cells[y as usize * self.width + x as usize].checked_sub(1)
    }
}

/// The trigger a watched entity was in last frame.
#[derive(Component, Debug, Default)]
pub struct InTrigger(Option<usize>);

#[derive(Resource, Debug, Default)]
pub struct LevelEnd {
    pub timer: Option<Timer>,
}

impl LevelEnd {
    /// The level is won: "Level complete", then the next map.
    pub fn complete(&mut self) {
        if self.timer.is_none() {
            info!("level complete");
            self.timer = Some(Timer::from_seconds(LEVEL_END_DELAY, TimerMode::Once));
        }
    }
}

pub struct TriggersPlugin;

impl Plugin for TriggersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelEnd>()
            .init_resource::<Triggers>()
            .add_map_action::<ActivateTrigger, _>(arm_triggers)
            .add_systems(OnEnter(GameState::Playing), build_triggers)
            .add_systems(
                Update,
                (watch_subscribers, check_triggers, finish_level)
                    .chain()
                    .before(crate::actions::ActionSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn build_triggers(mut commands: Commands, map: Res<LoadedMap>, mut end: ResMut<LevelEnd>) {
    end.timer = None;
    let (w, h) = (map.grid.width, map.grid.height);
    let mut t = Triggers {
        width: w,
        height: h,
        cells: vec![0; w * h],
        list: Vec::new(),
        subscribers: Vec::new(),
    };
    for data in map.data.triggers.iter() {
        t.list.push(TriggerState {
            id: data.id,
            area: data.area,
            remaining: data.amount,
            active: data.active,
            enter: data.actions.items.clone(),
            leave: data.leave_actions.items.clone(),
        });
        let i = t.list.len() - 1;
        t.paint(i, i + 1);
        t.subscribers.extend(&data.subscribers);
    }
    commands.insert_resource(t);
}

/// Subscribers and the player get an [`InTrigger`] once they exist.
#[allow(clippy::type_complexity)]
fn watch_subscribers(
    mut commands: Commands,
    triggers: Res<Triggers>,
    new: Query<(Entity, Option<&MapId>, Has<LocalPlayer>), Without<InTrigger>>,
) {
    for (e, id, player) in &new {
        if player || id.is_some_and(|id| triggers.subscribers.contains(&id.0)) {
            commands.entity(e).insert(InTrigger::default());
        }
    }
}

fn check_triggers(
    map: Res<LoadedMap>,
    mut triggers: ResMut<Triggers>,
    mut watched: Query<(&Transform, Option<&MapId>, &mut InTrigger), Without<Inactive>>,
    mut run: MessageWriter<RunAction>,
) {
    for (t, id, mut last) in &mut watched {
        let actor = id.map(|i| i.0);
        let (x, y) = map.cell.to_cells(t.translation.truncate());
        let now = triggers.at(x, y);
        if let Some(left) = last.0
            && last.0 != now
        {
            for name in &triggers.list[left].leave {
                run.write(RunAction {
                    name: name.clone(),
                    actor,
                });
            }
            if triggers.list[left].remaining <= 0 {
                triggers.paint(left, 0);
            }
        }
        match now {
            Some(i) if triggers.list[i].active => {
                if last.0 != Some(i) {
                    for name in &triggers.list[i].enter {
                        run.write(RunAction {
                            name: name.clone(),
                            actor,
                        });
                    }
                    last.0 = Some(i);
                    triggers.list[i].remaining -= 1;
                    if triggers.list[i].remaining <= 0 && triggers.list[i].leave.is_empty() {
                        triggers.paint(i, 0);
                        last.0 = None;
                    }
                }
            }
            _ => last.0 = None,
        }
    }
}

/// `ActivateTriggerActionData`, `ActivateTrigger(id)`, `DeactivateTrigger(id)`.
fn arm_triggers(In(run): In<Run<ActivateTrigger>>, mut triggers: ResMut<Triggers>) -> bool {
    for t in triggers
        .list
        .iter_mut()
        .filter(|t| run.data.trigger_ids.contains(&t.id))
    {
        t.active = !run.data.deactivate;
    }
    true
}

fn finish_level(time: Res<Time>, mut end: ResMut<LevelEnd>, mut change: MessageWriter<ChangeMap>) {
    if let Some(timer) = end.timer.as_mut()
        && timer.tick(time.delta()).just_finished()
    {
        change.write(ChangeMap::Next);
    }
}
