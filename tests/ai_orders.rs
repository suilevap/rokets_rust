//! `OrderExecutorAi` (`OrderExecutorFriend`, `OrderExecutorAi`): the order
//! queue (`OrdersManager`) and `ExecuteOrders`, run as the plugin runs them
//! (gather, tick, the act on the car) on a car that is a point moving as its
//! stick says, over a map's navigation graph.

use bevy::math::Vec2;
use flatbt_bevy::prelude::*;
use tilerider::ai::trees::order_executor_ai;
use tilerider::ai::{AiBlackboard, Body, Navigation, OrderKind, OrderQueue, Orders, is_live, tick_mode};
use tilerider::content::actions::defs::OrderType;
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives the point car, units a second.
const SPEED: f32 = 6.0;
/// How many map cells a character of the pictures is.
const SCALE: usize = 3;

fn grid(rows: &str) -> MapGrid {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// Every character of a picture made `SCALE` characters wide and tall.
fn scaled(rows: &str) -> String {
    let mut out = String::from("\n");
    for row in rows.trim().lines() {
        let wide: String = row.chars().flat_map(|c| std::iter::repeat_n(c, SCALE)).collect();
        for _ in 0..SCALE {
            out.push_str(&wide);
            out.push('\n');
        }
    }
    out
}

/// The middle of a cell of the picture, in the world (y up).
fn at(x: f32, y: f32) -> Vec2 {
    let k = SCALE as f32;
    Vec2::new((x + 0.5) * k, -(y + 0.5) * k)
}

/// A room, and a second one with no way into it.
const ROOMS: &str = "\n***********\n*.....*...*\n*.....*...*\n*.....*...*\n*.....*...*\n*.....*...*\n***********\n";

/// The order-executing car and its mind.
struct Car<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    stick: Vec2,
    grid: MapGrid,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Car<B> {
    fn new(builder: B, pos: Vec2) -> Self {
        let grid = grid(&scaled(ROOMS));
        let mut bb = AiBlackboard::new(7);
        bb.nav = Some(Navigation::build(&grid, tilerider::units::CellSize(1.0)));
        Car {
            bb,
            behavior: Behavior::for_tree(builder),
            tree: BehaviorTree::new(&builder, tick_mode),
            act: None,
            pos,
            stick: Vec2::ZERO,
            grid,
        }
    }

    fn path(&self) -> String {
        self.behavior.describe(self.tree.get()).to_string()
    }

    fn order(&mut self, to: Vec2, removal: OrderType) {
        self.bb.order_queue.enqueue(OrderKind::Move { to }, removal);
    }

    fn step(&mut self) -> Orders {
        let walls = &self.grid;
        let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let body = Body {
            pos: self.pos,
            heading: 0.0,
            team: 0,
            gun: None,
            velocity: self.stick * SPEED,
            radius: 1.0,
            can_move: true,
            stick: self.stick,
            health: 12,
            max_health: 12,
            shield: 0,
        };
        self.bb.gather(DT, self.act.is_some(), body, &[], &blocked);
        let mode = self.bb.tick.entry_mode().expect("gathered");
        self.act = self.behavior.tick(self.tree.get(), &mut self.bb, mode);
        let orders = self.act.unwrap_or_default();
        let cleared = self.bb.take_stick_cleared();
        match orders.move_dir {
            Some(m) => self.stick = m,
            None if cleared => self.stick = Vec2::ZERO,
            None => {}
        }
        let step = self.stick * SPEED * DT;
        let solid = |p: Vec2| self.grid.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
        for axis in [Vec2::X, Vec2::Y] {
            let to = self.pos + step * axis;
            if !solid(to) {
                self.pos = to;
            }
        }
        orders
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    /// Runs until the queue holds `left` orders, at most `secs`; the time
    /// it took.
    fn run_until_left(&mut self, left: usize, secs: f32) -> Option<f32> {
        for n in 0..(secs / DT).round() as usize {
            self.step();
            if self.bb.order_queue.len() == left {
                return Some(n as f32 * DT);
            }
        }
        None
    }
}

#[test]
fn both_order_executor_controls_are_live() {
    assert!(is_live("OrderExecutorFriend") && is_live("OrderExecutorAi"));
}

#[test]
fn the_queue_drops_an_order_as_its_remove_condition_says() {
    let mut queue = OrderQueue::default();
    // `EndOrder` with no order does nothing.
    queue.end(true);
    assert!(queue.is_empty());
    let to = OrderKind::Move { to: Vec2::ONE };
    for (removal, ok, dropped) in [
        (OrderType::RemoveOnSuccess, false, false),
        (OrderType::RemoveOnSuccess, true, true),
        (OrderType::RemoveOnTry, false, true),
        (OrderType::RemoveOnTry, true, true),
        (OrderType::Infinite, false, false),
        (OrderType::Infinite, true, false),
    ] {
        queue.clear();
        queue.enqueue(to, removal);
        queue.enqueue(to, OrderType::RemoveOnTry);
        let first = queue.peek().unwrap().serial();
        queue.end(ok);
        assert_eq!(queue.len(), if dropped { 1 } else { 2 }, "{removal:?} ended ok={ok}");
        assert_eq!(queue.peek().unwrap().serial() == first, !dropped, "only the first order is ended");
    }
}

#[test]
fn with_no_order_the_car_does_nothing() {
    let mut car = Car::new(order_executor_ai, at(1.0, 3.0));
    let start = car.pos;
    for _ in 0..120 {
        let orders = car.step();
        assert_eq!(orders, Orders::default());
        assert!(car.act.is_none(), "nothing runs: {}", car.path());
    }
    assert_eq!(car.pos, start);
}

#[test]
fn it_goes_to_each_point_in_turn_and_drops_each_order_when_there() {
    let mut car = Car::new(order_executor_ai, at(1.0, 1.0));
    let (first, second) = (at(4.0, 1.0), at(4.0, 4.0));
    car.order(first, OrderType::RemoveOnSuccess);
    car.order(second, OrderType::RemoveOnSuccess);
    car.run(0.1);
    assert!(car.path().contains("Order"), "{}", car.path());
    car.run_until_left(1, 5.0).expect("the first order is done");
    // Done within the car's radius plus 1 of the point.
    assert!(car.pos.distance(first) < 2.0 + 0.2, "{} from the first point", car.pos.distance(first));
    car.run_until_left(0, 5.0).expect("the second order is done");
    assert!(car.pos.distance(second) < 2.0 + 0.2, "{} from the second point", car.pos.distance(second));
    // With nothing more to do, the stick is let go and the car stays.
    car.run(0.5);
    let there = car.pos;
    car.run(1.0);
    assert!(car.act.is_none());
    assert_eq!(car.stick, Vec2::ZERO);
    assert_eq!(car.pos, there);
}

#[test]
fn an_order_with_no_way_there_stays_unless_it_is_remove_on_try() {
    // The second room has no way in.
    let shut_in = at(8.0, 3.0);
    let mut car = Car::new(order_executor_ai, at(1.0, 3.0));
    car.order(shut_in, OrderType::RemoveOnSuccess);
    car.run(1.0);
    assert_eq!(car.bb.order_queue.len(), 1, "tried again and again");
    assert!(car.act.is_none());

    let mut car = Car::new(order_executor_ai, at(1.0, 3.0));
    car.order(shut_in, OrderType::RemoveOnTry);
    car.order(at(4.0, 3.0), OrderType::RemoveOnSuccess);
    car.step();
    assert_eq!(car.bb.order_queue.len(), 1, "dropped at its first try");
    car.run_until_left(0, 5.0).expect("the next order is carried out");
}

#[test]
fn an_infinite_order_is_never_dropped() {
    let mut car = Car::new(order_executor_ai, at(1.0, 3.0));
    let point = at(4.0, 3.0);
    car.order(point, OrderType::Infinite);
    car.run(4.0);
    assert!(car.pos.distance(point) < 2.2, "it went there");
    assert_eq!(car.bb.order_queue.len(), 1);
}

#[test]
fn a_cleared_order_drives_on_until_the_next_think_and_leaves_the_stick() {
    let mut car = Car::new(order_executor_ai, at(1.0, 1.0));
    car.order(at(4.0, 4.0), OrderType::RemoveOnSuccess);
    car.run(0.5);
    let stick = car.stick;
    assert!(stick != Vec2::ZERO, "on its way");
    car.bb.order_queue.clear();
    // Between thinks the action goes on by itself (`Context.Run()`);
    // a think (every 0.25 s) ends it without completing it.
    let mut ended = None;
    for n in 0..30 {
        car.step();
        if car.act.is_none() {
            ended = Some(n);
            break;
        }
    }
    assert!(ended.is_some_and(|n| n <= 16), "ended at the next think: {ended:?}");
    // The stick is not let go (the action was not completed), and the path
    // stays.
    assert!(car.stick != Vec2::ZERO);
    assert!(car.bb.path.is_some());
    car.run(0.5);
    assert!(car.act.is_none());
}
