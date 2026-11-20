//! `SetControl` (template `SetControl(id,control)`) and `AIAdjustment`:
//! who drives a car and which team it is on (`ActionRunner.SetControlRun`,
//! `RunAIAdjustment`).
//!
//! - `SetControl` gives the car a new control, so a new team too
//!   ([`control_kind`]). For the player's car: `Local` (and the hot-seat
//!   ones) gives it back to the player, anything else takes it away
//!   (`NoneLocal` in intros; an AI control is not given to the player's car)
//!   and marks it [`NoInput`]. For a simple car or an AI's `Car` (another
//!   of the map's `Players`): the old AI control goes and
//!   the new one comes ([`crate::ai::set_control`], its state starting over
//!   as a new `IControl` object did: stick and aim at rest); `NoInput` when
//!   the new control is one of the `None` ones (`AINone`, which also stills
//!   a Hammer's saw), not when it is an AI type (one with no live control
//!   yet stands idle). On any car the new control says whether it is held
//!   for the player's devices ([`LocalDevice`]: no blows out of view, a
//!   `Car`'s engine and tyres at full loudness; `Car.Control`'s setter).
//! - `AIAdjustment` sets the car's team (unless `Team` is 255, "keep"), on
//!   any car whose control is not the player's own (C# did it on a
//!   `BaseControl`, which the player's composite control is not). On a car
//!   with a behaviour-tree control (`AIBTBaseControl`: the turrets so far)
//!   it also sets `ScanDistance` (when 0 or more), the gun's `MaxAngle` from
//!   `WeaponMaxAngleDeg` (when 0 or more, and the gun aims),
//!   `IsVisibleForAutoAim` (`Yes`/`No`, not `DoNotChange`), `FireAccuracy` and
//!   `ActionOnBTTrigger` (`MapActionOnTirgger`, the action
//!   `TriggerMapAction` runs; both always: the defaults put them back, as in
//!   C#).
//!
//! Not ported: the AI adjustments on the cars whose controls wait for the
//! behaviour-tree slice; a simple car or an AI's `Car` given
//! `Local` stays undriven by the player's devices (so Revenge's hand-over
//! of `SetControl(4001,Local)` waits); the health bar a car shows or not by
//! its first control; an AI control for the
//! player's car (`SetControl(player, AiChaser)`).

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::car::control::{LocalDevice, Team, control_kind};
use crate::sound::CarSounds;
use crate::ai::AiBlackboard;
use crate::car::control::AutoAimVisible;
use crate::content::actions::defs::{AIAdjustment, BoolSetterType, ControlType, SetControl};
use crate::controls::{LocalPlayer, NoInput, PlayerIntent, is_players};
use crate::weapon::Weapon;

pub struct ControlActionsPlugin;

impl Plugin for ControlActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetControl, _>(set_control)
            .add_map_action::<AIAdjustment, _>(ai_adjustment);
    }
}

/// Every car, with whether the player's devices drive it.
type Cars<'w, 's> = Query<'w, 's, (Has<LocalPlayer>, Has<NoInput>), With<Team>>;

fn set_control(
    In(run): In<Run<SetControl>>,
    ids: MapIds,
    cars: Cars,
    mut sounds: Query<&mut CarSounds>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((player, _)) = cars.get(e) else {
        return true;
    };
    let name = ControlType::NAMES[run.data.control as usize];
    let kind = control_kind(name);
    commands.entity(e).insert((Team(kind.team), AutoAimVisible(!kind.ai_none)));
    if kind.local_device {
        commands.entity(e).insert(LocalDevice);
    } else {
        commands.entity(e).remove::<LocalDevice>();
    }
    if let Ok(mut sounds) = sounds.get_mut(e) {
        sounds.set_loudness(kind.local_device);
    }
    if !player {
        // A new control starts at rest.
        crate::ai::set_control(&mut commands.entity(e), name, run.target(run.data.item_id).unwrap_or(0) as u64);
        commands.entity(e).insert(PlayerIntent::default());
    }
    let undriven = if player { !is_players(name) } else { kind.ai_none };
    if undriven {
        commands.entity(e).insert(NoInput);
    } else {
        commands.entity(e).remove::<NoInput>();
    }
    if !player && is_players(name) {
        debug!("a car other than the player's is not driven by the player's devices yet");
    }
    true
}

fn ai_adjustment(
    In(run): In<Run<AIAdjustment>>,
    ids: MapIds,
    cars: Cars,
    mut blackboards: Query<&mut AiBlackboard>,
    mut guns: Query<&mut Weapon>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let data = &run.data;
    if let Ok((player, no_input)) = cars.get(e)
        && data.team != 255
        // The player's own control is not a `BaseControl`.
        && (!player || no_input)
    {
        commands.entity(e).insert(Team(data.team.clamp(0, 254) as u8));
    }
    // `AIBTBaseControl`'s own: the turret.
    if let Ok(mut blackboard) = blackboards.get_mut(e) {
        if data.scan_distance >= 0.0 {
            blackboard.scan_distance = data.scan_distance;
        }
        if data.weapon_max_angle_deg >= 0.0
            && let Ok(mut gun) = guns.get_mut(e)
            && gun.kind.is_complex()
        {
            gun.aim.max_angle = data.weapon_max_angle_deg.to_radians();
        }
        match data.is_visible_for_auto_aim {
            BoolSetterType::DoNotChange => {}
            visible => {
                commands.entity(e).insert(AutoAimVisible(visible == BoolSetterType::Yes));
            }
        }
        blackboard.fire_accuracy = data.fire_accuracy;
        blackboard.map_action_on_trigger = data.action_on_bt_trigger.clone();
    }
    true
}
