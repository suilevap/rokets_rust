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
//! Not ported here: every other tree of `BTActionFactory` (`SimpleAttackDelayPos`,
//! `Attack`, `Chase*`, `SimpleAi`, `GuardAi`, … the ones with the nodes not
//! yet ported), and `TurretAiFireToAnyone` (`AiSimpleTurretFriendFireAvailableTarget`:
//! `SimpleAttackByAngle`, `FindAngleNearestEnemy`, `TryFireToAnyone`), which no
//! map uses.

use flatbt_bevy::prelude::*;

use super::blackboard::AiBlackboard;
use super::nodes::{FindNearestEnemy, FireToTarget, Idle, Scanning, SimpleFollowByTarget, all, quiet_start, timer};
use super::orders::Orders;

/// `SimpleAttackTarget(0.2f)`: how far ahead of the target the turret aims, s.
pub const AIM_AHEAD: f32 = 0.2;
/// `SimpleScanning`: `Timer(1.0f, …)` twice, s.
pub const IDLE_TIME: f32 = 1.0;
pub const SCAN_TIME: f32 = 1.0;

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
