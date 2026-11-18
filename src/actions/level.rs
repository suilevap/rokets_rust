//! Actions that end the level: win (`MoveToNextMap`, `ShowSuccess`) or
//! start it again (`RestartMap`).

use bevy::prelude::*;
use serde::Deserialize;

use super::{AddMapAction, MapAction, Run, Templates};
use crate::game::ChangeMap;
use crate::triggers::LevelEnd;

pub struct LevelActionsPlugin;

impl Plugin for LevelActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<NextMap, _>(win)
            .add_map_action::<Success, _>(win)
            .add_map_action::<Restart, _>(restart);
    }
}

/// `MoveToNextMap`, `NextMap()`, `NextMap(stars)`.
#[derive(Deserialize, Default)]
struct NextMap {}

impl MapAction for NextMap {
    const TYPE: &'static str = "MoveToNextMap";

    fn templates() -> Templates<Self> {
        vec![("NextMap", |_| Some(NextMap {}))]
    }
}

/// `ShowSuccess`, `ShowSuccess()`.
#[derive(Deserialize, Default)]
struct Success {}

impl MapAction for Success {
    const TYPE: &'static str = "ShowSuccess";

    fn templates() -> Templates<Self> {
        vec![("ShowSuccess", |_| Some(Success {}))]
    }
}

/// "Level complete", then the next map in `MapList.xml`.
fn win<T>(In(_): In<Run<T>>, mut end: ResMut<LevelEnd>) -> bool {
    end.complete();
    true
}

/// `RestartMap`, `RestartMap()`.
#[derive(Deserialize, Default)]
struct Restart {}

impl MapAction for Restart {
    const TYPE: &'static str = "RestartMap";

    fn templates() -> Templates<Self> {
        vec![("RestartMap", |_| Some(Restart {}))]
    }
}

fn restart(In(_): In<Run<Restart>>, mut change: MessageWriter<ChangeMap>) -> bool {
    change.write(ChangeMap::Restart);
    true
}
