//! The behaviour trees: `BTActionFactory`, built from [`super::nodes`].
//!
//! C# origin: `BTActionFactory` (2013–2016) put its trees together with
//! `BTBuilder` (`bt.Selector`, `bt.Sequence`, `bt.All`, `bt.While`, …) from
//! the nodes in `BTActions/`. Each function here is one of its trees or
//! subtrees, written beside its C#:
//!
//! ```text
//! C#                                              Rust
//! Timer(t, child) =                               timer(t, child)
//!   bt.While("Timer_ts", b => b.StartTimer(),       (decorator_condition: init, check, child)
//!     b => b.Timer.TotalSeconds < t ? Running : Ok, child)
//!
//! SimpleAttackTarget(p) =                         simple_attack_target(p) =
//!   bt.Selector("A",                                select((
//!     bt.All("Fire",                                  action(all((FireToTarget(p),
//!       FireToTarget(p),                                          SimpleFollowByTarget(p)))),
//!       SimpleFollowByTarget(p)),                     action(FireToTarget(p)),
//!     FireToTarget(p))                              ))
//!
//! SimpleAttack =                                  simple_attack() =
//!   bt.Sequence("Attack",                           seq((
//!     FindNearestEnemy,                               action(FindNearestEnemy),
//!     SimpleAttackTarget(0.2f))                       simple_attack_target(0.2),
//!                                                   ))
//! SimpleScanning =                                simple_scanning() =
//!   bt.Sequence("Idle",                             seq((
//!     Timer(1.0f, bt.Idle),                           timer(1.0, action(Idle)),
//!     Timer(1.0f, bt.Action("Scanning", ..)))         timer(1.0, action(Scanning)),
//!                                                   ))
//! TurretAi =                                      turret_ai() =
//!   bt.Selector("TurretBrain",                      select((
//!     SimpleAttack, SimpleScanning)                   simple_attack(), simple_scanning(),
//!                                                   ))
//! ```
//!
//! Every action is wrapped in `quiet_start` (not shown above): C#'s thinking
//! pass starts an action without ticking it (see the `ai` module docs).
//!
//! `SimpleAttackTarget`'s second branch (`FireToTarget` alone) is kept so the
//! tree is C#'s, though it can never run: the first branch starts exactly
//! when it would (`All` starts when `FireToTarget` and `SimpleFollowByTarget`
//! do, and the second needs a target, which `FireToTarget` needs too) and is
//! left under the same conditions, so whenever the first is not running the
//! second fails at its start. (With no weapon the first starts, is at once
//! not in progress and completes `Fail` (the target lives); the second does
//! the same.) It costs a failed start; `tests/ai_turret.rs` checks that it never runs.
//!
//! The trees that drive along paths (`MoveBase`, `MoveToPoint`, `FollowBy`):
//!
//! ```text
//! C#                                              Rust
//! Chase =                                         chase() =
//!   bt.Selector("ChaseAI",                          select((
//!     bt.Sequence("Chase",                            seq((
//!       FindNearestEnemy,                               action(FindNearestEnemy),
//!       WhileTargetAlive(                               while_target_alive(
//!         bt.All("Chase and fire",                        action(all((
//!           bt.Optional(FireToTarget()),                    optional(FireToTarget(0)),
//!           FollowBy("Target",                              FollowBy(target's safe position)
//!             b => b.Target.SafePosition)))),             )))))),
//!     MoveToLastKnownTargetPosition =                 seq((
//!       bt.Sequence(                                    check(has a live target),
//!         bt.Condition("HastTarget", …),                action(MoveToPoint(last known position)),
//!         MoveToPoint("MoveToLastKnownPos",           )),
//!           b => b.LastKnowTargetPosition)),          seq((action(Sleep(2)), action(Idle))),
//!     bt.Sequence("Sleep", Sleep2Sec, bt.Idle))     ))
//! ChaserAi = Chase                                chaser_ai() = chase()
//!
//! FollowPathAi =                                  follow_path_ai() =
//!   bt.Selector("Brain",                            select((
//!     bt.All("Fire",                                  action(all((TryFireToAnyone(3),
//!       TryFireToAnyone(3.0f),                                    MoveBase))),
//!       MoveBase("MoveByPath")),                      action(TriggerMapAction(once)),
//!     TriggerMapAction(true),                         action(TryFireToAnyone(3)),
//!     TryFireToAnyone(3.0f))                        ))
//! FollowPathAiSafe =                              follow_path_ai_safe() =
//!   bt.Selector("Brain",                            select((
//!     bt.Sequence("Danger",                           seq((action(FindNearestEnemy),
//!       FindNearestEnemy, Runaway),                           action(Runaway))),
//!     bt.All("Fire", TryFireToAnyone(3.0f),           action(all((TryFireToAnyone(3),
//!       MoveBase("MoveByPath")))                                  MoveBase))),
//!                                                   ))
//! GoToEnemySimplePredictionPosition =             go_to_enemy_simple_prediction_position() =
//!   bt.Sequence("FindAndFollow",                    seq((action(FindNearestEnemy),
//!     FindNearestEnemy,                                   action(MoveTowardsTargetNoPath(0.6))))
//!     MoveTowardsTargetNoPath(0.6f))
//! ```
//!
//! `FollowPathAi`'s third branch (`TryFireToAnyone` alone) is kept as C#'s
//! though it never runs: `TriggerMapAction` (optional) always succeeds, and
//! the selector stops at the first success. A car with no path therefore does
//! nothing but run its trigger action (once), and does not even fire.
//!
//! Not ported here: every other tree of `BTActionFactory` (`SimpleAttackDelayPos`,
//! `Attack`, `SimpleAi`, `GuardAi`, … the ones with the nodes not
//! yet ported: see `docs/BACKLOG.md`), and `TurretAiFireToAnyone`
//! (`AiSimpleTurretFriendFireAvailableTarget`:
//! `SimpleAttackByAngle`, `FindAngleNearestEnemy`, `TryFireToAnyone`), which no
//! map uses.

use flatbt_bevy::prelude::*;

use super::blackboard::AiBlackboard;
use super::nodes::{
    FindNearestEnemy, FireToTarget, FollowBy, Idle, MoveBase, MoveTowardsTargetNoPath, MoveToPoint, Runaway, Scanning,
    SimpleFollowByTarget, Sleep, TriggerMapAction, TryFireToAnyone, all, optional, quiet_start, timer, while_target_alive,
};
use super::orders::Orders;

/// `SimpleAttackTarget(0.2f)`: how far ahead of the target the turret aims, s.
pub const AIM_AHEAD: f32 = 0.2;
/// `SimpleScanning`: `Timer(1.0f, …)` twice, s.
pub const IDLE_TIME: f32 = 1.0;
pub const SCAN_TIME: f32 = 1.0;
/// `Sleep2Sec`: how long a chaser with nobody to chase waits, s.
pub const SLEEP_TIME: f32 = 2.0;
/// `TryFireToAnyone(3.0f)`: at most a shot every this many seconds, s.
pub const FIRE_TO_ANYONE_PERIOD: f32 = 3.0;
/// `GoToEnemySimplePredictionPosition`: how far ahead it steers, s.
pub const GO_TO_ENEMY_PREDICT: f32 = 0.6;
/// `AiControlFactory`'s `AiChaser`: `ScanDistance = World.RoomSize.Width +
/// World.RoomSize.Height` (800 × 4 and 480 × 4 sim units): the whole map.
pub const CHASER_SCAN_DISTANCE: f32 = 800.0 * 4.0 + 480.0 * 4.0;

/// `BTActionFactory.SimpleAttackTarget`: fires at the target while steering
/// at it, with the gun's aim `predict_sec` ahead of it.
pub fn simple_attack_target(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    select((
        action(quiet_start(all((FireToTarget::new(predict_sec), SimpleFollowByTarget::new(predict_sec))))).named("Fire"),
        action(quiet_start(FireToTarget::new(predict_sec))),
    ))
    .named("A")
}

/// `BTActionFactory.SimpleAttack`: find an enemy, then attack it.
pub fn simple_attack() -> impl BehaviorNode<AiBlackboard, Orders> {
    seq((action(FindNearestEnemy), simple_attack_target(AIM_AHEAD))).named("Attack")
}

/// `BTActionFactory.SimpleScanning`: stand still, then turn the gun to a
/// random direction.
pub fn simple_scanning() -> impl BehaviorNode<AiBlackboard, Orders> {
    seq((
        timer(IDLE_TIME, action(quiet_start(Idle)).named("Idle")).named("Timer_1s"),
        timer(SCAN_TIME, action(quiet_start(Scanning)).named("Scanning")).named("Timer_1s"),
    ))
    .named("Idle")
}

/// `BTActionFactory.TurretAi`: attack, else scan.
pub fn turret_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    select((simple_attack(), simple_scanning())).named("TurretBrain")
}

/// `BTActionFactory.Chase`'s `MoveToLastKnownTargetPosition`: with a target
/// (alive), go to where it was last seen.
fn move_to_last_known_target_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    seq((
        check(|bb: &AiBlackboard| bb.live_target().is_some()).named("HastTarget"),
        action(quiet_start(MoveToPoint::new(|bb| bb.last_known_target_position))).named("MoveToLastKnownPos"),
    ))
    .named("MoveToLastKnownTargetPosition")
}

/// `BTActionFactory.Chase` (`ChaserAi`): find an enemy and chase it, firing
/// when the gun points at it; failing that, go to where it was last seen;
/// failing that, sleep two seconds.
pub fn chase() -> impl BehaviorNode<AiBlackboard, Orders> {
    select((
        seq((
            action(FindNearestEnemy),
            while_target_alive(
                action(quiet_start(all((
                    optional(FireToTarget::new(0.0)),
                    FollowBy::new(|bb| bb.target_position_safe()),
                ))))
                .named("Chase and fire"),
            ),
        ))
        .named("Chase"),
        move_to_last_known_target_position(),
        seq((action(Sleep { secs: SLEEP_TIME }).named("Sleep2s"), action(quiet_start(Idle)).named("Idle"))).named("Sleep"),
    ))
    .named("ChaseAI")
}

/// `BTActionFactory.ChaserAi`.
pub fn chaser_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    chase()
}

/// `BTActionFactory.FollowPathAi`: follow the path the map gave it, firing at
/// anyone in the gun's line; with no path, run the map action it was given.
pub fn follow_path_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    select((
        action(quiet_start(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default())))).named("Fire"),
        action(quiet_start(TriggerMapAction::new(true))).named("Trigger"),
        action(quiet_start(TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD))),
    ))
    .named("Brain")
}

/// `BTActionFactory.FollowPathAiSafe`: follow the path, but run from an enemy
/// in sight.
pub fn follow_path_ai_safe() -> impl BehaviorNode<AiBlackboard, Orders> {
    select((
        seq((action(FindNearestEnemy), action(quiet_start(Runaway)).named("Runaway"))).named("Danger"),
        action(quiet_start(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default())))).named("Fire"),
    ))
    .named("Brain")
}

/// `BTActionFactory.GoToEnemySimplePredictionPosition`: find an enemy and go
/// straight at where it will be.
pub fn go_to_enemy_simple_prediction_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    seq((action(FindNearestEnemy), action(quiet_start(MoveTowardsTargetNoPath::new(GO_TO_ENEMY_PREDICT))))).named("FindAndFollow")
}
