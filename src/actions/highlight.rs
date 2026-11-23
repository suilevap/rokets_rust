//! `HighlightController` (`HighlightControllerOff()`, `…Weapon()`,
//! `…Gadget()`, `…Movement()`): tells the player which control to use,
//! by making that control's picture on the screen pulse
//! ([`crate::control_highlight`] draws it, in `Update`).
//!
//! C# origin: `ActionRunner.HighlightControllerRun`, which copied the
//! action's three flags to `ControlHelper`'s statics (2013, "Highlight
//! control works"; the tutorial-like maps JunkYard, Middleware, TakeCover,
//! Targets, TheGate and others turn it on while the player has to learn a
//! control, and `HighlightControllerOff()` once it is used). Each run
//! *replaces* all three flags, it does not add to them: the weapon
//! highlight is off after a `Gadget` one. `World.Clear` called
//! `ControlHelper.Reset()` (all off) when a map ended; here the flags are
//! cleared when a map starts or ends.
//!
//! The action only stores the flags, on the map's actions (`FixedUpdate`);
//! drawing is `crate::control_highlight`'s.

use bevy::prelude::*;

use super::{AddMapAction, Run};
use crate::content::actions::defs::HighlightController;
use crate::game::GameState;

/// Which controls pulse (`ControlHelper.HighlightGadgetControl`,
/// `HighlightWeaponControl`, `HighlightMovementControl`).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ControlHighlight {
    pub gadget: bool,
    pub weapon: bool,
    pub movement: bool,
}

pub struct HighlightActionsPlugin;

impl Plugin for HighlightActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControlHighlight>()
            .add_map_action::<HighlightController, _>(highlight_controller)
            .add_systems(OnEnter(GameState::Playing), clear)
            .add_systems(OnExit(GameState::Playing), clear);
    }
}

fn highlight_controller(In(run): In<Run<HighlightController>>, mut highlight: ResMut<ControlHighlight>) -> bool {
    *highlight = ControlHighlight {
        gadget: run.data.gadget_control,
        weapon: run.data.weapon_control,
        movement: run.data.movement_control,
    };
    true
}

fn clear(mut highlight: ResMut<ControlHighlight>) {
    *highlight = ControlHighlight::default();
}
