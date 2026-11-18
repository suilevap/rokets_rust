//! Actions that end the level: win (`MoveToNextMap`, `ShowSuccess`) or
//! start it again (`RestartMap`).

use bevy::prelude::*;

use super::{AddMapAction, Run};
use crate::content::actions::defs::{MoveToNextMap, RestartMap, ShowSuccess};
use crate::game::ChangeMap;
use crate::triggers::LevelEnd;

pub struct LevelActionsPlugin;

impl Plugin for LevelActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<MoveToNextMap, _>(win)
            .add_map_action::<ShowSuccess, _>(win)
            .add_map_action::<RestartMap, _>(restart);
    }
}

/// "Level complete", then the next map in `MapList.xml`.
fn win<T>(In(_): In<Run<T>>, mut end: ResMut<LevelEnd>) -> bool {
    end.complete();
    true
}

fn restart(In(_): In<Run<RestartMap>>, mut change: MessageWriter<ChangeMap>) -> bool {
    change.write(ChangeMap::Restart);
    true
}
