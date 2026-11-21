//! The orders a map gives an AI car: [`OrderQueue`] on its blackboard, and
//! the map actions that give and take them (`MoveOrder`, `AIClearOrders`).
//!
//! C# origin: `OrdersManager` (2013, "Implement Ai Orders"), the order
//! actions of `ActionRunner` (`MoveOrderRun`, `AIClearOrdersRun`), and
//! `OrderAction.OrderRemoveCondition` (December 2013: "Order actions have
//! OrderRemoveCondition"; `AIClearOrders` came in January 2014 with the map
//! 2WeeksAgo). A map scripts a friend (the truck of Escort, the buggies of
//! Assembly and FinalCredits) by queueing what it is to do; the car's tree
//! carries out the first order ([`super::nodes::ExecuteOrders`], `ExecuteOrders`)
//! and, as the order's `OrderRemoveCondition` says, drops it when it has
//! tried it (`RemoveOnTry`), when it has done it (`RemoveOnSuccess`, the
//! default: an order that fails is tried again), or never (`Infinite`).
//!
//! - `MoveOrder(item, position)`: go to a point of the map (cells, y down,
//!   turned into the world as everything else on the map:
//!   `Position * MapManager.CellSize`). `ItemId` 0 is the car that set the
//!   action off (`GetId`), as for every action with an item.
//! - `AIClearOrders(item)`: forget every order. What the car was doing for
//!   the first is not stopped: it drives on until its tree next thinks, and
//!   then stops giving orders (the stick stays where it was, as C#'s did).
//!
//! An order reaches only a car with a behaviour tree (C#: `Control as
//! AIBTBaseControl`, whichever tree that runs); any other target, or none,
//! is left as it was. The orders live on the car's blackboard, so a new
//! control (`SetControl`) starts with none (`AIBTBaseControl.Init` makes a
//! new blackboard; the line that would carry `Orders` over is commented out).
//!
//! Not ported: `MoveOrder`'s `DEBUG` build, which went by
//! `CustomAi.SmartMoveToPos` (an experiment of 2017 that first opens a
//! door in the way by its button or a box on it); the release build's
//! `MoveToPoint("MoveOrder", …)` is what the game shipped with. The other
//! orders (`MoveCargoOrder`, `GrabCargoOrder`, `PressButtonOrder`,
//! `AttackTargetOrder`) need the carrier, button and attack behaviours
//! (`docs/BACKLOG.md`); until then they are logged as action types not
//! ported. An order given to a car whose tree the port does not have yet
//! (`SupportAi*`, which also carries out orders) is lost, as the car has no
//! blackboard.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{AIClearOrders, MoveOrder, OrderType};
use crate::game::LoadedMap;

use super::blackboard::AiBlackboard;

/// What an order asks (`OrdersManager.Order.Node`, the tree the order action
/// built).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderKind {
    /// `MoveOrder`: `MoveToPoint` to this world point.
    Move { to: Vec2 },
}

/// One order in the queue (`OrdersManager.Order`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    pub kind: OrderKind,
    /// `CompletionType`: when the order is dropped.
    pub removal: OrderType,
    /// Tells this order from another of the same kind (C# compared the
    /// tree node it held).
    serial: u64,
}

impl Order {
    /// A number of its own among the orders of its queue.
    pub fn serial(&self) -> u64 {
        self.serial
    }
}

/// `OrdersManager`: the orders a car's tree carries out, first in first out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderQueue {
    queue: VecDeque<Order>,
    next_serial: u64,
}

impl OrderQueue {
    /// `EnqueueOrder`: the order goes last.
    pub fn enqueue(&mut self, kind: OrderKind, removal: OrderType) {
        let serial = self.next_serial;
        self.next_serial += 1;
        self.queue.push_back(Order { kind, removal, serial });
    }

    /// `PeekOrder`: the order to carry out, if any.
    pub fn peek(&self) -> Option<&Order> {
        self.queue.front()
    }

    /// `EndOrder`: the first order has ended, `ok` or not; it is dropped if
    /// its `OrderRemoveCondition` says so (`RemoveOnTry` always,
    /// `RemoveOnSuccess` when `ok`, `Infinite` never). Nothing with no
    /// order.
    pub fn end(&mut self, ok: bool) {
        let Some(order) = self.queue.front() else {
            return;
        };
        let drop = match order.removal {
            OrderType::RemoveOnTry => true,
            OrderType::RemoveOnSuccess => ok,
            OrderType::Infinite => false,
        };
        if drop {
            self.queue.pop_front();
        }
    }

    /// `Clear`: every order forgotten.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// How many orders wait.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// `MoveOrder` and `AIClearOrders`.
pub struct OrdersPlugin;

impl Plugin for OrdersPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<MoveOrder, _>(move_order).add_map_action::<AIClearOrders, _>(clear_orders);
    }
}

/// `ActionRunner.MoveOrderRun`: a `MoveToPoint` to the order's position for
/// the car's tree.
fn move_order(In(run): In<Run<MoveOrder>>, ids: MapIds, map: Res<LoadedMap>, mut brains: Query<&mut AiBlackboard>) -> bool {
    if let Some(car) = ids.entity(run.target(run.data.item_id))
        && let Ok(mut blackboard) = brains.get_mut(car)
    {
        let to = map.world(run.data.position.x, run.data.position.y);
        blackboard.order_queue.enqueue(OrderKind::Move { to }, run.data.order_remove_condition);
    }
    true
}

/// `ActionRunner.AIClearOrdersRun`.
fn clear_orders(In(run): In<Run<AIClearOrders>>, ids: MapIds, mut brains: Query<&mut AiBlackboard>) -> bool {
    if let Some(car) = ids.entity(run.target(run.data.item_id))
        && let Ok(mut blackboard) = brains.get_mut(car)
    {
        blackboard.order_queue.clear();
    }
    true
}
