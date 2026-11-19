//! The game's action nodes, `BTActions/*`.

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt::BtAction;

use crate::ai::blackboard::{AiBlackboard, Foe};
use crate::ai::orders::Orders;

/// `BTActions/FindNearestEnemy`: picks the target. Whoever last hurt the car
/// (of another team, whatever the distance or walls) is the one; else the
/// current target while it stays within `ScanDistance` and is in sight (or
/// will be in 0.5 s); else the nearest by `|dx| + |dy|` of those within
/// `ScanDistance` on both axes, other team, alive, active. A current target
/// that is within range but out of sight is not kept: any *other* enemy is
/// chosen, and when there is none this finds nothing (the old target is kept
/// as it was, to be looked at again next time). Succeeds when it found one.
///
/// An action that does its work in `Start` and is over at once
/// (`IsInProgress` false, `Complete` true). C# throws from its `Tick`
/// (never reached); here it writes nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct FindNearestEnemy;

impl BtAction<AiBlackboard, Orders> for FindNearestEnemy {
    type State = ();

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<()> {
        let team = bb.team;
        let hit_back = bb.damager().filter(|f| f.team != team).map(|f| f.entity);
        let found = hit_back.or_else(|| nearest_enemy(bb));
        // `Target`'s setter looks at it.
        bb.set_target(found?);
        Some(())
    }

    fn is_in_progress(&self, _: &(), _: &AiBlackboard, _: ()) -> bool {
        false
    }

    fn tick(&self, _: &mut (), _: &mut AiBlackboard, _: ()) -> Orders {
        Orders::default()
    }
}

/// `FindNearestEnemy.GetNearestEnemy`.
fn nearest_enemy(bb: &AiBlackboard) -> Option<Entity> {
    let scan = bb.scan_distance;
    // `EntitiesHelper.GetBest`: the least `|dx| + |dy|`, the first of equals.
    let best = |skip: Option<Entity>| {
        let mut best: Option<(f32, Entity)> = None;
        for f in bb.enemies().filter(|f| Some(f.entity) != skip) {
            let d = (f.pos - bb.pos).abs();
            let manhattan = d.x + d.y;
            if manhattan < best.map_or(f32::MAX, |b| b.0) {
                best = Some((manhattan, f.entity));
            }
        }
        best.map(|b| b.1)
    };
    match bb.live_target() {
        Some(last) if last.team != bb.team && last.pos.distance_squared(bb.pos) < scan * scan => {
            if bb.can_see_target() { Some(last.entity) } else { best(Some(last.entity)) }
        }
        _ => best(None),
    }
}

/// Where the target will be `predict` seconds from now (`GetSimplePredictionPosition`),
/// as a direction from the car: `ToGoWorld` and `SafeNormalize`.
fn direction_to(foe: &Foe, from: Vec2, predict: f32) -> Vec2 {
    (foe.pos + foe.vel * predict - from).normalize_or_zero()
}

/// `BTActions/FireToTarget(predictSec)`: aims the gun at where the target
/// will be, and pulls the trigger when the gun *as it points now* is within
/// the weapon's `TargetInFireRange` (the radius scaled by `FireAccuracy`:
/// more than 1 fires at things further off the line).
///
/// Starts when the target lives and is in sight now or in 0.5 s; goes on while
/// that holds and the car has a gun; completes `Ok` when the target is gone
/// (or dead), `Fail` when it is lost from sight.
#[derive(Clone, Copy, Debug)]
pub struct FireToTarget {
    /// `_predictSec`: how far ahead of the target it aims, s.
    pub predict: f32,
}

impl FireToTarget {
    pub fn new(predict: f32) -> FireToTarget {
        FireToTarget { predict }
    }
}

impl BtAction<AiBlackboard, Orders> for FireToTarget {
    type State = ();

    fn start(&self, ctx: &mut AiBlackboard, _: ()) -> Option<()> {
        (ctx.live_target().is_some() && ctx.can_see_target()).then_some(())
    }

    fn is_in_progress(&self, _: &(), ctx: &AiBlackboard, _: ()) -> bool {
        ctx.gun.is_some() && ctx.live_target().is_some() && ctx.can_see_target()
    }

    fn tick(&self, _: &mut (), ctx: &mut AiBlackboard, _: ()) -> Orders {
        let (Some(foe), Some(gun)) = (ctx.live_target(), ctx.gun) else {
            return Orders::default();
        };
        Orders {
            aim_dir: Some(direction_to(foe, ctx.pos, self.predict)),
            fire: gun.range.contains(foe.pos, foe.radius * ctx.fire_accuracy),
            ..Orders::default()
        }
    }

    fn complete(&self, _: &mut (), ctx: &mut AiBlackboard, _: ()) -> bool {
        ctx.live_target().is_none()
    }
}

/// `BTActions/SimpleFollowByTarget(predictSec)`: steers the car at where the
/// target will be and turns the gun the same way (a car that can move
/// chases; a static one only turns its gun, as its stick drives nothing).
/// Starts with a target and goes on while it lives.
///
/// C#'s aim here is `GetPredictionPosition` (the same, moved off a spot
/// inside a wall, which is not ported) through `ToGoRelative`, and
/// `FireToTarget`'s is `GetSimplePredictionPosition` through `ToGoWorld`:
/// both come to the same unit direction, written here once for each.
#[derive(Clone, Copy, Debug)]
pub struct SimpleFollowByTarget {
    pub predict: f32,
}

impl SimpleFollowByTarget {
    pub fn new(predict: f32) -> SimpleFollowByTarget {
        SimpleFollowByTarget { predict }
    }
}

impl BtAction<AiBlackboard, Orders> for SimpleFollowByTarget {
    type State = ();

    fn start(&self, ctx: &mut AiBlackboard, _: ()) -> Option<()> {
        ctx.target().map(|_| ())
    }

    fn is_in_progress(&self, _: &(), ctx: &AiBlackboard, _: ()) -> bool {
        ctx.live_target().is_some()
    }

    fn tick(&self, _: &mut (), ctx: &mut AiBlackboard, _: ()) -> Orders {
        let Some(foe) = ctx.live_target() else {
            return Orders::default();
        };
        let direction = direction_to(foe, ctx.pos, self.predict);
        Orders { move_dir: Some(direction), aim_dir: Some(direction), fire: false }
    }
}

/// `BTBuilder.Idle`: an infinite empty action.
#[derive(Clone, Copy, Debug, Default)]
pub struct Idle;

impl BtAction<AiBlackboard, Orders> for Idle {
    type State = ();

    fn start(&self, _: &mut AiBlackboard, _: ()) -> Option<()> {
        Some(())
    }

    fn is_in_progress(&self, _: &(), _: &AiBlackboard, _: ()) -> bool {
        true
    }

    fn tick(&self, _: &mut (), _: &mut AiBlackboard, _: ()) -> Orders {
        Orders::default()
    }
}

/// `SimpleScanning`'s `Scanning` action: picks a random direction within
/// 110 % of the gun's `MaxAngle` of the car's heading (a car with a gun that
/// does not aim, or none: any direction) and keeps the gun turned there, for
/// as long as it is run (it never ends by itself).
#[derive(Clone, Copy, Debug, Default)]
pub struct Scanning;

/// `Scanning`'s sweep is ±1.1 × the gun's `MaxAngle`.
pub const SCAN_SWEEP: f32 = 1.1;

impl BtAction<AiBlackboard, Orders> for Scanning {
    /// `TargetPosition`: where to look, world.
    type State = Vec2;

    fn start(&self, ctx: &mut AiBlackboard, _: ()) -> Option<Vec2> {
        let direction = match ctx.gun {
            Some(gun) if gun.complex => {
                let turn = (ctx.random() * 2.0 - 1.0) * SCAN_SWEEP * gun.max_angle;
                Vec2::from_angle(ctx.heading + turn)
            }
            _ => Vec2::new(ctx.random() * 2.0 - 1.0, ctx.random() * 2.0 - 1.0),
        };
        Some(ctx.pos + direction * ctx.scan_distance)
    }

    fn is_in_progress(&self, _: &Vec2, _: &AiBlackboard, _: ()) -> bool {
        true
    }

    /// `ToGoWorld(TargetPosition, ScanDistance)`: the way there, shorter
    /// than a unit when it is near.
    fn tick(&self, at: &mut Vec2, ctx: &mut AiBlackboard, _: ()) -> Orders {
        let scan = ctx.scan_distance;
        let to = *at - ctx.pos;
        let aim = if to.length_squared() < scan * scan { to / scan } else { to.normalize_or_zero() };
        Orders { aim_dir: Some(aim), ..Orders::default() }
    }
}
