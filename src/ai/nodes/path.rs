//! The actions that drive a car along a path: `MoveBase`, `MoveToPoint`,
//! `FollowBy`.
//!
//! C# origin: `BTActions/MoveBase.cs`, `MoveToPoint.cs`, `FollowBy.cs`
//! (2013). All three follow the blackboard's `Path` ([`NavPath`]): the car
//! steers at the path's *carrot* ([`NavPath::update`] runs it ahead of the car
//! at the car's speed, between `1 + d/4` and `2 + d` away, `d` being 1.5
//! times the speed), the stick as long as the way there (shorter than
//! `1 + d/4` it is a part of the full push), times the action's speed rate.
//!
//! - [`MoveBase`]: follows the path the blackboard already has (`SetPath`'s,
//!   `MoveByPath`). Starts if there is one.
//! - [`MoveToPoint`]: asks the navigation for a path to a point the tree names
//!   (`GoToPosition`) when it starts, then follows it as `MoveBase`.
//! - [`MoveToRandomVisiblePosition`]: the same, the point picked at random
//!   near a place the tree names (`GetRandomFreeVisiblePoint`).
//! - [`FollowBy`]: the same, and keeps the path up as the point moves: while
//!   the path is under two seconds old and the point has gone less than 6
//!   from its end, it is extended to the point once the point is more than
//!   2.7 from the end; otherwise a new path is made.
//!
//! The action is over (and fails or succeeds by `Complete`) when: there is no
//! path; on an open path two seconds have passed since the last check, the
//! stick was not zero and the car has moved less than half a unit since
//! (stuck); the car has arrived (the carrot is at the end of the path and the
//! car within its radius of it, or within its radius plus the action's
//! `min_dist` of the point it goes to); or the car cannot move. It ends `Ok`
//! when it has arrived, and by then clears the stick and drops the path.
//!
//! The first check of `PathPeriodControl` is made by [`MoveBase`] when it
//! starts; [`MoveToPoint`] (and so [`FollowBy`]) does not start it (they
//! override `Start`), so on a path they made the first check is at once.
//!
//! Not ported: `MoveBase`'s `blackboard.Path.Update` on a null path (C#
//! would throw; the action is not in progress then, so it is not ticked);
//! `FakePosition` (`ToGoWorld` measures from the car's drawn position: the
//! physics position here).

use bevy::math::Vec2;
use flatbt::BtAction;

use crate::ai::blackboard::AiBlackboard;
use crate::ai::navigation::Navigation;
use crate::ai::orders::Orders;
use crate::map::nav::{Blocker, NavPath};

/// What a position function reads off the blackboard, and may pick at
/// random from (`positionFunc`: `GetRandomNodePosition`,
/// `GetRandomFreeNonVisiblePoint`, … draw on the car's random stream).
pub type PositionFn = fn(&mut AiBlackboard) -> Vec2;

/// `VectorExtension.CloserThan`.
fn closer_than(a: Vec2, b: Vec2, distance: f32) -> bool {
    a.distance_squared(b) < distance * distance
}

/// `BaseControl.ToGoWorld` / `Aim`: the stick that goes from `from` to `to`:
/// the whole way over `max_length` is 1, a nearer point a part of it.
pub fn to_go_world(from: Vec2, to: Vec2, max_length: f32) -> Vec2 {
    let delta = to - from;
    if delta.length_squared() < max_length * max_length {
        delta / max_length
    } else {
        delta.normalize_or_zero()
    }
}

/// `MoveBase`'s speed rate and `minDist`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveParams {
    /// The stick is scaled by this (`speedRate`).
    pub speed_rate: f32,
    /// How far from the point is near enough (`minDist`), on top of the car's
    /// radius.
    pub min_dist: f32,
}

impl Default for MoveParams {
    fn default() -> MoveParams {
        MoveParams { speed_rate: 1.0, min_dist: 1.0 }
    }
}

/// `MoveBase.PathComplete` over plain values (shared with the player's
/// `GoTo`, `assist::goto`): the carrot is at the end of the open `path` and
/// the car at `pos` is within its `radius` of it, or the car is within its
/// radius plus `min_dist` of the `target` it goes to.
pub fn arrived(path: Option<&NavPath>, pos: Vec2, target: Vec2, radius: f32, min_dist: f32) -> bool {
    let at_the_end = path.is_some_and(|p| p.is_end() && closer_than(pos, p.current_position(), radius));
    at_the_end || closer_than(pos, target, radius + min_dist)
}

/// `MoveBase.PathComplete`.
fn path_complete(bb: &AiBlackboard, params: MoveParams) -> bool {
    arrived(bb.path.as_ref(), bb.pos, bb.target_position, bb.radius, params.min_dist)
}

/// `MoveBase.IsInProgress`.
fn in_progress(bb: &AiBlackboard, params: MoveParams) -> bool {
    let Some(path) = &bb.path else {
        return false;
    };
    let mut available = true;
    if !path.closed() && bb.path_period_available() {
        if bb.stick != Vec2::ZERO && closer_than(bb.pos, bb.previous_period_position(), 0.5) {
            available = false;
        } else {
            bb.path_period_update();
            bb.remember_period_position();
        }
    }
    available && !path_complete(bb, params) && bb.can_move
}

/// `MoveBase.Tick`'s steering over plain values (shared with the player's
/// `GoTo`): the carrot of `path` runs on ahead of a car at `pos` moving at
/// `velocity` (`step` seconds a frame), and the stick points at it, times
/// `speed_rate`.
pub fn follow_stick(path: &mut NavPath, pos: Vec2, velocity: Vec2, step: f32, speed_rate: f32) -> Vec2 {
    let speed = velocity.length();
    // `speed * ElapsedGameTime.Milliseconds / 1000`: whole milliseconds.
    let reach = speed * (step * 1000.0).trunc() / 1000.0;
    let d = speed * 1.5;
    let (min_d, max_d) = (1.0 + d / 4.0, 2.0 + d);
    path.update(pos, reach, min_d, max_d);
    to_go_world(pos, path.current_position(), min_d) * speed_rate
}

/// `MoveBase.Tick`.
fn tick(bb: &mut AiBlackboard, params: MoveParams) -> Orders {
    let (pos, velocity, step) = (bb.pos, bb.velocity, bb.step);
    let Some(path) = bb.path.as_mut() else {
        return Orders::default();
    };
    Orders { move_dir: Some(follow_stick(path, pos, velocity, step, params.speed_rate)), ..Orders::default() }
}

/// `MoveBase.Complete`: the stick is let go, the path dropped; `Ok` if the car
/// has arrived.
fn complete(bb: &mut AiBlackboard, params: MoveParams) -> bool {
    bb.clear_stick();
    let arrived = path_complete(bb, params);
    bb.path = None;
    arrived
}

/// `MoveBase.GoToPosition`: a path to `pos` for the blackboard, if there is
/// one and nothing blocks it; a blocked one leaves what blocks it in
/// `blocker` and is refused. A `pos` of exactly the origin is "no point"
/// (`Vector2.Zero`).
pub fn go_to_position(bb: &mut AiBlackboard, pos: Vec2) -> bool {
    if pos == Vec2::ZERO {
        return false;
    }
    match plan_path(bb.nav.as_ref(), bb.pos, pos) {
        Ok(path) => {
            bb.path = Some(path);
            bb.target_position = pos;
            bb.path_compute_time = bb.now();
            true
        }
        Err(blocker) => {
            bb.blocker = blocker;
            false
        }
    }
}

/// The part of `GoToPosition` that asks the navigation (shared with the
/// player's `GoTo`): a path from `from` to `to` that nothing blocks, or the
/// first thing that blocks it (`Err(Some)`, a closed door), or `Err(None)`
/// for no way at all (or no navigation graph).
pub fn plan_path(nav: Option<&Navigation>, from: Vec2, to: Vec2) -> Result<NavPath, Option<Blocker>> {
    match nav.and_then(|nav| nav.get_path(from, to)) {
        Some(path) if path.blockers().is_empty() => Ok(path),
        Some(path) => Err(path.blockers().first().copied().flatten()),
        None => Err(None),
    }
}

/// `MoveBase.AddPosition`: the path goes on to `pos`; with none, a path is made.
fn add_position(bb: &mut AiBlackboard, pos: Vec2) {
    match bb.path.as_mut() {
        Some(path) => path.add_point(pos),
        None => {
            go_to_position(bb, pos);
        }
    }
}

/// `MoveBase`: follows the blackboard's path.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveBase(pub MoveParams);

impl MoveBase {
    pub fn new(speed_rate: f32, min_dist: f32) -> MoveBase {
        MoveBase(MoveParams { speed_rate, min_dist })
    }
}

impl BtAction<AiBlackboard, Orders> for MoveBase {
    type State = ();

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<()> {
        bb.path_period_update();
        bb.path.is_some().then_some(())
    }

    fn is_in_progress(&self, _: &(), bb: &AiBlackboard, _: ()) -> bool {
        in_progress(bb, self.0)
    }

    fn tick(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> Orders {
        tick(bb, self.0)
    }

    fn complete(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> bool {
        complete(bb, self.0)
    }
}

/// `MoveToPoint`: makes a path to a point and follows it.
#[derive(Clone, Copy)]
pub struct MoveToPoint {
    /// `positionFunc`: where to go, asked when the action starts.
    pub position: PositionFn,
    pub params: MoveParams,
}

impl MoveToPoint {
    pub fn new(position: PositionFn) -> MoveToPoint {
        MoveToPoint { position, params: MoveParams::default() }
    }

    /// `MoveToPoint(…, speedRate, minDistanceToTarget)`.
    pub fn with_params(position: PositionFn, speed_rate: f32, min_dist: f32) -> MoveToPoint {
        MoveToPoint { position, params: MoveParams { speed_rate, min_dist } }
    }
}

impl BtAction<AiBlackboard, Orders> for MoveToPoint {
    type State = ();

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<()> {
        let pos = (self.position)(bb);
        go_to_position(bb, pos).then_some(())
    }

    fn is_in_progress(&self, _: &(), bb: &AiBlackboard, _: ()) -> bool {
        in_progress(bb, self.params)
    }

    fn tick(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> Orders {
        tick(bb, self.params)
    }

    fn complete(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> bool {
        complete(bb, self.params)
    }
}

/// `MoveToRandomVisiblePosition(posFunc, distanceFunc, speedRate,
/// minDistanceToTarget)`: [`MoveToPoint`] to a spot picked when it starts, at
/// most `distance` from `around` and in sight of it (or within the room
/// there): `GetRandomFreeVisiblePoint(around, distance)`. It fails to start
/// when no path leads to the spot, or the pick was no point (the origin).
#[derive(Clone, Copy)]
pub struct MoveToRandomVisiblePosition {
    /// `posFunc`: the place the spot is picked near.
    pub around: fn(&AiBlackboard) -> Vec2,
    /// `distanceFunc`: how far from it at most.
    pub distance: fn(&AiBlackboard) -> f32,
    pub params: MoveParams,
}

impl MoveToRandomVisiblePosition {
    pub fn new(
        around: fn(&AiBlackboard) -> Vec2,
        distance: fn(&AiBlackboard) -> f32,
        speed_rate: f32,
        min_dist: f32,
    ) -> MoveToRandomVisiblePosition {
        MoveToRandomVisiblePosition { around, distance, params: MoveParams { speed_rate, min_dist } }
    }
}

impl BtAction<AiBlackboard, Orders> for MoveToRandomVisiblePosition {
    type State = ();

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<()> {
        let (around, distance) = ((self.around)(bb), (self.distance)(bb));
        let pos = bb.random_free_visible_point(around, distance);
        go_to_position(bb, pos).then_some(())
    }

    fn is_in_progress(&self, _: &(), bb: &AiBlackboard, _: ()) -> bool {
        in_progress(bb, self.params)
    }

    fn tick(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> Orders {
        tick(bb, self.params)
    }

    fn complete(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> bool {
        complete(bb, self.params)
    }
}

/// `FollowBy`: [`MoveToPoint`] that keeps the path up with the point.
#[derive(Clone, Copy)]
pub struct FollowBy(pub MoveToPoint);

impl FollowBy {
    pub fn new(position: PositionFn) -> FollowBy {
        FollowBy(MoveToPoint::new(position))
    }

    /// `FollowBy(…, speedRate, minDist)`.
    pub fn with_params(position: PositionFn, speed_rate: f32, min_dist: f32) -> FollowBy {
        FollowBy(MoveToPoint::with_params(position, speed_rate, min_dist))
    }
}

/// `FollowBy.FollowToPosition`: `d` (6 × 6): how far the point may have gone
/// from the path's end for the path to be extended; `d0` (1.5 × 5): how far
/// before it is; `deltaTimeSec`: how old the path may be.
const EXTEND_RANGE_SQ: f32 = 6.0 * 6.0;
const EXTEND_FROM_SQ: f32 = 1.5 * 1.0 * 5.0;
const PATH_MAX_AGE: f64 = 2.0;

fn follow_to_position(bb: &mut AiBlackboard, pos: Vec2) {
    let end = bb.path.as_ref().filter(|p| p.full_length() > 0.0).map(|p| p.last_position());
    let Some(end) = end else {
        go_to_position(bb, pos);
        return;
    };
    let gap = end.distance_squared(pos);
    if bb.now() - bb.path_compute_time < PATH_MAX_AGE && gap < EXTEND_RANGE_SQ {
        if gap > EXTEND_FROM_SQ {
            add_position(bb, pos);
        }
    } else {
        go_to_position(bb, pos);
    }
}

impl BtAction<AiBlackboard, Orders> for FollowBy {
    type State = ();

    fn start(&self, bb: &mut AiBlackboard, s: ()) -> Option<()> {
        self.0.start(bb, s)
    }

    fn is_in_progress(&self, s: &(), bb: &AiBlackboard, p: ()) -> bool {
        self.0.is_in_progress(s, bb, p)
    }

    fn tick(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> Orders {
        let pos = (self.0.position)(bb);
        bb.target_position = pos;
        follow_to_position(bb, pos);
        tick(bb, self.0.params)
    }

    fn complete(&self, s: &mut (), bb: &mut AiBlackboard, p: ()) -> bool {
        self.0.complete(s, bb, p)
    }
}
