//! The turret's mind (`ai::turret`): `BTActionFactory.TurretAi` as the
//! C# game ran it, without the app.

use std::f32::consts::PI;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use tilerider::ai::turret::{
    DAMAGER_MEMORY, Foe, Gun, IDLE_TIME, Orders, Phase, SCAN_DISTANCE, SCAN_TIME, THINK_PERIOD, Scene, Turret,
};
use tilerider::weapon::model::{MINIGUN, target_in_fire_range};

const DT: f32 = 1.0 / 60.0;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

/// A car of team 0 (the player's) at `(x, y)`, standing still.
fn foe(n: u32, x: f32, y: f32) -> Foe {
    Foe {
        entity: id(n),
        team: 0,
        pos: Vec2::new(x, y),
        vel: Vec2::ZERO,
        radius: 1.0,
    }
}

/// Everything a turret at the origin facing right sees, and the gun that
/// points where `gun` says (a Minigun's numbers, as `TargetInFireRange`).
struct Setup {
    cars: Vec<Foe>,
    /// A wall between the turret and anything beyond this x.
    wall_at: Option<f32>,
    /// The gun's barrel, world radians; `None`: no gun.
    gun: Option<f32>,
    max_angle: f32,
    heading: f32,
}

impl Setup {
    fn new(cars: Vec<Foe>) -> Setup {
        Setup {
            cars,
            wall_at: None,
            gun: Some(0.0),
            max_angle: PI / 3.0,
            heading: 0.0,
        }
    }

    fn step(&self, turret: &mut Turret) -> Orders {
        let barrel = self.gun.unwrap_or(0.0);
        let in_range = move |target: Vec2, radius: f32| {
            target_in_fire_range(Vec2::ZERO, barrel, MINIGUN.range, MINIGUN.dispersion, target, radius)
        };
        let wall = self.wall_at;
        let blocked = move |a: Vec2, b: Vec2| wall.is_some_and(|w| (a.x < w) != (b.x < w));
        let scene = Scene {
            pos: Vec2::ZERO,
            heading: self.heading,
            team: 1,
            gun: self.gun.map(|_| Gun {
                complex: true,
                max_angle: self.max_angle,
                in_range: &in_range,
            }),
            cars: &self.cars,
            blocked: &blocked,
        };
        turret.step(DT, &scene)
    }

    /// Runs `secs` seconds, collecting the orders of every step.
    fn run(&self, turret: &mut Turret, secs: f32) -> Vec<Orders> {
        (0..(secs / DT).round() as usize).map(|_| self.step(turret)).collect()
    }
}

fn fired(orders: &[Orders]) -> usize {
    orders.iter().filter(|o| o.fire).count()
}

#[test]
fn it_attacks_an_enemy_in_sight_within_its_scan_distance() {
    let setup = Setup::new(vec![foe(1, 10.0, 0.0)]);
    let mut turret = Turret::new(1);
    // The next think is at most a quarter second away.
    setup.run(&mut turret, THINK_PERIOD + 0.1);
    assert_eq!(turret.phase(), Phase::Attack);
    assert_eq!(turret.target(), Some(id(1)));
    let orders = setup.run(&mut turret, 1.0);
    // The gun points at it, so the trigger is pulled every step, aimed at it.
    assert_eq!(fired(&orders), orders.len());
    assert!(orders.iter().all(|o| o.aim_dir == Some(Vec2::X)));
}

#[test]
fn it_aims_ahead_of_a_moving_target_and_fires_only_where_the_gun_points() {
    let mut runner = foe(1, 10.0, 0.0);
    runner.vel = Vec2::new(0.0, 10.0);
    let mut setup = Setup::new(vec![runner]);
    let mut turret = Turret::new(2);
    setup.run(&mut turret, 0.5);
    let o = setup.step(&mut turret);
    // 0.2 s ahead: two units up the way it runs.
    let want = Vec2::new(10.0, 2.0).normalize();
    assert!((o.aim_dir.unwrap() - want).length() < 1e-5, "{:?}", o.aim_dir);
    assert_eq!(o.move_dir, o.aim_dir, "the stick follows the aim");
    // The gun still points away from the target (it takes time to turn):
    // no shot, though it aims.
    setup.gun = Some(1.0);
    let o = setup.step(&mut turret);
    assert!(!o.fire && o.aim_dir.is_some());
}

#[test]
fn fire_accuracy_widens_what_counts_as_the_line() {
    // 3 units off the barrel's line at 10 away: a radius of 1 is not
    // enough, a fire accuracy of 4 is.
    let setup = Setup::new(vec![foe(1, 10.0, 3.0)]);
    let mut precise = Turret::new(3);
    let o = setup.run(&mut precise, 1.0);
    assert_eq!(fired(&o), 0);
    let mut sloppy = Turret::new(3);
    sloppy.fire_accuracy = 4.0;
    let o = setup.run(&mut sloppy, 1.0);
    assert!(fired(&o) > 0);
}

#[test]
fn friends_and_far_enemies_and_those_behind_walls_are_not_attacked() {
    let mut friend = foe(1, 5.0, 0.0);
    friend.team = 1;
    let far = foe(2, SCAN_DISTANCE + 1.0, 0.0);
    // Within the scan distance on both axes is what counts: 15 and 15 is
    // near, though it is further than 16 on the diagonal.
    let setup = Setup::new(vec![friend, far]);
    let mut turret = Turret::new(4);
    let o = setup.run(&mut turret, 2.0);
    assert_eq!(fired(&o), 0);
    assert_ne!(turret.phase(), Phase::Attack);

    let mut walled = Setup::new(vec![foe(3, 10.0, 0.0)]);
    walled.wall_at = Some(5.0);
    let mut turret = Turret::new(4);
    assert_eq!(fired(&walled.run(&mut turret, 2.0)), 0);
    // The wall gone, it is attacked within a think.
    walled.wall_at = None;
    walled.run(&mut turret, THINK_PERIOD + 0.1);
    assert_eq!(turret.phase(), Phase::Attack);
}

#[test]
fn chebyshev_distance_decides_who_is_found_not_the_straight_line() {
    // 15.5 away on both axes: 21.9 as the crow flies, but within the scan
    // distance on both axes, which is what `GetEnemies` asks.
    let setup = Setup::new(vec![foe(1, 15.5, 15.5)]);
    let mut turret = Turret::new(5);
    setup.run(&mut turret, 0.6);
    assert_eq!(turret.phase(), Phase::Attack);
    assert_eq!(turret.target(), Some(id(1)));
}

#[test]
fn it_keeps_its_target_and_does_not_leave_for_a_nearer_one() {
    let mut setup = Setup::new(vec![foe(1, 12.0, 0.0)]);
    let mut turret = Turret::new(6);
    setup.run(&mut turret, 0.6);
    assert_eq!(turret.target(), Some(id(1)));
    // A nearer enemy appears: the running attack goes on with the old one.
    setup.cars.push(foe(2, 4.0, 0.0));
    setup.run(&mut turret, 2.0);
    assert_eq!(turret.target(), Some(id(1)));
    // When the target dies it picks the other.
    setup.cars.retain(|f| f.entity != id(1));
    setup.run(&mut turret, 1.0);
    assert_eq!(turret.target(), Some(id(2)));
    assert_eq!(turret.phase(), Phase::Attack);
}

#[test]
fn a_target_out_of_sight_is_swapped_for_one_in_sight_else_scanning() {
    let mut setup = Setup::new(vec![foe(1, 12.0, 0.0), foe(2, 6.0, 0.0)]);
    setup.wall_at = Some(8.0);
    let mut turret = Turret::new(7);
    setup.run(&mut turret, 0.6);
    // The near one is in front of the wall.
    assert_eq!(turret.target(), Some(id(2)));
    assert_eq!(turret.phase(), Phase::Attack);
    // It leaves: the other is behind the wall, so it scans.
    setup.cars.retain(|f| f.entity != id(2));
    setup.run(&mut turret, 1.0);
    assert!(matches!(turret.phase(), Phase::Idle | Phase::Scanning | Phase::Done));
    assert_eq!(fired(&setup.run(&mut turret, 3.0)), 0);
}

#[test]
fn it_hits_back_at_whoever_hurt_it_however_far_and_for_five_seconds() {
    // 25 units off: out of its scan distance of 16.
    let setup = Setup::new(vec![foe(1, 25.0, 0.0)]);
    let mut turret = Turret::new(8);
    assert_eq!(fired(&setup.run(&mut turret, 2.0)), 0);
    turret.hurt_by(id(1));
    setup.run(&mut turret, THINK_PERIOD + 0.1);
    assert_eq!(turret.phase(), Phase::Attack);
    // Forgotten after five seconds: the next think after that drops it
    // (the target then is out of range of its own scan).
    setup.run(&mut turret, DAMAGER_MEMORY);
    // It stays on its target while it sees it; a fresh look finds nobody
    // once the target is gone.
    let mut gone = setup;
    gone.cars.clear();
    gone.run(&mut turret, 1.0);
    assert_ne!(turret.phase(), Phase::Attack);
}

#[test]
fn it_ignores_a_damager_of_its_own_team() {
    let mut friend = foe(1, 25.0, 0.0);
    friend.team = 1;
    let setup = Setup::new(vec![friend]);
    let mut turret = Turret::new(9);
    turret.hurt_by(id(1));
    assert_eq!(fired(&setup.run(&mut turret, 2.0)), 0);
    assert_ne!(turret.phase(), Phase::Attack);
}

#[test]
fn scanning_stands_a_second_then_turns_the_gun_within_the_sweep_for_a_second() {
    let setup = Setup::new(vec![]);
    let mut turret = Turret::new(10);
    // Starts at once (nothing running): a second of standing, no orders.
    let idle = setup.run(&mut turret, IDLE_TIME - 0.1);
    assert!(idle.iter().all(|o| *o == Orders::default()));
    assert_eq!(turret.phase(), Phase::Idle);
    // Then a second of aiming at one direction.
    let scan = setup.run(&mut turret, SCAN_TIME + 0.3);
    let aims: Vec<Vec2> = scan.iter().filter_map(|o| o.aim_dir).collect();
    assert!(aims.len() >= 55, "{} aim orders", aims.len());
    let first = aims[0];
    assert!(aims.iter().all(|a| (*a - first).length() < 1e-4), "one direction");
    assert!((first.length() - 1.0).abs() < 1e-4);
    assert!(first.to_angle().abs() <= 1.1 * PI / 3.0 + 1e-4, "within 110 % of MaxAngle");
    assert!(scan.iter().all(|o| !o.fire && o.move_dir.is_none()));
}

#[test]
fn scans_pick_different_directions_and_follow_the_cars_heading() {
    let mut setup = Setup::new(vec![]);
    setup.heading = PI / 2.0;
    let mut turret = Turret::new(11);
    let mut seen = Vec::new();
    for _ in 0..8 {
        let orders = setup.run(&mut turret, IDLE_TIME + SCAN_TIME + 0.2);
        let aim = orders.iter().rev().find_map(|o| o.aim_dir).expect("it aimed");
        seen.push(aim.to_angle());
    }
    assert!(seen.iter().all(|a| (a - PI / 2.0).abs() <= 1.1 * PI / 3.0 + 1e-3), "{seen:?}");
    let spread = seen.iter().cloned().fold(f32::MIN, f32::max) - seen.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread > 0.3, "directions differ: {seen:?}");
}

#[test]
fn a_narrow_gun_scans_a_narrow_sweep() {
    let mut setup = Setup::new(vec![]);
    setup.max_angle = 5.0_f32.to_radians();
    let mut turret = Turret::new(12);
    for _ in 0..6 {
        let orders = setup.run(&mut turret, IDLE_TIME + SCAN_TIME + 0.2);
        let aim = orders.iter().rev().find_map(|o| o.aim_dir).expect("it aimed");
        assert!(aim.to_angle().abs() <= 1.1 * 5.0_f32.to_radians() + 1e-4);
    }
}

#[test]
fn an_enemy_appearing_while_scanning_is_taken_up_within_a_think() {
    let mut setup = Setup::new(vec![]);
    let mut turret = Turret::new(13);
    setup.run(&mut turret, 1.5);
    assert_eq!(turret.phase(), Phase::Scanning);
    setup.cars.push(foe(1, 8.0, 0.0));
    setup.run(&mut turret, THINK_PERIOD + 0.1);
    assert_eq!(turret.phase(), Phase::Attack);
}

#[test]
fn a_turret_with_no_gun_never_attacks() {
    let mut setup = Setup::new(vec![foe(1, 5.0, 0.0)]);
    setup.gun = None;
    let mut turret = Turret::new(14);
    let orders = setup.run(&mut turret, 3.0);
    assert_ne!(turret.phase(), Phase::Attack);
    assert!(orders.iter().all(|o| !o.fire && o.move_dir.is_none()));
}

#[test]
fn the_stick_and_aim_are_left_alone_when_the_attack_ends() {
    let mut setup = Setup::new(vec![foe(1, 8.0, 0.0)]);
    let mut turret = Turret::new(15);
    setup.run(&mut turret, 0.6);
    assert_eq!(turret.phase(), Phase::Attack);
    setup.cars.clear();
    // From then on it writes no stick at all (the car keeps the last one).
    let orders = setup.run(&mut turret, 1.0);
    assert!(orders.iter().all(|o| o.move_dir.is_none() && !o.fire));
}
