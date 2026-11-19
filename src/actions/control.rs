//! `SetControl` (template `SetControl(id,control)`) and `AIAdjustment`:
//! who drives a car and which team it is on (`ActionRunner.SetControlRun`,
//! `RunAIAdjustment`).
//!
//! - `SetControl` gives the car a new control, so a new team too
//!   ([`control_kind`]). For the player's car: `Local` (and the hot-seat
//!   ones) gives it back to the player, anything else takes it away
//!   (`NoneLocal` in intros; the AI ones until there is an AI) and marks it
//!   [`NoInput`]. For a simple car: `NoInput` when the new control is one
//!   of the `None` ones (`AINone`, which also stills a Hammer's saw), not
//!   when it is an AI type (the AI itself is not ported: it will not drive).
//! - `AIAdjustment` sets the car's team (unless `Team` is 255, "keep"), on
//!   any car whose control is not the player's own (C# did it on a
//!   `BaseControl`, which the player's composite control is not).
//!
//! Not ported: the AI control and what `AIAdjustment` sets on it
//! (`ScanDistance`, `WeaponMaxAngleDeg`, `ActionOnBTTrigger`,
//! `IsVisibleForAutoAim`, `FireAccuracy`); a simple car given `Local` stays
//! undriven by the player's devices.

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::car::control::{Team, control_kind};
use crate::content::actions::defs::{AIAdjustment, ControlType, SetControl};
use crate::controls::{LocalPlayer, NoInput, is_players};

pub struct ControlActionsPlugin;

impl Plugin for ControlActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetControl, _>(set_control)
            .add_map_action::<AIAdjustment, _>(ai_adjustment);
    }
}

/// Every car, with whether the player's devices drive it.
type Cars<'w, 's> = Query<'w, 's, (Has<LocalPlayer>, Has<NoInput>), With<Team>>;

fn set_control(In(run): In<Run<SetControl>>, ids: MapIds, cars: Cars, mut commands: Commands) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((player, _)) = cars.get(e) else {
        return true;
    };
    let name = ControlType::NAMES[run.data.control as usize];
    let kind = control_kind(name);
    commands.entity(e).insert(Team(kind.team));
    let undriven = if player { !is_players(name) } else { kind.ai_none };
    if undriven {
        commands.entity(e).insert(NoInput);
    } else {
        commands.entity(e).remove::<NoInput>();
    }
    if !player && is_players(name) {
        debug!("a simple car is not driven by the player's devices yet");
    }
    true
}

fn ai_adjustment(In(run): In<Run<AIAdjustment>>, ids: MapIds, cars: Cars, mut commands: Commands) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if let Ok((player, no_input)) = cars.get(e)
        && run.data.team != 255
        // The player's own control is not a `BaseControl`.
        && (!player || no_input)
    {
        commands.entity(e).insert(Team(run.data.team.clamp(0, 254) as u8));
    }
    true
}
