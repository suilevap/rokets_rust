//! What a Minigun bullet and a Railgun beam do to a car that carries a gun
//! itself (`WeaponHelper.SimulateBullet` pushed whatever physical thing it
//! hit): it is shoved, as a car without one is. `fire_weapons` once left
//! such cars out of its bodies to keep Bevy's queries apart.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::car::control::Team;
use tilerider::controls::PlayerIntent;
use tilerider::explosion::{Health, Hit};
use tilerider::render::particles::Particles;
use tilerider::sound::PlaySound;
use tilerider::weapon::model::{Kind, for_type};
use tilerider::weapon::{Weapon, WeaponRng, fire_weapons, laser, rocket, shotgun, tesla, zombie};

const DT: f32 = 1.0 / 60.0;

fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(WeaponRng::new(1))
        .init_resource::<Particles>()
        .add_message::<Hit>()
        .add_message::<PlaySound>()
        .add_message::<zombie::ZombieWave>()
        .add_message::<tesla::TeslaStrike>()
        .add_message::<tesla::TeslaCrackle>()
        .add_message::<laser::LaserCharge>()
        .add_message::<rocket::RocketLaunched>()
        .add_message::<shotgun::ShotgunBlast>()
        .add_systems(FixedUpdate, fire_weapons);
    app
}

/// A dynamic car with a gun of `kind` (an enemy's, mounted at its middle) at
/// `at`, facing right; the shooter holds its trigger.
fn armed_car(app: &mut App, kind: &str, at: Vec2, team: u8, firing: bool) -> Entity {
    let (kind, options) = for_type(kind, false).unwrap();
    assert!(matches!(kind, Kind::Minigun | Kind::Railgun));
    let mut rng = SmallRng::seed_from_u64(5);
    let gun = Weapon::new(kind, options, Vec2::ZERO, Vec2::new(2.0, 1.0), false, &mut rng);
    let intent = PlayerIntent {
        fire: firing,
        fire_accurate: firing,
        ..default()
    };
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::rectangle(2.0, 1.0),
            Transform::from_translation(at.extend(0.0)),
            LinearDamping(0.0),
            Health::full(100),
            Team(team),
            gun,
            intent,
        ))
        .id()
}

/// Steps until the target moves, at most two seconds; its velocity then.
fn pushed_velocity(app: &mut App, target: Entity) -> Vec2 {
    app.finish();
    app.cleanup();
    for _ in 0..120 {
        // The pulse is taken by every step, so the trigger is pressed again.
        for mut intent in app.world_mut().query::<&mut PlayerIntent>().iter_mut(app.world_mut()) {
            intent.fire_accurate = intent.fire;
        }
        app.update();
        let v = app.world().get::<LinearVelocity>(target).unwrap().0;
        if v != Vec2::ZERO {
            return v;
        }
    }
    Vec2::ZERO
}

fn armed_target_is_pushed_by(gun: &str) {
    let mut app = world();
    let target = armed_car(&mut app, "Minigun", Vec2::new(8.0, 0.0), 0, false);
    armed_car(&mut app, gun, Vec2::ZERO, 1, true);
    let v = pushed_velocity(&mut app, target);
    assert!(v.x > 0.0, "{gun}: an armed car was not pushed along the shot: {v}");
}

#[test]
fn a_minigun_bullet_pushes_a_car_that_carries_a_gun() {
    armed_target_is_pushed_by("Minigun");
}

#[test]
fn a_railgun_beam_pushes_a_car_that_carries_a_gun() {
    armed_target_is_pushed_by("Railgun");
}
