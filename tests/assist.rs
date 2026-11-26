//! The smart controls' brain (`assist`): taps, commands, and what the
//! assist makes of a lock: the aim, the trigger, the pulses.

use std::sync::Arc;
use std::time::Duration;

use avian2d::prelude::{LinearVelocity, Position};
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::ai::SharedTerrain;
use tilerider::assist::tap::{Gesture, Press, TAP_MAX_SECS, TAP_SLOP_PX, classify, slop, tap_target};
use tilerider::assist::{Assist, AssistPlugin, Commanded, PULSE_SECS, PlayerCommand, Tapped, fires_on_pulse, lead_point};
use tilerider::car::CarRadius;
use tilerider::car::control::{AutoAimVisible, Team};
use tilerider::content::map::MapData;
use tilerider::controls::{LocalPlayer, NoInput, PlayerIntent};
use tilerider::explosion::Health;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;
use tilerider::settings::{ControlScheme, Settings};
use tilerider::weapon::model::{HAMMER, Kind, MINIGUN, Options, RAILGUN};
use tilerider::weapon::Weapon;

const DT: f32 = 1.0 / 60.0;

// ---- taps (plain numbers) ----

#[test]
fn a_press_lifted_soon_without_moving_is_a_tap() {
    let at = Vec2::new(100.0, 100.0);
    assert_eq!(classify(at, at, 0.1, false, 12.0), Gesture::Pending, "still down: not known yet");
    assert_eq!(classify(at, at + Vec2::new(3.0, 2.0), 0.1, true, 12.0), Gesture::Tap);
    assert_eq!(classify(at, at, TAP_MAX_SECS, true, 12.0), Gesture::Tap, "the limit itself is a tap");
}

#[test]
fn a_press_that_moves_or_lingers_is_the_stick() {
    let at = Vec2::new(100.0, 100.0);
    assert_eq!(classify(at, at + Vec2::new(12.0, 0.0), 0.05, false, 12.0), Gesture::Drag, "moved a slop");
    assert_eq!(classify(at, at + Vec2::new(0.0, -30.0), 0.05, true, 12.0), Gesture::Drag, "lifted, but it was a swipe");
    assert_eq!(classify(at, at, TAP_MAX_SECS + 0.01, false, 12.0), Gesture::Drag, "held: the stick, nothing pushed");
    assert_eq!(classify(at, at, TAP_MAX_SECS + 0.5, true, 12.0), Gesture::Drag, "a long press is not a tap");
}

#[test]
fn the_slop_grows_with_the_screen_but_not_below_the_reference() {
    assert_eq!(slop(Vec2::new(800.0, 480.0)), TAP_SLOP_PX);
    assert_eq!(slop(Vec2::new(480.0, 320.0)), TAP_SLOP_PX, "a small screen is not stricter");
    assert_eq!(slop(Vec2::new(1280.0, 720.0)), TAP_SLOP_PX * 1.5);
    assert_eq!(slop(Vec2::new(4000.0, 3000.0)), TAP_SLOP_PX * 2.0, "capped");
    // The shorter side counts, so portrait is the same as landscape.
    assert_eq!(slop(Vec2::new(720.0, 1280.0)), slop(Vec2::new(1280.0, 720.0)));
}

#[test]
fn a_press_tracked_over_time_ends_as_a_tap_or_a_drag() {
    let slop = 12.0;
    let mut press = Press::new(Vec2::new(50.0, 50.0), 10.0);
    assert_eq!(press.gesture(Some(Vec2::new(52.0, 51.0)), 10.05, slop), Gesture::Pending);
    // Lifted: the last place seen counts.
    assert_eq!(press.gesture(None, 10.1, slop), Gesture::Tap);
    assert_eq!(press.last, Vec2::new(52.0, 51.0));
    let mut press = Press::new(Vec2::new(50.0, 50.0), 10.0);
    assert_eq!(press.gesture(Some(Vec2::new(90.0, 50.0)), 10.05, slop), Gesture::Drag);
}

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

#[test]
fn a_tap_picks_the_nearest_enemy_within_the_radius() {
    let enemies = [(id(1), Vec2::new(5.0, 0.0)), (id(2), Vec2::new(3.0, 0.0)), (id(3), Vec2::new(20.0, 0.0))];
    assert_eq!(tap_target(enemies, Vec2::new(3.8, 0.0), 1.5), Some(id(2)), "nearer to the tap, not to anything else");
    assert_eq!(tap_target(enemies, Vec2::new(4.9, 0.0), 1.5), Some(id(1)));
    assert_eq!(tap_target(enemies, Vec2::new(12.0, 0.0), 1.5), None, "nobody within reach of the tap");
    assert_eq!(tap_target(enemies, Vec2::new(20.0, 1.5), 1.5), Some(id(3)), "on the edge");
    assert_eq!(tap_target([], Vec2::ZERO, 1.5), None);
}

// ---- the assist in an app ----

/// The Smart scheme's app: the assist is kept on the car.
fn app() -> App {
    smart_app()
}

/// No settings at all: Classic.
fn classic_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_plugins(AssistPlugin);
    app.update();
    app
}

fn smart_settings() -> Settings {
    let mut s = Settings::default();
    s.control_scheme = ControlScheme::Smart;
    s
}

/// An app whose player is under the Smart scheme.
fn smart_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(smart_settings())
        .add_plugins(AssistPlugin);
    app.update();
    app
}

fn gun(kind: Kind, options: Options) -> Weapon {
    Weapon::new(kind, options, Vec2::ZERO, Vec2::ONE, true, &mut SmallRng::seed_from_u64(1))
}

/// The player's car at the origin, its gun at the same place; `with_assist`
/// is a car already in the Smart scheme.
fn player(app: &mut App, weapon: Weapon, with_assist: bool) -> Entity {
    let car = app
        .world_mut()
        .spawn((LocalPlayer, PlayerIntent::default(), weapon, Position(Vec2::ZERO), Team(0), Health::full(100), AutoAimVisible(true), CarRadius(1.0)))
        .id();
    if with_assist {
        app.world_mut().entity_mut(car).insert(Assist::default());
    }
    car
}

fn car_at(app: &mut App, team: u8, at: Vec2) -> Entity {
    app.world_mut().spawn((Team(team), Position(at), AutoAimVisible(true), CarRadius(1.0), Health::full(10))).id()
}

fn command(app: &mut App, car: Entity, command: PlayerCommand) {
    app.world_mut().resource_mut::<Messages<Commanded>>().write(Commanded { car, command });
}

fn intent(app: &App, car: Entity) -> PlayerIntent {
    *app.world().get::<PlayerIntent>(car).unwrap()
}

fn assist(app: &App, car: Entity) -> Assist {
    app.world().get::<Assist>(car).unwrap().clone()
}

/// The gun points at `angle` (world radians).
fn point(app: &mut App, car: Entity, angle: f32) {
    app.world_mut().get_mut::<Weapon>(car).unwrap().direction = angle;
}

#[test]
fn a_lock_turns_into_an_aim_at_the_target() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    app.update();
    assert_eq!(assist(&app, car).lock, Some(enemy));
    let i = intent(&app, car);
    assert!((i.aim_dir - Vec2::Y).length() < 1e-5, "{:?}", i.aim_dir);
    assert!(i.look.y > 0.0, "the camera looks that way");
    assert!(!i.fire, "the gun does not point at it yet");
}

#[test]
fn the_trigger_is_held_once_the_gun_points_at_a_target_in_sight_and_range() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, std::f32::consts::FRAC_PI_2);
    app.update();
    assert!(intent(&app, car).fire);
    // Out of the gun's range (32): no.
    app.world_mut().get_mut::<Position>(enemy).unwrap().0 = Vec2::new(0.0, 40.0);
    // The devices write a fresh intent every frame.
    *app.world_mut().get_mut::<PlayerIntent>(car).unwrap() = PlayerIntent::default();
    app.update();
    assert!(!intent(&app, car).fire);
    assert_eq!(assist(&app, car).lock, Some(enemy), "but the lock holds: drive towards it");
}

#[test]
fn a_lock_clears_when_the_target_despawns_or_dies() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let first = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(first));
    app.update();
    assert_eq!(assist(&app, car).lock, Some(first));
    app.world_mut().despawn(first);
    app.update();
    assert_eq!(assist(&app, car).lock, None);

    let second = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(second));
    app.update();
    app.world_mut().get_mut::<Health>(second).unwrap().hp = 0;
    app.update();
    assert_eq!(assist(&app, car).lock, None, "a dead car is let go");
    assert!(assist(&app, car).soft.is_none());
}

#[test]
fn a_lock_is_let_go_far_beyond_the_range() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let enemy = car_at(&mut app, 1, Vec2::new(40.0, 0.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    app.update();
    assert_eq!(assist(&app, car).lock, Some(enemy), "out of range, but near enough to drive to");
    app.world_mut().get_mut::<Position>(enemy).unwrap().0 = Vec2::new(70.0, 0.0);
    app.update();
    assert_eq!(assist(&app, car).lock, None);
}

#[test]
fn clear_target_and_commands_for_allies_or_the_dead() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let ally = car_at(&mut app, 0, Vec2::new(5.0, 0.0));
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 5.0));
    command(&mut app, car, PlayerCommand::LockTarget(ally));
    app.update();
    assert_eq!(assist(&app, car).lock, None, "an ally is not a target");
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    app.update();
    assert_eq!(assist(&app, car).lock, Some(enemy));
    command(&mut app, car, PlayerCommand::ClearTarget);
    app.update();
    assert_eq!(assist(&app, car).lock, None);
    // A command for a car with no assist is ignored.
    let other = car_at(&mut app, 0, Vec2::new(9.0, 9.0));
    command(&mut app, other, PlayerCommand::LockTarget(enemy));
    app.update();
}

#[test]
fn with_auto_fire_off_the_car_aims_but_does_not_fire() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    app.world_mut().resource_mut::<Settings>().auto_fire = false;
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, std::f32::consts::FRAC_PI_2);
    app.update();
    let i = intent(&app, car);
    assert!(!i.fire && !i.fire_accurate);
    assert!((i.aim_dir - Vec2::Y).length() < 1e-5);
}

#[test]
fn with_auto_aim_off_the_aim_is_left_to_the_player_but_the_gun_still_fires_when_it_points() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    app.world_mut().resource_mut::<Settings>().auto_aim = false;
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 10.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, std::f32::consts::FRAC_PI_2);
    app.update();
    let i = intent(&app, car);
    assert_eq!(i.aim_dir, Vec2::ZERO);
    assert!(i.fire);
}

#[test]
fn without_a_lock_the_nearest_enemy_in_sight_is_the_soft_target() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let far = car_at(&mut app, 1, Vec2::new(20.0, 0.0));
    let near = car_at(&mut app, 1, Vec2::new(0.0, 8.0));
    let _ally = car_at(&mut app, 0, Vec2::new(1.0, 1.0));
    let _beyond = car_at(&mut app, 1, Vec2::new(0.0, 60.0));
    app.update();
    assert_eq!(assist(&app, car).soft, Some(near));
    assert_eq!(assist(&app, car).lock, None);
    assert!((intent(&app, car).aim_dir - Vec2::Y).length() < 1e-5);
    // It sticks to the one it has while that still qualifies, even if
    // another comes nearer by a little.
    let _closer = car_at(&mut app, 1, Vec2::new(0.0, -7.5));
    app.update();
    assert_eq!(assist(&app, car).soft, Some(near));
    // Once it is dead, the next is picked.
    app.world_mut().get_mut::<Health>(near).unwrap().hp = 0;
    app.update();
    assert_ne!(assist(&app, car).soft, Some(near));
    assert!(assist(&app, car).soft.is_some());
    let _ = far;
}

#[test]
fn a_lock_takes_the_place_of_the_soft_target() {
    let mut app = app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    let near = car_at(&mut app, 1, Vec2::new(0.0, 8.0));
    let far = car_at(&mut app, 1, Vec2::new(20.0, 0.0));
    app.update();
    assert_eq!(assist(&app, car).soft, Some(near));
    command(&mut app, car, PlayerCommand::LockTarget(far));
    app.update();
    let a = assist(&app, car);
    assert_eq!((a.lock, a.soft), (Some(far), None));
    assert!((intent(&app, car).aim_dir - Vec2::X).length() < 1e-5);
}

/// A map picture made `3` characters to a cell: two rooms of a wall
/// between them, the middle row open.
fn walled_terrain() -> SharedTerrain {
    let rows = "\n***********\n*....*....*\n*....*....*\n*....*....*\n*....*....*\n***********\n";
    let mut big = String::from("\n");
    for row in rows.trim().lines() {
        let wide: String = row.chars().flat_map(|c| std::iter::repeat_n(c, 3)).collect();
        for _ in 0..3 {
            big.push_str(&wide);
            big.push('\n');
        }
    }
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{big}</CharMap></Asset></XnaContent>"#
    );
    let grid = MapGrid::build(&MapData::parse(&xml).unwrap(), None);
    SharedTerrain(Arc::new(Terrain::from_grid(&grid, 1.0)))
}

#[test]
fn a_wall_hides_a_soft_target_and_stops_the_fire_but_not_the_lock() {
    let mut app = app();
    app.insert_resource(walled_terrain());
    // Left room, near its middle; the enemy is in the right room (y is
    // up: the map's rows go down from 0).
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    app.world_mut().get_mut::<Position>(car).unwrap().0 = Vec2::new(7.0, -7.0);
    app.world_mut().get_mut::<Weapon>(car).unwrap().at = Vec2::new(7.0, -7.0);
    let enemy = car_at(&mut app, 1, Vec2::new(22.0, -7.0));
    app.update();
    assert_eq!(assist(&app, car).soft, None, "behind a wall");
    assert!(!intent(&app, car).fire);
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, 0.0);
    app.update();
    assert_eq!(assist(&app, car).lock, Some(enemy), "the lock sees through walls");
    assert!((intent(&app, car).aim_dir - Vec2::X).length() < 1e-5, "the aim goes to it");
    assert!(!intent(&app, car).fire, "but no shot into the wall");
    // An enemy in the same room is in sight.
    let near = car_at(&mut app, 1, Vec2::new(7.0, -2.0));
    command(&mut app, car, PlayerCommand::ClearTarget);
    app.update();
    assert_eq!(assist(&app, car).soft, Some(near));
}

#[test]
fn an_accurate_gun_gets_pulses_at_a_steady_beat_while_it_points_at_the_target() {
    assert!(fires_on_pulse(Kind::Railgun) && fires_on_pulse(Kind::Rocket) && fires_on_pulse(Kind::Shotgun));
    assert!(!fires_on_pulse(Kind::Minigun) && !fires_on_pulse(Kind::Plasma));
    let mut app = app();
    let car = player(&mut app, gun(Kind::Railgun, RAILGUN), true);
    let enemy = car_at(&mut app, 1, Vec2::new(10.0, 0.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, 0.0);
    let mut pulses = Vec::new();
    for step in 0..180 {
        app.update();
        // The gun takes the pulse in the fixed step.
        let mut i = app.world_mut().get_mut::<PlayerIntent>(car).unwrap();
        if i.fire_accurate {
            pulses.push(step);
            i.fire_accurate = false;
        }
    }
    // Three seconds at a pulse every 0.3 s, the first at once.
    assert!((9..=11).contains(&pulses.len()), "{pulses:?}");
    assert_eq!(pulses[0], 0);
    let gap = (PULSE_SECS / DT).round() as i32;
    assert!(pulses.windows(2).all(|w| (w[1] - w[0] - gap).abs() <= 1), "{pulses:?}");
    // Not pointing: no pulses.
    point(&mut app, car, std::f32::consts::PI);
    for _ in 0..30 {
        app.update();
        assert!(!intent(&app, car).fire_accurate);
    }
}

#[test]
fn the_hammer_and_a_car_nobody_drives_are_left_alone() {
    let mut app = app();
    let saw = player(&mut app, gun(Kind::Hammer, HAMMER), true);
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 5.0));
    command(&mut app, saw, PlayerCommand::LockTarget(enemy));
    app.update();
    assert_eq!(intent(&app, saw), PlayerIntent::default(), "it works by itself");

    let mut app = smart_app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), true);
    app.world_mut().entity_mut(car).insert(NoInput);
    let enemy = car_at(&mut app, 1, Vec2::new(0.0, 5.0));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    point(&mut app, car, std::f32::consts::FRAC_PI_2);
    app.update();
    assert_eq!(intent(&app, car), PlayerIntent::default(), "an intro: the car is not the player's now");
}

#[test]
fn a_fast_target_is_led_for_bullets_that_fly_and_not_for_the_instant_ones() {
    // 10 units off, bullets at 20 a second: half a second of its drive.
    let led = lead_point(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(0.0, 4.0), 20.0);
    assert!((led - Vec2::new(10.0, 2.0)).length() < 1e-5, "{led}");
    assert_eq!(lead_point(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(0.0, 4.0), 0.0), Vec2::new(10.0, 0.0));

    let mut app = app();
    let mut plasma = tilerider::weapon::model::MINIGUN;
    plasma.bullet_speed = 20.0;
    let car = player(&mut app, gun(Kind::Plasma, plasma), true);
    let enemy = car_at(&mut app, 1, Vec2::new(10.0, 0.0));
    app.world_mut().entity_mut(enemy).insert(LinearVelocity(Vec2::new(0.0, 4.0)));
    command(&mut app, car, PlayerCommand::LockTarget(enemy));
    app.update();
    let aim = intent(&app, car).aim_dir;
    assert!(aim.y > 0.15 && aim.y < 0.25, "{aim}");
}

#[test]
fn taps_become_commands_by_what_is_under_the_finger() {
    let mut app = smart_app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), false);
    let enemy = car_at(&mut app, 1, Vec2::new(10.0, 0.0));
    let _ally = car_at(&mut app, 0, Vec2::new(0.0, 10.0));
    app.update();
    assert!(app.world().get::<Assist>(car).is_some(), "the Smart scheme gives the car an assist");
    let tap = |app: &mut App, ground: Vec2| {
        app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground });
        app.update();
    };
    tap(&mut app, Vec2::new(10.5, 1.0));
    assert_eq!(assist(&app, car).lock, Some(enemy));
    // On the ally, or on bare ground: the lock is let go.
    tap(&mut app, Vec2::new(0.0, 10.0));
    assert_eq!(assist(&app, car).lock, None, "an ally is not hit; empty ground clears");
    tap(&mut app, Vec2::new(10.0, 0.0));
    assert_eq!(assist(&app, car).lock, Some(enemy));
    tap(&mut app, Vec2::new(-20.0, 0.0));
    assert_eq!(assist(&app, car).lock, None);
}

#[test]
fn a_dead_enemy_cannot_be_tapped() {
    let mut app = smart_app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), false);
    let enemy = car_at(&mut app, 1, Vec2::new(10.0, 0.0));
    app.world_mut().get_mut::<Health>(enemy).unwrap().hp = 0;
    app.update();
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: Vec2::new(10.0, 0.0) });
    app.update();
    assert_eq!(assist(&app, car).lock, None);
}

#[test]
fn the_classic_scheme_has_no_assist_and_leaves_the_intent_alone() {
    // No settings at all, and the Classic ones.
    for settings in [None, Some(Settings::default())] {
        let mut app = classic_app();
        if let Some(s) = settings {
            app.insert_resource(s);
        }
        let car = player(&mut app, gun(Kind::Minigun, MINIGUN), false);
        let _enemy = car_at(&mut app, 1, Vec2::new(0.0, 5.0));
        point(&mut app, car, std::f32::consts::FRAC_PI_2);
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get::<Assist>(car).is_none());
        assert_eq!(intent(&app, car), PlayerIntent::default());
    }
}

#[test]
fn switching_the_scheme_adds_and_removes_the_assist_and_follows_its_options() {
    let mut app = smart_app();
    let car = player(&mut app, gun(Kind::Minigun, MINIGUN), false);
    app.update();
    assert!(assist(&app, car).auto_fire && assist(&app, car).auto_aim);
    {
        let mut s = app.world_mut().resource_mut::<Settings>();
        s.auto_fire = false;
        s.auto_aim = false;
    }
    app.update();
    let a = assist(&app, car);
    assert!(!a.auto_fire && !a.auto_aim);
    app.world_mut().resource_mut::<Settings>().set_control_scheme(ControlScheme::Classic);
    app.update();
    assert!(app.world().get::<Assist>(car).is_none());
}
