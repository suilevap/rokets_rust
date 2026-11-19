//! What a behaviour tree makes a car do: [`Orders`], flatbt's act.

use bevy::math::Vec2;
use bevy::prelude::Component;

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
}

impl Orders {
    /// These orders, then `later`'s on top: what two ticks in a row
    /// (`ParallelAllActionNode.Tick`) leave written.
    pub fn then(self, later: Orders) -> Orders {
        Orders {
            move_dir: later.move_dir.or(self.move_dir),
            aim_dir: later.aim_dir.or(self.aim_dir),
            fire: self.fire || later.fire,
        }
    }
}
