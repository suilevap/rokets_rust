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
//! | [`DecoratorCondition`], [`timer`] | `BTLib.DecoratorCondtion` (`While`), `BTActionFactory.Timer` |
//! | [`quiet_start`] | the way `Context.Think` starts an action without ticking it |
//! | [`all`] | `BTLib.ParallelAllActionNode` (`All`) |
//! | [`FindNearestEnemy`] | `BTActions/FindNearestEnemy` |
//! | [`FireToTarget`] | `BTActions/FireToTarget` |
//! | [`SimpleFollowByTarget`] | `BTActions/SimpleFollowByTarget` |
//! | [`Idle`] | `BTBuilder.Idle` |
//! | [`Scanning`] | the `Scanning` action of `BTActionFactory.SimpleScanning` |
//!
//! The rest of `BTActions/` (`FollowBy`, `MoveBase`, `MoveToPoint`,
//! `Flocking`, `ExecuteOrders`, `AimToTarget`, …) comes with the trees that
//! use it.

mod actions;
mod condition;
mod parallel;
mod quiet;

pub use actions::{FindNearestEnemy, FireToTarget, Idle, Scanning, SimpleFollowByTarget};
pub use condition::{DecoratorCondition, WhileState, decorator_condition, timer};
pub use parallel::{All, all};
pub use quiet::{QuietStart, quiet_start};
