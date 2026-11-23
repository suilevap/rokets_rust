//! What a behaviour tree makes a car do: [`Orders`], flatbt's act.

use bevy::math::Vec2;
use bevy::prelude::Component;

use crate::controls::PlayerIntent;

/// What a tree makes the car do (flatbt's act, a component on the car while
/// its tree runs): what the control writes into the intent this step. `None`
/// leaves the intent as it is, as an `IControl` not written to stayed.
///
/// C# origin: an action's `Tick` wrote `CurrentAxisControl`,
/// `WeaponAxisControl` and `Fire |= FireType.Shoot` of the car's `IControl`;
/// a tick here returns them instead, and `ai::carry_out` writes them.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Orders {
    /// `CurrentAxisControl`.
    pub move_dir: Option<Vec2>,
    /// `WeaponAxisControl`.
    pub aim_dir: Option<Vec2>,
    /// `Fire |= FireType.Shoot` this step.
    pub fire: bool,
    /// `Fire |= FireType.GadgetActivityStart` or `GadgetActivityEnd` this
    /// step. Like C#'s flags it is kept by the car's intent until the next
    /// one is written.
    pub gadget: Option<GadgetButton>,
    /// Where the gadget aims: a way, length 0..1 of its reach
    /// (`WeaponAxisControl` as the chain gadget read it).
    pub gadget_dir: Option<Vec2>,
}

/// What the tree does with the gadget's button: `GadgetActivityStart` holds
/// it, `GadgetActivityEnd` lets go (which links what the magnet aims at, or
/// drops what it tows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GadgetButton {
    Hold,
    Release,
}

impl Orders {
    /// These orders, then `later`'s on top: what two ticks in a row
    /// (`ParallelAllActionNode.Tick`) leave written.
    pub fn then(self, later: Orders) -> Orders {
        Orders {
            move_dir: later.move_dir.or(self.move_dir),
            aim_dir: later.aim_dir.or(self.aim_dir),
            fire: self.fire || later.fire,
            gadget: later.gadget.or(self.gadget),
            gadget_dir: later.gadget_dir.or(self.gadget_dir),
        }
    }
}

impl Orders {
    /// The gadget's part of these orders written into the car's intent, as
    /// `Car.ControlHandle` took the `Fire` flags: the button held from one
    /// tick until another lets go (`cancel`: the tree ended a gadget action
    /// badly), where it aims, and the stick cleared when the activity ends
    /// or is cancelled (`Control.ClearAxis()`).
    pub fn write_gadget(&self, intent: &mut PlayerIntent, cancel: bool) {
        let mut set = |button: Option<bool>| {
            if let Some(held) = button
                && intent.gadget != held
            {
                intent.gadget = held;
            }
        };
        let mut clear_stick = false;
        match self.gadget {
            Some(GadgetButton::Hold) => set(Some(true)),
            Some(GadgetButton::Release) => {
                set(Some(false));
                clear_stick = true;
            }
            None => {}
        }
        if let Some(dir) = self.gadget_dir
            && (intent.gadget_at.is_some() || intent.gadget_dir != Some(dir))
        {
            intent.gadget_at = None;
            intent.gadget_dir = Some(dir);
        }
        if cancel {
            set(Some(false));
            intent.gadget_cancel = true;
            clear_stick = true;
        }
        if clear_stick && intent.move_dir != Vec2::ZERO {
            intent.move_dir = Vec2::ZERO;
        }
    }
}
