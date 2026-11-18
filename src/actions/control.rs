//! `SetControl` (template `SetControl(id,control)`): who drives a car
//! (`ActionRunner.SetControlRun`). For the player's car: `Local` (and the
//! hot-seat ones) gives it back to the player, anything else takes it
//! away (`NoneLocal` during intros; the AI ones until there is an AI).
//!
//! Not ported: AI control (cars other than the player's are not spawned
//! yet).

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{ControlType, SetControl};
use crate::controls::{LocalPlayer, NoInput, is_players};

pub struct ControlActionsPlugin;

impl Plugin for ControlActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetControl, _>(set_control);
    }
}

fn set_control(In(run): In<Run<SetControl>>, ids: MapIds, players: Query<(), With<LocalPlayer>>, mut commands: Commands) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if !players.contains(e) {
        debug!("AI control is not ported yet");
        return true;
    }
    if is_players(ControlType::NAMES[run.data.control as usize]) {
        commands.entity(e).remove::<NoInput>();
    } else {
        commands.entity(e).insert(NoInput);
    }
    true
}
