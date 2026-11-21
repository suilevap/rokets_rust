//! The flocking nodes: `Flocking`, `FindNearestLeader`, and the small
//! actions of `ZombieFlocking` (`Power`, `Fire!`, `Uii`).
//!
//! C# origin: `BTActions/Flocking.cs` (September 2013: "Initial Flocking Ai
//! Action", tuned with the Zombie in October, "random traits to all bots"),
//! `BTActions/FindNearestLeader.cs` (December 2013, the local player first
//! in February 2014 for `Swarm`), and the `SimpleAction`s of
//! `BTActionFactory.ZombieFlocking` (the super attack, May 2015, and the
//! friends told of who hurt it, June 2015).
//!
//! [`Flocking`] is Reynolds' boids for one car: every half second (out of
//! step: the first look a random part of it after the start) the stick is
//! the mean of five pulls, each weighted by the car's traits:
//!
//! | Pull | What | Weight |
//! |---|---|---|
//! | cohesion | to the neighbours' middle, over the neighbour distance | `2 + 2·t1` |
//! | separation | away from each neighbour by `1/d` (the delta over `d²`), times a quarter of the neighbour distance | `1 + r/2 + 2·t2` (`r` the car's radius) |
//! | alignment | the neighbours' mean stick | `1 + t3/4` |
//! | obstacles | the distance field's gradient over the distance, when the room is under `ScanDistance` | `1 + t4` |
//! | target | a unit vector to the point the tree names (none: zero) | `(1/2 + t5/2) × target rate` |
//!
//! The neighbours are the first four friends within the neighbour distance
//! on both axes, picked every two seconds (out of step too) and kept
//! between; one gone since is left out of the means. The stick is the pull
//! made a unit and scaled by the speed rate and `1 − t0/2` (a trait of 0.9
//! or more, the slowest, may lead); a zero pull leaves the stick as it was.
//! The action never ends by itself.
//!
//! Not ported: `Flocking.Start`'s collision change for a car under half a
//! unit across its radius (`PhysicBodyCategory.Boid`: such cars pass
//! through each other and low walls): no flocking car of the maps is that
//! small (the Zombie's radius is 0.98), and the port has no collision
//! layers; `FlockingData.Draw` (the debug lines); the shared `ActionData`
//! slot (an action keeps its own state); a neighbour that is dead but not
//! yet removed counted in the means (only living friends are gathered); a
//! neighbour at the car's very spot (C#'s separation divides by zero, a
//! NaN stick; it is left out of the separation here).

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt::{BtAction, NodeResult};

use crate::ai::blackboard::{AiBlackboard, Alert, Effect, Mate};
use crate::ai::orders::Orders;

/// `FlockingData.NeighboursPeriod = new PeriodicUpdate(2.0f).Sparse()`, s.
pub const NEIGHBOURS_PERIOD: f64 = 2.0;
/// `FlockingData.AxisPeriod = new PeriodicUpdate(0.5f).Sparse()`, s.
pub const AXIS_PERIOD: f64 = 0.5;
/// `candidates.Take(4)`.
pub const NEIGHBOURS: usize = 4;
/// `FindNearestLeader`: an AI friend leads when its `Traits[0]` is over this.
pub const LEADER_TRAIT: f32 = 0.9;

/// Where a flock heads (`Func<AiBlackboard, Vector2> targetPos`).
pub type TargetFn = fn(&AiBlackboard) -> Vec2;

/// `BTActions/Flocking(speedRate, neignboursDistance, targetRate,
/// targetPos)`: see the module docs.
#[derive(Clone, Copy)]
pub struct Flocking {
    /// `_speedRate`.
    pub speed_rate: f32,
    /// `_neignboursDistance`.
    pub neighbour_distance: f32,
    /// `_targetRate`.
    pub target_rate: f32,
    /// `_targetPos`.
    pub target: Option<TargetFn>,
}

impl Flocking {
    /// `Flocking(speedRate, neignboursDistance)`: no target (its rate 1
    /// still weighs in the mean).
    pub fn new(speed_rate: f32, neighbour_distance: f32) -> Flocking {
        Flocking { speed_rate, neighbour_distance, target_rate: 1.0, target: None }
    }

    /// `Flocking(speedRate, neignboursDistance, targetRate, targetPos)`.
    pub fn toward(speed_rate: f32, neighbour_distance: f32, target_rate: f32, target: TargetFn) -> Flocking {
        Flocking { speed_rate, neighbour_distance, target_rate, target: Some(target) }
    }
}

/// `FlockingData`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlockState {
    /// `Neighbours`.
    neighbours: [Option<Entity>; NEIGHBOURS],
    /// `NeighboursPeriod._invalidatedTime`, s of game time.
    neighbours_until: f64,
    /// `AxisPeriod._invalidatedTime`.
    axis_until: f64,
    /// `PreviousAxis`.
    axis: Vec2,
}

impl FlockState {
    fn neighbours<'a>(&'a self, bb: &'a AiBlackboard) -> impl Iterator<Item = &'a Mate> {
        self.neighbours.iter().flatten().filter_map(|&e| bb.mate(e))
    }
}

/// `PeriodicUpdate.NeedUpdate`: due once the time is past the mark; the
/// mark moves a period on.
fn need_update(until: &mut f64, now: f64, period: f64) -> bool {
    let due = now > *until;
    if due {
        *until = now + period;
    }
    due
}

impl Flocking {
    /// `FlockingData.Update`: the neighbours every two seconds, the pull
    /// every half second; the last pull.
    fn update(&self, state: &mut FlockState, bb: &AiBlackboard) -> Vec2 {
        let now = bb.now();
        if need_update(&mut state.neighbours_until, now, NEIGHBOURS_PERIOD) {
            state.neighbours = [None; NEIGHBOURS];
            for (slot, mate) in state.neighbours.iter_mut().zip(bb.friends(self.neighbour_distance)) {
                *slot = Some(mate.entity);
            }
        }
        if need_update(&mut state.axis_until, now, AXIS_PERIOD) {
            state.axis = self.total(state, bb);
        }
        state.axis
    }

    /// `FlockingData.GetTotal`: the weighted mean of the five pulls.
    fn total(&self, state: &FlockState, bb: &AiBlackboard) -> Vec2 {
        let t = &bb.traits;
        let rates = [
            2.0 + t[1] * 2.0,
            1.0 + bb.radius / 2.0 + t[2] * 2.0,
            1.0 + t[3] * 0.25,
            1.0 + t[4],
            (0.5 + t[5] * 0.5) * self.target_rate,
        ];
        let pulls = [
            self.cohesion(state, bb),
            self.separation(state, bb),
            alignment(state, bb),
            avoid_obstacle(bb),
            self.target_pull(bb),
        ];
        let sum: Vec2 = pulls.iter().zip(rates).map(|(p, r)| *p * r).sum();
        sum / rates.iter().sum::<f32>()
    }

    /// `Cohesion`: to the neighbours' middle, over the neighbour distance.
    fn cohesion(&self, state: &FlockState, bb: &AiBlackboard) -> Vec2 {
        let (sum, count) = state.neighbours(bb).fold((Vec2::ZERO, 0), |(s, n), m| (s + m.pos, n + 1));
        if count == 0 {
            return Vec2::ZERO;
        }
        (sum / count as f32 - bb.pos) / self.neighbour_distance
    }

    /// `Separation`: away from each neighbour, `1/d` each, times a quarter
    /// of the neighbour distance (not over their count).
    fn separation(&self, state: &FlockState, bb: &AiBlackboard) -> Vec2 {
        let sum: Vec2 = state
            .neighbours(bb)
            .map(|m| bb.pos - m.pos)
            .filter(|d| *d != Vec2::ZERO)
            .map(|d| d / d.length_squared())
            .sum();
        sum * (self.neighbour_distance / 4.0)
    }

    /// `ToTarget`: a unit vector to the target (`SafeNormalize`), or zero.
    fn target_pull(&self, bb: &AiBlackboard) -> Vec2 {
        self.target.map_or(Vec2::ZERO, |f| (f(bb) - bb.pos).normalize_or_zero())
    }
}

/// `Alignment`: the neighbours' mean stick.
fn alignment(state: &FlockState, bb: &AiBlackboard) -> Vec2 {
    let (sum, count) = state.neighbours(bb).fold((Vec2::ZERO, 0), |(s, n), m| (s + m.stick, n + 1));
    if count == 0 { Vec2::ZERO } else { sum / count as f32 }
}

/// `AvoidObstacle`: the distance field's gradient over the distance, when
/// the room around is more than nothing and less than `ScanDistance`.
fn avoid_obstacle(bb: &AiBlackboard) -> Vec2 {
    let (d, gradient) = bb.room_at(bb.pos);
    if d > 0.0 && d < bb.scan_distance { gradient / d } else { Vec2::ZERO }
}

impl BtAction<AiBlackboard, Orders> for Flocking {
    type State = FlockState;

    /// `Start`: both periods start out of step (`Sparse`) and are asked at
    /// once (which, but for a random draw of nothing, does nothing).
    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<FlockState> {
        let now = bb.now();
        let mut state = FlockState {
            neighbours: [None; NEIGHBOURS],
            neighbours_until: now + bb.random() as f64 * NEIGHBOURS_PERIOD,
            axis_until: now + bb.random() as f64 * AXIS_PERIOD,
            axis: Vec2::ZERO,
        };
        self.update(&mut state, bb);
        Some(state)
    }

    fn is_in_progress(&self, _: &FlockState, _: &AiBlackboard, _: ()) -> bool {
        true
    }

    /// `Tick`: the stick along the pull at the speed rate and
    /// `1 − Traits[0]/2` (`ToGoRelative(axis, 1, 0)` leaves a stick no longer
    /// than 1 as it is); a zero pull writes nothing.
    fn tick(&self, state: &mut FlockState, bb: &mut AiBlackboard, _: ()) -> Orders {
        let axis = self.update(state, bb);
        if axis == Vec2::ZERO {
            return Orders::default();
        }
        let stick = axis.normalize_or_zero() * self.speed_rate * (1.0 - 0.5 * bb.traits[0]);
        Orders { move_dir: Some(stick), ..Orders::default() }
    }
}

/// `BTActions/FindNearestLeader`: picks the leader, succeeding when there
/// is one (a failed look keeps the old one).
///
/// - The last leader, alive and of the car's team, stays if the player
///   drives it; or the player is taken if the last leader (an AI) follows
///   the player (its leader's `LocalDevice`, alive or not).
/// - Otherwise, while the last leader is within `ScanDistance` (straight
///   line): it stays if the car sees its *target* now or in 0.5 s (C#'s
///   `CheckState(SeeTarget | WillSeeTarget)`, about the target, not the
///   leader); else the best of the other friends within `ScanDistance` on
///   both axes that may lead (none: no leader found).
/// - A last leader further off, dead, of another team, or none: the best
///   of the friends within `ScanDistance` that may lead.
///
/// May lead: the player's car, or an AI car whose `Traits[0]` is over 0.9.
/// The best is the player's car, else the AI with the least `Traits[1]`
/// (the first of equals, in the map's order). Over at once.
pub fn find_nearest_leader(bb: &mut AiBlackboard) -> NodeResult<Orders> {
    match nearest_leader(bb) {
        Some(leader) => {
            bb.set_leader(leader);
            NodeResult::Success
        }
        None => NodeResult::Failure,
    }
}

/// May lead: the player's, or an AI with `Traits[0] > 0.9`.
fn may_lead(m: &Mate) -> bool {
    m.local || m.mind.is_some_and(|mind| mind.traits[0] > LEADER_TRAIT)
}

/// `FindNearestLeader.GetBest` (`EntitiesHelper.GetBest`): the least
/// score under `float.MaxValue`: the player's `MinValue`, an AI's
/// `Traits[1]`, anything else never.
fn best<'a>(candidates: impl Iterator<Item = &'a Mate>) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;
    for m in candidates {
        let score = if m.local {
            f32::MIN
        } else if let Some(mind) = m.mind {
            mind.traits[1]
        } else {
            f32::MAX
        };
        if score < best.map_or(f32::MAX, |b| b.0) {
            best = Some((score, m.entity));
        }
    }
    best.map(|b| b.1)
}

/// `FindNearestLeader.GetNearestLeader`.
fn nearest_leader(bb: &AiBlackboard) -> Option<Entity> {
    let scan = bb.scan_distance;
    let best_friend = || best(bb.friends(scan).filter(|m| may_lead(m)));
    let Some(last) = bb.leader().filter(|l| l.team == bb.team) else {
        return best_friend();
    };
    let mut result = None;
    if last.local {
        result = Some(last.entity);
    } else if let Some(mind) = last.mind
        && mind.leader_local
    {
        result = mind.leader;
    }
    if result.is_some() {
        return result;
    }
    if last.pos.distance_squared(bb.pos) < scan * scan {
        if bb.can_see_target() {
            Some(last.entity)
        } else {
            best(bb.friends(scan).filter(|m| m.entity != last.entity && may_lead(m)))
        }
    } else {
        best_friend()
    }
}

/// `ZombieFlocking`'s `Power`: the car stands (a zero stick every step)
/// while it gathers its wave, with a one in five chance each step of a
/// small teleport streak (`SmallTeleportParticle(Position, Radius * 2)`:
/// from a random spot of the square two radii around it, flying in).
/// Never ends by itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct Power;

/// `RandomHelper.RandomEvent(0.2f)`.
pub const POWER_PARTICLE_CHANCE: f32 = 0.2;

impl BtAction<AiBlackboard, Orders> for Power {
    type State = ();

    fn start(&self, _: &mut AiBlackboard, _: ()) -> Option<()> {
        Some(())
    }

    fn is_in_progress(&self, _: &(), _: &AiBlackboard, _: ()) -> bool {
        true
    }

    fn tick(&self, _: &mut (), bb: &mut AiBlackboard, _: ()) -> Orders {
        if bb.random() < POWER_PARTICLE_CHANCE {
            // `NextVector2Fake()`: in the square of side 2; the streak flies
            // back along it, into the car.
            let delta = Vec2::new(bb.random() * 2.0 - 1.0, bb.random() * 2.0 - 1.0);
            let at = bb.pos + delta * bb.radius * 2.0;
            bb.effect(Effect::Teleport { at, direction: (-delta).to_angle() });
        }
        Orders { move_dir: Some(Vec2::ZERO), ..Orders::default() }
    }
}

/// `ZombieFlocking`'s `Fire!`: pulls the trigger in the thinking pass
/// (`Control.Fire = FireType.Shoot`, from `Start`) and is over at once.
pub fn fire_now(bb: &mut AiBlackboard) -> NodeResult<Orders> {
    bb.pull_trigger();
    NodeResult::Success
}

/// `ZombieFlocking`'s `Uii`: every AI friend within twice `ScanDistance`,
/// alive, with nobody to fight (no target, no damager), is told who hurt
/// this car (its `LastDamager`, even none, and when: the friend forgets it
/// when this car would), and a red ping flashes on it (`CreateBip2`, when
/// in view). Always fails (`return false`), so the tree goes on.
pub fn alert_friends(bb: &mut AiBlackboard) -> NodeResult<Orders> {
    let (damager, left) = (bb.damager_entity(), bb.damager_left());
    let friends: Vec<Entity> = bb
        .friends(bb.scan_distance * 2.0)
        .filter(|m| m.mind.is_some_and(|mind| mind.idle))
        .map(|m| m.entity)
        .collect();
    for friend in friends {
        bb.alert(Alert { friend, damager, left });
    }
    NodeResult::Failure
}

