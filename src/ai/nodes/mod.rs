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
//! | [`DecoratorCondition`], [`timer`], [`while_target_alive`] | `BTLib.DecoratorCondtion` (`While`), `BTActionFactory.Timer`, `BTActions/WhileTargetAlive` |
//! | [`quiet_start`] | the way `Context.Think` starts an action without ticking it |
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
mod quiet;

pub use actions::{
    AimToTarget, FindNearestEnemy, FireToTarget, Idle, MoveInInitDirection, MoveTowardsTargetNoPath, RememberTargetPos,
    Runaway, Scanning, SimpleFollowByTarget, Sleep, Stop, TriggerMapAction, TryFireToAnyone,
};
pub use flocking::{AlertFriends, FindNearestLeader, FireNow, FlockState, Flocking, Power, TargetFn};
pub use execute_orders::ExecuteOrders;
pub use condition::{DecoratorCondition, WhileState, decorator_condition, timer, while_target_alive};
pub use optional::{Optional, optional, optional_required};
pub use parallel::{All, all};
pub use path::{
    FollowBy, MoveBase, MoveParams, MoveToPoint, MoveToRandomVisiblePosition, PositionFn, go_to_position, to_go_world,
};
pub use quiet::{QuietStart, quiet_start};
