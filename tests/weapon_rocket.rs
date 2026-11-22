//! The rocket launchers (`weapon::model` presets, `weapon::rocket`): their
//! types, the rocket's launch, speed, homing and blast, in a small physics
//! world.

use std::f32::consts::PI;
use std::time::Duration;

use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::car::control::Team;
use tilerider::explosion::{Explosion, Health, Hit};
use tilerider::weapon::model::{
    Kind, ROCKET_CHASING, ROCKET_CHASING_360, ROCKET_DIRECT, Railgun, for_type,
};
use tilerider::weapon::rocket::{
    BLAST_FORCE, BLAST_RANGE, Candidate, Flight, LAUNCH_IMPULSE, MASS, MAX_SPEED, MAX_TRANSLATION,
    Rocket, RocketLaunched, SIZE, rockets_hit, STEP_FORCE, fly, fly_rockets, integrate, launch_impulse, limit_speed, nearest_goal, steer,
};

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_types_are_the_three_launchers_the_same_for_everyone() {
    for local in [false, true] {
        assert_eq!(for_type("RocketlauncherDirect", local), Some((Kind::Rocket, ROCKET_DIRECT)));
        assert_eq!(for_type("RocketlauncherChasing", local), Some((Kind::Rocket, ROCKET_CHASING)));
        assert_eq!(for_type("RocketlauncherChasing360", local), Some((Kind::Rocket, ROCKET_CHASING_360)));
    }
    // A straight rocket's blast does 16, a homing one's 20.
    assert_eq!((ROCKET_DIRECT.damage, ROCKET_DIRECT.homing), (16, false));
    assert_eq!((ROCKET_CHASING.damage, ROCKET_CHASING.homing), (20, true));
    assert_eq!((ROCKET_CHASING_360.damage, ROCKET_CHASING_360.homing), (20, true));
    // Every 1.2 s, straight, 0.1 of the energy; the 360 turns a half turn
    // either way and costs 0.5.
    for o in [ROCKET_DIRECT, ROCKET_CHASING] {
        assert_eq!((o.reload, o.dispersion, o.energy, o.max_angle, o.range), (1.2, 0.0, 0.1, PI / 3.0, 32.0));
    }
    assert_eq!((ROCKET_CHASING_360.reload, ROCKET_CHASING_360.energy, ROCKET_CHASING_360.max_angle), (1.2, 0.5, PI));
}

#[test]
fn it_aims_but_its_shot_flies_and_the_gun_is_not_automatic() {
    assert!(Kind::Rocket.is_complex());
    assert!(!Kind::Rocket.auto_fire());
    assert!(!Kind::Rocket.is_immediate());
}

#[test]
fn a_launcher_fires_once_a_reload_on_accurate_pulses_like_the_railgun() {
    // `weapon::Weapon` fires it with the Railgun's timer: the reload of the
    // options, the energy of the options.
    let mut gun = Railgun::new(&ROCKET_DIRECT, 0.0);
    assert!(gun.pull(&ROCKET_DIRECT, None));
    assert!(!gun.pull(&ROCKET_DIRECT, None));
    for _ in 0..71 {
        gun.step(DT);
    }
    assert!(!gun.can_fire(&ROCKET_DIRECT, None));
    gun.step(DT);
    gun.step(DT);
    assert!(gun.can_fire(&ROCKET_DIRECT, None));
}

#[test]
fn a_rocket_is_born_with_the_impulse_of_a_launch_and_a_tenth_of_the_shooters_speed() {
    let shooter = Vec2::new(0.0, 10.0);
    let impulse = launch_impulse(0.0, shooter);
    assert_eq!(impulse, Vec2::new(LAUNCH_IMPULSE, 1.0));
    let rocket = Rocket::new(Vec2::ZERO, 0.0, Entity::PLACEHOLDER, None, shooter, 16, false);
    // It points where the impulse does, and moves at the impulse over its mass.
    assert!((rocket.direction - impulse.to_angle()).abs() < 1e-6);
    assert!((rocket.velocity - impulse / MASS).length() < 1e-3);
    assert!((rocket.force_direction - impulse.normalize()).length() < 1e-6);
    // 1 by 0.27 units: a rectangle of density 1.
    assert!((SIZE.x - 1.015625).abs() < 1e-6 && (SIZE.y - 0.265625).abs() < 1e-6);
    assert!((MASS - 0.2698).abs() < 1e-3, "{MASS}");
}

#[test]
fn farseer_moves_a_body_two_units_a_step_at_most_and_the_rocket_is_cut_to_47() {
    let born = launch_impulse(0.0, Vec2::ZERO) / MASS;
    let v = integrate(born, Vec2::ZERO, DT);
    assert!((v.length() * DT - MAX_TRANSLATION).abs() < 1e-4, "{v}");
    assert!((limit_speed(v).length() - MAX_SPEED).abs() < 1e-4);
    // Slower than the top speed it keeps it.
    assert_eq!(limit_speed(Vec2::new(10.0, 0.0)), Vec2::new(10.0, 0.0));
}

#[test]
fn the_force_and_the_damping_keep_it_at_speed() {
    // A force of 32 on a mass of 0.27 adds 118 units a second² (1.97 a
    // step); the damping of 0.95 takes 0.016 of the speed.
    let v = integrate(Vec2::new(MAX_SPEED, 0.0), Vec2::X * STEP_FORCE, DT);
    assert!((v.x - (MAX_SPEED + STEP_FORCE / MASS * DT) * (1.0 - DT * 0.95)).abs() < 1e-4);
    assert!(v.x > MAX_SPEED - 0.1, "{v}");
    // Coasting it slows: 47 → 46.26 in a step.
    let c = integrate(Vec2::new(MAX_SPEED, 0.0), Vec2::ZERO, DT);
    assert!((c.x - MAX_SPEED * (1.0 - DT * 0.95)).abs() < 1e-4);
}

#[test]
fn the_force_points_at_the_goal_and_nowhere_at_the_same_point() {
    assert_eq!(steer(Vec2::ZERO, Vec2::new(0.0, 5.0)), Vec2::Y);
    assert_eq!(steer(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0)), Vec2::ZERO);
}

fn cand(i: u32, at: Vec2, velocity: Vec2, team: u8) -> Candidate {
    Candidate {
        entity: Entity::from_raw_u32(i).unwrap(),
        at,
        velocity,
        team: Some(Team(team)),
    }
}

#[test]
fn the_goal_is_the_nearest_moving_car_of_another_team() {
    let owner = Entity::from_raw_u32(1).unwrap();
    let me = Some(Team(1));
    let moving = Vec2::X;
    let cars = [
        // Me, though near and moving.
        cand(1, Vec2::new(1.0, 0.0), moving, 1),
        // A friend.
        cand(2, Vec2::new(2.0, 0.0), moving, 1),
        // A foe that does not move: a turret is never chased.
        cand(3, Vec2::new(3.0, 0.0), Vec2::ZERO, 0),
        // A foe barely moving (0.3 a second is the least: its square must pass 0.1).
        cand(4, Vec2::new(4.0, 0.0), Vec2::new(0.3, 0.0), 0),
        cand(5, Vec2::new(9.0, 0.0), moving, 0),
        cand(6, Vec2::new(7.0, 0.0), moving, 0),
    ];
    assert_eq!(nearest_goal(Vec2::ZERO, owner, me, cars), Some(cars[5].entity));
    // The slow one is nearest of them when it is faster than 0.316.
    let faster = [cand(4, Vec2::new(4.0, 0.0), Vec2::new(0.32, 0.0), 0), cars[5]];
    assert_eq!(nearest_goal(Vec2::ZERO, owner, me, faster), Some(faster[0].entity));
    assert_eq!(nearest_goal(Vec2::ZERO, owner, me, [cars[0], cars[1], cars[2]]), None);
    // No team on the shooter spares no one.
    assert_eq!(nearest_goal(Vec2::ZERO, owner, None, [cars[1]]), Some(cars[1].entity));
}

/// A world without gravity, its fixed step 60 Hz, each update one step.
fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)));
    app
}

fn settle(app: &mut App) {
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
}

fn wall(app: &mut App, at: Vec2, size: Vec2) -> Entity {
    app.world_mut()
        .spawn((RigidBody::Static, Collider::rectangle(size.x, size.y), Transform::from_translation(at.extend(0.0))))
        .id()
}

fn flight(app: &mut App, owner: Entity, from: Vec2, direction: f32, velocity: Vec2, passes: Option<Entity>) -> Flight {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let passes = |e| Some(e) == passes;
            fly(&spatial, &passes, owner, from, direction, velocity, DT)
        })
        .unwrap()
}

#[test]
fn a_rocket_flies_its_speed_a_step_and_strikes_with_its_nose() {
    let mut app = world();
    let target = wall(&mut app, Vec2::new(11.0, 0.0), Vec2::new(2.0, 10.0));
    let owner = app.world_mut().spawn_empty().id();
    settle(&mut app);
    let Flight::Flies(to) = flight(&mut app, owner, Vec2::ZERO, 0.0, Vec2::X * 47.0, None) else {
        panic!("struck nothing");
    };
    assert!((to.x - 47.0 * DT).abs() < 1e-5, "{to}");
    // Its nose (half its length ahead of its middle) touches the face at x = 10.
    match flight(&mut app, owner, Vec2::new(9.0, 0.0), 0.0, Vec2::X * 47.0, None) {
        Flight::Strikes { entity, at } => {
            assert_eq!(entity, target);
            assert!((at.x - (10.0 - SIZE.x / 2.0)).abs() < 1e-3, "{at}");
        }
        other => panic!("{other:?}"),
    }
    // Turned across its flight it touches with its width.
    match flight(&mut app, owner, Vec2::new(9.4, 0.0), PI / 2.0, Vec2::X * 47.0, None) {
        Flight::Strikes { at, .. } => assert!((at.x - (10.0 - SIZE.y / 2.0)).abs() < 1e-3, "{at}"),
        other => panic!("{other:?}"),
    }
    // What weapons fly through, it flies through.
    assert!(matches!(flight(&mut app, owner, Vec2::new(9.0, 0.0), 0.0, Vec2::X * 47.0, Some(target)), Flight::Flies(_)));
}

#[test]
fn a_rocket_flies_out_of_its_shooter() {
    let mut app = world();
    let shooter = wall(&mut app, Vec2::ZERO, Vec2::splat(2.0));
    settle(&mut app);
    assert!(matches!(flight(&mut app, shooter, Vec2::ZERO, 0.0, Vec2::X * 47.0, None), Flight::Flies(_)));
    assert!(matches!(flight(&mut app, Entity::PLACEHOLDER, Vec2::ZERO, 0.0, Vec2::X * 47.0, None), Flight::Strikes { .. }));
}

/// What the rockets did, collected each update.
#[derive(Resource, Default)]
struct Seen {
    blasts: Vec<(u32, Explosion)>,
    step: u32,
}

fn collect(mut blasts: MessageReader<Explosion>, mut seen: ResMut<Seen>) {
    seen.step += 1;
    let step = seen.step;
    seen.blasts.extend(blasts.read().map(|e| (step, *e)));
}

fn rockets_world() -> App {
    let mut app = world();
    app.add_message::<Explosion>()
        .add_message::<Hit>()
        .add_message::<RocketLaunched>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (rockets_hit, fly_rockets, collect).chain());
    app
}


fn car(app: &mut App, at: Vec2, team: u8, velocity: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Kinematic,
            Collider::circle(1.0),
            Transform::from_translation(at.extend(0.0)),
            LinearVelocity(velocity),
            Health::full(20),
            Team(team),
        ))
        .id()
}

fn fire(app: &mut App, at: Vec2, direction: f32, owner: Entity, damage: i32, homing: bool) -> Entity {
    app.world_mut().spawn(Rocket::new(at, direction, owner, Some(Team(1)), Vec2::ZERO, damage, homing)).id()
}

fn steps(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

#[test]
fn in_the_open_a_straight_rocket_goes_47_units_a_second_and_blows_up_after_two_seconds_active() {
    let mut app = rockets_world();
    let owner = app.world_mut().spawn_empty().id();
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::ZERO, 0.0, owner, 16, false);
    app.update();
    // The first step is the launch impulse cut to two units.
    let first = app.world().get::<Rocket>(rocket).unwrap().clone();
    assert!((first.at.x - 2.0).abs() < 1e-3, "{:?}", first.at);
    assert!((first.velocity.length() - MAX_SPEED).abs() < 1e-3);
    assert!(!first.active);
    // Active after 0.1 s (seven steps: six are not past it).
    steps(&mut app, 5);
    assert!(!app.world().get::<Rocket>(rocket).unwrap().active);
    app.update();
    let r = app.world().get::<Rocket>(rocket).unwrap().clone();
    assert!(r.active);
    // Then it flies straight at about its top speed (the damping takes a
    // hair, the force gives it back).
    steps(&mut app, 30);
    let r = app.world().get::<Rocket>(rocket).unwrap();
    assert!(r.velocity.y.abs() < 1e-3 && (r.velocity.x - MAX_SPEED).abs() < 1.0, "{:?}", r.velocity);
    // Two seconds active: it blows up in the air.
    while app.world().get_entity(rocket).is_ok() && app.world().resource::<Seen>().step < 400 {
        app.update();
    }
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.blasts.len(), 1);
    let (step, blast) = seen.blasts[0];
    // Fired in step 4, the first of the rocket's own: 0.1 s + 2 s + a step.
    assert!((126..=130).contains(&step) || (120..=135).contains(&step), "{step}");
    assert!(blast.at.x > 80.0 && blast.at.x < 100.0 && blast.at.y.abs() < 1e-3, "{:?}", blast.at);
    assert_eq!((blast.blast.range, blast.blast.force, blast.blast.damage, blast.owner), (BLAST_RANGE, BLAST_FORCE, 16, Some(owner)));
}

#[test]
fn it_blows_up_the_step_after_it_touches_a_wall_where_it_stopped() {
    let mut app = rockets_world();
    let owner = app.world_mut().spawn_empty().id();
    wall(&mut app, Vec2::new(12.0, 0.0), Vec2::new(2.0, 20.0));
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::ZERO, 0.0, owner, 20, true);
    let mut touched = None;
    for _ in 1..60 {
        app.update();
        if app.world().get_entity(rocket).is_ok() && app.world().get::<Rocket>(rocket).unwrap().struck && touched.is_none() {
            touched = Some(app.world().resource::<Seen>().step);
        }
        if app.world().get_entity(rocket).is_err() {
            break;
        }
    }
    let touched = touched.expect("never touched the wall");
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.blasts.len(), 1);
    let (step, blast) = seen.blasts[0];
    // Its nose is on the wall's face at x = 11; the blast is next step.
    assert!((blast.at.x - (11.0 - SIZE.x / 2.0)).abs() < 1e-3, "{:?}", blast.at);
    assert_eq!(step, touched + 1, "touched in {touched}, blast in {step}");
    assert_eq!(blast.blast.damage, 20);
}

#[test]
fn a_homing_rocket_turns_towards_the_nearest_moving_enemy() {
    let mut app = rockets_world();
    let owner = car(&mut app, Vec2::new(-30.0, 0.0), 1, Vec2::ZERO);
    // A foe off to the side, moving; one that does not move is nearer.
    let foe = car(&mut app, Vec2::new(30.0, 30.0), 0, Vec2::new(0.0, 1.0));
    car(&mut app, Vec2::new(15.0, 0.0), 0, Vec2::ZERO);
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::new(-28.0, 0.0), 0.0, owner, 20, true);
    steps(&mut app, 8);
    let r = app.world().get::<Rocket>(rocket).unwrap().clone();
    assert!(r.active);
    assert_eq!(r.goal, Some(foe));
    // Its force points at the foe, and the rocket bends towards it.
    steps(&mut app, 40);
    let r = app.world().get::<Rocket>(rocket).unwrap().clone();
    assert!(r.velocity.y > 5.0, "{:?}", r.velocity);
    assert!((r.velocity.length() - MAX_SPEED).abs() < 2.0, "{:?}", r.velocity);
    // It faces where it flies.
    assert!((r.direction - r.velocity.to_angle()).abs() < 0.1);
}

#[test]
fn a_straight_rocket_never_turns() {
    let mut app = rockets_world();
    let owner = car(&mut app, Vec2::new(-30.0, 0.0), 1, Vec2::ZERO);
    car(&mut app, Vec2::new(30.0, 30.0), 0, Vec2::new(0.0, 1.0));
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::new(-28.0, 0.0), 0.0, owner, 16, false);
    steps(&mut app, 60);
    let r = app.world().get::<Rocket>(rocket).unwrap();
    assert_eq!(r.goal, None);
    assert!(r.velocity.y.abs() < 1e-3 && r.direction == 0.0);
}

#[test]
fn a_homing_rocket_chooses_again_every_second() {
    let mut app = rockets_world();
    let owner = car(&mut app, Vec2::new(-30.0, 0.0), 1, Vec2::ZERO);
    let far = car(&mut app, Vec2::new(60.0, 0.0), 0, Vec2::new(0.0, 1.0));
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::new(-28.0, 0.0), 0.0, owner, 20, true);
    steps(&mut app, 8);
    assert_eq!(app.world().get::<Rocket>(rocket).unwrap().goal, Some(far));
    // A nearer foe appears: not chosen before the second is up.
    let near = car(&mut app, Vec2::new(20.0, 5.0), 0, Vec2::new(0.0, 1.0));
    steps(&mut app, 30);
    assert_eq!(app.world().get::<Rocket>(rocket).unwrap().goal, Some(far));
    steps(&mut app, 30);
    assert_eq!(app.world().get::<Rocket>(rocket).unwrap().goal, Some(near));
}

#[test]
fn a_blow_on_a_rocket_sets_it_off_next_step() {
    let mut app = rockets_world();
    let owner = app.world_mut().spawn_empty().id();
    settle(&mut app);
    let rocket = fire(&mut app, Vec2::ZERO, 0.0, owner, 16, false);
    steps(&mut app, 3);
    assert!(app.world().resource::<Seen>().blasts.is_empty());
    app.world_mut().write_message(Hit {
        target: rocket,
        damage: 3,
        from: 0.0,
        by: None,
    });
    steps(&mut app, 2);
    assert_eq!(app.world().resource::<Seen>().blasts.len(), 1);
    assert!(app.world().get_entity(rocket).is_err());
}
