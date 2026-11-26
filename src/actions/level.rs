//! Actions that end the level: win (`MoveToNextMap`) or start it again
//! (`RestartMap`).
//!
//! `ShowSuccess` is not one of them: it only stamps the screen
//! (`actions::success`).

use bevy::prelude::*;

use super::{AddMapAction, Run};
use crate::content::actions::defs::{MoveToNextMap, RestartMap};
use crate::game::ChangeMap;
use crate::triggers::LevelEnd;

pub struct LevelActionsPlugin;

impl Plugin for LevelActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<MoveToNextMap, _>(move_to_next_map)
            .add_map_action::<RestartMap, _>(restart);
    }
}

/// "Level complete", then the next map in `MapList.xml`.
fn move_to_next_map(In(_): In<Run<MoveToNextMap>>, mut end: ResMut<LevelEnd>) -> bool {
    end.complete();
    true
}

fn restart(In(_): In<Run<RestartMap>>, mut change: MessageWriter<ChangeMap>) -> bool {
    change.write(ChangeMap::Restart);
    true
}
