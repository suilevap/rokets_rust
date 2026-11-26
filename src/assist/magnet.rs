//! The assist's hand on the magnet: `Grab` and `DropAt`.
//!
//! New in the port (C# had no such scheme), built on the enemies' rules:
//! the button's timings and the conditions to let go are
//! [`crate::gadget::handling`], the same functions `GrabByChainGadget` and
//! `DropByChainGadget` use. A job ([`Handling`], kept in
//! [`Assist::cargo`](super::Assist)) goes through three phases:
//!
//! - **Waiting**: the target is out of reach (further than
//!   [`NEAR`](crate::gadget::handling::NEAR) or behind a wall). The request
//!   stays pending for [`PENDING_SECS`] while the player drives (the stick is
//!   never touched), and its marker stays on, so the player sees what the
//!   car will take.
//! - **Holding**: the magnet's button is held, aimed at the cargo (a unit
//!   way, as the AI aims) or at the drop point. A grab lets go when the
//!   magnet has the *cargo itself* in its sights and the first second
//!   is over (the AI lets go at any candidate); a drop when the towed cargo
//!   lies within its radius plus [`DROP_SLACK`] of the point, or after
//!   [`HOLD_MAX`](crate::gadget::handling::HOLD_MAX) s.
//! - **Releasing**: the button is up, for the magnet to see it in a fixed
//!   step; the job ends when the magnet shows the cargo linked (grab) or
//!   nothing on the rope (drop), or fails after [`RELEASE_SECS`].
//!
//! A job ends [`Finish::Done`], [`Finish::Gone`] (the cargo or the rope is
//! gone), [`Finish::TimedOut`] or [`Finish::Failed`]; the last two, and a
//! cargo lost while the button is held, cancel the gadget. While a device
//! holds the gadget button itself the assist leaves it alone (the manual
//! hand wins) and only notices when the job is done. A device writes the
//! button afresh every frame (`controls::read_gadget`), so what the intent
//! holds when the assist runs is the device's.
//!
//! [`step`] is a function of a [`Scene`] and the job; [`assist_cargo`]
//! reads the scene off the world and writes the result into the intent.
//!
//! Not ported (not in C# at all): driving to the cargo by itself; the stick
//! keeps driving, the magnet's own rule that a dragged cargo stands the
//! car (`gadget::use_magnets`) holds.

use avian2d::prelude::{Collider, Position};
use bevy::prelude::*;

use super::Assist;
use crate::controls::{LocalPlayer, NoInput, PlayerIntent, reach};
use crate::explosion::Health;
use crate::gadget::handling::{HOLD_MAX, NEAR, closer_than, drop_holds, grab_holds};
use crate::gadget::{Magnet, REACH};

/// A request that cannot be carried out yet waits this long, s.
pub const PENDING_SECS: f32 = 10.0;
/// After the button is let go the magnet must show the result within this, s.
pub const RELEASE_SECS: f32 = 0.5;
/// A dropped cargo counts as on its point when its middle is within its own
/// radius plus this of it (the AI's order radius `R`).
pub const DROP_SLACK: f32 = 0.75;
/// The size taken for a cargo without a collider, half the side.
const DEFAULT_RADIUS: f32 = 0.5;

/// What the assist was asked to do with the magnet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    /// Take this thing on the rope.
    Grab(Entity),
    /// Let the towed cargo go over this point (world).
    Drop(Vec2),
}

/// Where a job is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Waiting,
    Holding,
    Releasing,
}

/// A job and how far it has got.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handling {
    pub job: Job,
    pub phase: Phase,
    /// Seconds in the phase.
    since: f32,
    /// Seconds spent waiting, all in all.
    waited: f32,
}

impl Handling {
    pub fn new(job: Job) -> Handling {
        Handling { job, phase: Phase::Waiting, since: 0.0, waited: 0.0 }
    }

    /// Whether the magnet's button is in the assist's hand (held or just let
    /// go), so that replacing the job must cancel it.
    pub fn engaged(&self) -> bool {
        self.phase != Phase::Waiting
    }
}

/// A thing as the job sees it: where it is and how big.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub pos: Vec2,
    pub radius: f32,
}

/// What a job reads of the world each frame.
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    /// The car.
    pub from: Vec2,
    /// What is on the rope (`Magnet::towing`).
    pub towing: Option<Entity>,
    /// What the magnet would grab if let go now (`Magnet::candidate`).
    pub candidate: Option<Entity>,
    /// For a grab the thing to grab; for a drop what is towed. `None`: not
    /// there (gone, or nothing towed).
    pub cargo: Option<Spot>,
    /// No wall between the car and the target (the cargo or the point).
    pub in_sight: bool,
    /// A device holds the gadget button itself.
    pub manual: bool,
    pub dt: f32,
}

/// Where the gadget aims.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// A way, length 0..1 of the reach (`PlayerIntent::gadget_dir`).
    Dir(Vec2),
    /// A world point (`PlayerIntent::gadget_at`).
    At(Vec2),
}

/// How a job ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Done,
    /// The cargo, or the rope, was lost.
    Gone,
    /// Out of reach for [`PENDING_SECS`].
    TimedOut,
    /// Held or let go in vain.
    Failed,
}

/// What a job does this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    /// `Some(true)` the button is held, `Some(false)` it is up and the assist
    /// has it; `None` the assist does not touch it.
    pub hold: Option<bool>,
    pub aim: Option<Aim>,
    /// The gadget is cancelled (what it holds goes, nothing is linked).
    pub cancel: bool,
    pub finished: Option<Finish>,
}

impl Step {
    fn end(finish: Finish, cancel: bool) -> Step {
        Step { hold: cancel.then_some(false), aim: None, cancel, finished: Some(finish) }
    }
}

/// One frame of a job.
pub fn step(h: &mut Handling, s: &Scene) -> Step {
    match h.job {
        Job::Grab(cargo) => step_grab(h, s, cargo),
        Job::Drop(at) => step_drop(h, s, at),
    }
}

fn step_grab(h: &mut Handling, s: &Scene, cargo: Entity) -> Step {
    if s.towing == Some(cargo) {
        return Step { finished: Some(Finish::Done), ..Step::default() };
    }
    let Some(spot) = s.cargo else {
        return Step::end(Finish::Gone, h.engaged());
    };
    if s.manual {
        return Step::default();
    }
    let aim = Some(Aim::Dir((spot.pos - s.from).normalize_or_zero()));
    if h.phase == Phase::Waiting {
        h.waited += s.dt;
        if h.waited > PENDING_SECS {
            return Step::end(Finish::TimedOut, false);
        }
        if s.towing.is_none() && closer_than(spot.pos, s.from, NEAR) && s.in_sight {
            h.phase = Phase::Holding;
            h.since = 0.0;
        } else {
            return Step::default();
        }
    }
    h.since += s.dt;
    match h.phase {
        Phase::Holding => {
            if h.since >= HOLD_MAX {
                return Step::end(Finish::Failed, true);
            }
            if !closer_than(spot.pos, s.from, REACH) {
                // Out of reach again: let go of nothing, wait for the player.
                h.phase = Phase::Waiting;
                return Step { hold: Some(false), cancel: true, ..Step::default() };
            }
            if grab_holds(s.candidate == Some(cargo), h.since) {
                Step { hold: Some(true), aim, ..Step::default() }
            } else {
                h.phase = Phase::Releasing;
                h.since = 0.0;
                Step { hold: Some(false), aim, ..Step::default() }
            }
        }
        _ => {
            if h.since > RELEASE_SECS {
                return Step::end(Finish::Failed, true);
            }
            Step { hold: Some(false), aim, ..Step::default() }
        }
    }
}

fn step_drop(h: &mut Handling, s: &Scene, at: Vec2) -> Step {
    let Some(spot) = s.cargo else {
        // Nothing on the rope: let go where it was is a success.
        return if h.phase == Phase::Releasing {
            Step { finished: Some(Finish::Done), ..Step::default() }
        } else {
            Step::end(Finish::Gone, h.engaged())
        };
    };
    if s.manual {
        return Step::default();
    }
    let aim = Some(Aim::At(at));
    if h.phase == Phase::Waiting {
        h.waited += s.dt;
        if h.waited > PENDING_SECS {
            return Step::end(Finish::TimedOut, false);
        }
        if closer_than(at, s.from, NEAR) && s.in_sight {
            h.phase = Phase::Holding;
            h.since = 0.0;
        } else {
            return Step::default();
        }
    }
    h.since += s.dt;
    match h.phase {
        Phase::Holding => {
            // The car stands while it drags, so the point does not leave
            // the reach: the button is held until the cargo is over it, or
            // for as long as the AI holds it at most.
            if drop_holds(closer_than(at, spot.pos, spot.radius + DROP_SLACK), h.since) {
                Step { hold: Some(true), aim, ..Step::default() }
            } else {
                h.phase = Phase::Releasing;
                h.since = 0.0;
                Step { hold: Some(false), aim, ..Step::default() }
            }
        }
        _ => {
            if h.since > RELEASE_SECS {
                return Step::end(Finish::Failed, true);
            }
            Step { hold: Some(false), aim, ..Step::default() }
        }
    }
}

/// Layer 2, the magnet's part: the job of each [`Assist`] becomes the
/// car's gadget intent. Runs after the aim and trigger
/// ([`assist_drive`](super::assist_drive)).
#[allow(clippy::type_complexity)]
pub fn assist_cargo(
    time: Res<Time>,
    terrain: Option<Res<crate::ai::SharedTerrain>>,
    mut players: Query<(&mut Assist, &mut PlayerIntent, &Position, Option<&Magnet>, &Health, Has<NoInput>), With<LocalPlayer>>,
    things: Query<(&Position, Option<&Collider>, Option<&crate::car::CarRadius>), Without<LocalPlayer>>,
) {
    let dt = time.delta_secs();
    let blocked = |a: Vec2, b: Vec2| terrain.as_ref().is_some_and(|t| t.0.line_collides(a, b));
    let spot_of = |e: Entity| {
        things.get(e).ok().map(|(at, collider, car)| Spot {
            pos: at.0,
            radius: car.map(|c| c.0).or_else(|| collider.map(|c| c.shape_scaled().compute_local_aabb().half_extents().max_element())).unwrap_or(DEFAULT_RADIUS),
        })
    };
    for (mut assist, mut intent, position, magnet, health, off) in &mut players {
        if assist.cancel_gadget {
            assist.cancel_gadget = false;
            intent.gadget = false;
            intent.gadget_cancel = true;
        }
        let Some(mut job) = assist.cargo else {
            continue;
        };
        if health.hp <= 0 || off || magnet.is_none() {
            assist.cargo = None;
            continue;
        }
        let magnet = magnet.expect("checked above");
        let from = position.0;
        let (cargo, target) = match job.job {
            Job::Grab(e) => (spot_of(e), spot_of(e).map(|s| s.pos)),
            Job::Drop(at) => (magnet.towing().and_then(spot_of), Some(at)),
        };
        let scene = Scene {
            from,
            towing: magnet.towing(),
            candidate: magnet.candidate(),
            cargo,
            in_sight: target.is_none_or(|t| !blocked(from, t)),
            manual: intent.gadget,
            dt,
        };
        let out = step(&mut job, &scene);
        if let Some(hold) = out.hold {
            intent.gadget = hold;
        }
        match out.aim {
            Some(Aim::Dir(d)) => {
                intent.gadget_at = None;
                intent.gadget_dir = Some(d);
                intent.look = d;
            }
            Some(Aim::At(p)) => {
                intent.gadget_at = Some(p);
                intent.gadget_dir = None;
                intent.look = reach(p - from);
            }
            None => {}
        }
        if out.cancel {
            intent.gadget = false;
            intent.gadget_cancel = true;
        }
        match out.finished {
            Some(finish) => {
                debug!("assist: {:?} ended {finish:?}", job.job);
                assist.cargo = None;
            }
            None => assist.cargo = Some(job),
        }
    }
}
