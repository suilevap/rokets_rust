//! The Laser (`weapon::laser`, `weapon::model::LASER`): its types, the
//! trigger and the delay of `LaserWithDelay`, and the beam's reach, bounces
//! and damage in a small physics world.

use std::f32::consts::{FRAC_PI_4, PI};
use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::ai::fire::ShootPattern;
use tilerider::explosion::{Health, Hit};
use tilerider::weapon::laser::{Laser, LaserSpark, MAX_LEGS, recoil, run_lasers};
use tilerider::weapon::model::{Energy, Kind, LASER, LASER_CHARGE, MINIGUN, for_type};
use tilerider::weapon::{Weapon, has_aim_line, sprite_of, textures_of};

const DT: f32 = 1.0 / 60.0;

#[test]
fn the_three_types_are_one_laser_with_other_bounces_and_a_delay() {
    // `DamageConst.Laser = 3`, `LaserDefaultOptions`' energy, 256 units of
    // `_fireDistance`, two bounces from the factory.
    assert_eq!((LASER.damage, LASER.energy, LASER.range, LASER.reflections), (3, 0.001, 256.0, 2));
    assert_eq!((LASER.reload, LASER.impulse, LASER.dispersion), (0.0, 0.0, 0.0));
    assert_eq!(for_type("Laser", false), Some((Kind::Laser, LASER)));
    assert_eq!(for_type("Laser", true), Some((Kind::Laser, LASER)));
    let (kind, none) = for_type("LaserNoReflection", false).unwrap();
    assert_eq!((kind, none.reflections, none.charge), (Kind::Laser, 0, 0.0));
    let (kind, delayed) = for_type("LaserWithDelay", false).unwrap();
    assert_eq!((kind, delayed.reflections, delayed.charge), (Kind::Laser, 0, 0.9));
    assert_eq!(LASER_CHARGE, 0.9);
    // Nothing else of them differs.
    for o in [none, delayed] {
        assert_eq!((o.damage, o.energy, o.range), (3, 0.001, 256.0));
    }
}

#[test]
fn it_is_a_barrel_that_aims_and_fires_at_once_with_a_beam_for_a_line() {
    assert!(Kind::Laser.is_complex());
    assert!(Kind::Laser.is_immediate());
    assert!(!Kind::Laser.auto_fire());
    assert!(!has_aim_line(Kind::Laser));
    assert!(has_aim_line(Kind::Minigun));
    let sprite = sprite_of(Kind::Laser).unwrap();
    assert_eq!((sprite.name, sprite.origin), ("Weapons\\Laser", Vec2::new(9.0, 5.0)));
    let textures = textures_of(Kind::Laser);
    assert!(textures.contains(&"Weapons\\Laser") && textures.contains(&"Particles\\3Pixel"));
}

#[test]
fn the_kick_is_one_to_five_pixels() {
    assert_eq!((recoil(0.0), recoil(1.0)), (1.0, 5.0));
}

/// One step of a gun's trigger as `fire_weapons` takes it: held (both
/// flags, as `AlwaysShoot` sets them) or not.
fn step(gun: &mut Laser, held: bool) -> bool {
    gun.begin(DT, true);
    gun.trigger(held, held, || true).fired
}

#[test]
fn held_it_fires_every_step_and_let_go_it_stops() {
    let mut gun = Laser::new(0.0);
    for _ in 0..10 {
        assert!(step(&mut gun, true));
        assert!(gun.active);
    }
    assert!(!step(&mut gun, false));
    assert!(!gun.active && !gun.line && !gun.visible);
}

#[test]
fn only_the_held_flag_fires_and_a_pulse_alone_neither_fires_nor_lets_go() {
    let mut gun = Laser::new(0.0);
    gun.begin(DT, true);
    // The pulse of `ShootAccurate` is not `ShootMultiple`.
    assert!(!gun.trigger(true, false, || panic!("the energy is not asked")).fired);
    assert!(!gun.active);
    assert!(step(&mut gun, true));
    gun.begin(DT, true);
    // `NoShoot` is not called while a flag is set: it stays active.
    assert!(!gun.trigger(true, false, || true).fired);
    assert!(gun.active);
}

#[test]
fn a_dry_store_does_not_fire_and_a_player_pays_a_thousandth_a_step() {
    let mut energy = Energy::default();
    let mut gun = Laser::new(0.0);
    gun.begin(DT, true);
    assert!(gun.trigger(true, true, || energy.try_spend(LASER.energy)).fired);
    assert!((energy.current - 0.999).abs() < 1e-6);
    let mut dry = Energy::default();
    dry.current = 0.0;
    let mut gun = Laser::new(0.0);
    gun.begin(DT, true);
    assert!(!gun.trigger(true, true, || dry.try_spend(LASER.energy)).fired);
    assert!(!gun.active && dry.overloaded);
}

#[test]
fn the_line_is_wanted_only_while_aimed_and_shown_only_while_firing() {
    let mut gun = Laser::new(0.0);
    gun.begin(DT, true);
    assert!(gun.line);
    gun.trigger(false, false, || true);
    // `NoShoot` cleared it.
    assert!(!gun.line);
    gun.update();
    assert!(!gun.visible, "an idle laser draws nothing");
    // Fired without the aim (a pulled trigger, no aim): the beam shows
    // all the same.
    gun.begin(DT, false);
    gun.trigger(true, true, || true);
    gun.update();
    assert!(gun.visible && gun.line);
}

#[test]
fn the_step_the_beam_comes_on_hurts_nothing() {
    let mut gun = Laser::new(0.0);
    assert!(step(&mut gun, true));
    // No frame has drawn it yet: no collisions to hurt.
    assert!(!gun.update());
    gun.end_step();
    // Drawn now: it hurts from the next step on, every step.
    for _ in 0..5 {
        assert!(step(&mut gun, true));
        assert!(gun.update());
        gun.end_step();
    }
    // Let go and pulled again: off and on, the collisions are cleared.
    step(&mut gun, false);
    gun.update();
    gun.end_step();
    step(&mut gun, true);
    assert!(!gun.update());
    gun.end_step();
    step(&mut gun, true);
    assert!(gun.update());
}

#[test]
fn a_firing_gun_that_is_not_aimed_shows_its_beam_but_hurts_after_a_frame() {
    let mut gun = Laser::new(0.0);
    for i in 0..4 {
        gun.begin(DT, false);
        gun.trigger(true, true, || true);
        // The line is wanted off (`needAim` false) and forced on again,
        // which clears the collisions each step: nothing is ever hurt.
        assert!(!gun.update(), "step {i}");
        assert!(gun.visible && gun.line);
        gun.end_step();
    }
}

#[test]
fn cancelled_it_is_off_and_quiet() {
    let mut gun = Laser::new(0.0);
    step(&mut gun, true);
    gun.update();
    gun.end_step();
    gun.cancel();
    assert!(!gun.active && !gun.visible && !gun.line && gun.legs.is_empty());
    assert!(!gun.update());
}

/// Steps a delayed gun with the trigger `held` or not, the seconds of the
/// steps it fired.
fn run_delayed(script: &[(f32, bool)]) -> (Laser, Vec<f32>) {
    let mut gun = Laser::new(LASER_CHARGE);
    let mut fired = Vec::new();
    let mut t = 0.0;
    for &(secs, held) in script {
        for _ in 0..(secs / DT).round() as usize {
            if step(&mut gun, held) {
                fired.push(t);
            }
            gun.update();
            gun.end_step();
            t += DT;
        }
    }
    (gun, fired)
}

#[test]
fn a_delayed_gun_charges_for_nine_tenths_of_a_second_before_it_fires() {
    let (gun, fired) = run_delayed(&[(2.0, true)]);
    // The charge starts at step 0 and completes 54 steps (0.9 s) later.
    assert!((fired[0] - 0.9).abs() < DT * 0.5, "{fired:?}");
    // From then on, every step.
    assert_eq!(fired.len(), 120 - 54);
    assert!(gun.active);
    // Before it, the line was shown, and brightened from 0.1 to 1.
    let mut gun = Laser::new(LASER_CHARGE);
    let mut opacities = Vec::new();
    for _ in 0..30 {
        step(&mut gun, true);
        opacities.push(gun.opacity);
        gun.update();
        gun.end_step();
        assert!(gun.visible && gun.line && !gun.active);
        assert!(!gun.can_shoot(), "charging");
    }
    assert!((opacities[0] - 0.1).abs() < 1e-6);
    assert!((opacities[29] - 29.0 / 54.0).abs() < 1e-4, "{opacities:?}");
    assert!(opacities.windows(2).all(|w| w[1] >= w[0]));
}

#[test]
fn the_charge_starts_the_ring_once() {
    let mut gun = Laser::new(LASER_CHARGE);
    gun.begin(DT, true);
    assert!(gun.trigger(true, true, || true).charge_started);
    for _ in 0..10 {
        gun.begin(DT, true);
        assert!(!gun.trigger(true, true, || true).charge_started);
    }
}

#[test]
fn a_charge_goes_on_through_a_let_go_and_completes_at_the_next_pull() {
    // Held 0.5 s, let go 0.6 s, pulled: the charge ran its 0.9 s while let
    // go (nothing completes it but a pull), so the pull fires at once.
    let (mut gun, fired) = run_delayed(&[(0.5, true), (0.6, false)]);
    assert!(fired.is_empty());
    // Even unaimed, a charging gun keeps its line through a let go.
    gun.begin(DT, false);
    gun.trigger(false, false, || true);
    assert!(gun.line && !gun.active);
    assert!(step(&mut gun, true));
}

#[test]
fn a_warm_gun_fires_at_once_after_a_short_gap_and_starts_over_after_a_long_one() {
    // 1.2 s held (charged and firing), 0.5 s let go, held again.
    let (mut gun, _) = run_delayed(&[(1.2, true), (0.5, false)]);
    assert!(!gun.active);
    assert!(step(&mut gun, true), "warm: no new charge");
    // Let go for 1 s: cooled.
    let (mut cold, _) = run_delayed(&[(1.2, true), (1.0, false)]);
    assert!(!step(&mut cold, true), "a new charge");
    assert!(!cold.can_shoot());
    assert!(cold.line);
    // The line of an idle gun is out.
    let (idle, fired) = run_delayed(&[(1.0, false)]);
    assert!(fired.is_empty() && !idle.line && idle.opacity == 1.0);
}

#[test]
fn the_shoot_pattern_knows_how_much_of_a_silence_is_left() {
    let mut p = ShootPattern::new("0000 1111");
    assert_eq!(p.remaining(), 1.0);
    for _ in 0..15 {
        p.step(DT);
    }
    assert!((p.remaining() - 0.75).abs() < 1e-4, "{}", p.remaining());
}

#[test]
fn a_gun_reports_its_charge_and_whether_it_can_shoot() {
    let mut rng = SmallRng::seed_from_u64(0);
    let (kind, delayed) = for_type("LaserWithDelay", false).unwrap();
    let mut gun = Weapon::new(kind, delayed, Vec2::ZERO, Vec2::ONE, false, &mut rng);
    assert_eq!(gun.charge_time(), 0.9);
    assert!(gun.can_shoot());
    gun.laser.begin(DT, true);
    gun.laser.trigger(true, true, || true);
    assert!(!gun.can_shoot(), "charging");
    let minigun = Weapon::new(Kind::Minigun, MINIGUN, Vec2::ZERO, Vec2::ONE, false, &mut rng);
    assert_eq!(minigun.charge_time(), 0.0);
}

/// What the beam did, collected each step.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<Hit>,
    sparks: Vec<LaserSpark>,
}

fn collect(mut hits: MessageReader<Hit>, mut sparks: MessageReader<LaserSpark>, mut seen: ResMut<Seen>) {
    seen.hits.extend(hits.read().copied());
    seen.sparks.extend(sparks.read().copied());
}

/// A world without gravity, its fixed step 60 Hz, each update one step.
fn world() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Gravity(Vec2::ZERO))
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .add_message::<Hit>()
        .add_message::<LaserSpark>()
        .init_resource::<Seen>()
        .add_systems(FixedUpdate, (run_lasers, collect).chain());
    app.finish();
    app.cleanup();
    app
}

fn wall(app: &mut App, at: Vec2, size: Vec2) -> Entity {
    app.world_mut().spawn((RigidBody::Static, Collider::rectangle(size.x, size.y), Transform::from_translation(at.extend(0.0)))).id()
}

/// A target with `hp`: it can be hurt (has `Health`).
fn target(app: &mut App, at: Vec2, hp: i32) -> Entity {
    app.world_mut()
        .spawn((RigidBody::Static, Collider::circle(0.5), Transform::from_translation(at.extend(0.0)), Health { hp, max: 20 }))
        .id()
}

/// A laser gun of `kind_name` at the origin pointing `direction`, firing
/// (its beam on, as a held trigger leaves it).
fn laser(app: &mut App, kind_name: &str, direction: f32) -> Entity {
    let (kind, options) = for_type(kind_name, false).unwrap();
    let mut gun = Weapon::new(kind, options, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    gun.direction = direction;
    gun.at = Vec2::ZERO;
    gun.laser.active = true;
    gun.laser.line = true;
    app.world_mut().spawn((RigidBody::Static, Collider::circle(0.5), Transform::default(), gun)).id()
}

fn warm_up(app: &mut App, steps: usize) {
    for _ in 0..steps {
        app.update();
    }
}

fn legs(app: &App, shooter: Entity) -> Vec<(Vec2, Vec2)> {
    app.world().get::<Weapon>(shooter).unwrap().laser.legs.iter().map(|l| (l.start, l.end)).collect()
}

#[test]
fn the_beam_runs_to_a_wall_and_hurts_nothing_there_but_flies_sparks_at_it() {
    let mut app = world();
    wall(&mut app, Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0));
    let shooter = laser(&mut app, "LaserNoReflection", 0.0);
    warm_up(&mut app, 4);
    let l = legs(&app, shooter);
    assert_eq!(l.len(), 1, "{l:?}");
    assert!((l[0].1.x - 10.0).abs() < 1e-3);
    let seen = app.world().resource::<Seen>();
    // A wall cannot be hurt, but it is touched each step it is found.
    assert!(seen.hits.is_empty());
    assert!(!seen.sparks.is_empty());
    assert!(seen.sparks.iter().all(|s| (s.at.x - 10.0).abs() < 1e-3));
}

#[test]
fn the_beam_reaches_256_units_in_all() {
    let mut app = world();
    let shooter = laser(&mut app, "LaserNoReflection", 0.0);
    warm_up(&mut app, 3);
    let l = legs(&app, shooter);
    assert_eq!(l.len(), 1);
    assert!((l[0].1.x - 256.0).abs() < 1e-2, "{l:?}");
}

#[test]
fn a_laser_bounces_twice_off_what_it_cannot_hurt_the_others_not_at_all() {
    // A slanted beam between two walls, up and to the right.
    let walls = [
        (Vec2::new(11.0, 0.0), Vec2::new(2.0, 200.0)),
        (Vec2::new(-11.0, 0.0), Vec2::new(2.0, 200.0)),
    ];
    for (name, expected) in [("Laser", 3), ("LaserNoReflection", 1), ("LaserWithDelay", 1)] {
        let mut app = world();
        for (at, size) in walls {
            wall(&mut app, at, size);
        }
        let shooter = laser(&mut app, name, FRAC_PI_4);
        warm_up(&mut app, 3);
        let l = legs(&app, shooter);
        assert_eq!(l.len(), expected, "{name}: {l:?}");
        assert!(l.len() <= MAX_LEGS);
        if expected == 3 {
            // Each leg starts where the last ended; the legs share the
            // 256 units: they are 14.1 long, then 28.3, then what is left.
            assert_eq!(l[1].0, l[0].1);
            let total: f32 = l.iter().map(|(a, b)| a.distance(*b)).sum();
            assert!((total - 256.0).abs() < 1.0 || l[2].1.x.abs() < 10.5, "{total}");
        }
    }
}

#[test]
fn a_head_on_wall_stops_a_laser() {
    let mut app = world();
    wall(&mut app, Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0));
    let shooter = laser(&mut app, "Laser", 0.0);
    warm_up(&mut app, 3);
    assert_eq!(legs(&app, shooter).len(), 1);
}

#[test]
fn it_ends_at_what_can_be_hurt_and_hurts_it_three_a_step_after_a_free_step() {
    let mut app = world();
    let victim = target(&mut app, Vec2::new(5.0, 0.0), 20);
    wall(&mut app, Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0));
    let shooter = laser(&mut app, "Laser", 0.0);
    // The first step finds the beam; no frame has drawn it before.
    app.update();
    app.update();
    assert!(app.world().resource::<Seen>().hits.is_empty(), "the step it comes on is free");
    let l = legs(&app, shooter);
    assert_eq!(l.len(), 1, "it ends at what can be hurt, not bounces off it: {l:?}");
    assert!((l[0].1.x - 4.5).abs() < 1e-3);
    warm_up(&mut app, 10);
    let seen = app.world().resource::<Seen>();
    assert!(!seen.hits.is_empty());
    assert!(seen.hits.iter().all(|h| h.target == victim && h.damage == 3 && h.by == Some(shooter)));
    // Each step one blow, from the way the beam goes into the surface
    // (the surface faces back at the gun: normal pointing at -x).
    let h = seen.hits[0];
    assert!((h.from.cos() - 1.0).abs() < 1e-3, "{}", h.from);
    // 3 a step, 60 steps a second.
    assert!(seen.hits.len() >= 9);
}

#[test]
fn it_hurts_a_car_of_its_own_team_too() {
    // C# gave the beam no `ignoreDamage`: a victim with a team is hurt
    // all the same (`Hit` is all the beam sends; no team is asked).
    let mut app = world();
    let friend = target(&mut app, Vec2::new(5.0, 0.0), 20);
    app.world_mut().entity_mut(friend).insert(tilerider::car::control::Team(1));
    let shooter = laser(&mut app, "Laser", 0.0);
    app.world_mut().entity_mut(shooter).insert(tilerider::car::control::Team(1));
    warm_up(&mut app, 6);
    assert!(app.world().resource::<Seen>().hits.iter().any(|h| h.target == friend));
}

#[test]
fn a_beam_that_is_not_firing_hurts_nobody() {
    let mut app = world();
    let victim = target(&mut app, Vec2::new(5.0, 0.0), 20);
    let shooter = laser(&mut app, "Laser", 0.0);
    app.world_mut().get_mut::<Weapon>(shooter).unwrap().laser.active = false;
    warm_up(&mut app, 10);
    // The line is wanted (a charging gun shows it) but nothing is hurt.
    assert!(app.world().resource::<Seen>().hits.iter().all(|h| h.target != victim));
    assert_eq!(legs(&app, shooter).len(), 1, "the line is found");
}

#[test]
fn a_gun_that_is_not_a_laser_is_left_alone() {
    let mut app = world();
    target(&mut app, Vec2::new(5.0, 0.0), 20);
    let gun = Weapon::new(Kind::Minigun, MINIGUN, Vec2::ZERO, Vec2::ONE, false, &mut SmallRng::seed_from_u64(0));
    let shooter = app.world_mut().spawn((RigidBody::Static, Collider::circle(0.5), Transform::default(), gun)).id();
    warm_up(&mut app, 6);
    assert!(app.world().resource::<Seen>().hits.is_empty());
    assert!(legs(&app, shooter).is_empty());
}

#[test]
fn it_looks_every_way_round() {
    // A beam pointing any way ends at a wall that way.
    for angle in [0.0, PI / 2.0, PI, -PI / 2.0] {
        let mut app = world();
        let at = Vec2::from_angle(angle) * 11.0;
        let size = if at.x.abs() > 1.0 { Vec2::new(2.0, 100.0) } else { Vec2::new(100.0, 2.0) };
        wall(&mut app, at, size);
        let shooter = laser(&mut app, "LaserNoReflection", angle);
        warm_up(&mut app, 3);
        let l = legs(&app, shooter);
        assert!((l[0].1.length() - 10.0).abs() < 1e-3, "{angle}: {l:?}");
    }
}

fn leg(from: Vec2, end: Vec2) -> tilerider::weapon::beam::Leg {
    tilerider::weapon::beam::Leg {
        start: from,
        cast_from: from,
        end,
        direction: (end - from).to_angle(),
        hit: None,
    }
}

#[test]
fn the_glass_flashes_a_quarter_of_the_way_along_the_leg_shaken_by_half_a_unit() {
    use tilerider::weapon::laser::crossing_at;
    // Both axes at the middle of the shake: no shake.
    let still = Vec2::splat(0.5);
    assert_eq!(crossing_at(&leg(Vec2::new(2.0, 2.0), Vec2::new(10.0, 2.0)), still), Vec2::new(4.0, 2.0));
    // It goes from where the ray starts, not from the muzzle that draws it.
    let mut bounced = leg(Vec2::new(0.0, 0.0), Vec2::new(0.0, -8.0));
    bounced.start = Vec2::new(5.0, 5.0);
    assert_eq!(crossing_at(&bounced, still), Vec2::new(0.0, -2.0));
    // The shake is up to half a unit on each axis either way.
    let leg = leg(Vec2::ZERO, Vec2::new(4.0, 0.0));
    assert_eq!(crossing_at(&leg, Vec2::ONE), Vec2::new(1.5, 0.5));
    assert_eq!(crossing_at(&leg, Vec2::ZERO), Vec2::new(0.5, -0.5));
}
