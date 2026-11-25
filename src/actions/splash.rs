//! `ShowSplash` (`HideSplash()`): hands the splash to what draws it
//! (`crate::splash`, in `Update`) as a [`SplashCommand`], and runs its
//! `ActionOnHide` `AutoHideTimeSec` later (two seconds for one with no time,
//! left to a tap), so the maps that go on from a splash (an intro, then the
//! camera back on the car) go on. The splash starts to hide at that same
//! moment (`crate::splash` counts its own hold the same way), which is what
//! C# did: `ActionOnHide` is the splash's hide callback.
//!
//! C# origin: `ActionRunner.ShowSplashRun` (no name: `SplashScreen.HideScreen`,
//! else `SplashScreen.Show(name, clearPreviousState: true, …, autoHideTime)`
//! and `Clickable = action.Clickable`).
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
//! `crate::splash` reads `SplashBindText` in `Update`.
//! `BindVariableNum` is 1 to 5 in C#; slot 0 belongs to the splash's
//! countdown (`SplashScreen.DrawNormal`), which `crate::splash` writes.
//!
//! Not ported:
//! - `ActionOnHide` timing is not C#'s: C# ran it when the splash began to
//!   hide, `AutoHideTimeSec` after it had finished fading in; here it runs
//!   `AutoHideTimeSec` after the action ran (the splash hides then too, see
//!   `crate::splash`). A `HideSplash()` or another `ShowSplash` does not
//!   run the shown splash's `ActionOnHide` at once (`SplashScreen.Init`
//!   and `Hide` called the old hide callback), and a splash with no
//!   `AutoHideTimeSec` that is not clickable runs it after two seconds
//!   instead of never (none of the campaign's has an `ActionOnHide`);
//! - `Clickable` and a tap hiding the splash (`ControlHandler_Pressed`): no
//!   splash of the campaign is clickable (see `crate::splash`);
//! - `ShowSplashRun`'s `!UIManager.IsActive` (the menus) and the
//!   `World.GameMode.IsLevelInProgress` check before `ActionOnHide`: there
//!   are no menus, and `crate::game_mode` has no such flag;
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

/// What `ShowSplash` asks of the splash screen (`crate::splash`).
#[derive(Message, Debug, Clone, PartialEq)]
pub enum SplashCommand {
    /// `SplashScreen.Show`: the splash of that name replaces the shown one;
    /// it hides itself after `auto_hide` seconds, or stays when that is
    /// not above zero.
    Show { name: String, auto_hide: f32 },
    /// `SplashScreen.HideScreen`.
    Hide,
}

pub struct SplashActionsPlugin;

impl Plugin for SplashActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SplashBindText>()
            .add_message::<SplashCommand>()
            .add_map_action_fn::<ShowSplash>(show_splash)
            .add_map_action::<SplashSetBindVariable, _>(set_bind_variable)
            .add_systems(OnEnter(GameState::Playing), clear);
    }
}

fn show_splash(world: &mut World, run: Run<ShowSplash>) -> bool {
    let d = run.data;
    let Some(name) = d.splash_name.filter(|n| !n.trim().is_empty()) else {
        world.resource_mut::<Messages<SplashCommand>>().write(SplashCommand::Hide);
        return true;
    };
    world.resource_mut::<Messages<SplashCommand>>().write(SplashCommand::Show { name, auto_hide: d.auto_hide_time_sec });
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
    vars: Res<Variables>,
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
