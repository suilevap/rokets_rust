//! `ChangeMapEffectAmount` (`MapEffect(amount)`): tells the level's screen
//! effect how strong to be, `0` off and `1` full.
//!
//! C# origin: `ActionRunner.ChangeMapEffectAmountRun` (2013, `Add action to
//! change map effect amount`), which called `IMapEffect.ChangeAmount` on the
//! effect the map has (`MapManager.MapEffect`; nothing when there is none).
//! `LetItSnow` uses it: the room with the stove (`MapEffect(-5)` on
//! entering, `MapEffect(1)` on leaving) melts the frost and stops the snow.
//!
//! What the amount means is the effect's own: the screen snow takes it as
//! how much snow falls, the frozen glass as the direction its frost grows
//! in. So the action only hands the number on, as a [`MapEffectAmount`]
//! message, and each effect (`render::snow`, `render::frozen_glass`) reads
//! it in `Update`.
//!
//! Not ported: nothing; `MapEffectAmount` is simply not read by a map whose
//! effect ignores it (the dirty lens of a plain map, which C#'s
//! `DirtyLensEffectDualTexture` did too).

use bevy::prelude::*;

use super::{AddMapAction, Run};
use crate::content::actions::defs::ChangeMapEffectAmount;

/// `IMapEffect.ChangeAmount(amount)`: the level effect's new amount.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct MapEffectAmount(pub f32);

pub struct MapEffectActionsPlugin;

impl Plugin for MapEffectActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MapEffectAmount>().add_map_action::<ChangeMapEffectAmount, _>(change_amount);
    }
}

fn change_amount(In(run): In<Run<ChangeMapEffectAmount>>, mut amounts: MessageWriter<MapEffectAmount>) -> bool {
    amounts.write(MapEffectAmount(run.data.amount));
    true
}
