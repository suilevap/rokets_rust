//! The trees that drive along paths (`ai::trees::follow_path_ai`, `chase`)
//! and the nodes they are made of (`MoveBase`, `MoveToPoint`, `FollowBy`,
//! `TryFireToAnyone`, `TriggerMapAction`, `Optional`), run as the plugin runs
//! them (gather, tick, the act on the car) on a car that is a point moving
//! as its stick says.

mod common;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt_bevy::prelude::*;
use tilerider::ai::nodes::{MoveBase, go_to_position, optional, optional_required};
use tilerider::ai::trees::{CHASER_SCAN_DISTANCE, FIRE_TO_ANYONE_PERIOD, chaser_ai, chaser_target_from_local_variable, follow_path_ai, follow_path_ai_safe};
use tilerider::ai::{AiBlackboard, Body, Foe, Gun, Navigation, Orders, tick_mode};
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::nav::{Blocker, NavGraph, NavPath};
use tilerider::weapon::model::{FireRange, MINIGUN};

const DT: f32 = 1.0 / 60.0;
/// How fast the stick drives the point car, units a second.
const SPEED: f32 = 6.0;

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

/// Every character of a picture made `k` characters wide and tall.
fn scaled(rows: &str, k: usize) -> String {
    let mut out = String::from("\n");
    for row in rows.trim().lines() {
        let wide: String = row.chars().flat_map(|c| std::iter::repeat_n(c, k)).collect();
        for _ in 0..k {
            out.push_str(&wide);
            out.push('\n');
        }
    }
    out
}

/// The middle of a cell of the picture, in the world (y up), a cell of the
/// picture being `TWO_ROOMS_SCALE` cells of the map.
fn at(x: f32, y: f32) -> Vec2 {
    let k = SCALE as f32;
    Vec2::new((x + 0.5) * k, -(y + 0.5) * k)
}

/// How many map cells a character of the pictures below is: corridors a car
/// of radius 1 can pass.
const SCALE: usize = 3;

const TWO_ROOMS: &str = "\n***********\n*....*....*\n*....*....*\n*.........*\n*....*....*\n*....*....*\n***********\n";

const OPEN_ROOM: &str = "\n***********\n*.........*\n*.........*\n*.........*\n*.........*\n*.........*\n***********\n";

fn two_rooms() -> MapGrid {
    grid(&scaled(TWO_ROOMS, SCALE))
}

fn open_room() -> MapGrid {
    grid(&scaled(OPEN_ROOM, SCALE))
}

/// A car and its mind.
struct Car<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    stick: Vec2,
    /// Whether the stick moves it (a stuck car does not), and how fast a full
    /// stick does.
    free: bool,
    speed: f32,
    /// What the cars around it are, and the walls.
    others: Vec<Foe>,
    walls: MapGrid,
    gun_at_enemies: bool,
    /// The orders written, one per step.
    fired: Vec<bool>,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Car<B> {
    fn new(builder: B, pos: Vec2, nav: Option<&Navigation>) -> Self {
        let mut bb = AiBlackboard::new(7);
        bb.nav = nav.cloned();
        Car {
            bb,
            behavior: Behavior::for_tree(builder),
            tree: BehaviorTree::new(&builder, tick_mode),
            act: None,
            pos,
            stick: Vec2::ZERO,
            free: true,
            speed: SPEED,
            others: Vec::new(),
            walls: open_room(),
            gun_at_enemies: false,
            fired: Vec::new(),
        }
    }

    fn path(&self) -> String {
        self.behavior.describe(self.tree.get()).to_string()
    }

    /// One fixed step: the gather, the tick, what `carry_out` does with the act.
    fn step(&mut self) -> Orders {
        let walls = &self.walls;
        let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let gun = self.gun_at_enemies.then(|| {
            let toward = self.others.first().map_or(Vec2::X, |f| f.pos - self.pos);
            Gun {
                complex: true,
                max_angle: 1.0,
                range: FireRange { from: self.pos, direction: toward.to_angle(), range: MINIGUN.range, dispersion: MINIGUN.dispersion },
                immediate: true,
            }
        });
        let body = Body {
            pos: self.pos,
            heading: 0.0,
            team: 1,
            gun,
            velocity: if self.free { self.stick * self.speed } else { Vec2::ZERO },
            radius: 1.0,
            can_move: true,
            stick: self.stick,
            health: 12,
            max_health: 12,
            shield: 0,
        };
        self.bb.gather(DT, self.act.is_some(), body, &self.others, &blocked);
        let mode = self.bb.tick.entry_mode().expect("gathered");
        self.act = self.behavior.tick(self.tree.get(), &mut self.bb, mode);
        let orders = self.act.unwrap_or_default();
        let cleared = self.bb.take_stick_cleared();
        match orders.move_dir {
            Some(m) => self.stick = m,
            None if cleared => self.stick = Vec2::ZERO,
            None => {}
        }
        if self.free {
            // The walls stop it, a wall along an axis only on that axis.
            let step = self.stick * self.speed * DT;
            let solid = |p: Vec2| self.walls.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
            for axis in [Vec2::X, Vec2::Y] {
                let to = self.pos + step * axis;
                if !solid(to) {
                    self.pos = to;
                }
            }
        }
        self.fired.push(orders.fire);
        orders
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }
}

/// A square in the open room (the map's y goes down: negative here).
fn square() -> Vec<Vec2> {
    vec![Vec2::new(5.0, -5.0), Vec2::new(28.0, -5.0), Vec2::new(28.0, -16.0), Vec2::new(5.0, -16.0)]
}

#[test]
fn a_car_on_a_closed_path_goes_round_it_for_ever() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.bb.path = Some(NavPath::new(square(), true));
    let corners = square();
    let mut nearest = [f32::MAX; 4];
    for _ in 0..(40.0 / DT) as usize {
        car.step();
        for (n, c) in nearest.iter_mut().zip(&corners) {
            *n = n.min(car.pos.distance(*c));
        }
    }
    // It has been (almost) to every corner, cutting them a little as the
    // carrot ahead of it does, and is still driving a path of its own.
    assert!(nearest.iter().all(|d| *d < 3.0), "{nearest:?}");
    assert!(car.bb.path.as_ref().is_some_and(|p| !p.is_end()));
    assert!(car.path().contains("Fire"), "{}", car.path());
    // Never far from the path.
    let on_path = |p: Vec2| {
        (0..4).map(|i| distance_to_segment(p, corners[i], corners[(i + 1) % 4])).fold(f32::MAX, f32::min)
    };
    assert!(on_path(car.pos) < 3.0);
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[test]
fn a_car_on_an_open_path_arrives_stops_and_runs_the_trigger_once() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.bb.path = Some(NavPath::new(vec![Vec2::new(5.0, -5.0), Vec2::new(28.0, -5.0)], false));
    car.bb.map_action_on_trigger = Some("PathDone".into());
    let mut sent = Vec::new();
    for _ in 0..(15.0 / DT) as usize {
        car.step();
        sent.extend(car.bb.take_actions());
    }
    assert!(car.bb.path.is_none(), "the path is dropped when it is done");
    assert!(car.pos.x > 26.0 && car.pos.x < 29.5, "{:?}", car.pos);
    assert_eq!(car.stick, Vec2::ZERO, "the stick is let go");
    assert_eq!(sent, vec!["PathDone".to_string()], "run once, after the path");
    assert!(car.bb.map_action_on_trigger.is_none());
}

#[test]
fn a_car_with_no_path_only_runs_its_trigger_and_does_not_move_or_fire() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.bb.map_action_on_trigger = Some("Go".into());
    car.gun_at_enemies = true;
    car.bb.watch_enemies = true;
    car.others = vec![foe(1, 0, 8.0, -5.0)];
    let mut sent = Vec::new();
    for _ in 0..120 {
        car.step();
        sent.extend(car.bb.take_actions());
    }
    assert_eq!(sent, vec!["Go".to_string()]);
    assert_eq!(car.pos, Vec2::new(5.0, -5.0));
    // `TriggerMapAction` always succeeds, so the selector never reaches the
    // `TryFireToAnyone` after it.
    assert!(car.fired.iter().all(|f| !f));
}

fn foe(n: u32, team: u8, x: f32, y: f32) -> Foe {
    Foe { entity: id(n), team, pos: Vec2::new(x, y), vel: Vec2::ZERO, radius: 1.0, weapon_range: None }
}

#[test]
fn a_stuck_car_gives_up_its_path_after_two_checks() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.free = false;
    car.bb.path = Some(NavPath::new(vec![Vec2::new(5.0, -5.0), Vec2::new(28.0, -5.0)], false));
    // The first check (2 s in) lets it be: it was at the origin before.
    car.run(2.5);
    assert!(car.bb.path.is_some());
    assert_ne!(car.stick, Vec2::ZERO, "it is pushing");
    // The next one (2 s on) sees it has not moved.
    car.run(2.5);
    assert!(car.bb.path.is_none());
    assert_eq!(car.stick, Vec2::ZERO);
}

#[test]
fn a_closed_path_is_never_checked_for_being_stuck() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.free = false;
    car.bb.path = Some(NavPath::new(square(), true));
    car.run(8.0);
    assert!(car.bb.path.is_some());
}

#[test]
fn it_fires_at_anyone_in_the_guns_line_once_in_three_seconds() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.bb.path = Some(NavPath::new(square(), true));
    car.bb.watch_enemies = true;
    car.gun_at_enemies = true;
    car.free = false;
    // Team 0, in the line and in range of the Minigun.
    car.others = vec![foe(1, 0, 5.0, -12.0)];
    car.run(7.0);
    let shots: Vec<usize> = car.fired.iter().enumerate().filter(|(_, f)| **f).map(|(i, _)| i).collect();
    assert_eq!(shots.len(), 3, "{shots:?}");
    let period = (FIRE_TO_ANYONE_PERIOD / DT).round() as usize;
    assert!(shots[1] - shots[0] >= period && shots[1] - shots[0] <= period + 1, "{shots:?}");
    assert!(shots[0] < 5, "the first at once: {shots:?}");
}

#[test]
fn friends_are_not_fired_at() {
    let mut car = Car::new(follow_path_ai, Vec2::new(5.0, -5.0), None);
    car.bb.path = Some(NavPath::new(square(), true));
    car.bb.watch_enemies = true;
    car.gun_at_enemies = true;
    car.others = vec![foe(1, 1, 5.0, -12.0)];
    car.run(4.0);
    assert!(car.fired.iter().all(|f| !f));
}

#[test]
fn the_safe_tree_runs_from_an_enemy_in_sight_and_follows_its_path_otherwise() {
    let mut car = Car::new(follow_path_ai_safe, Vec2::new(10.0, -10.0), None);
    car.bb.path = Some(NavPath::new(vec![Vec2::new(10.0, -10.0), Vec2::new(28.0, -10.0)], false));
    car.run(1.0);
    assert!(car.stick.x > 0.0, "along the path: {:?}", car.stick);
    // An enemy ahead of it, in sight and near: away from it, a part of a
    // full stick (it is under `ScanDistance` away).
    car.others = vec![foe(1, 0, car.pos.x + 6.0, -10.0)];
    car.run(1.0);
    assert!(car.stick.x < 0.0 && car.stick.length() < 1.0, "{:?}", car.stick);
    assert!(car.path().contains("Runaway"), "{}", car.path());
}

#[test]
fn optional_runs_whatever_its_child_does() {
    fn tree() -> impl BehaviorNode<AiBlackboard, Orders> {
        action(optional(MoveBase::default()))
    }
    fn strict() -> impl BehaviorNode<AiBlackboard, Orders> {
        action(optional_required(MoveBase::default()))
    }
    // `MoveBase` does not start with no path; the optional one runs anyway
    // and never ends.
    let mut car = Car::new(tree, Vec2::new(3.0, 3.0), None);
    car.run(2.0);
    assert_ne!(car.path(), "not running");
    // Given a path it ticks the child, which goes along it.
    car.bb.path = Some(NavPath::new(vec![Vec2::new(30.0, 30.0), Vec2::new(60.0, 30.0)], false));
    car.run(1.0);
    assert_ne!(car.stick, Vec2::ZERO);
    // Required: it starts only when the child does.
    let mut car = Car::new(strict, Vec2::new(3.0, 3.0), None);
    car.run(1.0);
    assert_eq!(car.path(), "not running");
}

// ---- navigation ----------------------------------------------------------

fn nav_of(rows: &str) -> Navigation {
    Navigation::new(NavGraph::from_grid(&grid(&scaled(rows, SCALE)), 1.0))
}

#[test]
fn a_chaser_goes_round_by_the_corridor_to_an_enemy_in_the_next_room() {
    let nav = nav_of(TWO_ROOMS);
    let mut car = Car::new(chaser_ai, at(1.0, 1.0), Some(&nav));
    car.walls = two_rooms();
    car.bb.scan_distance = CHASER_SCAN_DISTANCE;
    let target = at(8.0, 1.0);
    car.others = vec![foe(1, 0, target.x, target.y)];
    let mut through_gap = false;
    for _ in 0..(25.0 / DT) as usize {
        car.step();
        // The gap in the middle wall is the picture's row 3, column 5.
        through_gap |= (15.0..18.0).contains(&car.pos.x) && (-12.0..-9.0).contains(&car.pos.y);
    }
    assert!(through_gap, "it went through the gap");
    assert!(car.pos.distance(target) < 3.0, "{:?}", car.pos);
    assert_eq!(car.bb.target(), Some(id(1)));
}

#[test]
fn a_chaser_with_nobody_to_chase_sleeps_two_seconds() {
    let nav = nav_of(TWO_ROOMS);
    let mut car = Car::new(chaser_ai, at(1.0, 1.0), Some(&nav));
    car.walls = two_rooms();
    car.bb.scan_distance = CHASER_SCAN_DISTANCE;
    car.run(0.5);
    assert!(car.path().contains("Idle"), "{}", car.path());
    // An enemy turns up; the chaser does not look until it wakes (the
    // sleep and a period).
    car.others = vec![foe(1, 0, at(8.0, 1.0).x, at(8.0, 1.0).y)];
    car.run(1.0);
    assert!(car.path().contains("Idle") && car.bb.target().is_none());
    car.run(1.5);
    assert_eq!(car.bb.target(), Some(id(1)));
    assert!(car.bb.path.is_some());
}

#[test]
fn a_chaser_extends_its_path_as_the_target_wanders_and_makes_a_new_one_when_it_is_old() {
    let nav = nav_of(TWO_ROOMS);
    let mut car = Car::new(chaser_ai, at(1.0, 1.0), Some(&nav));
    car.walls = two_rooms();
    car.bb.scan_distance = CHASER_SCAN_DISTANCE;
    let target = at(8.0, 1.0);
    car.others = vec![foe(1, 0, target.x, target.y)];
    car.speed = 1.0;
    car.run(1.0);
    let (points, made) = {
        let p = car.bb.path.as_ref().expect("a path");
        (p.points().len(), car.bb.path_compute_time)
    };
    // The target moves 3 units (more than 2.7, less than 6): the path goes on.
    car.others[0].pos += Vec2::new(0.0, -3.0);
    car.run(0.2);
    let p = car.bb.path.as_ref().unwrap();
    assert_eq!(p.points().len(), points + 1);
    assert_eq!(car.bb.path_compute_time, made);
    assert_eq!(p.last_position(), car.others[0].pos);
    // After two seconds even a small move makes a new path.
    car.run(2.0);
    car.others[0].pos += Vec2::new(0.0, -3.0);
    car.run(0.1);
    assert!(car.bb.path_compute_time > made);
}

#[test]
fn a_blocked_way_is_refused_and_remembers_what_blocks_it() {
    let rows = "\n***********\n*....*....*\n*....*....*\n*....*....*\n**.*****.**\n**.*****.**\n***********\n";
    let nav = nav_of(rows);
    let (a, b) = (at(1.0, 1.0), at(8.0, 3.0));
    let links = nav.write(|g| g.link_points(at(4.0, 2.0), at(6.0, 2.0), false, Some(Blocker(9))));
    nav.write(|g| links.iter().for_each(|l| g.set_link(*l, 32.0, true)));
    let mut bb = AiBlackboard::new(1);
    bb.nav = Some(nav.clone());
    bb.pos = a;
    assert!(!go_to_position(&mut bb, b));
    assert!(bb.path.is_none());
    assert_eq!(bb.blocker, Some(Blocker(9)));
    // Opened, it is a path.
    nav.write(|g| links.iter().for_each(|l| g.set_link(*l, 2.0, false)));
    assert!(go_to_position(&mut bb, b));
    assert_eq!(bb.target_position, b);
    assert!(bb.path.is_some());
    // The origin is "no point" (`Vector2.Zero`), and with no way, none is made.
    assert!(!go_to_position(&mut bb, Vec2::ZERO));
    assert!(!go_to_position(&mut bb, Vec2::new(-50.0, 50.0)));
    assert_eq!(bb.blocker, None);
}

#[test]
fn a_chaser_of_a_named_target_goes_after_that_car_whatever_its_team() {
    let nav = nav_of(TWO_ROOMS);
    let mut car = Car::new(chaser_target_from_local_variable, at(1.0, 1.0), Some(&nav));
    car.walls = two_rooms();
    // A car of its own team, and nearer, an enemy it does not care about.
    let target = at(8.0, 1.0);
    car.others = vec![foe(1, 1, target.x, target.y), foe(2, 0, at(2.0, 2.0).x, at(2.0, 2.0).y)];
    car.bb.variable_target = Some(id(1));
    car.run(25.0);
    assert!(car.pos.distance(target) < 3.0, "{:?}", car.pos);
    assert_eq!(car.bb.target(), Some(id(1)));
}

#[test]
fn a_chaser_with_no_named_target_stays() {
    let nav = nav_of(TWO_ROOMS);
    let start = at(1.0, 1.0);
    let mut car = Car::new(chaser_target_from_local_variable, start, Some(&nav));
    car.walls = two_rooms();
    car.others = vec![foe(1, 0, at(8.0, 1.0).x, at(8.0, 1.0).y)];
    car.run(5.0);
    assert_eq!(car.pos, start);
    assert_eq!(car.bb.target(), None);
}
