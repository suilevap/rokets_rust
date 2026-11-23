//! `SetImmortal` (`SetImmortal(id,value)`): a car's shield stops taking
//! harm, or starts again.
//!
//! C# origin: `ActionRunner.SetImmortalRun`, added with
//! `ShieldComponent.Immortal` in July 2015 ("Add Shield.Immortal property
//! (ignore damage)") for BattleCity's turrets, which `ActionOnCreate`
//! makes immortal so the player has to go round them; Escape's laser
//! turret 3015 is the other user. What it does lives in [`Shield`]
//! (`spend` passes nothing on while the shield has points) and the health
//! bar (none for an immortal shield).
//!
//! Branches of `SetImmortalRun`, all ported:
//! - the target is `GetEntity(ItemId, itemId)`: an `ItemId` of 0 or less
//!   means whoever set the action off (`Run::target`);
//! - the target must be a car (`as ControlableEntity`) with a shield
//!   (`Shield != null`); a shield is there only for a car whose
//!   `Cars.xml` entry has one. Anything else is left alone;
//! - the shield's `Immortal` is set to `IsImmortal`.
//!
//! It succeeds whatever it found: C#'s `Register` wraps a handler that
//! returns nothing as always true, so a `NextAction` follows even when the
//! target is missing or has no shield (other actions here report false for
//! a missing target).
//!
//! Not ported: the shield's burst effect when `Immortal` goes from true to
//! false (`CreateShieldDestroyEffect`) and the bubble drawn in full while
//! it is true (`ShieldComponent.Draw`), as the shield's bubble is not
//! drawn at all yet (see `car::shield`); nor the score the player gets for
//! hurting a car (`ScoreManager.HandleDamage` skips immortal ones; there
//! is no score yet).

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::car::shield::Shield;
use crate::content::actions::defs::SetImmortal;

pub struct ImmortalActionPlugin;

impl Plugin for ImmortalActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetImmortal, _>(set_immortal);
    }
}

fn set_immortal(In(run): In<Run<SetImmortal>>, ids: MapIds, mut shields: Query<&mut Shield>) -> bool {
    if let Some(e) = ids.entity(run.target(run.data.item_id))
        && let Ok(mut shield) = shields.get_mut(e)
    {
        shield.set_immortal(run.data.is_immortal);
    }
    true
}
