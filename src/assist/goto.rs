//! The assist's driving hand: `GoTo`.
//!
//! New in the port. C# had an unported ancestor, `PlayerCarControlPath`
//! (2013, commented out of `PlayersManager`): while the mouse button (or a
//! touch) was *held*, the car followed the cursor along a path from the
//! navigation (`AIPathCarControl.FollowToPosition`, a new path every second).
//! The intent is the same, a point on the ground the car drives to by
//! itself; here it is one command, [`PlayerCommand::GoTo`], that an AI
//! player can issue as well as a finger (the Smart scheme's double tap).
//!
//! A job ([`Going`], kept in [`Assist::goto`](super::Assist)) plans a path
//! with the navigation ([`plan_path`](crate::ai::nodes::plan_path), the AI's
//! `GoToPosition`) and steers it as the AI's `MoveBase` does
//! ([`follow_stick`](crate::ai::nodes::follow_stick): the carrot ahead of the
//! car, the stick at it), writing the car's `PlayerIntent::move_dir`:
//!
//! - **Plan.** No path, or one a closed door blocks, is refused. With no
//!   navigation graph or no way in it, the car drives straight if no wall
//!   is in the way ([`SharedTerrain`](crate::ai::SharedTerrain)); a refused
//!   point is shown by a short red ring ([`Assist::refused`](super::Assist)).
//! - **Re-plan** every [`REPLAN_SECS`] from where the car is (a failed
//!   re-plan keeps the old path); a car that has moved less than
//!   [`STUCK_DIST`] in [`STUCK_SECS`] is re-planned, and one that does so
//!   twice running gives up ([`Finish::Stuck`]).
//! - **Arrival**: the car's middle within its radius of the point, or the
//!   carrot at the end of the path with the car within its radius of it
//!   ([`arrived`](crate::ai::nodes::arrived), `MoveBase.PathComplete`); the
//!   stick is let go and the car coasts. C# stopped within 1 unit.
//! - **Any manual drive input** ends the job at once: while the job is
//!   active the intent's `move_dir`, which the devices write afresh every
//!   frame, is zero unless a finger, a key or a stick pushed it. A car
//!   nobody drives (`NoInput`) or a dead one drops the job.
//! - **Time out** after [`TIMEOUT_SECS`].
//!
//! Aim and fire (the lock, the soft target) and the magnet's and the
//! accelerator's jobs are independent of the drive and go on.
//!
//! Not ported (not in C# at all): none of `AIPathCarControl`'s path
//! extension for a moving point (`FollowBy`): the point stands still.
//! Differences from the AI's `MoveBase`: the car is considered stuck on the
//! player's own check (twice running), not `PathPeriodControl`'s one miss;
//! the first path is made when the job starts, not on a think.
//!
//! [`step`] is a pure function of a [`Going`] and a [`Scene`] with the
//! planner passed in, so it is tested with no world; [`assist_goto`] reads
//! the scene off the car and writes the stick into its intent.

use avian2d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;

use super::Assist;
use crate::ai::nodes::{arrived, follow_stick, plan_path};
use crate::ai::{Navigation, SharedTerrain};
use crate::car::CarRadius;
use crate::controls::{LocalPlayer, NoInput, PlayerIntent};
use crate::explosion::Health;
use crate::map::nav::NavPath;

/// A job that has not arrived by then is given up, s.
pub const TIMEOUT_SECS: f32 = 30.0;
/// The path is made again from the car's place this often, s.
pub const REPLAN_SECS: f32 = 3.0;
/// A stuck check looks back this far, s ...
pub const STUCK_SECS: f32 = 2.0;
/// ... and the car has not moved this much (the AI's own measure).
pub const STUCK_DIST: f32 = 0.5;
/// How long the red ring of a refused point stays, s.
pub const REFUSED_SECS: f32 = 0.8;
/// `MoveBase`'s `min_dist` for the job: arrived is within the car's radius.
const MIN_DIST: f32 = 0.0;

/// A drive to a point, and how far it has got.
#[derive(Clone, Debug, PartialEq)]
pub struct Going {
    /// Where to, world.
    pub target: Vec2,
    path: Option<NavPath>,
    /// Seconds since the job began, since the path was made, and since the
    /// last stuck check; where the car was at that check.
    elapsed: f32,
    since_plan: f32,
    since_check: f32,
    check_pos: Vec2,
    /// Stuck checks that found the car not moving, running.
    stuck: u8,
}

impl Going {
    pub fn new(target: Vec2) -> Going {
        Going { target, path: None, elapsed: 0.0, since_plan: 0.0, since_check: 0.0, check_pos: Vec2::ZERO, stuck: 0 }
    }

    /// The path being followed, if one is made yet.
    pub fn path(&self) -> Option<&NavPath> {
        self.path.as_ref()
    }
}

/// What a job reads of the car each frame.
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    pub pos: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub dt: f32,
    /// A device pushes the stick itself.
    pub manual: bool,
}

/// How a job ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Arrived,
    /// A device drove.
    Manual,
    /// No path to the point (or one a closed door blocks): refused.
    NoPath,
    /// The car did not get anywhere, twice running.
    Stuck,
    TimedOut,
}

/// What a job does this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    /// The stick to write; `None`: the job does not touch it.
    pub stick: Option<Vec2>,
    pub finished: Option<Finish>,
}

fn done(finish: Finish) -> Step {
    Step { stick: None, finished: Some(finish) }
}

/// One frame of a job. `plan` makes a path from one point to another, or
/// `None` where there is none.
pub fn step(h: &mut Going, s: &Scene, plan: &mut dyn FnMut(Vec2, Vec2) -> Option<NavPath>) -> Step {
    if s.manual {
        return done(Finish::Manual);
    }
    h.elapsed += s.dt;
    if h.elapsed > TIMEOUT_SECS {
        return done(Finish::TimedOut);
    }
    if arrived(h.path.as_ref(), s.pos, h.target, s.radius, MIN_DIST) {
        return done(Finish::Arrived);
    }
    if h.path.is_none() {
        let Some(path) = plan(s.pos, h.target) else {
            return done(Finish::NoPath);
        };
        h.path = Some(path);
        h.since_plan = 0.0;
        h.since_check = 0.0;
        h.check_pos = s.pos;
    } else {
        h.since_plan += s.dt;
        h.since_check += s.dt;
        let mut replan = h.since_plan >= REPLAN_SECS;
        if h.since_check >= STUCK_SECS {
            if s.pos.distance(h.check_pos) < STUCK_DIST {
                h.stuck += 1;
                if h.stuck >= 2 {
                    return done(Finish::Stuck);
                }
                replan = true;
            } else {
                h.stuck = 0;
            }
            h.since_check = 0.0;
            h.check_pos = s.pos;
        }
        if replan {
            // A failed re-plan keeps the path in hand.
            if let Some(path) = plan(s.pos, h.target) {
                h.path = Some(path);
            }
            h.since_plan = 0.0;
        }
    }
    let path = h.path.as_mut().expect("planned above");
    Step { stick: Some(follow_stick(path, s.pos, s.velocity, s.dt, 1.0)), finished: None }
}

/// Layer 2, the driving part: the job of each [`Assist`] becomes the car's
/// stick. Runs after the other jobs ([`super::boost::assist_boost`]).
#[allow(clippy::type_complexity)]
pub fn assist_goto(
    time: Res<Time>,
    navigation: Option<Res<Navigation>>,
    terrain: Option<Res<SharedTerrain>>,
    mut players: Query<
        (&mut Assist, &mut PlayerIntent, &Position, Option<&LinearVelocity>, &CarRadius, &Health, Has<NoInput>),
        With<LocalPlayer>,
    >,
) {
    let dt = time.delta_secs();
    let nav = navigation.as_deref();
    let mut plan = |from: Vec2, to: Vec2| match plan_path(nav, from, to) {
        Ok(path) => Some(path),
        // No way in the graph (or no graph): straight, if no wall is in it.
        Err(None) if terrain.as_ref().is_none_or(|t| !t.0.line_collides(from, to)) => Some(NavPath::new(vec![from, to], false)),
        Err(_) => None,
    };
    for (mut assist, mut intent, at, velocity, radius, health, off) in &mut players {
        if let Some((_, left)) = assist.refused.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                assist.refused = None;
            }
        }
        let Some(mut job) = assist.goto.take() else {
            continue;
        };
        if off || health.hp <= 0 {
            continue;
        }
        let scene = Scene {
            pos: at.0,
            velocity: velocity.map_or(Vec2::ZERO, |v| v.0),
            radius: radius.0,
            dt,
            manual: intent.move_dir != Vec2::ZERO,
        };
        let out = step(&mut job, &scene, &mut plan);
        if let Some(stick) = out.stick {
            intent.move_dir = stick;
        }
        match out.finished {
            Some(finish) => {
                debug!("assist: go to {:?} ended {finish:?}", job.target);
                if finish == Finish::NoPath {
                    assist.refused = Some((job.target, REFUSED_SECS));
                }
            }
            None => assist.goto = Some(job),
        }
    }
}
