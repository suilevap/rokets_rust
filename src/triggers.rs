//! Triggers: areas of the map that run actions when the player enters.
//!
//! All of a map's triggers are read; for now only the level-end actions
//! run — `NextMap()` (the built-in template, `MoveToNextMap`) and named
//! actions of type `MoveToNextMap`, `ShowSuccess` or an `ActionSequence`
//! containing one. They show "Level complete" and go to the next map in
//! `MapList.xml` after a moment. Every other action is reported once.

use std::collections::BTreeSet;

use bevy::prelude::*;

use crate::content::actions::ActionRef;
use crate::content::map::MapData;
use crate::controls::LocalPlayer;
use crate::game::{ChangeMap, GameState, LoadedMap, MapEntity};

/// Seconds between "Level complete" and the next map.
const LEVEL_END_DELAY: f32 = 2.0;

/// What a trigger's actions add up to, as far as the game supports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    LevelComplete,
    Unsupported,
}

#[derive(Component, Debug)]
pub struct Trigger {
    pub id: i64,
    /// Area in map cells.
    pub area: crate::content::xna::XRect,
    pub remaining: i64,
    pub effects: Vec<Effect>,
    inside: bool,
}

#[derive(Resource, Debug, Default)]
pub struct LevelEnd {
    pub timer: Option<Timer>,
}

pub struct TriggersPlugin;

impl Plugin for TriggersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelEnd>()
            .add_systems(OnEnter(GameState::Playing), spawn_triggers)
            .add_systems(Update, (check_triggers, finish_level).chain().run_if(in_state(GameState::Playing)));
    }
}

/// Resolves an action reference to what it does.
pub fn resolve(map: &MapData, reference: &str, depth: usize) -> Effect {
    let r = ActionRef::parse(reference);
    if r.name == "NextMap" && r.args.is_some() {
        return Effect::LevelComplete;
    }
    let Some(action) = map
        .actions
        .iter()
        .find(|a| a.name.as_deref() == Some(r.name.as_str()))
    else {
        return Effect::Unsupported;
    };
    match action.short_kind() {
        "MoveToNextMap" | "ShowSuccess" => Effect::LevelComplete,
        "ActionSequence" if depth < 8 => {
            let inner = action.actions.as_ref().map(|a| a.items.as_slice()).unwrap_or(&[]);
            if inner.iter().any(|i| resolve(map, i, depth + 1) == Effect::LevelComplete) {
                Effect::LevelComplete
            } else {
                Effect::Unsupported
            }
        }
        _ => Effect::Unsupported,
    }
}

fn spawn_triggers(mut commands: Commands, map: Res<LoadedMap>, mut end: ResMut<LevelEnd>) {
    end.timer = None;
    let mut unsupported = BTreeSet::new();
    for t in map.data.triggers.iter() {
        let effects: Vec<Effect> = t
            .actions
            .iter()
            .map(|a| {
                let e = resolve(&map.data, a, 0);
                if e == Effect::Unsupported {
                    unsupported.insert(ActionRef::parse(a).name);
                }
                e
            })
            .collect();
        commands.spawn((
            Name::new(format!("Trigger {}", t.id)),
            MapEntity,
            Trigger {
                id: t.id,
                area: t.area,
                remaining: t.amount.max(1),
                effects,
                inside: false,
            },
        ));
    }
    if !unsupported.is_empty() {
        info!("trigger actions not supported yet: {unsupported:?}");
    }
}

fn check_triggers(
    map: Res<LoadedMap>,
    players: Query<&Transform, With<LocalPlayer>>,
    mut triggers: Query<&mut Trigger>,
    mut end: ResMut<LevelEnd>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let (x, y) = map.cell.to_cells(player.translation.truncate());
    for mut t in &mut triggers {
        let inside = t.area.contains(x, y);
        let entered = inside && !t.inside;
        t.inside = inside;
        if !entered || t.remaining <= 0 {
            continue;
        }
        t.remaining -= 1;
        if t.effects.contains(&Effect::LevelComplete) && end.timer.is_none() {
            info!("trigger {}: level complete", t.id);
            end.timer = Some(Timer::from_seconds(LEVEL_END_DELAY, TimerMode::Once));
        }
    }
}

fn finish_level(time: Res<Time>, mut end: ResMut<LevelEnd>, mut change: MessageWriter<ChangeMap>) {
    if let Some(timer) = end.timer.as_mut()
        && timer.tick(time.delta()).just_finished()
    {
        change.write(ChangeMap::Next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_level_end() {
        let map = MapData::parse(
            r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name>
            <Actions>
              <Item Type="GameLibrary.ShowSuccess"><Name>Win</Name></Item>
              <Item Type="GameLibrary.ActionSequence"><Name>Seq</Name><Actions><Item>Win</Item></Actions></Item>
              <Item Type="GameLibrary.PlaySound"><Name>Beep</Name></Item>
            </Actions></Asset></XnaContent>"#,
        )
        .unwrap();
        assert_eq!(resolve(&map, "NextMap()", 0), Effect::LevelComplete);
        assert_eq!(resolve(&map, "Win", 0), Effect::LevelComplete);
        assert_eq!(resolve(&map, "Seq", 0), Effect::LevelComplete);
        assert_eq!(resolve(&map, "Beep", 0), Effect::Unsupported);
        assert_eq!(resolve(&map, "CloseDoor(1001)", 0), Effect::Unsupported);
    }
}
