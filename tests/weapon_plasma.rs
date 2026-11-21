//! The Plasma gun (`weapon::model::Plasma`, `weapon::plasma`): its presets,
//! its reload and spread, and its bullet's flight, blow and push in a small
//! physics world.

use std::f32::consts::PI;
use std::time::Duration;

use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use tilerider::car::control::Team;
use tilerider::explosion::{Health, Hit};
use tilerider::weapon::model::{
    AimLook, Energy, Kind, MINIGUN, PLASMA, PLASMA_SLOW, Plasma, for_type, plasma_directions, plasma_recoil,
};
use tilerider::weapon::plasma::{BULLET_RADIUS, Flight, PlasmaBullet, PlasmaEnd, fly, move_bullets};

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_types_are_the_plasma_gun_the_same_for_everyone() {
    assert_eq!(for_type("PlasmaGun", false), Some((Kind::Plasma, PLASMA)));
    assert_eq!(for_type("PlasmaGun", true), Some((Kind::Plasma, PLASMA)));
    assert_eq!(for_type("PlasmaGunSlow", false), Some((Kind::Plasma, PLASMA_SLOW)));
    assert_eq!(for_type("PlasmaGunSlow", true), Some((Kind::Plasma, PLASMA_SLOW)));
    // 8 damage, a push of 18, one straight bullet, 0.1 energy.
    assert_eq!((PLASMA.damage, PLASMA.impulse, PLASMA.bullets, PLASMA.dispersion, PLASMA.energy), (8, 18.0, 1, 0.0, 0.1));
    // Red and fast every half second; yellow and half as fast every second.
    assert_eq!((PLASMA.reload, PLASMA.bullet_speed, PLASMA.bullet_color), (0.5, 32.0, [1.0, 0.0, 0.0]));
    assert_eq!((PLASMA_SLOW.reload, PLASMA_SLOW.bullet_speed, PLASMA_SLOW.bullet_color), (1.0, 16.0, [1.0, 1.0, 0.0]));
    assert_eq!(PLASMA_SLOW.damage, PLASMA.damage);
}

#[test]
fn its_shot_flies_the_others_arrive_at_once() {
    assert!(Kind::Plasma.is_complex());
    assert!(!Kind::Plasma.auto_fire());
    assert!(!Kind::Plasma.is_immediate());
    for kind in [Kind::Minigun, Kind::Railgun, Kind::Hammer, Kind::Zombie] {
        assert!(kind.is_immediate(), "{kind:?}");
    }
}

/// Seconds at which a trigger held from the start fires, over `secs`.
fn shots(options: &tilerider::weapon::model::Options, mut energy: Option<&mut Energy>, secs: f32) -> Vec<f32> {
    let mut gun = Plasma::new(options, 0.0);
    let mut fired = Vec::new();
    for step in 0..(secs / DT).round() as usize {
        if gun.pull(options, energy.as_deref_mut()) {
            fired.push(step as f32 * DT);
        }
        gun.step(DT);
        if let Some(e) = energy.as_deref_mut() {
            e.step(DT);
        }
    }
    fired
}

#[test]
fn held_it_fires_once_a_reload() {
    let fast = shots(&PLASMA, None, 5.0);
    assert_eq!(fast.len(), 10, "{fast:?}");
    for pair in fast.windows(2) {
        assert!((pair[1] - pair[0] - 0.5).abs() < 0.02, "{fast:?}");
    }
    let slow = shots(&PLASMA_SLOW, None, 5.0);
    assert_eq!(slow.len(), 5, "{slow:?}");
}

#[test]
fn the_first_shot_waits_its_random_part_of_the_reload() {
    let mut gun = Plasma::new(&PLASMA, 0.5);
    assert!(!gun.can_fire(&PLASMA, None));
    for _ in 0..15 {
        gun.step(DT);
    }
    assert!(gun.pull(&PLASMA, None));
}

#[test]
fn the_player_pays_a_tenth_of_the_store_a_shot_and_a_dry_store_fires_nothing() {
    let mut energy = Energy::default();
    let mut gun = Plasma::new(&PLASMA, 0.0);
    assert!(gun.pull(&PLASMA, Some(&mut energy)));
    assert!((energy.current - 0.9).abs() < 1e-6);
    energy.current = 0.05;
    gun.step(1.0);
    assert!(!gun.pull(&PLASMA, Some(&mut energy)));
    assert!(energy.overloaded);
    // The aim line shows it low while it cannot pay, normal while it
    // reloads with the energy there.
    assert_eq!(gun.look(&PLASMA, Some(&mut energy)), AimLook::Low);
    let mut full = Energy::default();
    assert!(gun.pull(&PLASMA, Some(&mut full)));
    assert_eq!(gun.look(&PLASMA, Some(&mut full)), AimLook::Normal);
}

#[test]
fn bullets_spread_randomly_alone_and_evenly_together() {
    // The presets: one bullet along the aim (no dispersion).
    assert_eq!(plasma_directions(1.0, &PLASMA, || 0.9), vec![1.0]);
    // One bullet within ± the dispersion.
    let wide = tilerider::weapon::model::Options { dispersion: PI / 16.0, ..PLASMA };
    let one = plasma_directions(0.0, &wide, || 1.0);
    assert!((one[0] - PI / 16.0).abs() < 1e-6);
    // Three: both edges and the middle, whatever the random numbers.
    let three = plasma_directions(0.0, &tilerider::weapon::model::Options { bullets: 3, ..wide }, || 0.3);
    assert_eq!(three.len(), 3);
    assert!((three[0] - PI / 16.0).abs() < 1e-6 && three[1].abs() < 1e-6 && (three[2] + PI / 16.0).abs() < 1e-6, "{three:?}");
    assert!(plasma_directions(0.0, &tilerider::weapon::model::Options { bullets: 0, ..PLASMA }, || 0.5).is_empty());
    // The kick is set to 1 to 5 pixels.
    assert_eq!((plasma_recoil(0.0), plasma_recoil(1.0)), (1.0, 5.0));
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
    // The plugins' `finish` (the physics' diagnostics), as `App::run` does.
    app.finish();
    app.cleanup();
    // Let the physics set up its colliders and the spatial index.
    for _ in 0..3 {
        app.update();
    }
}

fn wall(app: &mut App, at: Vec2, size: Vec2) -> Entity {
    app.world_mut()
        .spawn((RigidBody::Static, Collider::rectangle(size.x, size.y), Transform::from_translation(at.extend(0.0))))
        .id()
}

fn flight(app: &mut App, owner: Entity, from: Vec2, velocity: Vec2, passes: Option<Entity>) -> Flight {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let passes = |e| Some(e) == passes;
            fly(&spatial, &passes, owner, from, velocity, DT)
        })
        .unwrap()
}

#[test]
fn a_bullet_flies_its_speed_a_step_and_strikes_with_its_edge() {
    let mut app = world();
    let target = wall(&mut app, Vec2::new(11.0, 0.0), Vec2::new(2.0, 10.0));
    let owner = app.world_mut().spawn_empty().id();
    settle(&mut app);
    // 32 units a second: 0.533 a step, in the open.
    let Flight::Flies(to) = flight(&mut app, owner, Vec2::ZERO, Vec2::X * 32.0, None) else {
        panic!("struck nothing");
    };
    assert!((to.x - 32.0 * DT).abs() < 1e-5, "{to}");
    // Its circle touches the wall's face at x = 10 within the step.
    match flight(&mut app, owner, Vec2::new(9.2, 0.0), Vec2::X * 32.0, None) {
        Flight::Strikes { entity, at } => {
            assert_eq!(entity, target);
            assert!((at.x - (10.0 - BULLET_RADIUS)).abs() < 1e-3, "{at}");
        }
        other => panic!("{other:?}"),
    }
    // What bullets fly through, it flies through.
    assert!(matches!(flight(&mut app, owner, Vec2::new(9.2, 0.0), Vec2::X * 32.0, Some(target)), Flight::Flies(_)));
}

#[test]
fn a_bullet_flies_out_of_its_shooter_and_strikes_what_it_starts_in() {
    let mut app = world();
    let shooter = wall(&mut app, Vec2::ZERO, Vec2::splat(2.0));
    let other = wall(&mut app, Vec2::new(5.0, 0.0), Vec2::splat(2.0));
    settle(&mut app);
    assert!(matches!(flight(&mut app, shooter, Vec2::ZERO, Vec2::X * 16.0, None), Flight::Flies(_)));
    match flight(&mut app, shooter, Vec2::new(5.0, 0.0), Vec2::X * 16.0, None) {
        Flight::Strikes { entity, at } => {
            assert_eq!(entity, other);
            assert_eq!(at, Vec2::new(5.0, 0.0));
        }
        other => panic!("{other:?}"),
    }
}

/// What the bullets did, collected each update.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<Hit>,
    ends: Vec<PlasmaEnd>,
}

fn collect(mut hits: MessageReader<Hit>, mut ends: MessageReader<PlasmaEnd>, mut seen: ResMut<Seen>) {
    seen.hits.extend(hits.read().copied());
    seen.ends.extend(ends.read().copied());
}

/// A dynamic car of `team` with health at `at`.
fn car(app: &mut App, at: Vec2, team: u8) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::circle(1.0),
            Transform::from_translation(at.extend(0.0)),
            Health::full(20),
            Team(team),
        ))
        .id()
}

fn fire(app: &mut App, from: Vec2, owner: Entity, team: u8) -> Entity {
    let bullet = PlasmaBullet::new(from, 0.0, owner, Some(Team(team)), &PLASMA_SLOW);
    app.world_mut().spawn(bullet).id()
}

fn bullets_world() -> App {
    let mut app = world();
    app.add_message::<Hit>()
        .add_message::<PlasmaEnd>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (move_bullets, collect).chain());
    app
}

#[test]
fn a_bullet_hurts_and_pushes_an_enemy_and_is_gone() {
    let mut app = bullets_world();
    let shooter = car(&mut app, Vec2::ZERO, 1);
    let enemy = car(&mut app, Vec2::new(6.0, 0.0), 0);
    settle(&mut app);
    let bullet = fire(&mut app, Vec2::new(1.5, 0.0), shooter, 1);
    // 16 units a second over 3.5 units to the enemy's edge less its own
    // radius: about 0.19 s.
    let mut steps = 0;
    while app.world().get_entity(bullet).is_ok() && steps < 60 {
        app.update();
        steps += 1;
    }
    assert!(app.world().get_entity(bullet).is_err(), "still flying");
    assert!((10..=16).contains(&steps), "{steps} steps");
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.hits.len(), 1);
    let hit = seen.hits[0];
    assert_eq!((hit.target, hit.damage, hit.by, hit.from), (enemy, 8, Some(shooter), 0.0));
    assert_eq!(seen.ends.len(), 1);
    assert!((seen.ends[0].at.x - (5.0 - BULLET_RADIUS)).abs() < 0.05, "{:?}", seen.ends[0]);
    // Shoved along the bullet's flight.
    app.update();
    let v = app.world().get::<LinearVelocity>(enemy).unwrap().0;
    assert!(v.x > 0.0, "{v}");
}

#[test]
fn a_bullet_pushes_a_friend_unhurt() {
    let mut app = bullets_world();
    let shooter = car(&mut app, Vec2::ZERO, 1);
    let friend = car(&mut app, Vec2::new(6.0, 0.0), 1);
    settle(&mut app);
    let bullet = fire(&mut app, Vec2::new(1.5, 0.0), shooter, 1);
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().get_entity(bullet).is_err());
    let seen = app.world().resource::<Seen>();
    assert!(seen.hits.is_empty());
    assert_eq!(seen.ends.len(), 1);
    assert!(app.world().get::<LinearVelocity>(friend).unwrap().0.x > 0.0);
}

#[test]
fn the_minigun_preset_has_no_flying_bullet() {
    assert_eq!(MINIGUN.bullet_speed, 0.0);
}
