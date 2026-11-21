//! `HammerAi` (`HammerAi`, `HammerAiFriend`) and its `RemeberTargetPos`, run
//! as the plugin runs them (gather, tick, the act on the car) on a car that
//! is a point moving as its stick says, over a map's navigation graph and
//! floor plan.

use std::sync::Arc;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt_bevy::prelude::*;
use tilerider::ai::trees::hammer_ai;
use tilerider::ai::{AiBlackboard, Body, Foe, Navigation, Orders, is_live, tick_mode};
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;

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

/// The hammer car and its mind.
struct Car<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    stick: Vec2,
    others: Vec<Foe>,
    world: World,
    /// The running path of each step.
    paths: Vec<String>,
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
            stick: Vec2::ZERO,
            others: Vec::new(),
            world,
            paths: Vec::new(),
            orders: Vec::new(),
        }
    }

    fn path(&self) -> String {
        self.behavior.describe(self.tree.get()).to_string()
    }

    fn step(&mut self) -> Orders {
        let walls = &self.world.grid;
        let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let body = Body {
            pos: self.pos,
            heading: 0.0,
            team: 1,
            gun: None,
            velocity: self.stick * SPEED,
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
        let step = self.stick * SPEED * DT;
        let solid = |p: Vec2| self.world.grid.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
        for axis in [Vec2::X, Vec2::Y] {
            let to = self.pos + step * axis;
            if !solid(to) {
                self.pos = to;
            }
        }
        // The enemies go their way.
        for foe in &mut self.others {
            foe.pos += foe.vel * DT;
        }
        self.orders.push(orders);
        self.paths.push(self.path());
        orders
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    /// Runs until the running path contains `what`, at most `secs`; the
    /// time it took.
    fn run_until(&mut self, what: &str, secs: f32) -> Option<f32> {
        for n in 0..(secs / DT).round() as usize {
            self.step();
            if self.path().contains(what) {
                return Some(n as f32 * DT);
            }
        }
        None
    }
}

fn enemy(n: u32, pos: Vec2, vel: Vec2) -> Foe {
    Foe { entity: id(n), team: 0, pos, vel, radius: 1.0, weapon_range: None }
}

#[test]
fn both_hammer_controls_are_live() {
    assert!(is_live("HammerAi") && is_live("HammerAiFriend"));
}

#[test]
fn it_chases_a_second_then_charges_at_where_the_target_was_not_following_it() {
    let mut car = Car::new(hammer_ai, World::new(OPEN_ROOM), at(1.0, 2.0));
    // An enemy across the room, driving down it (y down in the picture).
    car.others = vec![enemy(1, at(5.0, 1.0), Vec2::new(0.0, -1.0))];
    car.run(0.3);
    assert!(car.path().contains("ChaseAI"), "it chases first: {}", car.path());
    assert_eq!(car.bb.target(), Some(id(1)));
    // The chase lasts a second (ended at a think: 1 to 1.25 s).
    let charge = car.run_until("MoveTo_PrevTargetPos", 2.0).expect("it charges");
    assert!((0.6..=1.0).contains(&charge), "the chase ended {charge} s after 0.3 s");
    // The charge goes to where the target was when it started, with no
    // prediction (0 s), and stays there as the target drives on.
    let aimed = car.bb.target_position;
    let target_then = car.others[0].pos;
    assert!(aimed.distance(target_then) < 0.05, "{aimed} vs {target_then}");
    car.run(0.5);
    assert_eq!(car.bb.target_position, aimed, "not following it");
    assert!(car.others[0].pos.distance(aimed) > 0.4);
    // At full stick.
    let strongest = car.orders.iter().rev().take(20).filter_map(|o| o.move_dir).map(|m| m.length()).fold(0.0, f32::max);
    assert!(strongest > 0.9, "{strongest}");
}

#[test]
fn after_the_charge_it_runs_up_at_half_speed_to_a_random_place_in_sight_then_again() {
    let mut car = Car::new(hammer_ai, World::new(OPEN_ROOM), at(1.0, 2.0));
    car.others = vec![enemy(1, at(5.0, 3.0), Vec2::ZERO)];
    let at_run_up = car.run_until("MoveTo_Rnd", 6.0).expect("it runs up");
    assert!(car.path().contains("AttackHammer"), "{}", car.path());
    let start = car.orders.len();
    // The run-up lasts a second, at half the stick at most.
    let again = car.run_until("ChaseAI", 2.0).expect("and attacks again");
    assert!((0.7..=1.3).contains(&again), "the run-up lasted {again} s (from {at_run_up} s)");
    let strongest = car.orders[start..car.orders.len() - 1]
        .iter()
        .filter_map(|o| o.move_dir)
        .map(|m| m.length())
        .fold(0.0, f32::max);
    assert!(strongest <= 0.5 + 1e-4, "{strongest}");
    // Never the idle branch while the target lives.
    assert!(car.paths.iter().all(|p| !p.contains("Hammer Ai (select) > Idle")));
}

#[test]
fn a_charge_cut_short_by_the_target_dying_goes_to_the_idle_branch() {
    let mut car = Car::new(hammer_ai, World::new(OPEN_ROOM), at(1.0, 2.0));
    car.others = vec![enemy(1, at(5.0, 3.0), Vec2::ZERO)];
    car.run_until("MoveTo_PrevTargetPos", 2.0).expect("it charges");
    // Gone: `CheckTarget` fails once the charge is over.
    car.others.clear();
    car.run_until("MoveTo_RandomPos", 6.0).expect("it wanders");
    assert!(car.paths.iter().all(|p| !p.contains("MoveTo_Rnd")), "no run-up");
}

#[test]
fn with_nobody_it_sleeps_then_wanders_at_half_speed_to_places_in_sight() {
    let mut car = Car::new(hammer_ai, World::new(OPEN_ROOM), at(1.0, 2.0));
    car.run(0.1);
    // `Chase` finds nobody and sleeps (`Sleep2Sec`) inside the timer.
    assert!(car.path().contains("ChaseAI (select) > Sleep"), "{}", car.path());
    // At the next think, 2.25 s on, the timer is over, `RemberPos` fails
    // with no target and `Idle` sleeps and starts the wander at once.
    let wander = car.run_until("MoveTo_RandomPos", 4.0).expect("it wanders");
    assert!((2.0..=2.4).contains(&wander), "{wander}");
    assert!(car.orders.iter().take(120).all(|o| o.move_dir.is_none_or(|m| m == Vec2::ZERO)), "still until then");
    car.run(1.5);
    assert!(car.pos.distance(at(1.0, 2.0)) > 0.5, "it went somewhere: {}", car.pos);
    let strongest = car.orders.iter().filter_map(|o| o.move_dir).map(|m| m.length()).fold(0.0, f32::max);
    assert!(strongest <= 0.5 + 1e-4, "{strongest}");
}

#[test]
fn an_enemy_beyond_the_scan_distance_is_not_chased() {
    // `ScanDistance` stays `AIBTBaseControl`'s 16.
    let mut car = Car::new(hammer_ai, World::new("\n************************\n*......................*\n************************\n"), at(1.0, 1.0));
    assert_eq!(car.bb.scan_distance, 16.0);
    car.others = vec![enemy(1, at(20.0, 1.0), Vec2::ZERO)];
    car.run(3.0);
    assert_eq!(car.bb.target(), None, "{}", car.path());
}

#[test]
fn the_charge_point_is_the_predicted_place_moved_out_of_walls() {
    let world = World::new(OPEN_ROOM);
    let mut bb = AiBlackboard::new(1);
    bb.terrain = Some(world.terrain.clone());
    let walls = &world.grid;
    let blocked = |a: Vec2, b: Vec2| walls.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
    let body = Body {
        pos: at(1.0, 2.0),
        heading: 0.0,
        team: 1,
        gun: None,
        velocity: Vec2::ZERO,
        radius: 1.0,
        can_move: true,
        stick: Vec2::ZERO,
        health: 12,
        max_health: 12,
        shield: 0,
    };
    // `GetPredictionPosition`: where it will be in a second.
    let cars = [enemy(1, at(5.0, 2.0), Vec2::new(2.0, 0.0))];
    bb.gather(DT, false, body, &cars, &blocked);
    bb.set_target(id(1));
    let t = *bb.live_target().unwrap();
    assert_eq!(bb.prediction_position(t.pos, t.vel, 1.0), at(5.0, 2.0) + Vec2::new(2.0, 0.0));
    // A prediction in a wall is moved to a free place.
    let cars = [enemy(1, at(5.0, 2.0), Vec2::new(0.0, 20.0))];
    bb.gather(DT, false, body, &cars, &blocked);
    let t = *bb.live_target().unwrap();
    let p = bb.prediction_position(t.pos, t.vel, 1.0);
    assert!(world.terrain.is_point_free(p), "{p}");
}
