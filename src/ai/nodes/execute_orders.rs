//! `ExecuteOrders`: carry out the first of the car's orders.
//!
//! C# origin: `BTActions/ExecuteOrders` (2013), a `BTLib.DynamicNode`: each
//! update it ran the tree the first order held (`Orders.PeekOrder()`), or
//! failed with none, and when that tree ended (`Ok` or `Fail`) told the
//! queue (`OnComplete` → `Orders.EndOrder(status)`), which drops the order
//! as its `OrderRemoveCondition` says ([`OrderQueue::end`]).
//!
//! Here the orders are data ([`OrderKind`]) and the node is one action that
//! carries out the first as the tree its C# order action built: a
//! `MoveOrder` is `MoveToPoint` to its point ([`MoveToPoint`], speed rate 1,
//! near enough within 1). The branches of `DynamicNode.OnUpdate` and
//! `ActionNode.OnUpdate` it keeps:
//!
//! - no order: fails at its start (`EndOrder` on an empty queue does
//!   nothing);
//! - the order's action does not start (no path there): fails, and the
//!   order has ended `Fail` (a `RemoveOnTry` order is dropped, a
//!   `RemoveOnSuccess` one is tried again at the next think);
//! - the order's action ends: its `Complete` says `Ok` or `Fail`, and the
//!   order has ended so;
//! - the order is no longer first (`AIClearOrders`): between thinks the
//!   action is still asked and ticked by itself (`Context.Run()` does not go
//!   through the `DynamicNode`); the next thinking pass finds another order
//!   or none and the node fails, the action left without its `Complete`
//!   (the stick and the path stay as they were) and no order ended.
//!
//! Not ported: when a new order is first by that thinking pass (cleared and
//! given anew within a quarter second), C# ran the new order's tree as if
//! it were already running: `MoveToPoint` went on along the old path, never
//! asked the new point, and on arriving ended the new order `Ok`. Here the
//! node fails, and the new order starts afresh at the next step. C#'s
//! `OnComplete(Fail)` then also ended the new order unrun (dropped if
//! `RemoveOnTry`); here it waits its turn.

use bevy::math::Vec2;
use flatbt::BtAction;
use flatbt_bevy::Tick;

use super::path::{MoveParams, MoveToPoint};
use crate::ai::blackboard::AiBlackboard;
use crate::ai::order_queue::{Order, OrderKind, OrderQueue};
use crate::ai::orders::Orders;

/// Where the first order goes, or no point (the origin) when it is not a
/// `MoveOrder`.
fn move_order_target(bb: &mut AiBlackboard) -> Vec2 {
    match bb.order_queue.peek() {
        Some(Order { kind: OrderKind::Move { to }, .. }) => *to,
        None => Vec2::ZERO,
    }
}

/// `MoveOrderRun`'s `new MoveToPoint("MoveOrder", b => p)`: the defaults,
/// speed rate 1 and near enough within 1.
const MOVE_ORDER: MoveToPoint = MoveToPoint {
    position: move_order_target,
    params: MoveParams { speed_rate: 1.0, min_dist: 1.0 },
};

/// Whether `order` is still the first in the queue.
fn is_first(queue: &OrderQueue, order: &Order) -> bool {
    queue.peek().is_some_and(|first| first.serial() == order.serial())
}

/// `ExecuteOrders`: the first order's action, the queue told when it ends.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecuteOrders;

impl BtAction<AiBlackboard, Orders> for ExecuteOrders {
    /// The order being carried out.
    type State = Order;

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<Order> {
        let order = *bb.order_queue.peek()?;
        let started = match order.kind {
            OrderKind::Move { .. } => MOVE_ORDER.start(bb, ()).is_some(),
        };
        if !started {
            bb.order_queue.end(false);
            return None;
        }
        Some(order)
    }

    fn is_in_progress(&self, order: &Order, bb: &AiBlackboard, _: ()) -> bool {
        // A thinking pass asks the queue first (`GetCurrentNode`); a step
        // that only ticks asks the action alone (`Context.Run()`).
        if bb.tick == Tick::Evaluate && !is_first(&bb.order_queue, order) {
            return false;
        }
        match order.kind {
            OrderKind::Move { .. } => MOVE_ORDER.is_in_progress(&(), bb, ()),
        }
    }

    fn tick(&self, order: &mut Order, bb: &mut AiBlackboard, _: ()) -> Orders {
        match order.kind {
            OrderKind::Move { .. } => MOVE_ORDER.tick(&mut (), bb, ()),
        }
    }

    fn complete(&self, order: &mut Order, bb: &mut AiBlackboard, _: ()) -> bool {
        if !is_first(&bb.order_queue, order) {
            // Cleared: the action is left, not completed.
            return false;
        }
        let ok = match order.kind {
            OrderKind::Move { .. } => MOVE_ORDER.complete(&mut (), bb, ()),
        };
        bb.order_queue.end(ok);
        ok
    }
}
