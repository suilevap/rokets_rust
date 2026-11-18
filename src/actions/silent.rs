//! Actions with nothing to show in the port: saving progress, screenshots
//! for the level select, achievements and stats. They succeed quietly, so
//! the actions chained after them still run.

use bevy::prelude::*;
use serde::Deserialize;

use super::{AddMapAction, MapAction, Run, Templates};

pub struct SilentActionsPlugin;

impl Plugin for SilentActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<Save, _>(nothing::<Save>)
            .add_map_action::<Capture, _>(nothing::<Capture>)
            .add_map_action::<Achievement, _>(nothing::<Achievement>)
            .add_map_action::<Stat, _>(nothing::<Stat>);
    }
}

fn nothing<T: MapAction>(In(_): In<Run<T>>) -> bool {
    true
}

macro_rules! silent {
    ($name:ident, $kind:literal, [$($template:literal),*]) => {
        #[derive(Deserialize, Default)]
        struct $name {}

        impl MapAction for $name {
            const TYPE: &'static str = $kind;

            fn templates() -> Templates<Self> {
                vec![$(($template, |_| Some($name {}))),*]
            }
        }
    };
}

silent!(Save, "SaveAction", ["Save", "SaveWithActionOnRespawn"]);
silent!(Capture, "CaptureScreen", ["CaptureScreen", "CaptureScreenForced"]);
silent!(Achievement, "UnlockAchievement", ["UnlockAch"]);
silent!(Stat, "SetAchievementStat", ["SetStat"]);
