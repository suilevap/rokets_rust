//! The assist's hand on the accelerator: `Boost`.
//!
//! New in the port (C# had the pad's gadget button only). A boost is the
//! button held until [`Accelerator::active`] shows the gadget has worked
//! (it works once the button has been held for
//! [`CHARGE`](crate::gadget::accelerator::CHARGE) s), then let go. The
//! time is the gadget's own, read off it, not copied here.
//!
//! - **Refused** without a ready accelerator ([`Accelerator::ready`]: the
//!   energy bar is whole): a press that is not ready only fizzles for a
//!   quarter of the boost and pings, so the tap does nothing at all
//!   (`apply_commands`, and a job whose gadget stops being ready before it
//!   fires ends [`Finish::NotReady`]).
//! - **A device wins**: when a device holds the gadget button (the intent's
//!   `gadget` is set when the assist runs) the job is dropped and nothing is
//!   written; a boost the player pressed himself is not repeated.
//! - A job that does not see the gadget work within [`HOLD_MAX`] s lets go
//!   and cancels it (it never happens with the real gadget, which works in
//!   [`CHARGE`](crate::gadget::accelerator::CHARGE) s).
//!
//! [`step`] is a pure function of a [`Scene`] and the job, as
//! [`super::magnet::step`] is; [`assist_boost`] reads the scene off the car
//! and writes the result into its intent.

use bevy::prelude::*;

use super::Assist;
use crate::controls::{LocalPlayer, NoInput, PlayerIntent};
use crate::explosion::Health;
use crate::gadget::accelerator::{Accelerator, CHARGE};

/// The longest the button is held waiting for the gadget to work, s: the
/// charge and half a second for slow frames.
pub const HOLD_MAX: f32 = CHARGE + 0.5;

/// A boost in hand: the button is held.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boosting {
    /// Seconds the button has been held.
    held: f32,
}

/// What a boost reads of the car each frame.
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    /// The gadget's energy is whole.
    pub ready: bool,
    /// The gadget has worked for this hold.
    pub active: bool,
    /// A device holds the gadget button itself.
    pub manual: bool,
    pub dt: f32,
}

/// How a boost ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// The gadget worked and the button is up.
    Done,
    /// A device took the button, or the car lost its gadget.
    Gone,
    /// The energy was not whole when it was to start.
    NotReady,
    /// Held for [`HOLD_MAX`] s without the gadget working.
    Failed,
}

/// What a boost does this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    /// `Some(true)` the button is held, `Some(false)` it is let go; `None`
    /// the assist does not touch it.
    pub hold: Option<bool>,
    /// The gadget is cancelled (a hold that did not work).
    pub cancel: bool,
    pub finished: Option<Finish>,
}

/// One frame of a boost.
pub fn step(h: &mut Boosting, s: &Scene) -> Step {
    if s.manual {
        return Step { finished: Some(Finish::Gone), ..Step::default() };
    }
    if s.active {
        return Step { hold: Some(false), finished: Some(Finish::Done), ..Step::default() };
    }
    if h.held == 0.0 && !s.ready {
        return Step { finished: Some(Finish::NotReady), ..Step::default() };
    }
    h.held += s.dt.max(f32::EPSILON);
    if h.held > HOLD_MAX {
        return Step { hold: Some(false), cancel: true, finished: Some(Finish::Failed) };
    }
    Step { hold: Some(true), ..Step::default() }
}

/// Layer 2, the accelerator's part: the boost of each [`Assist`] becomes the
/// car's gadget button. Runs after the magnet's ([`super::magnet::assist_cargo`]).
#[allow(clippy::type_complexity)]
pub fn assist_boost(
    time: Res<Time>,
    mut players: Query<(&mut Assist, &mut PlayerIntent, Option<&Accelerator>, &Health, Has<NoInput>), With<LocalPlayer>>,
) {
    let dt = time.delta_secs();
    for (mut assist, mut intent, accelerator, health, off) in &mut players {
        let Some(mut job) = assist.boost else {
            continue;
        };
        let (Some(accelerator), true, false) = (accelerator, health.hp > 0, off) else {
            assist.boost = None;
            continue;
        };
        let scene = Scene { ready: accelerator.ready(), active: accelerator.active(), manual: intent.gadget, dt };
        let out = step(&mut job, &scene);
        if let Some(hold) = out.hold {
            intent.gadget = hold;
        }
        if out.cancel {
            intent.gadget = false;
            intent.gadget_cancel = true;
        }
        match out.finished {
            Some(finish) => {
                debug!("assist: boost ended {finish:?}");
                assist.boost = None;
            }
            None => assist.boost = Some(job),
        }
    }
}
