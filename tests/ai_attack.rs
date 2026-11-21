//! `AttackerAiVer2` (`AttackAiVer2`) and the small trees beside it
//! (`SimpleAttackNearestEnemy`, `SimpleFollowByEnemyAi`,
//! `MoveAndFireInInitDirection`), run as the plugin runs them (gather, tick,
//! the act on the car) on a car that is a point moving as its stick says,
//! over a map's navigation graph and floor plan.

use std::sync::Arc;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt_bevy::prelude::*;
use tilerider::ai::trees::{
    SIMPLE_SCAN_DISTANCE, attacker_ai, attacker_ai_ver2, move_in_one_direction_ai, panic_behaviour, simple_attack_nearest_enemy, simple_follow_by_enemy_ai,
};
use tilerider::ai::{AiBlackboard, Body, Foe, Gun, Navigation, Orders, tick_mode};
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;
use tilerider::weapon::model::{FireRange, MINIGUN};

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives the point car, units a second.
const SPEED: f32 = 6.0;
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

/// Two rooms joined by a corridor.
const TWO_ROOMS: &str = "\n***********\n*....*....*\n*....*....*\n*.........*\n*....*....*\n*....*....*\n***********\n";
const LONG_ROOM: &str = "\n************************\n*......................*\n************************\n";
const OPEN_ROOM: &str = "\n***********\n*.........*\n*.........*\n*.........*\n*.........*\n*.........*\n***********\n";

struct World {
    grid: MapGrid,
    nav: Navigation,
    terrain: Arc<Terrain>,
}

impl World {
    fn new(picture: &str) -> World {
        let grid = grid(&scaled(picture));
        let nav = Navigation::build(&grid, tilerider::units::CellSize(1.0));
        let terrain = Arc::new(Terrain::from_grid(&grid, 1.0));
        World { grid, nav, terrain }
    }
}

/// A car and its mind.
struct Car<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    heading: f32,
    stick: Vec2,
    others: Vec<Foe>,
    world: World,
    /// Whether it has a gun, and where it points (world radians; by default
    /// at the first of `others`).
    gun: bool,
    gun_direction: Option<f32>,
    immediate: bool,
    health: i32,
    max_health: i32,
    /// The orders of each step.
    orders: Vec<Orders>,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Car<B> {
    fn new(builder: B, world: World, pos: Vec2) -> Self {
        let mut bb = AiBlackboard::new(7);
        bb.nav = Some(world.nav.clone());
        bb.terrain = Some(world.terrain.clone());
        Car {
            bb,
            behavior: Behavior::for_tree(builder),
            tree: BehaviorTree::new(&builder, tick_mode),
            act: None,
            pos,
            heading: 0.0,
            stick: Vec2::ZERO,
            others: Vec::new(),
            world,
            gun: true,
            gun_direction: None,
            immediate: true,
            health: 12,
            max_health: 12,
            orders: Vec::new(),
        }
    }

    fn path(&self) -> String {
        self.behavior.describe(self.tree.get()).to_string()
    }

    fn step(&mut self) -> Orders {
        let walls = &self.world.grid;
        let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let gun = self.gun.then(|| {
            let toward = self.others.first().map_or(Vec2::X, |f| f.pos - self.pos);
            Gun {
                complex: true,
                max_angle: 1.0,
                range: FireRange { from: self.pos, direction: self.gun_direction.unwrap_or(toward.to_angle()), range: MINIGUN.range, dispersion: MINIGUN.dispersion },
                immediate: self.immediate,
            }
        });
        let body = Body {
            pos: self.pos,
            heading: self.heading,
            team: 1,
            gun,
            velocity: self.stick * SPEED,
            radius: 1.0,
            can_move: true,
            stick: self.stick,
            health: self.health,
            max_health: self.max_health,
            shield: 0,
        };
        let mut cars = self.others.clone();
        cars.push(Foe {
            entity: id(99),
            team: 1,
            pos: self.pos,
            vel: Vec2::ZERO,
            radius: 1.0,
            weapon_range: Some(MINIGUN.range),
        });
        self.bb.gather(DT, self.act.is_some(), body, &cars, &blocked);
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
        let solid = |p: Vec2| self.world.grid.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
        for axis in [Vec2::X, Vec2::Y] {
            let to = self.pos + step * axis;
            if !solid(to) {
                self.pos = to;
            }
        }
        self.orders.push(orders);
        orders
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }
}

fn enemy(n: u32, pos: Vec2) -> Foe {
    Foe { entity: id(n), team: 0, pos, vel: Vec2::ZERO, radius: 1.0, weapon_range: Some(MINIGUN.range) }
}

fn shots(orders: &[Orders]) -> usize {
    orders.iter().filter(|o| o.fire).count()
}

#[test]
fn it_stands_aims_and_fires_at_an_enemy_in_sight_and_range_then_moves_to_another_place() {
    let mut car = Car::new(attacker_ai_ver2, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.others = vec![enemy(1, at(6.0, 2.0))];
    car.run(0.6);
    // It fires at where the target is, the car standing (the stop let the
    // stick go) with its gun on it.
    assert_eq!(car.bb.target(), Some(id(1)));
    assert_eq!(car.stick, Vec2::ZERO, "{}", car.path());
    car.run(2.0);
    assert!(shots(&car.orders) > 30, "{} shots: {}", shots(&car.orders), car.path());
    assert!(car.orders.iter().filter_map(|o| o.aim_dir).all(|a| a.x > 0.99), "aimed at it");
    // It goes on in rounds: stand and aim (half a second), fire (a second
    // and a half), manoeuvre to a new place in sight of where the target was
    // (the stick moving it), again.
    car.run(12.0);
    assert!(car.orders.iter().any(|o| o.move_dir.is_some_and(|m| m != Vec2::ZERO)), "{}", car.path());
    assert!(car.pos.distance(at(2.0, 2.0)) > 5.0, "it moved: {}", car.pos);
    let mut rounds = 0;
    for pair in car.orders.windows(2) {
        rounds += usize::from(pair[0].fire && !pair[1].fire);
    }
    assert!(rounds >= 3, "{rounds} rounds of fire");
}

#[test]
fn a_gun_not_on_target_makes_the_car_turn_before_it_fires() {
    let target = at(6.0, 2.0);
    // Facing straight away from the target: a unit stick a quarter turn to
    // the side (`RotateLeft90` of C#, a right turn on a y-up plane) swings it.
    let mut car = Car::new(attacker_ai_ver2, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.others = vec![enemy(1, target)];
    car.heading = std::f32::consts::PI;
    car.gun_direction = Some(std::f32::consts::PI);
    car.run(0.6);
    assert!(car.path().contains("AimToTarget"), "{}", car.path());
    let turning: Vec<&Orders> = car.orders.iter().filter(|o| o.move_dir.is_some()).collect();
    assert!(!turning.is_empty());
    // (The point car then drives round, so only the first order is exact.)
    assert_eq!(turning[0].move_dir, Some(Vec2::new(0.0, -1.0)));
    assert_eq!(turning[0].aim_dir, Some(Vec2::X));
    assert_eq!(shots(&car.orders), 0, "no fire while the gun is off");

    // Facing across it (a quarter turn off): the stick at a quarter of the
    // way to the target.
    let mut car = Car::new(attacker_ai_ver2, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.others = vec![enemy(1, target)];
    car.heading = std::f32::consts::FRAC_PI_2;
    car.gun_direction = Some(std::f32::consts::FRAC_PI_2);
    car.run(0.6);
    let turning: Vec<&Orders> = car.orders.iter().filter(|o| o.move_dir.is_some()).collect();
    assert!(!turning.is_empty());
    assert_eq!(turning[0].move_dir, Some(Vec2::new(0.25, 0.0)));

    // Within 60 degrees the car is left as it is: no stick written at all.
    let mut car = Car::new(attacker_ai_ver2, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.others = vec![enemy(1, target)];
    car.gun_direction = Some(1.0);
    car.run(0.6);
    assert!(car.path().contains("AimToTarget"), "{}", car.path());
    assert!(car.orders.iter().all(|o| o.move_dir.is_none()));
}

#[test]
fn a_target_beyond_the_guns_range_is_followed_to_where_it_was_last_seen() {
    let mut car = Car::new(attacker_ai_ver2, World::new(LONG_ROOM), at(2.0, 1.0));
    car.bb.scan_distance = 100.0;
    // In sight along the room, 45 units away: more than the Minigun's 32.
    car.others = vec![enemy(1, at(17.0, 1.0))];
    car.run(2.0);
    assert!(car.bb.sees_target());
    assert!(car.path().ends_with("Follow (action)"), "{}", car.path());
    assert_eq!(shots(&car.orders), 0);
    assert!(car.stick.x > 0.5, "on its way: {}", car.stick);
    // Close enough, it attacks.
    car.run(8.0);
    assert!(car.pos.x > 12.0, "{}", car.pos);
    assert!(shots(&car.orders) > 0, "{}", car.path());
}

#[test]
fn hurt_below_half_health_it_hides_from_the_enemy() {
    let mut car = Car::new(attacker_ai_ver2, World::new(TWO_ROOMS), at(1.0, 1.0));
    car.others = vec![enemy(1, at(3.0, 3.0))];
    car.run(0.5);
    assert!(!car.path().contains("Panic"), "healthy: {}", car.path());
    // Hit: under half its health, and it lost some since it last looked.
    car.health = 5;
    car.run(0.5);
    assert!(car.path().contains("Panic") || car.path().contains("Hide"), "{}", car.path());
    // It goes to a place it hides in (`HideMove`): not firing, the stick
    // moving it, and it leaves where it was.
    let before = car.orders.len();
    car.run(1.0);
    assert!(car.path().contains("Hide"), "{}", car.path());
    assert!(car.orders[before..].iter().all(|o| !o.fire));
    car.run(4.0);
    assert!(car.pos.distance(at(1.0, 1.0)) > 3.0, "{}", car.pos);
}

#[test]
fn healthy_it_does_not_hide_whatever_hurt_it() {
    let mut car = Car::new(attacker_ai_ver2, World::new(TWO_ROOMS), at(1.0, 1.0));
    car.others = vec![enemy(1, at(3.0, 3.0))];
    car.run(0.5);
    // Lost a point of twelve: still above half.
    car.health = 11;
    car.run(0.5);
    assert!(!car.path().contains("Panic"), "{}", car.path());
}

#[test]
fn with_nobody_near_it_walks_to_a_random_node_and_rests_two_seconds() {
    let mut car = Car::new(attacker_ai_ver2, World::new(TWO_ROOMS), at(1.0, 1.0));
    car.run(0.4);
    assert!(car.path().contains("Rnd") || car.path().contains("MoveToRandom"), "{}", car.path());
    car.run(4.0);
    assert!(car.pos.distance(at(1.0, 1.0)) > 2.0, "it went somewhere: {}", car.pos);
    assert_eq!(shots(&car.orders), 0);
    // It moves at half the stick (`speedRate` 0.5).
    let strongest = car.orders.iter().filter_map(|o| o.move_dir).map(|m| m.length()).fold(0.0, f32::max);
    assert!(strongest <= 0.5 + 1e-4, "{strongest}");
}

#[test]
fn simple_attack_nearest_enemy_attacks_like_the_turret_and_idles_otherwise() {
    let mut car = Car::new(simple_attack_nearest_enemy, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.bb.scan_distance = SIMPLE_SCAN_DISTANCE;
    car.run(1.0);
    assert_eq!(shots(&car.orders), 0);
    assert!(car.path().contains("Idle"), "{}", car.path());
    car.others = vec![enemy(1, at(6.0, 2.0))];
    car.orders.clear();
    car.run(1.0);
    assert!(shots(&car.orders) > 30, "{}", car.path());
    assert!(car.stick.x > 0.9, "it steers at it: {}", car.stick);
}

#[test]
fn simple_follow_by_enemy_goes_at_the_enemy_and_does_not_fire() {
    let mut car = Car::new(simple_follow_by_enemy_ai, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.bb.scan_distance = SIMPLE_SCAN_DISTANCE;
    car.others = vec![enemy(1, at(6.0, 4.0))];
    car.run(1.0);
    assert_eq!(shots(&car.orders), 0);
    let to = (at(6.0, 4.0) - at(2.0, 2.0)).normalize();
    assert!(car.stick.dot(to) > 0.99, "{} {}", car.stick, car.path());
    assert!(car.orders.iter().filter_map(|o| o.aim_dir).all(|a| a.dot(to) > 0.9), "its gun the same way");
}

#[test]
fn move_and_fire_in_init_direction_keeps_the_stick_and_gun_where_it_faced_and_fires() {
    let mut car = Car::new(move_in_one_direction_ai, World::new(OPEN_ROOM), at(2.0, 4.0));
    car.heading = 0.0;
    car.run(0.5);
    // It faced +x at the start: that stays, whatever it turns to.
    car.heading = 1.0;
    car.run(0.5);
    // From the first step on, the action starting and ticking at once.
    assert!(car.orders.iter().all(|o| o.move_dir == Some(Vec2::X) && o.aim_dir == Some(Vec2::X) && o.fire));
}

#[test]
fn a_chicken_hides_from_an_enemy_in_sight_unhurt() {
    // `ChickenAi` is `PanicBehaviour` alone: no health check.
    assert!(tilerider::ai::is_live("ChickenAi"));
    let mut car = Car::new(panic_behaviour, World::new(TWO_ROOMS), at(1.0, 1.0));
    car.others = vec![enemy(1, at(3.0, 3.0))];
    car.run(1.0);
    assert!(car.path().contains("Hide"), "{}", car.path());
    car.run(4.0);
    assert!(car.pos.distance(at(1.0, 1.0)) > 3.0, "{}", car.pos);
    assert!(car.orders.iter().all(|o| !o.fire));
}

#[test]
fn the_first_attacker_fires_in_rounds_without_aiming_first_or_hiding() {
    assert!(tilerider::ai::is_live("AttackAi") && tilerider::ai::is_live("AttackAiFriend"));
    let mut car = Car::new(attacker_ai, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.others = vec![enemy(1, at(6.0, 2.0))];
    car.run(0.2);
    // No aiming step (`AimToTarget`): it stands and fires at once.
    assert_eq!(car.bb.target(), Some(id(1)));
    assert!(car.path().contains("FireToTarget"), "{}", car.path());
    assert!(!car.path().contains("AimToTarget"), "{}", car.path());
    // Hurt below half, it does not hide (the first version has no panic).
    car.health = 3;
    car.run(14.0);
    assert!(!car.path().contains("Panic") && !car.path().contains("Hide"), "{}", car.path());
    assert!(car.pos.distance(at(2.0, 2.0)) > 5.0, "it manoeuvred: {}", car.pos);
    let mut rounds = 0;
    for pair in car.orders.windows(2) {
        rounds += usize::from(pair[0].fire && !pair[1].fire);
    }
    assert!(rounds >= 3, "{rounds} rounds of fire");
}

#[test]
fn the_first_attacker_with_no_enemy_walks_to_random_places() {
    let mut car = Car::new(attacker_ai, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.run(3.0);
    assert!(car.path().contains("MoveToRandom"), "{}", car.path());
    assert_eq!(shots(&car.orders), 0);
}

#[test]
fn with_a_slow_gun_it_fires_without_the_aiming_turn_then_manoeuvres() {
    // The Plasma gun's shot flies (`IsImmidiate` false): `AttackAiVer2`
    // takes `SmartAttackTargetSlowWeapon`.
    assert!(!tilerider::weapon::model::Kind::Plasma.is_immediate());
    let mut car = Car::new(attacker_ai_ver2, World::new(OPEN_ROOM), at(2.0, 2.0));
    car.immediate = false;
    car.others = vec![enemy(1, at(6.0, 2.0))];
    car.run(0.2);
    assert_eq!(car.bb.target(), Some(id(1)));
    assert!(car.path().contains("SmartAttackTargetSlowWeapon"), "{}", car.path());
    assert!(!car.path().contains("AimToTarget"), "{}", car.path());
    // It stands and fires at once (the immediate gun's half second of
    // aiming is skipped), a second and a half, then moves on.
    assert_eq!(car.stick, Vec2::ZERO);
    assert!(shots(&car.orders) > 5, "{} shots", shots(&car.orders));
    car.run(12.0);
    assert!(car.pos.distance(at(2.0, 2.0)) > 5.0, "it manoeuvred: {}", car.pos);
    let mut rounds = 0;
    for pair in car.orders.windows(2) {
        rounds += usize::from(pair[0].fire && !pair[1].fire);
    }
    assert!(rounds >= 3, "{rounds} rounds of fire");
}
