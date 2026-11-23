//! `ExecuteOrders`: carry out the first of the car's orders.
//!
//! C# origin: `BTActions/ExecuteOrders` (2013), a `BTLib.DynamicNode`: each
//! update it ran the tree the first order held (`Orders.PeekOrder()`), or
//! failed with none, and when that tree ended (`Ok` or `Fail`) told the
//! queue (`OnComplete` → `Orders.EndOrder(status)`), which drops the order
//! as its `OrderRemoveCondition` says ([`OrderQueue::end`]).
//!
//! Here the orders are data ([`OrderKind`]) and the node holds the trees the
//! order actions built, one for each kind ([`trees::move_order`],
//! [`trees::carrier`], [`trees::grab_cargo`], [`trees::press_button`]),
//! picking the one of the first order when it starts. The trees read what
//! the order says off the blackboard's [`executing`](AiBlackboard::executing)
//! (C# built them around the order's values). The branches of
//! `DynamicNode.OnUpdate` and `ActionNode.OnUpdate` it keeps:
//!
//! - no order: fails at its start (`EndOrder` on an empty queue does
//!   nothing);
//! - the order's tree does not start (no path there, a cargo that is gone):
//!   fails, and the order has ended `Fail` (a `RemoveOnTry` order is
//!   dropped, a `RemoveOnSuccess` one is tried again at the next think);
//! - the order's tree ends: its result says `Ok` or `Fail`, and the order
//!   has ended so;
//! - the order is no longer first (`AIClearOrders`): between thinks the
//!   tree is still ticked by itself (`Context.Run()` does not go through
//!   the `DynamicNode`); the next thinking pass finds another order or none
//!   and the node fails, the tree left without finishing (the stick, the
//!   path and the gadget's button stay as they were) and no order ended. A
//!   tree that ends before that thinking pass ends no order either.
//!
//! Not ported: when a new order is first by that thinking pass (cleared and
//! given anew within a quarter second), C# ran the new order's tree as if
//! it were already running: `MoveToPoint` went on along the old path, never
//! asked the new point, and on arriving ended the new order `Ok`. Here the
//! node fails, and the new order starts afresh at the next step. C#'s
//! `OnComplete(Fail)` then also ended the new order unrun (dropped if
//! `RemoveOnTry`); here it waits its turn.
//!
//! [`trees::move_order`]: crate::ai::trees::move_order
//! [`trees::carrier`]: crate::ai::trees::carrier
//! [`trees::grab_cargo`]: crate::ai::trees::grab_cargo
//! [`trees::press_button`]: crate::ai::trees::press_button

use flatbt::inspect::{Inspector, NodeInfo};
use flatbt::{BtNode, Entry, EntryMode, NodeResult};

use crate::ai::blackboard::AiBlackboard;
use crate::ai::order_queue::{Order, OrderKind, OrderQueue};
use crate::ai::orders::Orders;

/// Whether `order` is still the first in the queue.
fn is_first(queue: &OrderQueue, order: &Order) -> bool {
    queue.peek().is_some_and(|first| first.serial() == order.serial())
}

/// Which of the four trees carries out an order.
fn tree_of(order: &Order) -> usize {
    match order.kind {
        OrderKind::Move { .. } => 0,
        OrderKind::MoveCargo { .. } => 1,
        OrderKind::GrabCargo { .. } => 2,
        OrderKind::PressButton { .. } => 3,
    }
}

/// `ExecuteOrders`: the first order's tree, the queue told when it ends.
pub struct ExecuteOrders<Move, Carry, Grab, Press> {
    trees: (Move, Carry, Grab, Press),
}

/// [`ExecuteOrders`] over the trees for `MoveOrder`, `MoveCargoOrder`,
/// `GrabCargoOrder` and `PressButtonOrder`.
pub fn execute_orders<Move, Carry, Grab, Press>(
    move_order: Move,
    carry_cargo: Carry,
    grab_cargo: Grab,
    press_button: Press,
) -> ExecuteOrders<Move, Carry, Grab, Press> {
    ExecuteOrders { trees: (move_order, carry_cargo, grab_cargo, press_button) }
}

/// One invocation: the order being carried out and the state of its tree
/// (only the one for its kind is in use).
#[derive(Default)]
pub struct OrderState<Move, Carry, Grab, Press> {
    order: Option<Order>,
    trees: (Move, Carry, Grab, Press),
}

impl<Move, Carry, Grab, Press> BtNode<AiBlackboard, Orders> for ExecuteOrders<Move, Carry, Grab, Press>
where
    Move: BtNode<AiBlackboard, Orders>,
    Carry: BtNode<AiBlackboard, Orders>,
    Grab: BtNode<AiBlackboard, Orders>,
    Press: BtNode<AiBlackboard, Orders>,
{
    type State = OrderState<Move::State, Carry::State, Grab::State, Press::State>;
    type Memory = (Move::Memory, Carry::Memory, Grab::Memory, Press::Memory);
    const NODES: usize = 1 + Move::NODES + Carry::NODES + Grab::NODES + Press::NODES;

    fn update(
        &self,
        state: &mut Self::State,
        memory: &mut Self::Memory,
        bb: &mut AiBlackboard,
        _: (),
        entry: Entry<'_>,
    ) -> NodeResult<Orders> {
        let (order, fresh) = match state.order {
            Some(order) => {
                // A thinking pass asks the queue first (`GetCurrentNode`); a
                // step that only ticks asks the tree alone (`Context.Run()`).
                if entry.mode() == EntryMode::Evaluate && !is_first(&bb.order_queue, &order) {
                    return NodeResult::Failure;
                }
                (order, false)
            }
            None => {
                let Some(order) = bb.order_queue.peek().copied() else {
                    return NodeResult::Failure;
                };
                state.order = Some(order);
                bb.executing = Some(order);
                (order, true)
            }
        };
        // The offsets of the four trees after this node, in preorder.
        let offsets = [1, 1 + Move::NODES, 1 + Move::NODES + Carry::NODES, 1 + Move::NODES + Carry::NODES + Grab::NODES];
        macro_rules! run {
            ($n:tt) => {
                if fresh {
                    entry.run_candidate(offsets[$n], &self.trees.$n, &mut state.trees.$n, &mut memory.$n, bb, ())
                } else {
                    entry.run(offsets[$n], &self.trees.$n, &mut state.trees.$n, &mut memory.$n, bb, ())
                }
            };
        }
        let result = match tree_of(&order) {
            0 => run!(0),
            1 => run!(1),
            2 => run!(2),
            _ => run!(3),
        };
        if !result.is_running() && is_first(&bb.order_queue, &order) {
            bb.order_queue.end(matches!(result, NodeResult::Success));
        }
        result
    }

    fn inspect(&self, state: Option<&Self::State>, memory: &Self::Memory, inspector: &mut dyn Inspector) {
        let node = NodeInfo::new("execute_orders", state.is_some());
        inspector.node(node, |inspector| {
            let running = state.and_then(|s| s.order.as_ref()).map(tree_of);
            let of = |n: usize| state.filter(|_| running == Some(n));
            BtNode::<AiBlackboard, Orders>::inspect(&self.trees.0, of(0).map(|s| &s.trees.0), &memory.0, inspector);
            BtNode::<AiBlackboard, Orders>::inspect(&self.trees.1, of(1).map(|s| &s.trees.1), &memory.1, inspector);
            BtNode::<AiBlackboard, Orders>::inspect(&self.trees.2, of(2).map(|s| &s.trees.2), &memory.2, inspector);
            BtNode::<AiBlackboard, Orders>::inspect(&self.trees.3, of(3).map(|s| &s.trees.3), &memory.3, inspector);
        });
    }
}
