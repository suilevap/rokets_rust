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
//!   `Car`'s engine and tyres at full loudness; `Car.Control`'s setter), and
//!   a gun on it takes the player's aim (`SetLocalControlPreferences`).
//! - `Local` (or a hot-seat control) on a car other than the player's is a
//!   hand-over (Revenge's `SetControl(4001,Local)` after its Scout dies):
//!   the car becomes the one with [`LocalPlayer`], so the devices, the
//!   weapon and gadget input and the HUD follow it, it gets the camera's
//!   [`CameraTarget`] (if it had none) and the energy store for its gun or
//!   accelerator; its death is the player's (wreck, life or restart, and it
//!   comes back as itself). The car that had the devices loses
//!   [`LocalPlayer`], [`LocalDevice`], its camera target and energy store
//!   and stands ([`NoInput`]); if it lies wrecked it neither restarts the
//!   level nor comes back ([`Abandoned`]).
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
//! behaviour-tree slice; the hand-over's differences from C#: there both
//! cars kept `Local` and the devices drove both (`WhoHasLocalControl` is a
//! list, two cars split the screen's HUD), here the devices have one car
//! and the old one stands, and a wreck of the old car does not come back
//! as C#'s would (`OnRespawn` of the Scout after `SetControl(4001,Local)`);
//! the gun's options are the ones it was made with (C# only changed the
//! aim); the health bar a car shows or not by its control, whenever it
//! changes (only the first control's is made); an AI control for the
//! player's car (`SetControl(player, AiChaser)`).

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::car::control::{LocalDevice, Team, control_kind};
use crate::sound::CarSounds;
use crate::ai::AiBlackboard;
use crate::car::control::AutoAimVisible;
use crate::content::actions::defs::{AIAdjustment, BoolSetterType, ControlType, SetControl};
use crate::car::simple::SimpleDrive;
use crate::car::{Car, CarRadius};
use crate::controls::{LocalPlayer, NoInput, PlayerIntent, is_players};
use crate::gadget::accelerator::Accelerator;
use crate::game_mode::{Abandoned, Wreck};
use crate::render::camera::CameraTarget;
use crate::weapon::{PlayerEnergy, Weapon};

pub struct ControlActionsPlugin;

impl Plugin for ControlActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetControl, _>(set_control)
            .add_map_action::<AIAdjustment, _>(ai_adjustment);
    }
}

/// Every car, with whether the player's devices drive it.
type Cars<'w, 's> = Query<'w, 's, (Has<LocalPlayer>, Has<NoInput>), With<Team>>;

/// What the hand-over looks at of the car that becomes the player's.
type Heir<'a> = (Has<CameraTarget>, Has<PlayerEnergy>, Has<Accelerator>, Option<&'a Car>, Option<&'a SimpleDrive>, Option<&'a CarRadius>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn set_control(
    In(run): In<Run<SetControl>>,
    ids: MapIds,
    cars: Cars,
    heirs: Query<Heir>,
    previous: Query<Entity, With<LocalPlayer>>,
    wrecks: Query<Entity, With<Wreck>>,
    mut guns: Query<&mut Weapon>,
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
    // `Control` setter: a gun on a car the devices drive takes the player's
    // aim (`SetLocalControlPreferences`), one that was theirs and is not
    // any more gives it up. `AINone` (`NoneLocal`) is not the devices'.
    let drives = is_players(name);
    if let Ok(mut gun) = guns.get_mut(e) {
        gun.set_local_preferences(drives);
    }
    if !player {
        // A new control starts at rest.
        crate::ai::set_control(&mut commands.entity(e), name, run.target(run.data.item_id).unwrap_or(0) as u64);
        commands.entity(e).insert(PlayerIntent::default());
    }
    if !player && drives {
        hand_over(&mut commands, e, &heirs, &previous, &wrecks, &mut guns, &mut sounds);
    }
    let undriven = if player { !drives } else { kind.ai_none };
    if undriven {
        commands.entity(e).insert(NoInput);
    } else {
        commands.entity(e).remove::<NoInput>();
    }
    true
}

/// `SetControl(id,Local)` on a car other than the player's: the devices, the
/// camera and the energy store go to it (C#'s `Control` setter added it to
/// `WhoHasLocalControl`: `PlayerCarControl` there reads the devices, the
/// camera frames the local cars, `EnergyComponent` is the car's own).
/// The car that had them loses them: a living one stands where it is, a
/// wreck is no longer the one whose death restarts the level or spends a
/// life ([`Abandoned`]); the new car's death is the player's.
fn hand_over(
    commands: &mut Commands,
    heir: Entity,
    heirs: &Query<Heir>,
    previous: &Query<Entity, With<LocalPlayer>>,
    wrecks: &Query<Entity, With<Wreck>>,
    guns: &mut Query<&mut Weapon>,
    sounds: &mut Query<&mut CarSounds>,
) {
    for old in previous.iter().filter(|&o| o != heir) {
        commands
            .entity(old)
            .remove::<(LocalPlayer, LocalDevice, CameraTarget, PlayerEnergy)>()
            .insert((PlayerIntent::default(), NoInput));
        if let Ok(mut gun) = guns.get_mut(old) {
            gun.set_local_preferences(false);
        }
        if let Ok(mut sounds) = sounds.get_mut(old) {
            sounds.set_loudness(false);
        }
    }
    for wreck in wrecks.iter().filter(|&w| w != heir) {
        commands.entity(wreck).insert(Abandoned).remove::<(LocalDevice, CameraTarget)>();
    }
    let Ok((has_target, has_energy, accelerator, car, simple, radius)) = heirs.get(heir) else {
        return;
    };
    commands.entity(heir).insert(LocalPlayer);
    if !has_target {
        let max_speed = car.map(|c| c.params.max_speed).or(simple.map(|s| s.max_speed)).unwrap_or(0.0);
        commands.entity(heir).insert(CameraTarget { radius: radius.map_or(1.0, |r| r.0), max_speed });
    }
    // The gun and the accelerator spend the player's store.
    if !has_energy && (accelerator || guns.contains(heir)) {
        commands.entity(heir).insert(PlayerEnergy::default());
    }
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
