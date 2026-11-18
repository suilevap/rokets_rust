//! Actions with nothing to show in the port: screenshots
//! for the level select, achievements and stats. They succeed quietly, so
//! the actions chained after them still run.

use bevy::prelude::*;

use super::{AddMapAction, Run};
use crate::content::actions::defs::{
    CaptureScreen, SetAchievementStat, UnlockAchievement,
};

pub struct SilentActionsPlugin;

impl Plugin for SilentActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CaptureScreen, _>(nothing::<CaptureScreen>)
            .add_map_action::<UnlockAchievement, _>(nothing::<UnlockAchievement>)
            .add_map_action::<SetAchievementStat, _>(nothing::<SetAchievementStat>);
    }
}

fn nothing<T>(In(_): In<Run<T>>) -> bool {
    true
}
