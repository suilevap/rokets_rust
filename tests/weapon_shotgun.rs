//! The Shotgun (`weapon::model::SHOTGUN`, `weapon::shotgun`): its type and
//! numbers, its rhythm, the spread of its pellets and the kick, and a
//! blast's pellets and kick in a small physics world.

use std::f32::consts::PI;
use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::car::control::Team;
use tilerider::explosion::{Health, Hit};
use tilerider::weapon::model::{
    AimLook, Energy, Kind, MINIGUN, Options, RAILGUN, Railgun, SHOTGUN, SHOTGUN_RECOIL, for_type, shotgun_directions, shotgun_kick,
};
use tilerider::weapon::shotgun::{Landed, ShotgunBlast, ShotgunShot, shotgun_blasts};
use tilerider::weapon::{IgnoresWeapons, Weapon, has_aim_line, sprite_of, textures_of};

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_type_is_the_shotgun_the_same_for_everyone() {
    assert_eq!(for_type("Shotgun", false), Some((Kind::Shotgun, SHOTGUN)));
    assert_eq!(for_type("Shotgun", true), Some((Kind::Shotgun, SHOTGUN)));
    // `DamageConst.Shotgun` 4 and `ShotgunImpulse` 6 a pellet, five pellets.
    assert_eq!((SHOTGUN.damage, SHOTGUN.impulse, SHOTGUN.bullets), (4, 6.0, 5));
    // ±π/8 of dispersion, a 2 s reload, the default 0.1 energy and 32
    // units of range, half a turn either way.
    assert_eq!((SHOTGUN.dispersion, SHOTGUN.reload, SHOTGUN.energy, SHOTGUN.range), (PI / 8.0, 2.0, 0.1, 32.0));
    assert_eq!(SHOTGUN.max_angle, PI);
    // `FeedbackImpulse = ShotgunImpulse * 5 / 2`.
    assert_eq!(SHOTGUN.feedback, 15.0);
    assert_eq!(SHOTGUN_RECOIL, 30.0);
    // No other gun kicks the car.
    assert_eq!(MINIGUN.feedback, 0.0);
    assert_eq!(RAILGUN.feedback, 0.0);
}

#[test]
fn it_is_a_barrel_that_aims_and_hits_at_once_with_a_sprite_and_an_aim_line() {
    assert!(Kind::Shotgun.is_complex());
    assert!(Kind::Shotgun.is_immediate());
    assert!(!Kind::Shotgun.auto_fire());
    assert!(has_aim_line(Kind::Shotgun));
    let sprite = sprite_of(Kind::Shotgun).unwrap();
    assert_eq!((sprite.name, sprite.size, sprite.origin), ("Weapons\\Shotgun", Vec2::new(40.0, 11.0), Vec2::new(9.0, 5.0)));
    assert_eq!(textures_of(Kind::Shotgun), vec!["Weapons\\Shotgun"]);
}

#[test]
fn it_fires_once_a_two_second_reload_on_a_pull() {
    // It takes the Railgun's rhythm (`_fireOnlyAccurate`).
    let mut gun = Railgun::new(&SHOTGUN, 0.0);
    let mut shots = Vec::new();
    for i in 0..(7.0 / DT) as usize {
        if gun.pull(&SHOTGUN, None) {
            shots.push(i as f32 * DT);
        }
        gun.step(DT);
    }
    assert_eq!(shots.len(), 4, "{shots:?}");
    for pair in shots.windows(2) {
        assert!((pair[1] - pair[0] - 2.0).abs() < 0.05, "{shots:?}");
    }
}

#[test]
fn the_player_pays_a_tenth_of_the_store_a_blast_and_a_dry_store_fires_nothing() {
    let mut energy = Energy::default();
    let mut gun = Railgun::new(&SHOTGUN, 0.0);
    assert!(gun.pull(&SHOTGUN, Some(&mut energy)));
    assert!((energy.current - 0.9).abs() < 1e-6);
    energy.current = 0.05;
    gun.step(3.0);
    assert!(!gun.pull(&SHOTGUN, Some(&mut energy)));
    assert!(energy.overloaded);
    // Low on energy, then the reload's look while it runs.
    assert_eq!(gun.look(&SHOTGUN, Some(&mut energy)), AimLook::Low);
    let mut full = Energy::default();
    assert!(gun.pull(&SHOTGUN, Some(&mut full)));
    gun.step(0.5);
    assert!(matches!(gun.look(&SHOTGUN, Some(&mut full)), AimLook::Reload(_)));
}

#[test]
fn every_pellet_flies_its_own_part_of_the_dispersion_off_the_aim() {
    let mut draws = [0.0, 1.0, 0.5, 0.25, 0.75].into_iter();
    let angles = shotgun_directions(1.0, &SHOTGUN, || draws.next().unwrap());
    let off: Vec<f32> = angles.iter().map(|a| (a - 1.0) / (PI / 8.0)).collect();
    let want = [-1.0, 1.0, 0.0, -0.5, 0.5];
    assert_eq!(off.len(), 5);
    for (o, w) in off.iter().zip(want) {
        assert!((o - w).abs() < 1e-5, "{off:?}");
    }
    // As many pellets as the gun has, none for none.
    assert!(shotgun_directions(0.0, &Options { bullets: 0, ..SHOTGUN }, || 0.5).is_empty());
    assert_eq!(shotgun_directions(0.0, &Options { bullets: 9, ..SHOTGUN }, || 0.5).len(), 9);
}

#[test]
fn the_kick_is_back_along_the_barrel() {
    let k = shotgun_kick(0.0, &SHOTGUN);
    assert!((k - Vec2::new(-15.0, 0.0)).length() < 1e-5, "{k}");
    let k = shotgun_kick(PI / 2.0, &SHOTGUN);
    assert!((k - Vec2::new(0.0, -15.0)).length() < 1e-4, "{k}");
}

/// A world without gravity, its fixed step 60 Hz, each update one step.
fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_message::<Hit>()
        .add_message::<ShotgunBlast>()
        .add_message::<ShotgunShot>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (shotgun_blasts, collect).chain());
    app.finish();
    app.cleanup();
    app
}

/// What the blasts did, collected each step.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<Hit>,
    shots: Vec<ShotgunShot>,
}

fn collect(mut hits: MessageReader<Hit>, mut shots: MessageReader<ShotgunShot>, mut seen: ResMut<Seen>) {
    seen.hits.extend(hits.read().copied());
    seen.shots.extend(shots.read().cloned());
}

/// A shooter of `team` at the origin with a Shotgun, a dynamic body of mass
/// 2 unless `fixed`.
fn shooter(app: &mut App, team: u8, fixed: bool, options: Options) -> Entity {
    let gun = Weapon::new(Kind::Shotgun, options, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    app.world_mut()
        .spawn((
            if fixed { RigidBody::Static } else { RigidBody::Dynamic },
            Collider::circle(0.5),
            Mass(2.0),
            Transform::default(),
            Team(team),
            gun,
        ))
        .id()
}

/// A box of `size` at `at` (a car of `team` with 20 health if given, else
/// a plain wall).
fn target(app: &mut App, at: Vec2, size: Vec2, team: Option<u8>, dynamic: bool) -> Entity {
    let mut e = app.world_mut().spawn((
        if dynamic { RigidBody::Dynamic } else { RigidBody::Static },
        Collider::rectangle(size.x, size.y),
        Mass(4.0),
        Transform::from_translation(at.extend(0.0)),
    ));
    if let Some(t) = team {
        e.insert((Team(t), Health { hp: 20, max: 20 }));
    }
    e.id()
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

/// Fires from the origin, barrel along +x, and runs two steps.
fn blast(app: &mut App, car: Entity) {
    app.world_mut().write_message(ShotgunBlast {
        car,
        from: Vec2::ZERO,
        muzzle: Vec2::new(1.0, 0.0),
        direction: 0.0,
    });
    app.update();
    app.update();
}

#[test]
fn a_blast_hurts_with_every_pellet_and_pushes_what_it_hits_and_the_car() {
    let mut app = world();
    let car = shooter(&mut app, 1, false, SHOTGUN);
    // A tall car-sized box 4 units off: all pellets (at most 22.5° off,
    // 1.66 units at that distance) land on it.
    let foe = target(&mut app, Vec2::new(4.0, 0.0), Vec2::new(1.0, 10.0), Some(0), true);
    settle(&mut app);
    blast(&mut app, car);
    let seen = app.world().resource::<Seen>();
    // Five pellets, each 4 from the shooter: 20 in all.
    assert_eq!(seen.shots.len(), 1);
    assert_eq!(seen.shots[0].pellets.len(), 5);
    assert_eq!(seen.hits.len(), 5);
    assert!(seen.hits.iter().all(|h| h.target == foe && h.damage == 4 && h.by == Some(car)));
    assert!(seen.shots[0].pellets.iter().all(|p| p.landed == Landed::Hurt { living: true }));
    assert_eq!((seen.shots[0].at, seen.shots[0].muzzle), (Vec2::ZERO, Vec2::new(1.0, 0.0)));
    // The pellets end on the box's near face at x = 3.5, spread along it.
    let ends: Vec<Vec2> = seen.shots[0].pellets.iter().map(|p| p.end).collect();
    assert!(ends.iter().all(|e| (e.x - 3.5).abs() < 1e-3 && e.y.abs() < 1.7), "{ends:?}");
    assert!(ends.windows(2).any(|w| (w[0].y - w[1].y).abs() > 1e-3), "{ends:?}");
    // The box is pushed away (5 × 6 = 30 over a mass of 4, partly across
    // for the swing), the car shoved back by 15 over its mass of 2.
    let v = |e| app.world().get::<LinearVelocity>(e).map_or(Vec2::ZERO, |v| v.0);
    assert!(v(foe).x > 3.0, "{}", v(foe));
    assert!((v(car) - Vec2::new(-7.5, 0.0)).length() < 0.2, "{}", v(car));
}

#[test]
fn the_pellets_stay_within_the_dispersion_and_each_run_is_the_same() {
    let run = || {
        let mut app = world();
        let car = shooter(&mut app, 1, true, SHOTGUN);
        // A far wall: the pellets' angles read off where they land.
        target(&mut app, Vec2::new(20.0, 0.0), Vec2::new(1.0, 60.0), None, false);
        settle(&mut app);
        for _ in 0..4 {
            blast(&mut app, car);
        }
        app.world().resource::<Seen>().shots.iter().flat_map(|s| s.pellets.clone()).collect::<Vec<_>>()
    };
    let pellets = run();
    assert_eq!(pellets.len(), 20);
    for p in &pellets {
        assert!(p.direction.abs() <= PI / 8.0 + 1e-6, "{}", p.direction);
        assert_eq!(p.landed, Landed::Nothing);
        // Along the pellet's own angle to the wall's face at x = 19.5.
        assert!((p.end.x - 19.5).abs() < 1e-2 && (p.end.y - 19.5 * p.direction.tan()).abs() < 1e-2, "{p:?}");
    }
    // Not all the same: independent angles, both sides of the aim.
    assert!(pellets.iter().any(|p| p.direction > 0.05) && pellets.iter().any(|p| p.direction < -0.05));
    // Seeded: the same blasts fall the same.
    assert_eq!(run(), pellets);
}

#[test]
fn a_car_of_the_shooters_team_is_pushed_but_not_hurt() {
    let mut app = world();
    let car = shooter(&mut app, 1, false, SHOTGUN);
    let friend = target(&mut app, Vec2::new(4.0, 0.0), Vec2::new(1.0, 10.0), Some(1), true);
    settle(&mut app);
    blast(&mut app, car);
    let seen = app.world().resource::<Seen>();
    assert!(seen.hits.is_empty());
    assert!(seen.shots[0].pellets.iter().all(|p| p.landed == Landed::Spared));
    assert!(app.world().get::<LinearVelocity>(friend).unwrap().0.x > 3.0);
}

#[test]
fn a_wall_stops_the_pellets_and_low_boxes_and_sensors_let_them_through() {
    let mut app = world();
    let car = shooter(&mut app, 1, true, SHOTGUN);
    let wall = target(&mut app, Vec2::new(3.0, 0.0), Vec2::new(1.0, 20.0), None, false);
    let behind = target(&mut app, Vec2::new(6.0, 0.0), Vec2::new(1.0, 20.0), Some(0), false);
    settle(&mut app);
    blast(&mut app, car);
    assert!(app.world().resource::<Seen>().hits.is_empty(), "the wall shelters the car behind it");
    // A wall of low boxes is no cover.
    app.world_mut().entity_mut(wall).insert(IgnoresWeapons);
    blast(&mut app, car);
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.hits.len(), 5);
    assert!(seen.hits.iter().all(|h| h.target == behind));
    // And a sensor.
    app.world_mut().entity_mut(wall).remove::<IgnoresWeapons>().insert(Sensor);
    app.world_mut().resource_mut::<Seen>().hits.clear();
    blast(&mut app, car);
    assert_eq!(app.world().resource::<Seen>().hits.len(), 5);
}

#[test]
fn a_car_that_cannot_move_is_not_kicked_and_a_pellet_in_the_open_hits_nothing() {
    let mut app = world();
    let car = shooter(&mut app, 1, true, SHOTGUN);
    settle(&mut app);
    blast(&mut app, car);
    let seen = app.world().resource::<Seen>();
    assert!(seen.hits.is_empty());
    assert_eq!(seen.shots[0].pellets.len(), 5);
    // Flew its full reach, 1024 pixels.
    assert!(seen.shots[0].pellets.iter().all(|p| (p.end.length() - 32.0).abs() < 1e-3 && p.landed == Landed::Nothing));
    assert_eq!(app.world().get::<LinearVelocity>(car).map_or(Vec2::ZERO, |v| v.0), Vec2::ZERO);
}

#[test]
fn a_blast_from_a_gun_that_is_not_a_shotgun_does_nothing() {
    let mut app = world();
    let gun = Weapon::new(Kind::Minigun, MINIGUN, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    let car = app
        .world_mut()
        .spawn((RigidBody::Dynamic, Collider::circle(0.5), Transform::default(), Team(1), gun))
        .id();
    target(&mut app, Vec2::new(4.0, 0.0), Vec2::new(1.0, 10.0), Some(0), true);
    settle(&mut app);
    blast(&mut app, car);
    let seen = app.world().resource::<Seen>();
    assert!(seen.hits.is_empty() && seen.shots.is_empty());
    assert_eq!(app.world().get::<LinearVelocity>(car).unwrap().0, Vec2::ZERO);
}
