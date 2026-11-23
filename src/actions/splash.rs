//! `ShowSplash` (`HideSplash()`), for now without the splash itself: its
//! `ActionOnHide` runs when it would have hidden, `AutoHideTimeSec` later
//! (two seconds for one left to a tap), so the maps that go on from a
//! splash (an intro, then the camera back on the car) still do.
//!
//! `SplashSetBindVariable` (`ActionRunner.SplashSetBindVariableRun`, added
//! in 2014 with the Door's Dream map for its two scores) puts the value of
//! a variable (`Value`: a number, or a variable's name, as `GetInt` reads
//! it: global, else the actor's; 0 when unknown) into one of the splash
//! text slots (`SplashScreen.BindText`) as text. A splash layer with a
//! `BindTextId` draws that slot's text, so a splash can show a live score
//! or counter: Door's Dream's two scores (`PongScore`), the friends left
//! in `Swarm` and the coins in `WinnersRoom` (`TimerAndCounter`). The
//! action only stores the text, on the map's actions (`FixedUpdate`);
//! what draws it reads `SplashBindText` in `Update`. Until splashes are
//! drawn, the debug text (`hud`) shows the slots that are set.
//! `BindVariableNum` is 1 to 5 in C#; slot 0 belongs to the splash's
//! countdown (`SplashScreen.DrawNormal`), which this port does not have.
//!
//! Not ported:
//! - drawing splashes (`Splashes/Splashes.xml`), taps, and so the
//!   countdown in slot 0;
//! - `Fifth` (5): C#'s `BindText` has five slots (0 to 4), so it throws
//!   `IndexOutOfRangeException`; here it is logged and ignored (no map uses
//!   it);
//! - `BindText` is static in C# and keeps its text from one map to the
//!   next (and the UI writes slots 1 and 2 for its own screens); here the
//!   slots are cleared with the map, so a restart looks like a first start.

use bevy::prelude::*;

use super::variables::Variables;
use super::{AddMapAction, Run, RunAction, run_later};
use crate::content::actions::defs::{ShowSplash, SplashSetBindVariable};
use crate::game::GameState;

/// Seconds a splash left to a tap stays (none: until tapped).
const TAP_WAIT: f32 = 2.0;

/// Text slots in `SplashScreen.BindText`.
const BIND_SLOTS: usize = 5;

/// The text bound to splashes, by slot (`SplashScreen.BindText`): what
/// `SplashSetBindVariable` set, `None` until then. Slot 0 is the splash
/// countdown, never set by an action.
#[derive(Resource, Debug, Default)]
pub struct SplashBindText {
    slots: [Option<String>; BIND_SLOTS],
}

impl SplashBindText {
    pub fn get(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot)?.as_deref()
    }

    pub fn set(&mut self, slot: usize, text: String) -> bool {
        match self.slots.get_mut(slot) {
            Some(s) => {
                *s = Some(text);
                true
            }
            None => false,
        }
    }

    /// The slots that are set, as `(slot, text)`.
    pub fn set_slots(&self) -> impl Iterator<Item = (usize, &str)> {
        self.slots.iter().enumerate().filter_map(|(i, t)| Some((i, t.as_deref()?)))
    }
}

pub struct SplashActionsPlugin;

impl Plugin for SplashActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SplashBindText>()
            .add_map_action_fn::<ShowSplash>(show_splash)
            .add_map_action::<SplashSetBindVariable, _>(set_bind_variable)
            .add_systems(OnEnter(GameState::Playing), clear);
    }
}

fn show_splash(world: &mut World, run: Run<ShowSplash>) -> bool {
    let d = run.data;
    if d.splash_name.as_deref().is_none_or(|n| n.trim().is_empty()) {
        return true;
    }
    let after = if d.auto_hide_time_sec > 0.0 { d.auto_hide_time_sec } else { TAP_WAIT };
    if let Some(name) = d.action_on_hide.filter(|a| !a.trim().is_empty()) {
        run_later(world, after, RunAction { name, actor: run.actor });
    }
    true
}

fn clear(mut text: ResMut<SplashBindText>) {
    *text = SplashBindText::default();
}

fn set_bind_variable(
    In(run): In<Run<SplashSetBindVariable>>,
    mut vars: ResMut<Variables>,
    mut text: ResMut<SplashBindText>,
) -> bool {
    let d = &run.data;
    // C#'s `BindVariableNum` starts at 1: 0 is the splash countdown.
    let slot = d.bind_variable_number as usize + 1;
    // No `Value` is a variable of no name: not a number, nothing set, 0.
    let value = vars.int(run.actor, d.value.as_deref().unwrap_or_default());
    if !text.set(slot, value.to_string()) {
        warn!("SplashSetBindVariable: no text slot {slot}");
    }
    true
}
