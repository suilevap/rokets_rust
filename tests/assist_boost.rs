//! The Smart scheme's hand on the accelerator: a tap on the player's own car
//! is `Boost`, the assist holds the gadget button until the gadget works
//! (against the real accelerator) and lets go; refusals; the nearest pick
//! decides between the own car and an enemy.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::assist::boost::{Boosting, Finish, HOLD_MAX, Scene, step};
use tilerider::assist::magnet::{Handling, Job};
use tilerider::assist::{Assist, AssistPlugin, Commanded, PlayerCommand, Tapped};
use tilerider::car::CarRadius;
use tilerider::car::control::{AutoAimVisible, Team};
use tilerider::controls::{LocalPlayer, PlayerIntent};
use tilerider::explosion::{Explosion, Health};
use tilerider::gadget::accelerator::{Accelerator, Boosted, CHARGE, Ready, SPEED, use_accelerators};
use tilerider::gadget::Magnet;
use tilerider::settings::{ControlScheme, Settings};
use tilerider::weapon::PlayerEnergy;

const DT: f32 = 1.0 / 60.0;

fn scene(ready: bool, active: bool, manual: bool) -> Scene {
    Scene { ready, active, manual, dt: DT }
}

// ---- the job (plain numbers) ----

#[test]
fn the_button_is_held_until_the_gadget_works_then_let_go() {
    let mut h = Boosting::default();
    for _ in 0..10 {
        let s = step(&mut h, &scene(true, false, false));
        assert_eq!((s.hold, s.finished), (Some(true), None));
    }
    let s = step(&mut h, &scene(false, true, false));
    assert_eq!((s.hold, s.finished, s.cancel), (Some(false), Some(Finish::Done), false));
}

#[test]
fn a_boost_that_is_not_ready_at_the_start_does_nothing() {
    let mut h = Boosting::default();
    let s = step(&mut h, &scene(false, false, false));
    assert_eq!((s.hold, s.finished), (None, Some(Finish::NotReady)));
}

#[test]
fn a_device_holding_the_button_wins() {
    let mut h = Boosting::default();
    let s = step(&mut h, &scene(true, false, true));
    assert_eq!((s.hold, s.finished), (None, Some(Finish::Gone)));
}

#[test]
fn a_hold_that_never_works_is_let_go_and_cancelled() {
    let mut h = Boosting::default();
    let mut last = step(&mut h, &scene(true, false, false));
    for _ in 0..((HOLD_MAX / DT) as usize + 5) {
        if last.finished.is_some() {
            break;
        }
        last = step(&mut h, &scene(true, false, false));
    }
    assert_eq!(last.finished, Some(Finish::Failed));
    assert!(last.cancel && last.hold == Some(false));
}

// ---- the assist in an app, with the real accelerator ----

fn app_with(scheme: ControlScheme) -> App {
    let mut settings = Settings::default();
    settings.control_scheme = scheme;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(settings)
        .add_message::<Explosion>()
        .add_message::<Boosted>()
        .add_message::<Ready>()
        .add_plugins(AssistPlugin)
        .add_systems(FixedUpdate, use_accelerators);
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn smart_app() -> App {
    app_with(ControlScheme::Smart)
}

/// The player's car at the origin facing right; `accelerator` gives it the gadget.
fn player(app: &mut App, accelerator: bool) -> Entity {
    let car = app
        .world_mut()
        .spawn((
            LocalPlayer,
            PlayerIntent::default(),
            Position(Vec2::ZERO),
            Rotation::radians(0.0),
            LinearVelocity::default(),
            Team(0),
            Health::full(100),
            AutoAimVisible(true),
            CarRadius(1.0),
            PlayerEnergy::default(),
            Assist::default(),
        ))
        .id();
    if accelerator {
        app.world_mut().entity_mut(car).insert(Accelerator::new());
    }
    car
}

fn enemy_at(app: &mut App, at: Vec2) -> Entity {
    app.world_mut().spawn((Team(1), Position(at), AutoAimVisible(true), CarRadius(1.0), Health::full(10))).id()
}

/// A frame as the devices leave it: the gadget button up unless a hand holds it.
fn frame(app: &mut App, car: Entity) {
    app.world_mut().get_mut::<PlayerIntent>(car).unwrap().gadget = false;
    app.update();
}

fn tap(app: &mut App, at: Vec2) -> Vec<PlayerCommand> {
    app.world_mut().resource_mut::<Messages<Tapped>>().write(Tapped { ground: at });
    app.update();
    app.world_mut().resource_mut::<Messages<Commanded>>().drain().map(|c| c.command).collect()
}

fn assist(app: &App, car: Entity) -> Assist {
    app.world().get::<Assist>(car).unwrap().clone()
}

fn velocity(app: &App, car: Entity) -> Vec2 {
    app.world().get::<LinearVelocity>(car).unwrap().0
}

#[test]
fn a_tap_on_the_own_car_is_a_boost_and_the_accelerator_fires_after_its_charge() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let sent = tap(&mut app, Vec2::new(0.4, -0.3));
    assert_eq!(sent, vec![PlayerCommand::Boost]);
    assert!(assist(&app, car).boost.is_some());
    let mut held = 0.0;
    for _ in 0..60 {
        frame(&mut app, car);
        if app.world().get::<PlayerIntent>(car).unwrap().gadget {
            held += DT;
        }
        if assist(&app, car).boost.is_none() {
            break;
        }
    }
    assert!(held >= CHARGE, "the button was held {held} s, the charge is {CHARGE}");
    assert!(held < CHARGE + 0.2, "and not much longer: {held}");
    let v = velocity(&app, car);
    assert!((v - Vec2::new(SPEED, 0.0)).length() < 1e-3, "a full boost along the car: {v}");
    assert_eq!(app.world().get::<PlayerEnergy>(car).unwrap().0.current, 0.0, "the whole bar went");
    assert!(!app.world().get::<PlayerIntent>(car).unwrap().gadget, "the button is up again");
    // And only once: nothing is held on the next frames.
    for _ in 0..30 {
        frame(&mut app, car);
        assert!(!app.world().get::<PlayerIntent>(car).unwrap().gadget);
    }
    assert!((velocity(&app, car).x - SPEED).abs() < 1e-3, "no second boost");
}

#[test]
fn without_a_whole_bar_the_tap_does_nothing() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    app.world_mut().get_mut::<PlayerEnergy>(car).unwrap().0.current = 0.4;
    // The gadget notices the bar in its next fixed step.
    for _ in 0..3 {
        frame(&mut app, car);
    }
    let sent = tap(&mut app, Vec2::ZERO);
    assert_eq!(sent, vec![PlayerCommand::Boost], "the tap is a boost; the assist refuses it");
    assert!(assist(&app, car).boost.is_none());
    for _ in 0..30 {
        frame(&mut app, car);
        assert!(!app.world().get::<PlayerIntent>(car).unwrap().gadget);
    }
    assert!(velocity(&app, car).length() < 1e-3, "not even a fizzle");
}

#[test]
fn a_device_holding_the_button_is_not_fought() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    tap(&mut app, Vec2::ZERO);
    app.world_mut().get_mut::<PlayerIntent>(car).unwrap().gadget = true;
    app.update();
    assert!(assist(&app, car).boost.is_none(), "the player's own press ends the job");
}

#[test]
fn a_car_without_an_accelerator_gets_no_boost_and_keeps_its_lock() {
    let mut app = smart_app();
    let car = player(&mut app, false);
    let foe = enemy_at(&mut app, Vec2::new(30.0, 0.0));
    app.world_mut().write_message(Commanded { car, command: PlayerCommand::LockTarget(foe) });
    app.update();
    app.world_mut().resource_mut::<Messages<Commanded>>().drain().for_each(drop);
    let sent = tap(&mut app, Vec2::new(0.2, 0.2));
    assert!(sent.is_empty(), "{sent:?}");
    assert_eq!(assist(&app, car).lock, Some(foe), "the lock is not cleared");
    assert!(assist(&app, car).boost.is_none());
}

#[test]
fn the_own_car_wins_only_as_the_nearest_pick() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let foe = enemy_at(&mut app, Vec2::new(1.4, 0.0));
    // Nearer the enemy: the lock; nearer the own car: the boost.
    assert_eq!(tap(&mut app, Vec2::new(1.3, 0.0)), vec![PlayerCommand::LockTarget(foe)]);
    assert_eq!(tap(&mut app, Vec2::new(0.3, 0.0)), vec![PlayerCommand::Boost]);
    assert_eq!(tap(&mut app, Vec2::new(0.69, 0.0)), vec![PlayerCommand::Boost]);
    assert_eq!(tap(&mut app, Vec2::new(0.7, 0.0)), vec![PlayerCommand::LockTarget(foe)], "equally near: the gun first");
    assert_eq!(tap(&mut app, Vec2::new(0.8, 0.0)), vec![PlayerCommand::LockTarget(foe)]);
    let _ = car;
}

#[test]
fn a_tap_beyond_the_own_car_is_the_ground() {
    let mut app = smart_app();
    let car = player(&mut app, true);
    let sent = tap(&mut app, Vec2::new(5.0, 0.0));
    assert_eq!(sent, vec![PlayerCommand::ClearTarget]);
    assert!(assist(&app, car).boost.is_none());
}

#[test]
fn with_a_magnet_the_own_car_gives_up_a_pending_job_and_otherwise_does_nothing() {
    let mut app = smart_app();
    let car = player(&mut app, false);
    app.world_mut().entity_mut(car).insert(Magnet::new(Vec2::ZERO));
    let foe = enemy_at(&mut app, Vec2::new(30.0, 0.0));
    app.world_mut().write_message(Commanded { car, command: PlayerCommand::LockTarget(foe) });
    app.update();
    app.world_mut().resource_mut::<Messages<Commanded>>().drain().for_each(drop);
    assert!(tap(&mut app, Vec2::ZERO).is_empty(), "nothing pending: ignored");
    assert_eq!(assist(&app, car).lock, Some(foe));
    app.world_mut().get_mut::<Assist>(car).unwrap().cargo = Some(Handling::new(Job::Drop(Vec2::new(5.0, 0.0))));
    assert_eq!(tap(&mut app, Vec2::ZERO), vec![PlayerCommand::CancelCargo]);
    assert!(assist(&app, car).cargo.is_none());
    assert_eq!(assist(&app, car).lock, Some(foe));
}

#[test]
fn classic_has_no_taps_and_no_boost() {
    let mut app = app_with(ControlScheme::Classic);
    let car = player(&mut app, true);
    app.world_mut().entity_mut(car).remove::<Assist>();
    let sent = tap(&mut app, Vec2::ZERO);
    assert!(sent.is_empty());
    for _ in 0..30 {
        frame(&mut app, car);
    }
    assert!(velocity(&app, car).length() < 1e-3);
    assert!(app.world().get::<Assist>(car).is_none());
}
