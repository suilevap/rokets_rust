//! `ApplyGadgetToPosition`: where the action puts a car's gadget (the
//! target body's frame, C#'s y down), and what the magnet and the
//! accelerator do when told to work there.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::content::map::MapData;
use tilerider::explosion::Explosion;
use tilerider::game::MapId;
use tilerider::gadget::accelerator::{Accelerator, Boosted, apply_to_accelerators};
use tilerider::gadget::apply::{ApplyGadget, ApplyGadgetPlugin, apply_to_magnets};
use tilerider::gadget::static_cargo::{StaticWhenFree, wake_towed};
use tilerider::gadget::{Linkable, Magnet, REACH};
use tilerider::sound::PlaySound;

const DT: f32 = 1.0 / 60.0;

// -- the action: where --

/// An app with the action, a map whose `Actions` are one `Grab` of `owner`
/// on `target` at `rel`, and the cars and bodies the tests add.
fn action_app(owner: i64, target: i64, rel: &str) -> App {
    let item = format!(
        r#"<Item Type="GameLibrary.ApplyGadgetToPosition"><Name>Grab</Name><GadgetOwnerId>{owner}</GadgetOwnerId><TargetItemId>{target}</TargetItemId><RelativePosition>{rel}</RelativePosition></Item>"#
    );
    let xml = format!(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{item}</Actions></Asset></XnaContent>"#);
    let map = MapData::parse(&xml).unwrap();
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, ApplyGadgetPlugin)).insert_resource(CurrentActions(map.actions));
    app
}

fn body(app: &mut App, id: i64, at: Vec2, angle: f32) -> Entity {
    app.world_mut().spawn((MapId(id), Position(at), Rotation::radians(angle))).id()
}

fn truck(app: &mut App, id: i64) -> Entity {
    let e = body(app, id, Vec2::ZERO, 0.0);
    app.world_mut().entity_mut(e).insert(Magnet::new(Vec2::ZERO));
    e
}

fn applied(app: &mut App) -> Vec<ApplyGadget> {
    app.world_mut().resource_mut::<Messages<ApplyGadget>>().drain().collect()
}

#[test]
fn the_place_is_a_point_of_the_targets_frame_with_y_to_the_right() {
    // C#: `RelativePosition (1, 2)` is one ahead and two to the right of
    // the body, y down; in the world (y up) that is below it.
    let mut app = action_app(4004, 3005, "1 2");
    let car = truck(&mut app, 4004);
    body(&mut app, 3005, Vec2::new(10.0, 5.0), 0.0);
    assert!(run(app.world_mut(), "Grab", None, 0));
    assert_eq!(applied(&mut app), [ApplyGadget { car, at: Vec2::new(11.0, 3.0) }]);
}

#[test]
fn the_frame_turns_with_the_target() {
    // Facing up the screen (a quarter turn), "ahead" is up.
    let mut app = action_app(4004, 3005, "1 0");
    let car = truck(&mut app, 4004);
    body(&mut app, 3005, Vec2::new(10.0, 5.0), std::f32::consts::FRAC_PI_2);
    run(app.world_mut(), "Grab", None, 0);
    let a = applied(&mut app);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].car, car);
    assert!(a[0].at.distance(Vec2::new(10.0, 6.0)) < 1e-5, "{:?}", a[0].at);
}

#[test]
fn without_a_target_it_is_a_world_point_in_c_axes() {
    let mut app = action_app(4004, 9999, "3 2");
    let car = truck(&mut app, 4004);
    run(app.world_mut(), "Grab", None, 0);
    assert_eq!(applied(&mut app), [ApplyGadget { car, at: Vec2::new(3.0, -2.0) }]);
}

#[test]
fn an_item_id_of_zero_is_the_entity_that_set_the_action_off() {
    // `Escort`'s `InitTruck`: owner 0, the truck itself.
    let mut app = action_app(0, 1004, "1 0");
    let car = truck(&mut app, 4001);
    body(&mut app, 1004, Vec2::new(4.0, 4.0), 0.0);
    run(app.world_mut(), "Grab", Some(4001), 0);
    assert_eq!(applied(&mut app), [ApplyGadget { car, at: Vec2::new(5.0, 4.0) }]);
    // No actor, no owner.
    run(app.world_mut(), "Grab", None, 0);
    assert!(applied(&mut app).is_empty());
}

#[test]
fn an_owner_with_no_gadget_or_none_at_all_does_nothing_and_still_succeeds() {
    let mut app = action_app(4004, 3005, "0 0");
    body(&mut app, 4004, Vec2::ZERO, 0.0);
    body(&mut app, 3005, Vec2::new(1.0, 1.0), 0.0);
    assert!(run(app.world_mut(), "Grab", None, 0), "C#'s handler returns nothing: true");
    assert!(applied(&mut app).is_empty());
    let mut app = action_app(4004, 3005, "0 0");
    assert!(run(app.world_mut(), "Grab", None, 0));
    assert!(applied(&mut app).is_empty());
}

// -- the gadgets: what happens there --

fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_message::<ApplyGadget>()
        .add_message::<PlaySound>()
        .add_message::<Explosion>()
        .add_message::<Boosted>()
        .add_systems(FixedUpdate, (apply_to_magnets, apply_to_accelerators, wake_towed).chain());
    app.finish();
    app.cleanup();
    app
}

fn car(app: &mut App, at: Vec2) -> Entity {
    app.world_mut()
        .spawn((RigidBody::Dynamic, Collider::rectangle(1.0, 1.0), Position(at), Magnet::new(Vec2::ZERO)))
        .id()
}

fn crate_at(app: &mut App, at: Vec2) -> Entity {
    app.world_mut().spawn((RigidBody::Dynamic, Collider::rectangle(1.0, 1.0), Position(at), Linkable)).id()
}

fn wall(app: &mut App, at: Vec2) -> Entity {
    app.world_mut().spawn((RigidBody::Static, Collider::rectangle(1.0, 4.0), Position(at))).id()
}

/// Settles the bodies into the spatial index, then works the gadget.
fn apply(app: &mut App, car: Entity, at: Vec2) {
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(ApplyGadget { car, at });
    app.update();
}

fn sounds(app: &mut App) -> Vec<&'static str> {
    app.world_mut().resource_mut::<Messages<PlaySound>>().drain().map(|c| c.name).collect()
}

fn towing(app: &App, car: Entity) -> Option<Entity> {
    app.world().get::<Magnet>(car).unwrap().towing()
}

#[test]
fn the_magnet_takes_what_is_in_the_way_of_the_place() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    let cargo = crate_at(&mut app, Vec2::new(4.0, 0.0));
    apply(&mut app, truck, Vec2::new(4.0, 0.0));
    assert_eq!(towing(&app, truck), Some(cargo));
    assert_eq!(sounds(&mut app), ["ChainStart", "ChainConnect"]);
}

#[test]
fn a_cargo_stays_where_the_map_put_it() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    let far = crate_at(&mut app, Vec2::new(0.0, 5.0));
    apply(&mut app, truck, Vec2::new(0.0, 5.0));
    assert_eq!(towing(&app, truck), Some(far));
    assert_eq!(app.world().get::<Position>(far).unwrap().0, Vec2::new(0.0, 5.0));
}

#[test]
fn the_ray_is_the_magnets_reach_long_not_as_far_as_the_place() {
    // The place is 2 units off, the crate 6 along the same way: found.
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    let cargo = crate_at(&mut app, Vec2::new(6.0, 0.0));
    apply(&mut app, truck, Vec2::new(2.0, 0.0));
    assert_eq!(towing(&app, truck), Some(cargo));
}

#[test]
fn a_place_out_of_reach_does_nothing() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    crate_at(&mut app, Vec2::new(REACH - 1.0, 0.0));
    apply(&mut app, truck, Vec2::new(REACH, 0.0));
    assert_eq!(towing(&app, truck), None);
    assert!(sounds(&mut app).is_empty(), "not even the start sound");
}

#[test]
fn a_wall_first_or_nothing_there_takes_nothing() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    wall(&mut app, Vec2::new(2.0, 0.0));
    crate_at(&mut app, Vec2::new(5.0, 0.0));
    apply(&mut app, truck, Vec2::new(5.0, 0.0));
    assert_eq!(towing(&app, truck), None);
    assert_eq!(sounds(&mut app), ["ChainStart", "ChainStart"], "started, then cancelled");

    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    apply(&mut app, truck, Vec2::new(3.0, 0.0));
    assert_eq!(towing(&app, truck), None);
}

#[test]
fn a_place_on_the_car_has_no_direction_and_finds_nothing() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    crate_at(&mut app, Vec2::new(2.0, 0.0));
    apply(&mut app, truck, Vec2::ZERO);
    assert_eq!(towing(&app, truck), None);
}

#[test]
fn only_what_the_magnet_may_grab_is_taken() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    app.world_mut().spawn((RigidBody::Static, Collider::rectangle(1.0, 1.0), Position(Vec2::new(3.0, 0.0))));
    apply(&mut app, truck, Vec2::new(3.0, 0.0));
    assert_eq!(towing(&app, truck), None, "not Linkable");
}

#[test]
fn sensors_are_not_in_the_way() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    app.world_mut().spawn((RigidBody::Static, Collider::rectangle(1.0, 1.0), Sensor, Position(Vec2::new(2.0, 0.0))));
    let cargo = crate_at(&mut app, Vec2::new(4.0, 0.0));
    apply(&mut app, truck, Vec2::new(4.0, 0.0));
    assert_eq!(towing(&app, truck), Some(cargo));
}

#[test]
fn what_another_magnet_has_is_not_taken() {
    let mut app = world();
    let first = car(&mut app, Vec2::new(0.0, 6.0));
    let second = car(&mut app, Vec2::new(0.0, -6.0));
    let cargo = crate_at(&mut app, Vec2::ZERO);
    apply(&mut app, first, Vec2::ZERO);
    assert_eq!(towing(&app, first), Some(cargo));
    sounds(&mut app);
    app.world_mut().write_message(ApplyGadget { car: second, at: Vec2::ZERO });
    app.update();
    assert_eq!(towing(&app, second), None);
    assert_eq!(towing(&app, first), Some(cargo));
    assert_eq!(sounds(&mut app), ["ChainStart", "ChainStart"]);
}

#[test]
fn a_magnet_with_cargo_lets_it_go_and_takes_nothing_new() {
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    let cargo = crate_at(&mut app, Vec2::new(3.0, 0.0));
    let other = crate_at(&mut app, Vec2::new(0.0, 3.0));
    apply(&mut app, truck, Vec2::new(3.0, 0.0));
    assert_eq!(towing(&app, truck), Some(cargo));
    app.world_mut().entity_mut(cargo).insert(LinearVelocity(Vec2::new(2.0, 1.0)));
    sounds(&mut app);
    app.world_mut().write_message(ApplyGadget { car: truck, at: Vec2::new(0.0, 3.0) });
    app.update();
    assert_eq!(towing(&app, truck), None, "{other:?} is not taken either");
    assert_eq!(app.world().get::<LinearVelocity>(cargo).unwrap().0, Vec2::ZERO);
    assert_eq!(sounds(&mut app), ["ChainStart", "ChainBrake", "ChainStart"]);
}

#[test]
fn the_accelerator_works_on_a_place_within_a_unit_and_not_beyond() {
    let mut app = world();
    let car = app
        .world_mut()
        .spawn((RigidBody::Dynamic, Collider::rectangle(1.0, 1.0), Position(Vec2::ZERO), Rotation::radians(std::f32::consts::FRAC_PI_2), LinearVelocity::ZERO, Accelerator::new()))
        .id();
    apply(&mut app, car, Vec2::new(1.5, 0.0));
    let v = app.world().get::<LinearVelocity>(car).unwrap().0;
    assert!(v.length() < 1e-3, "{v}: too far");
    app.world_mut().write_message(ApplyGadget { car, at: Vec2::new(0.0, 0.5) });
    app.update();
    let v = app.world().get::<LinearVelocity>(car).unwrap().0;
    assert!(v.distance(Vec2::new(0.0, 30.0)) < 1.0, "{v}: along the way it faces");
    let boosts: Vec<_> = app.world_mut().resource_mut::<Messages<Boosted>>().drain().collect();
    assert_eq!(boosts.len(), 1);
    assert_eq!(boosts[0].rate, 1.0);
    assert!(!app.world().get::<Accelerator>(car).unwrap().ready(), "spent until the energy is whole");
    // Not ready: the second is a fizzle.
    app.world_mut().write_message(ApplyGadget { car, at: Vec2::ZERO });
    app.update();
    let boosts: Vec<_> = app.world_mut().resource_mut::<Messages<Boosted>>().drain().collect();
    assert_eq!(boosts.iter().map(|b| b.rate).collect::<Vec<_>>(), [0.25]);
}

#[test]
fn a_static_turret_is_dynamic_while_the_rope_has_it_and_static_again_after() {
    // Derby's truck takes its turret (`CanMove` false, `CanLink` true).
    let mut app = world();
    let truck = car(&mut app, Vec2::ZERO);
    let turret = app
        .world_mut()
        .spawn((RigidBody::Static, Collider::rectangle(1.0, 1.0), Position(Vec2::new(3.0, 0.0)), Linkable, StaticWhenFree))
        .id();
    apply(&mut app, truck, Vec2::new(3.0, 0.0));
    assert_eq!(towing(&app, truck), Some(turret));
    app.update();
    assert_eq!(*app.world().get::<RigidBody>(turret).unwrap(), RigidBody::Dynamic);
    app.world_mut().entity_mut(turret).insert(LinearVelocity(Vec2::new(1.0, 1.0)));
    app.world_mut().write_message(ApplyGadget { car: truck, at: Vec2::new(3.0, 0.0) });
    app.update();
    assert_eq!(towing(&app, truck), None);
    assert_eq!(*app.world().get::<RigidBody>(turret).unwrap(), RigidBody::Static);
    assert_eq!(app.world().get::<LinearVelocity>(turret).unwrap().0, Vec2::ZERO);
}

#[test]
fn a_towed_turret_moves_when_the_rope_pulls_it() {
    // The static body gets its mass back when it turns dynamic.
    let mut app = world();
    app.add_systems(FixedUpdate, |mut q: Query<Forces, With<StaticWhenFree>>| {
        for mut f in &mut q {
            f.apply_force(Vec2::new(-40.0, 0.0));
        }
    });
    let truck = car(&mut app, Vec2::ZERO);
    let turret = app
        .world_mut()
        .spawn((RigidBody::Static, Collider::rectangle(1.0, 1.0), Mass(3.875), Position(Vec2::new(3.0, 0.0)), Linkable, StaticWhenFree))
        .id();
    apply(&mut app, truck, Vec2::new(3.0, 0.0));
    assert_eq!(towing(&app, truck), Some(turret));
    for _ in 0..30 {
        app.update();
    }
    let x = app.world().get::<Position>(turret).unwrap().0.x;
    assert!(x < 2.9, "pulled towards the truck: {x}");
}
