//! The AI's cargo and button orders (`MoveCargoOrder`, `GrabCargoOrder`,
//! `PressButtonOrder`: `CreateCarrierBehaviour`, `CreateGrabCargoBehaviour`,
//! `CreatePressButtonBehaviour`), run as the plugin runs them (gather, tick,
//! the act written into the car's intent) on a car that is a point moving as
//! its stick says, over a map's navigation graph and floor plan, with a
//! magnet that answers the intent as `gadget::use_magnets` does: the button
//! held 0.2 s shoots it out, a ray along the aim finds the cargo, letting go
//! links it, holding again drags it, letting go drops it, a cancel lets it
//! go. (The magnet itself is tested in the game: `README`.)

use std::sync::Arc;

use avian2d::prelude::*;
use bevy::math::Vec2;
use bevy::prelude::*;
use flatbt_bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::ai::order_queue::OrdersPlugin;
use tilerider::ai::trees::order_executor_ai;
use tilerider::ai::{AiBlackboard, Body, GadgetButton, MagnetState, Navigation, OrderKind, Orders, Thing, tick_mode};
use tilerider::button::Press;
use tilerider::content::actions::defs::OrderType;
use tilerider::content::map::MapData;
use tilerider::controls::PlayerIntent;
use tilerider::game::{LoadedMap, MapId};
use tilerider::gadget::{CHARGE, REACH};
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;
use tilerider::units::CellSize;

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives the point car, units a second.
const SPEED: f32 = 6.0;
/// How fast the dragged cargo follows the aim, units a second.
const DRAG_SPEED: f32 = 4.0;
/// How many map cells a character of the pictures is.
const SCALE: usize = 3;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

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

/// One big room.
const ROOM: &str = "\n***********\n*.........*\n*.........*\n*.........*\n*.........*\n*.........*\n***********\n";
/// A room cut in two by a wall with a gap at the bottom.
const WALLED: &str = "\n***********\n*....*....*\n*....*....*\n*....*....*\n*....*....*\n*.........*\n***********\n";

/// The cargo of the pictures: a box.
const CARGO: u32 = 11;
const BUTTON: u32 = 12;
const CAR_RADIUS: f32 = 1.0;

/// The box the car carries.
struct Cargo {
    pos: Vec2,
    radius: f32,
    /// `CanBeLinked`: the magnet finds it.
    linkable: bool,
    /// `Removed`.
    gone: bool,
}

/// The car's magnet, as `gadget::use_magnets` answers the intent.
#[derive(Default)]
struct Magnet {
    present: bool,
    held: f32,
    active: bool,
    dragging: bool,
    candidate: bool,
    towing: bool,
    /// How many times the gadget was cancelled.
    cancelled: usize,
    /// When the button was first held for a grab or a drop, and let go.
    held_since: Option<f32>,
    released_at: Vec<f32>,
}

/// A floor button, pressed by `button::Press` as the game looks at it.
struct Button {
    pos: Vec2,
    radius: f32,
    press: Press,
    until_look: f32,
}

/// The order-executing car and its mind, and what it works on.
struct Rig<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    intent: PlayerIntent,
    can_move: bool,
    grid: MapGrid,
    time: f32,
    cargo: Cargo,
    magnet: Magnet,
    button: Button,
    /// Map actions the tree asked for, in order.
    ran: Vec<String>,
    /// The nearest the car came to each of the points asked after.
    nearest: Vec<(Vec2, f32)>,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Rig<B> {
    fn new(builder: B, picture: &str, pos: Vec2) -> Self {
        let grid = grid(&scaled(picture));
        let mut bb = AiBlackboard::new(7);
        bb.nav = Some(Navigation::build(&grid, CellSize(1.0)));
        bb.terrain = Some(Arc::new(Terrain::from_grid(&grid, 1.0)));
        Rig {
            bb,
            behavior: Behavior::for_tree(builder),
            tree: BehaviorTree::new(&builder, tick_mode),
            act: None,
            pos,
            intent: PlayerIntent::default(),
            can_move: true,
            grid,
            time: 0.0,
            cargo: Cargo { pos: Vec2::ZERO, radius: 0.5, linkable: true, gone: false },
            magnet: Magnet { present: true, ..Magnet::default() },
            button: Button { pos: Vec2::ZERO, radius: 2.5, press: Press::new(false), until_look: 0.25 },
            ran: Vec::new(),
            nearest: Vec::new(),
        }
    }

    fn path(&self) -> String {
        self.behavior.describe(self.tree.get()).to_string()
    }

    fn order(&mut self, kind: OrderKind, removal: OrderType) {
        self.bb.order_queue.enqueue(kind, removal);
    }

    fn move_cargo(&mut self, to: Vec2, radius: f32, drop_at: Option<Vec2>, removal: OrderType) {
        self.order(OrderKind::MoveCargo { cargo: id(CARGO), to, radius, drop_at }, removal);
    }

    /// Whether the cargo is on the rope.
    fn towing(&self) -> bool {
        self.magnet.towing
    }

    /// What the world tells the blackboard (`ai::gather_things`).
    fn tell_the_blackboard(&mut self) {
        self.bb.magnet = self.magnet.present.then(|| MagnetState {
            towing: self.magnet.towing.then(|| id(CARGO)),
            candidate: self.magnet.candidate.then(|| id(CARGO)),
        });
        let watched: Vec<Entity> = self.bb.watched().collect();
        self.bb.things.clear();
        for entity in watched {
            let thing = if entity == id(CARGO) && !self.cargo.gone {
                Thing { entity, pos: self.cargo.pos, radius: self.cargo.radius, pressed: false }
            } else if entity == id(BUTTON) {
                Thing { entity, pos: self.button.pos, radius: self.button.radius, pressed: self.button.press.pressed }
            } else {
                continue;
            };
            if !self.bb.things.iter().any(|t| t.entity == entity) {
                self.bb.things.push(thing);
            }
        }
    }

    /// `gadget::use_magnets`, the part the AI's intent reaches.
    fn work_the_magnet(&mut self) {
        let m = &mut self.magnet;
        if !m.present {
            return;
        }
        if std::mem::take(&mut self.intent.gadget_cancel) {
            m.cancelled += 1;
            *m = Magnet { present: true, cancelled: m.cancelled, released_at: std::mem::take(&mut m.released_at), ..Magnet::default() };
            return;
        }
        if self.intent.gadget {
            m.held += DT;
            if m.held < CHARGE {
                return;
            }
            if !m.active {
                m.active = true;
                m.dragging = m.towing;
                m.held_since.get_or_insert(self.time);
            }
            let aim = self.pos + self.intent.gadget_dir.unwrap_or(Vec2::X) * REACH;
            if m.dragging {
                self.intent.move_dir = Vec2::ZERO;
                let to = (aim - self.cargo.pos).clamp_length_max(DRAG_SPEED * DT);
                self.cargo.pos += to;
            } else {
                // A ray along the aim hits the box.
                let (ray, to) = ((aim - self.pos).normalize_or_zero(), self.cargo.pos - self.pos);
                m.candidate = self.cargo.linkable
                    && !self.cargo.gone
                    && to.length() <= REACH
                    && ray.dot(to) > 0.0
                    && ray.perp_dot(to).abs() <= self.cargo.radius;
            }
        } else {
            if m.held >= CHARGE && m.active {
                m.towing = m.candidate && !m.dragging && !self.cargo.gone;
                m.released_at.push(self.time);
            }
            m.held = 0.0;
            m.active = false;
            m.dragging = false;
            m.candidate = false;
            m.held_since = None;
        }
    }

    fn step(&mut self) -> Orders {
        self.time += DT;
        self.tell_the_blackboard();
        let walls = &self.grid;
        let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let body = Body {
            pos: self.pos,
            heading: 0.0,
            team: 0,
            gun: None,
            velocity: self.intent.move_dir * SPEED,
            radius: CAR_RADIUS,
            can_move: self.can_move,
            stick: self.intent.move_dir,
            health: 12,
            max_health: 12,
            shield: 0,
        };
        self.bb.gather(DT, self.act.is_some(), body, &[], &blocked);
        let mode = self.bb.tick.entry_mode().expect("gathered");
        self.act = self.behavior.tick(self.tree.get(), &mut self.bb, mode);
        let orders = self.act.unwrap_or_default();
        // `ai::carry_out`.
        let cleared = self.bb.take_stick_cleared();
        match orders.move_dir {
            Some(m) => self.intent.move_dir = m,
            None if cleared => self.intent.move_dir = Vec2::ZERO,
            None => {}
        }
        let cancel = self.bb.take_gadget_cancel();
        orders.write_gadget(&mut self.intent, cancel);
        self.ran.extend(self.bb.take_actions());
        self.work_the_magnet();
        // The car: it drives as its stick says (a static one stands).
        if self.can_move {
            let step = self.intent.move_dir * SPEED * DT;
            let solid = |p: Vec2| self.grid.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
            for axis in [Vec2::X, Vec2::Y] {
                let to = self.pos + step * axis;
                if !solid(to) {
                    self.pos = to;
                }
            }
        }
        // What the rope does between the grab and the drop: the cargo
        // follows the car, no nearer than a rope's length.
        if self.magnet.towing && !self.magnet.dragging {
            let behind = self.cargo.pos - self.pos;
            if behind.length() > 3.5 {
                self.cargo.pos -= behind.normalize() * (behind.length() - 3.5).min(SPEED * DT);
            }
        }
        // The button, as `button::look` does every quarter second.
        self.button.until_look -= DT;
        if self.button.until_look <= 0.0 {
            self.button.until_look += 0.25;
            let d = (self.pos - self.button.pos).abs();
            let standing = d.x < self.button.radius + CAR_RADIUS && d.y < self.button.radius + CAR_RADIUS;
            self.button.press.look(standing.then_some(None));
        }
        for (point, nearest) in &mut self.nearest {
            *nearest = nearest.min(self.pos.distance(*point));
        }
        orders
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    /// Runs until the queue is empty, at most `secs`; the time it took.
    fn run_until_done(&mut self, secs: f32) -> Option<f32> {
        for n in 0..(secs / DT).round() as usize {
            self.step();
            if self.bb.order_queue.is_empty() {
                return Some(n as f32 * DT);
            }
        }
        None
    }
}

fn rig(picture: &str, car: Vec2) -> Rig<impl TreeBuilder<AiBlackboard, Orders> + Copy> {
    Rig::new(order_executor_ai, picture, car)
}

#[test]
fn a_move_cargo_order_grabs_the_cargo_tows_it_and_drops_it_within_r_of_the_point() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(6.0, 2.0);
    let to = at(2.0, 4.0);
    car.move_cargo(to, 1.0, None, OrderType::RemoveOnSuccess);
    car.run_until_done(60.0).expect("the order is done");
    // The cargo is where it was asked, within its own radius and `R`, and
    // let go of; the magnet was never cancelled and the button is let go.
    assert!(car.cargo.pos.distance(to) < car.cargo.radius + 1.0, "cargo {} from the point", car.cargo.pos.distance(to));
    assert!(!car.towing() && !car.intent.gadget);
    assert_eq!(car.magnet.cancelled, 0);
    // The car stood within 7 units of the cargo and the point for the grab
    // and the drop: the button was let go twice (the grab, the drop).
    assert_eq!(car.magnet.released_at.len(), 2, "{:?}", car.magnet.released_at);
    assert!(car.ran.is_empty(), "no `ActionOnGrab` was given");
}

#[test]
fn the_button_is_held_a_second_at_least_for_the_grab_and_again_for_the_drop() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(4.0, 2.0);
    car.move_cargo(at(1.0, 4.0), 1.0, None, OrderType::RemoveOnSuccess);
    let mut held = vec![];
    let mut since = None;
    for _ in 0..(60.0 / DT) as usize {
        car.step();
        match (car.intent.gadget, since) {
            (true, None) => since = Some(car.time),
            (false, Some(from)) => {
                held.push(car.time - from);
                since = None;
            }
            _ => {}
        }
        if car.bb.order_queue.is_empty() {
            break;
        }
    }
    assert_eq!(held.len(), 2, "{held:?}");
    // `_durationMin`: a second at least (and the grab's 5 s at most).
    assert!(held.iter().all(|h| (0.99..5.5).contains(h)), "{held:?}");
}

#[test]
fn with_a_drop_position_the_car_drives_there_before_it_drops() {
    let mut car = rig(ROOM, at(1.0, 1.0));
    car.cargo.pos = at(3.0, 1.0);
    let (to, drop_at) = (at(8.0, 4.0), at(7.0, 4.0));
    car.nearest.push((drop_at, f32::MAX));
    car.move_cargo(to, 1.0, Some(drop_at), OrderType::RemoveOnSuccess);
    car.run_until_done(90.0).expect("the order is done");
    // `MoveToPoint("DropPos")` takes it to within its radius plus 1 of the
    // drop position, and the cargo ends within reach of the point.
    assert!(car.nearest[0].1 < CAR_RADIUS + 1.0 + 0.2, "nearest to the drop position {}", car.nearest[0].1);
    assert!(car.cargo.pos.distance(to) < car.cargo.radius + 1.0);
    assert_eq!(car.magnet.cancelled, 0);
}

#[test]
fn a_grab_order_takes_the_cargo_and_runs_the_action_on_grab_once() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(7.0, 3.0);
    car.bb.map_action_on_trigger = Some("OnGrab".into());
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnSuccess);
    car.run_until_done(60.0).expect("the order is done");
    assert!(car.towing(), "the cargo is on the rope");
    assert_eq!(car.ran, ["OnGrab"]);
    assert_eq!(car.bb.map_action_on_trigger, None, "`TriggerMapAction(true)`: once");
    // It stands near the cargo (4 to 7 units, the magnet's reach), in sight.
    let d = car.pos.distance(car.cargo.pos);
    assert!(d < REACH, "{d}");
    // It goes on holding nothing: the button is let go.
    car.run(1.0);
    assert!(!car.intent.gadget && car.towing());
}

#[test]
fn a_grab_order_with_the_cargo_already_on_the_rope_does_nothing_and_fails() {
    // `HasNoCargo`: C# fails the order (a `RemoveOnSuccess` one is tried
    // again at every think).
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(2.0, 2.0);
    car.magnet.towing = true;
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnSuccess);
    car.run(3.0);
    assert_eq!(car.bb.order_queue.len(), 1);
    assert!(car.ran.is_empty() && !car.intent.gadget);
}

#[test]
fn a_cargo_already_on_the_rope_is_dropped_without_being_grabbed_again() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(1.0, 3.0);
    car.magnet.towing = true;
    car.bb.map_action_on_trigger = Some("OnGrab".into());
    car.move_cargo(at(2.0, 4.0), 1.0, None, OrderType::RemoveOnSuccess);
    car.run_until_done(60.0).expect("the order is done");
    assert!(!car.towing());
    assert!(car.ran.is_empty(), "the first branch of the `Carrier` drops; the grab (and its action) is not run");
    assert_eq!(car.magnet.released_at.len(), 1);
}

#[test]
fn a_cargo_that_is_gone_fails_the_order_which_stays_unless_it_is_remove_on_try() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(6.0, 2.0);
    car.cargo.gone = true;
    car.move_cargo(at(2.0, 4.0), 1.0, None, OrderType::RemoveOnSuccess);
    let start = car.pos;
    car.run(2.0);
    assert_eq!(car.bb.order_queue.len(), 1, "tried again and again");
    assert_eq!(car.pos, start, "and the car goes nowhere");
    assert!(!car.intent.gadget);

    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.gone = true;
    car.move_cargo(at(2.0, 4.0), 1.0, None, OrderType::RemoveOnTry);
    car.step();
    assert!(car.bb.order_queue.is_empty(), "dropped at its first try");
}

#[test]
fn a_magnet_that_finds_nothing_gives_up_after_five_seconds_and_is_cancelled() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(4.0, 2.0);
    // Not linkable: the ray never finds it.
    car.cargo.linkable = false;
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnTry);
    let done = car.run_until_done(30.0).expect("the order ends (it is `RemoveOnTry`)");
    assert!(!car.towing());
    assert_eq!(car.magnet.cancelled, 1, "`Fire = GadgetActivityCancel`");
    assert!(!car.intent.gadget);
    // About a second of standing, then the 5 s the grab may take (and the
    // think it is noticed at).
    assert!((6.0..7.5).contains(&done), "{done}");
    assert!(car.ran.is_empty(), "no grab, no `ActionOnGrab`");
}

#[test]
fn a_car_with_no_magnet_cannot_grab() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(4.0, 2.0);
    car.magnet.present = false;
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnSuccess);
    car.run(5.0);
    assert_eq!(car.bb.order_queue.len(), 1);
    assert!(!car.intent.gadget && !car.towing());
}

#[test]
fn a_car_that_cannot_move_grabs_and_drops_what_is_in_its_reach_without_driving() {
    let mut car = rig(ROOM, at(4.0, 2.0));
    car.can_move = false;
    car.cargo.pos = at(6.0, 2.0);
    let (start, to) = (car.pos, at(3.0, 3.0));
    car.move_cargo(to, 1.0, None, OrderType::RemoveOnSuccess);
    car.run_until_done(60.0).expect("the order is done");
    assert_eq!(car.pos, start, "`IsStatic`: no drive");
    assert!(car.cargo.pos.distance(to) < car.cargo.radius + 1.0);
    assert!(!car.towing());
}

#[test]
fn the_car_drives_round_a_wall_to_where_it_sees_the_cargo() {
    // The cargo is in the other half of the walled room, in no sight.
    let mut car = rig(WALLED, at(1.0, 1.0));
    car.cargo.pos = at(7.0, 1.0);
    assert!(car.bb.line_collides(car.pos, car.cargo.pos));
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnSuccess);
    car.run_until_done(90.0).expect("the order is done");
    assert!(car.towing());
    assert!(!car.bb.line_collides(car.pos, car.cargo.pos), "it grabbed in sight");
}

#[test]
fn a_press_button_order_drives_onto_the_button_and_waits_for_it_to_be_pressed() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.button.pos = at(6.0, 2.0);
    car.order(OrderKind::PressButton { button: id(BUTTON) }, OrderType::RemoveOnSuccess);
    let mut arrived = None;
    for n in 0..(60.0 / DT) as usize {
        car.step();
        let d = (car.pos - car.button.pos).abs();
        if arrived.is_none() && d.x < car.button.radius && d.y < car.button.radius {
            arrived = Some(n as f32 * DT);
        }
        if car.bb.order_queue.is_empty() {
            let waited = n as f32 * DT - arrived.expect("it was on the button");
            // `Timer(ActivationTimeSec + 0.2f)`, noticed at a think.
            assert!((1.2..1.75).contains(&waited), "waited {waited}");
            assert!(car.button.press.pressed);
            return;
        }
    }
    panic!("the order was not done: {}", car.path());
}

#[test]
fn a_button_that_is_not_pressed_fails_the_order() {
    // A car that cannot get onto it: the button stays unpressed, the
    // `Check state` fails, and the order is tried again.
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.button.pos = at(6.0, 2.0);
    // The game's button never sees the car: nothing presses it.
    car.button.radius = 2.5;
    car.button.until_look = f32::MAX;
    car.order(OrderKind::PressButton { button: id(BUTTON) }, OrderType::RemoveOnSuccess);
    car.run(30.0);
    assert_eq!(car.bb.order_queue.len(), 1, "tried again");
    assert!(!car.button.press.pressed);
}

#[test]
fn a_button_that_is_gone_fails_the_order_at_once() {
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.order(OrderKind::PressButton { button: id(99) }, OrderType::RemoveOnTry);
    car.step();
    assert!(car.bb.order_queue.is_empty(), "`Removed`: failed, and dropped as `RemoveOnTry`");
    assert!(car.act.is_none());
}

#[test]
fn clearing_the_orders_leaves_the_gadget_button_as_it_was() {
    // C# kept its `Fire` flags: a car cleared while it holds the button
    // goes on holding it.
    let mut car = rig(ROOM, at(1.0, 2.0));
    car.cargo.pos = at(4.0, 2.0);
    car.order(OrderKind::GrabCargo { cargo: id(CARGO) }, OrderType::RemoveOnSuccess);
    for _ in 0..(10.0 / DT) as usize {
        car.step();
        if car.intent.gadget {
            break;
        }
    }
    assert!(car.intent.gadget, "the button is held for the grab");
    car.bb.order_queue.clear();
    car.run(1.0);
    assert!(car.act.is_none());
    assert!(car.intent.gadget, "still held");
}

#[test]
fn the_gadget_orders_write_the_button_the_aim_and_the_stick_into_the_intent() {
    let aim = Vec2::new(0.25, -0.5);
    let mut intent = PlayerIntent { move_dir: Vec2::X, gadget_at: Some(Vec2::ONE), ..default() };
    let hold = Orders { gadget: Some(GadgetButton::Hold), gadget_dir: Some(aim), ..default() };
    hold.write_gadget(&mut intent, false);
    assert!(intent.gadget);
    assert_eq!((intent.gadget_dir, intent.gadget_at), (Some(aim), None), "a way, not the player's point");
    assert_eq!(intent.move_dir, Vec2::X, "holding the button does not touch the stick");
    // No gadget order leaves it as it is.
    Orders::default().write_gadget(&mut intent, false);
    assert!(intent.gadget);
    // `GadgetActivityEnd` lets go and `ControlHandle` clears the axis.
    Orders { gadget: Some(GadgetButton::Release), ..default() }.write_gadget(&mut intent, false);
    assert!(!intent.gadget);
    assert_eq!(intent.move_dir, Vec2::ZERO);
    // A cancel is a pulse for the magnet, with the button let go.
    intent.gadget = true;
    intent.move_dir = Vec2::Y;
    Orders::default().write_gadget(&mut intent, true);
    assert!(!intent.gadget && intent.gadget_cancel);
    assert_eq!(intent.move_dir, Vec2::ZERO);
    // Orders in a row: the later gadget order wins.
    let both = Orders { gadget: Some(GadgetButton::Hold), ..default() }.then(Orders { gadget: Some(GadgetButton::Release), ..default() });
    assert_eq!(both.gadget, Some(GadgetButton::Release));
}

/// An app with the order actions and a map of three cells' worth of
/// `Actions`, a car with a blackboard (map id 4002), a box (1007) and a
/// button (5005).
fn app(items: &str) -> (App, Entity, Entity, Entity) {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>
.
</CharMap><Actions>{items}</Actions></Asset></XnaContent>"#
    );
    let data = MapData::parse(&xml).unwrap();
    let grid = MapGrid::build(&data, None);
    let light = tilerider::light::bake::LightMap::bake(&data, &grid, &tilerider::light::raster::Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let actions = data.actions.clone();
    let map = LoadedMap { data, grid, light, cell: CellSize::from_map_scale(64.0), cars: default(), list: default(), sprites: default() };
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, OrdersPlugin)).insert_resource(CurrentActions(actions)).insert_resource(map);
    let car = app.world_mut().spawn((MapId(4002), AiBlackboard::new(1))).id();
    let cargo = app.world_mut().spawn((MapId(1007), Position(Vec2::ZERO))).id();
    let button = app.world_mut().spawn(MapId(5005)).id();
    (app, car, cargo, button)
}

fn queue(app: &App, car: Entity) -> Vec<OrderKind> {
    let bb = app.world().entity(car).get::<AiBlackboard>().unwrap();
    let mut queue = bb.order_queue.clone();
    let mut kinds = vec![];
    while let Some(order) = queue.peek() {
        kinds.push(order.kind);
        queue.end(true);
        if queue.peek().is_some_and(|o| Some(o.kind) == kinds.last().copied()) {
            queue.clear();
        }
    }
    kinds
}

#[test]
fn the_cargo_orders_are_queued_with_their_points_and_radius_in_the_world() {
    let (mut app, car, cargo, _) = app(
        r#"<Item Type="GameLibrary.MoveCargoOrder"><Name>Carry</Name><Position>20 12</Position><R>1.5</R>
             <ItemId>4002</ItemId><CargoItemId>1007</CargoItemId><DropPosition>19 14.5</DropPosition></Item>
           <Item Type="GameLibrary.MoveCargoOrder"><Name>CarryAnywhere</Name><OrderRemoveCondition>RemoveOnTry</OrderRemoveCondition>
             <Position>3 4</Position><ItemId>4002</ItemId><CargoItemId>1007</CargoItemId><DropPosition>0 0</DropPosition></Item>"#,
    );
    let cell = app.world().resource::<LoadedMap>().cell;
    assert!(run(app.world_mut(), "Carry", None, 0));
    assert!(run(app.world_mut(), "CarryAnywhere", None, 0));
    let bb = app.world().entity(car).get::<AiBlackboard>().unwrap();
    assert_eq!(bb.order_queue.len(), 2);
    let kinds = queue(&app, car);
    assert_eq!(
        kinds[0],
        OrderKind::MoveCargo { cargo, to: cell.to_world(20.0, 12.0), radius: 1.5 * cell.0, drop_at: Some(cell.to_world(19.0, 14.5)) }
    );
    // A `DropPosition` of exactly `0 0` is "find a place yourself".
    assert_eq!(kinds[1], OrderKind::MoveCargo { cargo, to: cell.to_world(3.0, 4.0), radius: 1.0 * cell.0, drop_at: None });
}

#[test]
fn a_grab_order_sets_the_action_on_grab_at_once_and_item_zero_is_the_actor() {
    let (mut app, car, cargo, _) = app(
        r#"<Item Type="GameLibrary.GrabCargoOrder"><Name>Grab</Name><ItemId>0</ItemId><CargoItemId>0</CargoItemId>
             <ActionOnGrab>OnGrab</ActionOnGrab></Item>
           <Item Type="GameLibrary.GrabCargoOrder"><Name>GrabPlain</Name><ItemId>4002</ItemId><CargoItemId>1007</CargoItemId></Item>"#,
    );
    // Item 0 is the car (or the cargo) that set the action off.
    assert!(run(app.world_mut(), "GrabPlain", Some(4002), 0));
    assert_eq!(queue(&app, car), [OrderKind::GrabCargo { cargo }]);
    assert_eq!(app.world().entity(car).get::<AiBlackboard>().unwrap().map_action_on_trigger, None);
    // The cargo is the actor: a car grabbing itself, but the action is set
    // before the order's turn.
    app.world_mut().entity_mut(car).get_mut::<AiBlackboard>().unwrap().order_queue.clear();
    run(app.world_mut(), "Grab", Some(4002), 0);
    let bb = app.world().entity(car).get::<AiBlackboard>().unwrap();
    assert_eq!(bb.map_action_on_trigger.as_deref(), Some("OnGrab"));
    assert_eq!(bb.order_queue.len(), 1);
}

#[test]
fn a_press_button_order_names_the_button_by_its_own_id() {
    let (mut app, car, _, button) = app(
        r#"<Item Type="GameLibrary.PressButtonOrder"><Name>Press</Name><ButtonId>5005</ButtonId><ItemId>4002</ItemId></Item>
           <Item Type="GameLibrary.PressButtonOrder"><Name>PressNone</Name><ButtonId>5999</ButtonId><ItemId>4002</ItemId></Item>"#,
    );
    run(app.world_mut(), "PressNone", None, 0);
    assert!(queue(&app, car).is_empty(), "no such button: no order");
    run(app.world_mut(), "Press", None, 0);
    assert_eq!(queue(&app, car), [OrderKind::PressButton { button }]);
}

#[test]
fn an_order_for_a_car_or_cargo_that_is_not_there_is_not_queued() {
    let (mut app, car, _, _) = app(
        r#"<Item Type="GameLibrary.MoveCargoOrder"><Name>NoCargo</Name><Position>1 1</Position><ItemId>4002</ItemId><CargoItemId>1999</CargoItemId></Item>
           <Item Type="GameLibrary.MoveCargoOrder"><Name>NoCar</Name><Position>1 1</Position><ItemId>4999</ItemId><CargoItemId>1007</CargoItemId></Item>"#,
    );
    run(app.world_mut(), "NoCargo", None, 0);
    run(app.world_mut(), "NoCar", None, 0);
    assert!(queue(&app, car).is_empty());
}
