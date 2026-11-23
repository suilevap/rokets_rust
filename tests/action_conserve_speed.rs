//! `AddConserveMaxReachedSpeedComponent`: the pong ball keeps its speed.

use avian2d::prelude::LinearVelocity;
use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, ConserveMaxReachedSpeed, CurrentActions, run};
use tilerider::game::MapId;

mod common;

fn app() -> App {
    let (map, _) = common::load_map("DoorsDream1");
    let mut app = App::new();
    app.add_plugins(ActionsPlugin).insert_resource(CurrentActions(map.actions));
    app
}

#[test]
fn a_slower_body_is_scaled_back_along_its_direction() {
    let mut c = ConserveMaxReachedSpeed::default();
    // The first update takes the speed the kick gave.
    assert_eq!(c.conserve(Vec2::new(-12.0, -2.0)), Vec2::new(-12.0, -2.0));
    let kept = c.conserve(Vec2::new(-3.0, 4.0));
    assert!((kept.length() - Vec2::new(-12.0, -2.0).length()).abs() < 1e-4);
    assert!((kept.normalize() - Vec2::new(-0.6, 0.8)).length() < 1e-5, "direction is kept");
}

#[test]
fn a_faster_body_raises_the_highest_speed() {
    let mut c = ConserveMaxReachedSpeed::default();
    c.conserve(Vec2::new(5.0, 0.0));
    assert_eq!(c.conserve(Vec2::new(0.0, 8.0)), Vec2::new(0.0, 8.0));
    assert_eq!(c.max_speed, 8.0);
    assert!((c.conserve(Vec2::new(6.0, 0.0)).length() - 8.0).abs() < 1e-5);
}

#[test]
fn a_body_that_stopped_stays_still_instead_of_turning_nan() {
    let mut c = ConserveMaxReachedSpeed { max_speed: 9.0 };
    assert_eq!(c.conserve(Vec2::ZERO), Vec2::ZERO);
}

#[test]
fn the_balls_create_sequence_adds_it_to_the_ball_that_was_created() {
    let mut app = app();
    let ball = app.world_mut().spawn((MapId(3003), LinearVelocity::default())).id();
    let other = app.world_mut().spawn((MapId(3001), LinearVelocity::default())).id();
    assert!(run(app.world_mut(), "ConserveMaxReachedSpeedComponent", Some(3003), 0));
    assert!(app.world().get::<ConserveMaxReachedSpeed>(ball).is_some());
    assert!(app.world().get::<ConserveMaxReachedSpeed>(other).is_none());
}

#[test]
fn running_it_again_keeps_the_highest_speed() {
    let mut app = app();
    let ball = app.world_mut().spawn((MapId(3003), LinearVelocity::default())).id();
    run(app.world_mut(), "ConserveMaxReachedSpeedComponent", Some(3003), 0);
    app.world_mut().get_mut::<ConserveMaxReachedSpeed>(ball).unwrap().max_speed = 7.0;
    run(app.world_mut(), "ConserveMaxReachedSpeedComponent", Some(3003), 0);
    assert_eq!(app.world().get::<ConserveMaxReachedSpeed>(ball).unwrap().max_speed, 7.0);
}

#[test]
fn it_fails_without_a_target_or_a_body() {
    let mut app = app();
    assert!(!run(app.world_mut(), "ConserveMaxReachedSpeedComponent", Some(9999), 0));
    app.world_mut().spawn(MapId(3004));
    assert!(!run(app.world_mut(), "ConserveMaxReachedSpeedComponent", Some(3004), 0));
}

#[test]
fn doorsdream1_has_the_action_to_run() {
    let (map, _) = common::load_map("DoorsDream1");
    assert!(map.actions.0.iter().any(|a| a.kind == "AddConserveMaxReachedSpeedComponent"));
}
