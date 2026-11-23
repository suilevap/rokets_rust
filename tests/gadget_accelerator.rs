//! The accelerator gadget (`gadget::accelerator`, C#'s `AccelerationGadget`):
//! the button held for a quarter of a second spends the energy bar for a
//! burst of speed along the car and a blast; without the energy it only
//! fizzles; a car with no energy store always boosts in full.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::actions::Inactive;
use tilerider::car::CarRadius;
use tilerider::controls::PlayerIntent;
use tilerider::explosion::{Explosion, blast_impulse};
use tilerider::gadget::accelerator::{
    Accelerator, BLAST, Boosted, CHARGE, COST, FIZZLE_RATE, Ready, SPEED, use_accelerators,
};
use tilerider::weapon::PlayerEnergy;

const DT: f32 = 1.0 / 60.0;

/// A world of the gadget's step alone, each update one step of 1/60 s, and
/// what it sent.
fn world() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_message::<Explosion>()
        .add_message::<Boosted>()
        .add_message::<Ready>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (use_accelerators, collect).chain());
    app.finish();
    app.cleanup();
    app.update();
    app
}

#[derive(Resource, Default)]
struct Seen {
    blasts: Vec<Explosion>,
    boosts: Vec<Boosted>,
    ready: Vec<Ready>,
}

fn collect(
    mut blasts: MessageReader<Explosion>,
    mut boosts: MessageReader<Boosted>,
    mut ready: MessageReader<Ready>,
    mut seen: ResMut<Seen>,
) {
    seen.blasts.extend(blasts.read().copied());
    seen.boosts.extend(boosts.read().copied());
    seen.ready.extend(ready.read().copied());
}

/// A car at the origin facing `angle`, its energy store whole when
/// `energy`.
fn car(app: &mut App, angle: f32, energy: bool) -> Entity {
    let mut car = app.world_mut().spawn((
        Accelerator::new(),
        PlayerIntent::default(),
        Position(Vec2::ZERO),
        Rotation::radians(angle),
        LinearVelocity::default(),
        CarRadius(1.0),
    ));
    if energy {
        car.insert(PlayerEnergy::default());
    }
    car.id()
}

fn hold(app: &mut App, car: Entity, held: bool) {
    app.world_mut().get_mut::<PlayerIntent>(car).unwrap().gadget = held;
}

fn run(app: &mut App, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        app.update();
    }
}

fn speed(app: &App, car: Entity) -> Vec2 {
    app.world().get::<LinearVelocity>(car).unwrap().0
}

fn energy(app: &App, car: Entity) -> f32 {
    app.world().get::<PlayerEnergy>(car).unwrap().0.current
}

fn seen(app: &App) -> &Seen {
    app.world().resource::<Seen>()
}

#[test]
fn a_press_shorter_than_the_charge_does_nothing() {
    let mut app = world();
    let car = car(&mut app, 0.0, true);
    hold(&mut app, car, true);
    run(&mut app, CHARGE - 0.05);
    hold(&mut app, car, false);
    run(&mut app, 0.5);
    assert_eq!(speed(&app, car), Vec2::ZERO);
    assert!(seen(&app).boosts.is_empty() && seen(&app).blasts.is_empty());
    assert_eq!(energy(&app, car), 1.0);
}

#[test]
fn held_for_the_charge_it_throws_the_car_ahead_and_blasts() {
    let mut app = world();
    // Facing up the map, the button's aim somewhere else.
    let car = car(&mut app, std::f32::consts::FRAC_PI_2, true);
    app.world_mut().get_mut::<PlayerIntent>(car).unwrap().gadget_at = Some(Vec2::new(-9.0, 0.0));
    hold(&mut app, car, true);
    run(&mut app, CHARGE - 0.05);
    assert!(seen(&app).boosts.is_empty(), "not before the charge");
    run(&mut app, 0.1);
    let v = speed(&app, car);
    assert!((v - Vec2::new(0.0, SPEED)).length() < 1e-3, "30 units a second along the car, whatever the aim: {v}");
    let boost = seen(&app).boosts[0];
    assert_eq!((boost.car, boost.rate, boost.at), (car, 1.0, Vec2::ZERO));
    // The blast of the gadget, its own car's team spared by its owner.
    let blast = seen(&app).blasts[0];
    assert_eq!((blast.blast.range, blast.blast.force, blast.blast.damage), (4.0, 5.0, 20));
    assert_eq!((blast.blast, blast.owner, blast.at), (BLAST, Some(car), Vec2::ZERO));
    // The whole bar went.
    assert_eq!(energy(&app, car), 0.0);
    assert!(!app.world().get::<Accelerator>(car).unwrap().ready());
}

#[test]
fn it_works_once_a_hold() {
    let mut app = world();
    let car = car(&mut app, 0.0, false);
    hold(&mut app, car, true);
    run(&mut app, 2.0);
    assert_eq!(seen(&app).boosts.len(), 1);
    hold(&mut app, car, false);
    run(&mut app, DT * 2.0);
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.05);
    assert_eq!(seen(&app).boosts.len(), 2, "let go and press again");
}

#[test]
fn without_energy_it_fizzles_with_a_quarter_of_the_boost_and_no_blast() {
    let mut app = world();
    let car = car(&mut app, 0.0, true);
    app.world_mut().get_mut::<PlayerEnergy>(car).unwrap().0.current = 0.4;
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.05);
    let v = speed(&app, car);
    assert!((v - Vec2::new(SPEED * FIZZLE_RATE, 0.0)).length() < 1e-3, "{v}");
    assert_eq!(seen(&app).boosts[0].rate, FIZZLE_RATE);
    assert!(seen(&app).blasts.is_empty());
    assert!(energy(&app, car) >= 0.4, "a fizzle spends nothing (the bar only refills)");
}

#[test]
fn an_overloaded_bar_is_not_enough() {
    let mut app = world();
    let car = car(&mut app, 0.0, true);
    {
        let mut e = app.world_mut().get_mut::<PlayerEnergy>(car).unwrap();
        e.0.overloaded = true;
    }
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.05);
    assert_eq!(seen(&app).boosts[0].rate, FIZZLE_RATE);
}

#[test]
fn the_energy_comes_back_and_a_ring_tells_so() {
    let mut app = world();
    let car = car(&mut app, 0.0, true);
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.05);
    hold(&mut app, car, false);
    assert!(seen(&app).ready.is_empty());
    // A second's rest, then half a bar a second: the bar is whole after
    // three seconds, not before two.
    run(&mut app, 1.8);
    assert!(seen(&app).ready.is_empty() && !app.world().get::<Accelerator>(car).unwrap().ready());
    run(&mut app, 1.4);
    assert!(app.world().get::<Accelerator>(car).unwrap().ready());
    assert_eq!(energy(&app, car), COST);
    // One ring, on the car's tail.
    assert_eq!(seen(&app).ready.len(), 1);
    assert_eq!((seen(&app).ready[0].car, seen(&app).ready[0].at), (car, Vec2::new(-1.0, 0.0)));
    // And the next press is a full one.
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.05);
    assert_eq!(seen(&app).boosts.last().unwrap().rate, 1.0);
}

#[test]
fn a_car_the_devices_do_not_drive_boosts_in_full_every_time() {
    let mut app = world();
    let car = car(&mut app, 0.0, false);
    for n in 1..=3 {
        hold(&mut app, car, true);
        run(&mut app, CHARGE + 0.1);
        hold(&mut app, car, false);
        run(&mut app, 0.1);
        assert_eq!(seen(&app).boosts.len(), n);
    }
    assert!(seen(&app).boosts.iter().all(|b| b.rate == 1.0));
    assert_eq!(seen(&app).blasts.len(), 3);
}

#[test]
fn a_cancel_ends_the_activity_so_a_button_still_held_works_it_again() {
    let mut app = world();
    let car = car(&mut app, 0.0, false);
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.1);
    assert_eq!(seen(&app).boosts.len(), 1);
    app.world_mut().get_mut::<PlayerIntent>(car).unwrap().gadget_cancel = true;
    run(&mut app, 0.1);
    assert_eq!(seen(&app).boosts.len(), 2);
    assert!(!app.world().get::<PlayerIntent>(car).unwrap().gadget_cancel, "a pulse");
}

#[test]
fn a_car_out_of_the_game_does_nothing() {
    let mut app = world();
    let car = car(&mut app, 0.0, true);
    app.world_mut().entity_mut(car).insert(Inactive);
    hold(&mut app, car, true);
    run(&mut app, CHARGE + 0.2);
    assert!(seen(&app).boosts.is_empty());
    assert_eq!(speed(&app, car), Vec2::ZERO);
}

#[test]
fn a_blast_on_a_bodys_middle_does_not_push_it() {
    // The accelerator's, on its own car: Farseer's rays cancel.
    assert_eq!(blast_impulse(Vec2::new(3.0, 2.0), &BLAST, Vec2::new(3.0, 2.0), 1.0), Vec2::ZERO);
    // A box beside the car is thrown away from it.
    let push = blast_impulse(Vec2::ZERO, &BLAST, Vec2::new(2.0, 0.0), 0.5);
    assert!(push.x > 0.0 && push.y.abs() < 1e-4, "{push}");
    assert_eq!(blast_impulse(Vec2::ZERO, &BLAST, Vec2::new(5.0, 0.0), 0.5), Vec2::ZERO, "out of the range of 4");
}
