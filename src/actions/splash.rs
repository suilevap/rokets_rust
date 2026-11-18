//! `ShowSplash` (`HideSplash()`), for now without the splash itself: its
//! `ActionOnHide` runs when it would have hidden, `AutoHideTimeSec` later
//! (two seconds for one left to a tap), so the maps that go on from a
//! splash (an intro, then the camera back on the car) still do.
//!
//! Not ported: drawing splashes (`Splashes/Splashes.xml`), taps.

use bevy::prelude::*;

use super::{AddMapAction, Run, RunAction, run_later};
use crate::content::actions::defs::ShowSplash;

/// Seconds a splash left to a tap stays (none: until tapped).
const TAP_WAIT: f32 = 2.0;

pub struct SplashActionsPlugin;

impl Plugin for SplashActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action_fn::<ShowSplash>(show_splash);
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
