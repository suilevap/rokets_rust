//! Contact friction as C# had it: a car's own fixture has none
//! (`ControlableEntity.LinearDragCoefficient = 0`), a box's is Farseer's
//! default 0.2, a wall's 0.01, and two are mixed by their geometric mean
//! (`Settings.MixFriction`). So a car slides along a wall or a box, and
//! pushing a box sideways does not drag it along.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::car::CAR_FRICTION;
use tilerider::map::spawn::BOX_FRICTION;

const DT: f32 = 1.0 / 60.0;

fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)));
    app.finish();
    app.cleanup();
    app
}

fn friction(value: f32) -> Friction {
    Friction::new(value).with_combine_rule(CoefficientCombine::GeometricMean)
}

fn mover(app: &mut App, at: Vec2, velocity: Vec2, value: f32) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::rectangle(2.0, 1.0),
            friction(value),
            Position(at),
            LinearVelocity(velocity),
        ))
        .id()
}

fn run(app: &mut App, steps: usize) {
    for _ in 0..steps {
        app.update();
    }
}

#[test]
fn a_car_pressed_against_a_wall_keeps_sliding() {
    let mut app = world();
    // A wall below the car, which moves along it and presses into it.
    app.world_mut().spawn((RigidBody::Static, Collider::rectangle(100.0, 1.0), Friction::new(0.01), Position(Vec2::new(0.0, -1.0))));
    let car = mover(&mut app, Vec2::new(0.0, 0.0), Vec2::new(6.0, -2.0), CAR_FRICTION);
    run(&mut app, 60);
    let v = app.world().get::<LinearVelocity>(car).unwrap().0;
    assert!((v.x - 6.0).abs() < 0.01, "the car lost speed along the wall: {v}");
}

#[test]
fn a_car_passing_a_box_does_not_drag_it_along() {
    let mut app = world();
    // The box is above the car, which is pressed up into it while it drives right.
    let box_ = mover(&mut app, Vec2::new(0.0, 1.0), Vec2::ZERO, BOX_FRICTION);
    mover(&mut app, Vec2::new(0.0, 0.0), Vec2::new(6.0, 2.0), CAR_FRICTION);
    run(&mut app, 10);
    let v = app.world().get::<LinearVelocity>(box_).unwrap().0;
    assert!(v.x.abs() < 0.01, "the box was dragged along: {v}");
}

#[test]
fn two_boxes_do_drag_each_other() {
    // The same pair with a box for a car: friction is there.
    let mut app = world();
    let box_ = mover(&mut app, Vec2::new(0.0, 1.0), Vec2::ZERO, BOX_FRICTION);
    mover(&mut app, Vec2::new(0.0, 0.0), Vec2::new(6.0, 2.0), BOX_FRICTION);
    run(&mut app, 10);
    let v = app.world().get::<LinearVelocity>(box_).unwrap().0;
    assert!(v.x > 0.1, "friction between boxes dragged nothing: {v}");
}
