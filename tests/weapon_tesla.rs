//! The Tesla (`weapon::model::Tesla`, `weapon::tesla`): its preset, its
//! reload, its reach and push, the walk of its bolts, and its strike on
//! the cars around it in a small physics world.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use tilerider::car::CarRadius;
use tilerider::car::control::Team;
use tilerider::explosion::{Health, Hit};
use tilerider::game::MapId;
use tilerider::particles::styles::{TESLA_LINK_LENGTH, TESLA_TRAIL_LENGTH};
use tilerider::weapon::Weapon;
use tilerider::weapon::model::{Energy, Kind, MINIGUN, TESLA, Tesla, for_type, target_in_fire_range, tesla_push, tesla_reaches};
use tilerider::weapon::tesla::{
    BOLT_SPREAD, MAX_LINKS, TeslaBolt, TeslaStrike, bolt_between, bolt_out, spark_on_line, tesla_strikes,
};

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_type_is_the_tesla_the_same_for_everyone() {
    assert_eq!(for_type("Tesla", false), Some((Kind::Tesla, TESLA)));
    assert_eq!(for_type("Tesla", true), Some((Kind::Tesla, TESLA)));
    // `DamageConst.Tesla`, `TeslaRange`, `BulletImpulse = 5`, a quarter
    // second's reload, a whole turn's dispersion, the default energy.
    assert_eq!((TESLA.damage, TESLA.damage_max, TESLA.range, TESLA.impulse), (8, 8, 8.0, 5.0));
    assert_eq!((TESLA.reload, TESLA.dispersion, TESLA.energy), (0.25, 2.0 * PI, 0.1));
}

#[test]
fn it_neither_aims_nor_flies_nor_works_by_itself() {
    assert!(!Kind::Tesla.is_complex());
    assert!(!Kind::Tesla.auto_fire());
    assert!(Kind::Tesla.is_immediate());
    assert!(tilerider::weapon::sprite_of(Kind::Tesla).is_none());
    assert_eq!(tilerider::weapon::textures_of(Kind::Tesla).len(), 3);
    assert_eq!(tilerider::weapon::textures_of(Kind::Minigun), vec!["Weapons\\Minigun"]);
}

#[test]
fn the_enemies_trigger_asks_only_the_range() {
    // Whatever way the car faces, a target closer than 8 is in range.
    for angle in [0.0, FRAC_PI_2, PI, -2.5] {
        assert!(target_in_fire_range(Vec2::ZERO, 0.0, TESLA.range, TESLA.dispersion, Vec2::from_angle(angle) * 7.5, 0.5));
    }
    assert!(!target_in_fire_range(Vec2::ZERO, 0.0, TESLA.range, TESLA.dispersion, Vec2::new(-8.5, 0.0), 0.5));
}

/// Seconds at which a trigger held from the start strikes, over `secs`.
fn strikes(mut energy: Option<&mut Energy>, secs: f32) -> Vec<f32> {
    let mut gun = Tesla::new(&TESLA, 0.0);
    let mut fired = Vec::new();
    for step in 0..(secs / DT).round() as usize {
        if gun.pull(&TESLA, energy.as_deref_mut()) {
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
fn held_it_strikes_four_times_a_second() {
    let fired = strikes(None, 5.0);
    assert_eq!(fired.len(), 20, "{fired:?}");
    for pair in fired.windows(2) {
        // 15 steps of 1/60 s.
        assert!((pair[1] - pair[0] - 0.25).abs() < 0.01, "{fired:?}");
    }
}

#[test]
fn a_pull_arms_it_before_the_reload_is_over() {
    let mut gun = Tesla::new(&TESLA, 0.8);
    assert!(!gun.armed);
    assert!(!gun.can_fire(&TESLA, None));
    assert!(!gun.pull(&TESLA, None));
    assert!(gun.armed);
    // The first strike waits its random part of the reload (0.2 s).
    for _ in 0..12 {
        gun.step(DT);
    }
    assert!(gun.can_fire(&TESLA, None));
    assert!(gun.pull(&TESLA, None));
}

#[test]
fn the_player_pays_a_tenth_of_the_store_a_strike_and_a_dry_store_strikes_nothing() {
    let mut energy = Energy::default();
    let mut gun = Tesla::new(&TESLA, 0.0);
    assert!(gun.pull(&TESLA, Some(&mut energy)));
    assert!((energy.current - 0.9).abs() < 1e-6);
    // Held, a full store pays for nine strikes (the tenth tenth falls
    // short by the float sums), a quarter second apart; then it is dry.
    let mut full = Energy::default();
    let fired = strikes(Some(&mut full), 2.4);
    assert_eq!(fired.len(), 9, "{fired:?}");
    assert!(full.overloaded);
    energy.current = 0.05;
    gun.step(1.0);
    assert!(!gun.pull(&TESLA, Some(&mut energy)));
    assert!(energy.overloaded);
}

#[test]
fn it_reaches_strictly_within_its_range_and_pushes_from_car_to_car() {
    assert!(tesla_reaches(Vec2::ZERO, Vec2::new(7.99, 0.0), &TESLA));
    assert!(!tesla_reaches(Vec2::ZERO, Vec2::new(8.0, 0.0), &TESLA));
    assert!(tesla_reaches(Vec2::new(1.0, 1.0), Vec2::new(1.0 - 5.0, 1.0 + 6.0), &TESLA));
    assert_eq!(tesla_push(Vec2::ZERO, Vec2::new(0.0, 3.0), &TESLA), Vec2::new(0.0, 5.0));
    assert_eq!(tesla_push(Vec2::ONE, Vec2::ONE, &TESLA), Vec2::ZERO);
}

#[test]
fn a_bolt_walks_link_by_link_to_within_a_link_of_its_end() {
    let mut rng = SmallRng::seed_from_u64(1);
    for _ in 0..200 {
        let to = Vec2::new(rng.random_range(-8.0..8.0), rng.random_range(-8.0..8.0));
        let (links, end, _) = bolt_between(Vec2::ZERO, to, BOLT_SPREAD, || rng.random());
        assert!(links.len() < MAX_LINKS, "{to}: {} links", links.len());
        assert!(end.distance(to) < TESLA_LINK_LENGTH, "{to}: ends at {end}");
        // Each link half a link on from the last link's far end, turned at
        // most a quarter turn off the way to the end.
        let mut at = Vec2::ZERO;
        for &(middle, angle) in &links {
            let along = Vec2::from_angle(angle) * TESLA_LINK_LENGTH / 2.0;
            assert!((at + along).distance(middle) < 1e-4);
            let off = (angle - (to - at).to_angle() + 3.0 * PI).rem_euclid(TAU) - PI;
            assert!(off.abs() <= BOLT_SPREAD + 1e-4, "{off}");
            at = middle + along;
        }
    }
    // Straight (no spread): along the line, within a link of the end.
    let (links, end, angle) = bolt_between(Vec2::ZERO, Vec2::new(7.0, 0.0), 0.0, || 0.9);
    assert_eq!(links.len(), 3);
    assert_eq!((end, angle), (Vec2::new(6.0, 0.0), 0.0));
    // Already there: no link, the flash where it starts.
    let (links, end, _) = bolt_between(Vec2::ONE, Vec2::ONE, BOLT_SPREAD, || 0.5);
    assert!(links.is_empty() && end == Vec2::ONE);
}

#[test]
fn the_crackles_bolt_wanders_out_to_the_range() {
    let mut rng = SmallRng::seed_from_u64(2);
    for _ in 0..200 {
        let start = rng.random::<f32>() * TAU;
        let (links, end, _) = bolt_out(Vec2::ZERO, start, BOLT_SPREAD, TESLA.range, || rng.random());
        assert!(!links.is_empty());
        // Its first link goes along the start; it stops once out of reach
        // (or at the bound).
        assert!((links[0].1 - start).abs() < 1e-6);
        assert!(end.length() >= TESLA.range || links.len() == MAX_LINKS, "{end} after {}", links.len());
    }
    // Straight out: four links of 2 to the range of 8.
    let (links, end, _) = bolt_out(Vec2::ZERO, 0.0, 0.0, 8.0, || 0.3);
    assert_eq!(links.len(), 4);
    assert!(end.distance(Vec2::new(8.0, 0.0)) < 1e-5);
}

#[test]
fn a_spark_starts_between_half_a_spark_in_and_half_a_spark_short() {
    let w = TESLA_TRAIL_LENGTH;
    assert_eq!(w, 4.0);
    assert_eq!(spark_on_line(Vec2::ZERO, 0.0, 8.0, 0.0), Some(Vec2::new(w / 2.0, 0.0)));
    let far = spark_on_line(Vec2::ZERO, 0.0, 8.0, 1.0).unwrap();
    assert!((far.x - (8.0 - w / 2.0)).abs() < 1e-6);
    assert_eq!(spark_on_line(Vec2::ZERO, 0.0, w, 0.5), None);
}

/// A world without gravity, its fixed step 60 Hz, each update one step.
fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_message::<Hit>()
        .add_message::<TeslaStrike>()
        .add_message::<TeslaBolt>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (tesla_strikes, collect).chain());
    app.finish();
    app.cleanup();
    app
}

/// What the strikes did, collected each step.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<Hit>,
    bolts: Vec<TeslaBolt>,
}

fn collect(mut hits: MessageReader<Hit>, mut bolts: MessageReader<TeslaBolt>, mut seen: ResMut<Seen>) {
    seen.hits.extend(hits.read().copied());
    seen.bolts.extend(bolts.read().copied());
}

/// A car (map id `id`) of `team` at `at` with `hp` of 20 health, a dynamic
/// body unless `fixed`.
fn car(app: &mut App, id: i64, at: Vec2, team: u8, hp: i32, fixed: bool) -> Entity {
    app.world_mut()
        .spawn((
            if fixed { RigidBody::Static } else { RigidBody::Dynamic },
            Collider::circle(0.5),
            Transform::from_translation(at.extend(0.0)),
            Health { hp, max: 20 },
            Team(team),
            MapId(id),
            CarRadius(0.5),
        ))
        .id()
}

fn tesla(app: &mut App, at: Vec2, team: u8) -> Entity {
    let gun = Weapon::new(Kind::Tesla, TESLA, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    let shooter = car(app, 1, at, team, 20, true);
    app.world_mut().entity_mut(shooter).insert(gun);
    shooter
}

#[test]
fn a_strike_hurts_and_pushes_every_enemy_in_reach_and_no_one_else() {
    let mut app = world();
    let shooter = tesla(&mut app, Vec2::ZERO, 1);
    let near = car(&mut app, 5, Vec2::new(0.0, 7.5), 0, 20, false);
    let other = car(&mut app, 3, Vec2::new(-4.0, -4.0), 2, 20, false);
    let fixed = car(&mut app, 4, Vec2::new(6.0, 0.0), 0, 20, true);
    let far = car(&mut app, 6, Vec2::new(8.5, 0.0), 0, 20, false);
    let friend = car(&mut app, 7, Vec2::new(2.0, 0.0), 1, 20, false);
    let dead = car(&mut app, 8, Vec2::new(-2.0, 0.0), 0, 0, false);
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(TeslaStrike { car: shooter, at: Vec2::ZERO });
    app.update();
    app.update();
    let seen = app.world().resource::<Seen>();
    // In the order of the map ids, each for 8 from the shooter.
    let struck: Vec<_> = seen.hits.iter().map(|h| (h.target, h.damage, h.by)).collect();
    assert_eq!(struck, vec![(other, 8, Some(shooter)), (fixed, 8, Some(shooter)), (near, 8, Some(shooter))]);
    // The blow comes along the line from the shooter (y up: the car above
    // is struck from below).
    assert!((seen.hits[2].from - FRAC_PI_2).abs() < 1e-5);
    // A bolt to each, from the coil.
    assert_eq!(seen.bolts.len(), 3);
    assert!(seen.bolts.iter().all(|b| b.from == Vec2::ZERO));
    assert_eq!(seen.bolts[2].to, Vec2::new(0.0, 7.5));
    // The dynamic ones are pushed away from the shooter; the rest stay.
    let v = |e| app.world().get::<LinearVelocity>(e).map_or(Vec2::ZERO, |v| v.0);
    assert!(v(near).y > 0.0, "{}", v(near));
    assert!(v(other).x < 0.0 && v(other).y < 0.0, "{}", v(other));
    for e in [fixed, far, friend, dead] {
        assert_eq!(v(e), Vec2::ZERO, "{e:?}");
    }
}

#[test]
fn a_strike_from_a_gun_that_is_not_a_tesla_does_nothing() {
    let mut app = world();
    let gun = Weapon::new(Kind::Minigun, MINIGUN, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    let shooter = car(&mut app, 1, Vec2::ZERO, 1, 20, true);
    app.world_mut().entity_mut(shooter).insert(gun);
    car(&mut app, 2, Vec2::new(1.0, 0.0), 0, 20, false);
    app.update();
    app.world_mut().write_message(TeslaStrike { car: shooter, at: Vec2::ZERO });
    app.update();
    app.update();
    assert!(app.world().resource::<Seen>().hits.is_empty());
}
