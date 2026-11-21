//! The shared nodes of the behaviour trees: `BTLib`'s generic ones and the
//! game's (`BTActions/*`), as `flatbt` nodes over the [`AiBlackboard`](super::blackboard::AiBlackboard).
//!
//! Each names its C# origin; the trees are put together from them in
//! [`super::trees`] (`BTActionFactory`). A node never changes the world: an
//! action's `tick` returns the [`Orders`](super::bt::Orders) it writes, and
//! the control applies them. What a node may change is the blackboard, as in C#: the
//! target, the timer.
//!
//! | Node | C# |
//! |---|---|
//! | [`timer`], [`time_limit`] ([`ThinkTimer`]), [`while_target_alive`] | `BTActionFactory.Timer` and `LeaderAlive` (`BTLib.DecoratorCondtion`, `While`), `BTActions/WhileTargetAlive` |
//! | [`act`] | an action named after its type (C#'s thinking pass started it without ticking it; not kept) |
//! | [`all`] | `BTLib.ParallelAllActionNode` (`All`) |
//! | [`optional`] | `BTLib.OptionalActionNode` (`bt.Optional`) |
//! | [`FindNearestEnemy`] | `BTActions/FindNearestEnemy` |
//! | [`FireToTarget`] | `BTActions/FireToTarget` |
//! | [`SimpleFollowByTarget`] | `BTActions/SimpleFollowByTarget` |
//! | [`MoveTowardsTargetNoPath`] | `BTActions/MoveTowardsTargetNoPath` |
//! | [`MoveBase`], [`MoveToPoint`], [`FollowBy`], [`MoveToRandomVisiblePosition`] | `BTActions/MoveBase`, `MoveToPoint`, `FollowBy`, `MoveToRandomVisiblePosition`: drive along a [`NavPath`](crate::map::nav::NavPath) |
//! | [`AimToTarget`] | `BTActions/AimToTarget` (in `FireToTarget.cs`) |
//! | [`Stop`] | `BTActionFactory.StopAction` |
//! | [`RememberTargetPos`] | `BTActionFactory.RemeberTargetPos` |
//! | [`MoveInInitDirection`] | `BTActionFactory.MoveInOneDirectionAi`'s action |
//! | [`TryFireToAnyone`] | `BTActions/TryFireToAnyone` |
//! | [`TriggerMapAction`] | `BTActions/TriggerMapAction` |
//! | [`Sleep`] | `BTActionFactory.Sleep2Sec` |
//! | [`Runaway`] | the `Runaway` action of `BTActionFactory.FollowPathAiSafe` |
//! | [`Idle`] | `BTBuilder.Idle` |
//! | [`Scanning`] | the `Scanning` action of `BTActionFactory.SimpleScanning` |
//! | [`Flocking`] | `BTActions/Flocking` |
//! | [`FindNearestLeader`] | `BTActions/FindNearestLeader` |
//! | [`Power`], [`FireNow`], [`AlertFriends`] | the `Power`, `Fire!` and `Uii` actions of `BTActionFactory.ZombieFlocking` |
//! | [`ExecuteOrders`] | `BTActions/ExecuteOrders` (a `BTLib.DynamicNode` over `OrdersManager`) |
//!
//! The rest of `BTActions/` (`FindAngleNearestEnemy`,
//! `GetTargetFromVariables`, the chain gadget ones, …)
//! comes with the trees that use it (`docs/BACKLOG.md` lists which).

mod actions;
mod condition;
mod execute_orders;
mod flocking;
mod optional;
mod parallel;
mod path;
mod act;

pub use actions::{
    AimToTarget, FireToTarget, Idle, MoveInInitDirection, MoveTowardsTargetNoPath, Runaway, Scanning, SimpleFollowByTarget,
    TriggerMapAction, TryFireToAnyone, find_nearest_enemy, remember_target_pos, sleep, stop_moving,
};
pub use flocking::{FlockState, Flocking, Power, TargetFn, alert_friends, find_nearest_leader, fire_now};
pub use execute_orders::ExecuteOrders;
pub use condition::{Condition, ThinkTimer, TimerState, WhileTargetAlive, has_live_target, never, time_limit, timer, while_target_alive};
pub use optional::{Optional, optional, optional_required};
pub use parallel::{All, all};
pub use path::{
    FollowBy, MoveBase, MoveParams, MoveToPoint, MoveToRandomVisiblePosition, PositionFn, go_to_position, to_go_world,
};
pub use act::act;
