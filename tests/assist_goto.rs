//! The Smart scheme's driving hand: a double tap on bare ground is `GoTo`,
//! and the assist drives the car to the point along a path from the
//! navigation (a real graph on a small map), stopping there; any manual
//! drive input, a refusal, a stuck car and the time-out end it.

use std::sync::Arc;
use std::time::Duration;

use avian2d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::ai::{Navigation, SharedTerrain};
use tilerider::assist::goto::{Finish, Going, REPLAN_SECS, Scene, STUCK_SECS, TIMEOUT_SECS, step};
use tilerider::assist::tap::{DOUBLE_TAP_RADIUS, DOUBLE_TAP_SECS, LastTap, is_double_tap};
use tilerider::assist::{Assist, AssistPlugin, AssistSystems, Commanded, PlayerCommand, Tapped};
use tilerider::car::CarRadius;
use tilerider::car::control::{AutoAimVisible, Team};
use tilerider::content::map::MapData;
use tilerider::controls::{LocalPlayer, NoInput, PlayerIntent};
use tilerider::explosion::Health;
use tilerider::map::grid::MapGrid;
use tilerider::map::nav::{NavGraph, NavPath};
use tilerider::map::terrain::Terrain;
use tilerider::settings::{ControlScheme, Settings};

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives the test car, units a second.
const SPEED: f32 = 6.0;

// ---- double tap (plain numbers) ----

#[test]
fn a_second_tap_near_and_soon_after_the_first_is_a_double_tap() {
    let first = Some(LastTap { ground: Vec2::new(5.0, 5.0), at: 10.0 });
    assert!(is_double_tap(first, Vec2::new(5.3, 5.2), 10.2));
    assert!(is_double_tap(first, Vec2::new(5.0, 5.0), 10.0 + DOUBLE_TAP_SECS), "the limit itself counts");
    assert!(is_double_tap(first, Vec2::new(5.0 + DOUBLE_TAP_RADIUS, 5.0), 10.1), "so does the radius");
}

#[test]
fn a_tap_that_is_late_far_or_alone_is_no_double_tap() {
    let first = Some(LastTap { ground: Vec2::new(5.0, 5.0), at: 10.0 });
    assert!(!is_double_tap(first, Vec2::new(5.0, 5.0), 10.0 + DOUBLE_TAP_SECS + 0.01), "too slow");
    assert!(!is_double_tap(first, Vec2::new(5.0 + DOUBLE_TAP_RADIUS + 0.1, 5.0), 10.1), "too far");
    assert!(!is_double_tap(first, Vec2::new(5.0, 5.0), 9.9), "not before the first");
    assert!(!is_double_tap(None, Vec2::new(5.0, 5.0), 10.1), "no first tap");
}

// ---- the job (plain numbers) ----

fn scene(pos: Vec2, manual: bool) -> Scene {
    Scene { pos, velocity: Vec2::ZERO, radius: 1.0, dt: DT, manual }
}

fn straight(from: Vec2, to: Vec2) -> Option<NavPath> {
    Some(NavPath::new(vec![from, to], false))
}

#[test]
fn a_job_steers_at_the_point_and_stops_when_the_car_is_on_it() {
    let mut job = Going::new(Vec2::new(10.0, 0.0));
    let out = step(&mut job, &scene(Vec2::ZERO, false), &mut straight);
    let stick = out.stick.expect("driving");
    assert!(stick.x > 0.9 && stick.y.abs() < 0.01, "{stick:?}");
    assert_eq!(out.finished, None);
    let out = step(&mut job, &scene(Vec2::new(9.5, 0.0), false), &mut straight);
    assert_eq!((out.stick, out.finished), (None, Some(Finish::Arrived)));
}

#[test]
fn a_point_under_the_car_is_arrived_at_once_with_no_planning() {
    let mut job = Going::new(Vec2::new(0.4, 0.0));
    let mut asked = 0;
    let out = step(&mut job, &scene(Vec2::ZERO, false), &mut |a, b| {
        asked += 1;
        straight(a, b)
    });
    assert_eq!(out.finished, Some(Finish::Arrived));
    assert_eq!(asked, 0);
}

#[test]
fn a_manual_stick_ends_the_job_at_once() {
    let mut job = Going::new(Vec2::new(10.0, 0.0));
    step(&mut job, &scene(Vec2::ZERO, false), &mut straight);
    let out = step(&mut job, &scene(Vec2::ZERO, true), &mut straight);
    assert_eq!((out.stick, out.finished), (None, Some(Finish::Manual)));
}

#[test]
fn no_path_refuses_the_job() {
    let mut job = Going::new(Vec2::new(10.0, 0.0));
    let out = step(&mut job, &scene(Vec2::ZERO, false), &mut |_, _| None);
    assert_eq!((out.stick, out.finished), (None, Some(Finish::NoPath)));
}

/// A car that keeps moving, so that it is never stuck.
fn moving(frame: usize) -> Scene {
    scene(Vec2::new(frame as f32 * 0.01, 0.0), false)
}

#[test]
fn the_path_is_made_again_from_the_car_every_few_seconds_and_a_failed_replan_keeps_the_old() {
    let mut job = Going::new(Vec2::new(1000.0, 0.0));
    let mut asked = Vec::new();
    let mut fail_from = usize::MAX;
    let mut frame = 0;
    for _ in 0..((REPLAN_SECS * 2.5 / DT) as usize) {
        let s = moving(frame);
        let out = step(&mut job, &s, &mut |a, b| {
            asked.push(a);
            if frame >= fail_from { None } else { straight(a, b) }
        });
        assert_eq!(out.finished, None);
        assert!(out.stick.is_some_and(|v| v.x > 0.9));
        frame += 1;
        if asked.len() == 2 {
            fail_from = frame;
        }
    }
    assert_eq!(asked.len(), 3, "at the start, then every {REPLAN_SECS} s: {asked:?}");
    assert!(asked[1].x > asked[0].x, "from where the car was by then");
}

#[test]
fn a_car_that_does_not_move_is_replanned_once_and_then_given_up() {
    let mut job = Going::new(Vec2::new(1000.0, 0.0));
    let mut asked = 0;
    let mut ended = None;
    let mut secs = 0.0;
    while ended.is_none() && secs < STUCK_SECS * 4.0 {
        ended = step(&mut job, &scene(Vec2::ZERO, false), &mut |a, b| {
            asked += 1;
            straight(a, b)
        })
        .finished;
        secs += DT;
    }
    assert_eq!(ended, Some(Finish::Stuck));
    assert!((secs - STUCK_SECS * 2.0).abs() < 0.1, "after two checks: {secs}");
    assert_eq!(asked, 2, "the first plan and the re-plan of the first check");
}

#[test]
fn a_job_that_never_arrives_times_out() {
    let mut job = Going::new(Vec2::new(1.0e6, 0.0));
    let mut ended = None;
    let mut frame = 0;
    while ended.is_none() && frame < ((TIMEOUT_SECS + 5.0) / DT) as usize {
        ended = step(&mut job, &moving(frame), &mut straight).finished;
        frame += 1;
    }
    assert_eq!(ended, Some(Finish::TimedOut));
    assert!((frame as f32 * DT - TIMEOUT_SECS).abs() < 0.1);
}

// ---- driving in an app, on a map with a real navigation graph ----

/// Two rooms with a gap in the wall between them (row 3).
const TWO_ROOMS: &str = "\n***********\n*....*....*\n*....*....*\n*.........*\n*....*....*\n*....*....*\n***********\n";
/// The same with no gap: the right room cannot be reached.
const SEALED: &str = "\n***********\n*....*....*\n*....*....*\n*....*....*\n*....*....*\n*....*....*\n***********\n";
/// How many map cells a character of the pictures is.
const SCALE: usize = 3;

fn grid(rows: &str) -> MapGrid {
    let mut big = String::from("\n");
    for row in rows.trim().lines() {
        let wide: String = row.chars().flat_map(|c| std::iter::repeat_n(c, SCALE)).collect();
        for _ in 0..SCALE {
            big.push_str(&wide);
            big.push('\n');
        }
    }
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{big}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// The middle of a picture's character, in the world (y up).
fn at(x: f32, y: f32) -> Vec2 {
    let k = SCALE as f32;
    Vec2::new((x + 0.5) * k, -(y + 0.5) * k)
}

/// The walls, for the test's car to bump into.
#[derive(Resource)]
struct Walls(MapGrid);

/// What a hand on the stick pushes this frame (nothing, unless a test says).
#[derive(Resource, Default)]
struct Pushed(Vec2);

/// A frame as the devices leave it: the stick is what a hand pushes.
fn devices(pushed: Res<Pushed>, mut cars: Query<&mut PlayerIntent>) {
    for mut intent in &mut cars {
        intent.move_dir = pushed.0;
    }
}

/// The car drives as its stick says, a wall stopping it along an axis.
fn physics(walls: Res<Walls>, mut cars: Query<(&PlayerIntent, &mut Position, &mut LinearVelocity)>) {
    for (intent, mut at, mut velocity) in &mut cars {
        velocity.0 = intent.move_dir * SPEED;
        let step = velocity.0 * DT;
        let solid = |p: Vec2| walls.0.is_wall(p.x.floor() as i32, (-p.y).floor() as i32);
        for axis in [Vec2::X, Vec2::Y] {
            let to = at.0 + step * axis;
            if !solid(to) {
                at.0 = to;
            }
        }
    }
}

fn smart_settings(tap_to_go: bool) -> Settings {
    let mut s = Settings::default();
    s.control_scheme = ControlScheme::Smart;
    s.tap_to_go = tap_to_go;
    s
}

/// A Smart app on a map; `nav`: whether it has the navigation graph.
fn map_app(rows: &str, nav: bool) -> App {
    let walls = grid(rows);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(smart_settings(true))
        .insert_resource(Pushed::default())
        .insert_resource(SharedTerrain(Arc::new(Terrain::from_grid(&walls, 1.0))))
        .add_plugins(AssistPlugin)
        .add_systems(PreUpdate, devices.before(AssistSystems))
        .add_systems(Update, physics);
    if nav {
        app.insert_resource(Navigation::new(NavGraph::from_grid(&walls, 1.0)));
    }
    app.insert_resource(Walls(walls));
    app.update();
    app
}

fn player(app: &mut App, pos: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            LocalPlayer,
            PlayerIntent::default(),
            Position(pos),
            LinearVelocity::default(),
            Team(0),
            Health::full(100),
            AutoAimVisible(true),
            CarRadius(1.0),
            Assist::default(),
        ))
        .id()
}

fn command(app: &mut App, car: Entity, command: PlayerCommand) {
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command });
}

fn assist(app: &App, car: Entity) -> Assist {
    app.world().get::<Assist>(car).unwrap().clone()
}

fn pos(app: &App, car: Entity) -> Vec2 {
    app.world().get::<Position>(car).unwrap().0
}

/// Runs for `secs`; the car's places on the way.
fn run(app: &mut App, car: Entity, secs: f32) -> Vec<Vec2> {
    (0..(secs / DT).round() as usize)
        .map(|_| {
            app.update();
            pos(app, car)
        })
        .collect()
}

#[test]
fn the_car_drives_through_the_gap_to_a_point_in_the_other_room_and_stops() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    let goal = at(8.0, 1.0);
    command(&mut app, car, PlayerCommand::GoTo(goal));
    let way = run(&mut app, car, 20.0);
    assert!(pos(&app, car).distance(goal) < 1.5, "at {:?}, goal {goal:?}", pos(&app, car));
    assert!(assist(&app, car).goto.is_none(), "arrived: the job is over");
    // It went through the gap (row 3 of the wall's column 5), not the wall.
    let gap = |p: &Vec2| p.x > 15.0 && p.x < 18.0 && p.y > -12.0 && p.y < -9.0;
    assert!(way.iter().any(gap), "through the gap");
    // And it let go of the stick: nothing is written once there.
    let before = pos(&app, car);
    run(&mut app, car, 1.0);
    assert!(app.world().get::<PlayerIntent>(car).unwrap().move_dir == Vec2::ZERO);
    assert!(pos(&app, car).distance(before) < 0.01, "standing");
}

#[test]
fn without_a_navigation_graph_the_car_drives_straight_where_no_wall_is_in_the_way() {
    let mut app = map_app(TWO_ROOMS, false);
    let car = player(&mut app, at(1.0, 1.0));
    command(&mut app, car, PlayerCommand::GoTo(at(3.0, 4.0)));
    run(&mut app, car, 10.0);
    assert!(pos(&app, car).distance(at(3.0, 4.0)) < 1.5);
    assert!(assist(&app, car).goto.is_none());
    // A wall in the line, and no graph: refused.
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    app.update();
    let a = assist(&app, car);
    assert!(a.goto.is_none());
    assert_eq!(a.refused.map(|r| r.0), Some(at(8.0, 1.0)));
}

#[test]
fn a_point_that_cannot_be_reached_is_refused_with_a_marker_that_fades() {
    let mut app = map_app(SEALED, true);
    let car = player(&mut app, at(1.0, 1.0));
    let start = pos(&app, car);
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    run(&mut app, car, 0.2);
    let a = assist(&app, car);
    assert!(a.goto.is_none(), "no job");
    assert_eq!(a.refused.map(|r| r.0), Some(at(8.0, 1.0)), "the point is marked");
    assert!(pos(&app, car).distance(start) < 0.01, "the car did not move");
    run(&mut app, car, 1.0);
    assert!(assist(&app, car).refused.is_none(), "the marker is gone");
}

#[test]
fn a_manual_stick_cancels_the_drive_and_the_car_is_the_players_again() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    run(&mut app, car, 1.0);
    assert!(assist(&app, car).goto.is_some(), "driving by itself");
    assert!(app.world().get::<PlayerIntent>(car).unwrap().move_dir != Vec2::ZERO);
    // A finger, a key or a stick pushes: up.
    app.world_mut().resource_mut::<Pushed>().0 = Vec2::new(0.0, -0.5);
    app.update();
    assert!(assist(&app, car).goto.is_none(), "cancelled by the very frame");
    assert_eq!(app.world().get::<PlayerIntent>(car).unwrap().move_dir, Vec2::new(0.0, -0.5), "the hand's stick, untouched");
    // Let go of it: the car stays where the player left it.
    app.world_mut().resource_mut::<Pushed>().0 = Vec2::ZERO;
    let before = pos(&app, car);
    run(&mut app, car, 1.0);
    assert!(pos(&app, car).distance(before) < 0.01);
}

#[test]
fn a_new_goto_replaces_the_old_and_cancel_goto_gives_it_up() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    app.update();
    command(&mut app, car, PlayerCommand::GoTo(at(1.0, 4.0)));
    app.update();
    assert_eq!(assist(&app, car).goto.map(|g| g.target), Some(at(1.0, 4.0)));
    command(&mut app, car, PlayerCommand::CancelGoTo);
    app.update();
    assert!(assist(&app, car).goto.is_none());
    assert_eq!(app.world().get::<PlayerIntent>(car).unwrap().move_dir, Vec2::ZERO);
}

#[test]
fn a_dead_car_or_one_nobody_drives_takes_no_drive() {
    let mut app = map_app(TWO_ROOMS, true);
    let dead = player(&mut app, at(1.0, 1.0));
    app.world_mut().get_mut::<Health>(dead).unwrap().hp = 0;
    command(&mut app, dead, PlayerCommand::GoTo(at(8.0, 1.0)));
    app.update();
    assert!(assist(&app, dead).goto.is_none(), "a dead car is not sent");
    app.world_mut().entity_mut(dead).despawn();
    let off = player(&mut app, at(1.0, 1.0));
    command(&mut app, off, PlayerCommand::GoTo(at(8.0, 1.0)));
    app.update();
    app.world_mut().entity_mut(off).insert(NoInput);
    app.update();
    assert!(assist(&app, off).goto.is_none(), "nobody drives it");
    assert_eq!(app.world().get::<PlayerIntent>(off).unwrap().move_dir, Vec2::ZERO);
}

// ---- the double tap in an app ----

/// A frame's `secs` later a tap on the ground; the commands it led to.
fn tap(app: &mut App, ground: Vec2, wait: f32) -> Vec<PlayerCommand> {
    for _ in 0..(wait / DT).round() as usize {
        app.update();
    }
    app.world_mut().resource_mut::<Messages<Commanded>>().clear();
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground });
    app.update();
    app.world_mut().resource_mut::<Messages<Commanded>>().drain().map(|c| c.command).collect()
}

#[test]
fn a_double_tap_on_bare_ground_clears_the_lock_and_goes_there() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    let ground = at(2.0, 4.0);
    let first = tap(&mut app, ground, 1.0);
    assert_eq!(first, [PlayerCommand::ClearTarget], "the first tap acts at once, as a single tap");
    let second = tap(&mut app, ground + Vec2::new(0.3, 0.2), 0.15);
    assert_eq!(second, [PlayerCommand::ClearTarget, PlayerCommand::GoTo(ground + Vec2::new(0.3, 0.2))]);
    assert!(assist(&app, car).goto.is_some());
    // A third tap soon after begins afresh: no second GoTo.
    let third = tap(&mut app, ground, 0.15);
    assert_eq!(third, [PlayerCommand::ClearTarget]);
}

#[test]
fn two_taps_too_far_apart_or_too_slow_are_two_single_taps() {
    let mut app = map_app(TWO_ROOMS, true);
    let _car = player(&mut app, at(1.0, 1.0));
    tap(&mut app, at(2.0, 4.0), 1.0);
    assert_eq!(tap(&mut app, at(2.0, 4.0) + Vec2::new(2.0, 0.0), 0.1), [PlayerCommand::ClearTarget], "far");
    assert_eq!(tap(&mut app, at(2.0, 4.0) + Vec2::new(2.0, 0.0), 1.0), [PlayerCommand::ClearTarget], "slow");
}

#[test]
fn with_tap_to_go_off_a_double_tap_goes_nowhere() {
    let mut app = map_app(TWO_ROOMS, true);
    app.insert_resource(smart_settings(false));
    let car = player(&mut app, at(1.0, 1.0));
    tap(&mut app, at(2.0, 4.0), 1.0);
    let second = tap(&mut app, at(2.0, 4.0), 0.1);
    assert_eq!(second, [PlayerCommand::ClearTarget]);
    assert!(assist(&app, car).goto.is_none());
    // The command itself still works for an AI player.
    command(&mut app, car, PlayerCommand::GoTo(at(2.0, 4.0)));
    app.update();
    assert!(assist(&app, car).goto.is_some());
}

#[test]
fn a_double_tap_on_an_enemy_is_a_lock_twice_and_never_a_drive() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    let enemy = app
        .world_mut()
        .spawn((Team(1), Position(at(3.0, 4.0)), AutoAimVisible(true), CarRadius(1.0), Health::full(10)))
        .id();
    assert_eq!(tap(&mut app, at(3.0, 4.0), 1.0), [PlayerCommand::LockTarget(enemy)]);
    assert_eq!(tap(&mut app, at(3.0, 4.0), 0.1), [PlayerCommand::LockTarget(enemy)]);
    assert!(assist(&app, car).goto.is_none());
}

#[test]
fn a_tap_on_the_own_car_gives_up_a_drive_and_never_begins_one() {
    let mut app = map_app(TWO_ROOMS, true);
    let car = player(&mut app, at(1.0, 1.0));
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    app.update();
    let on_car = pos(&app, car);
    assert_eq!(tap(&mut app, on_car, 0.0), [PlayerCommand::CancelGoTo]);
    assert!(assist(&app, car).goto.is_none());
    assert!(tap(&mut app, on_car, 0.1).is_empty(), "a double tap on the car does nothing more");
}

#[test]
fn the_classic_scheme_has_no_drive() {
    let mut app = map_app(TWO_ROOMS, true);
    app.insert_resource(Settings::default());
    let car = app
        .world_mut()
        .spawn((LocalPlayer, PlayerIntent::default(), Position(at(1.0, 1.0)), LinearVelocity::default(), Team(0), Health::full(100), CarRadius(1.0)))
        .id();
    app.update();
    assert!(app.world().get::<Assist>(car).is_none());
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: at(2.0, 4.0) });
    app.update();
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: at(2.0, 4.0) });
    command(&mut app, car, PlayerCommand::GoTo(at(8.0, 1.0)));
    run(&mut app, car, 1.0);
    assert!(app.world().get::<Assist>(car).is_none());
    assert_eq!(app.world().get::<PlayerIntent>(car).unwrap().move_dir, Vec2::ZERO);
    assert_eq!(pos(&app, car), at(1.0, 1.0));
}
