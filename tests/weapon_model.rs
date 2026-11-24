//! The weapon's numbers and timing (`weapon::model`), as `MinigunWeapon`,
//! `EnergyComponent` and `BaseComplexWeapon` behaved.

use std::f32::consts::PI;

use bevy::math::Vec2;
use tilerider::weapon::model::{
    Aim, AimLook, Energy, HAMMER, HAMMER_TRACKING_PERIOD, Hammer, Kind, MINIGUN, MINIGUN_PLAYER, Minigun, RAILGUN,
    REFLECTION_MIN_ANGLE, Railgun, STRONG_IMPULSE, Shot, Tracking, WEAK_IMPULSE, for_type, hammer_damage, reflect,
    reflected_damage, saw_turn,
};

const DT: f32 = 1.0 / 60.0;

/// Holds the trigger for `secs`, counting bullets; `random` as given.
fn hold(gun: &mut Minigun, energy: &mut Option<Energy>, secs: f32, random: f32) -> Vec<Shot> {
    let mut shots = Vec::new();
    for _ in 0..(secs / DT).round() as usize {
        if let Some(e) = energy.as_mut() {
            e.step(DT);
        }
        gun.pull(&MINIGUN_PLAYER, energy.as_mut());
        shots.extend(gun.step(DT, &MINIGUN_PLAYER, energy.as_mut(), || random));
    }
    shots
}

fn bullets(shots: &[Shot]) -> usize {
    shots.iter().filter(|s| matches!(s, Shot::Bullet { .. })).count()
}

#[test]
fn types_pick_the_players_preset() {
    assert_eq!(for_type("Minigun360", true), Some((Kind::Minigun, MINIGUN_PLAYER)));
    assert_eq!(for_type("Minigun", false), Some((Kind::Minigun, MINIGUN)));
    assert_eq!(for_type("Minigun180", false).unwrap().1.max_angle, PI / 2.0);
    // One preset for all, with the default energy.
    let slow = for_type("MinigunSlowInacurate", true).unwrap().1;
    assert_eq!((slow.bullets, slow.energy), (5, 0.1));
    assert_eq!(for_type("Rocketlauncher", true), None);
}

#[test]
fn the_railgun_has_one_preset_for_everyone() {
    assert_eq!(for_type("Railgun", true), Some((Kind::Railgun, RAILGUN)));
    assert_eq!(for_type("Railgun", false), Some((Kind::Railgun, RAILGUN)));
    assert_eq!((RAILGUN.damage, RAILGUN.impulse, RAILGUN.reload), (25, 60.0, 1.5));
    assert_eq!((RAILGUN.energy, RAILGUN.reflections, RAILGUN.dispersion), (0.6, 1, 0.0));
    // The variants keep the default energy; one bounces no more, one turns
    // only 5°.
    let none = for_type("RailgunNoReflection", true).unwrap().1;
    assert_eq!((none.reflections, none.energy, none.damage), (0, 0.1, 25));
    let angle5 = for_type("RailgunAngle5", false).unwrap().1;
    assert_eq!((angle5.reflections, angle5.energy), (1, 0.1));
    assert!((angle5.max_angle - PI / 36.0).abs() < 1e-6);
}

/// Pulls the trigger every step for `secs`, counting shots.
fn pull_for(gun: &mut Railgun, energy: &mut Option<Energy>, secs: f32) -> usize {
    let mut shots = 0;
    for _ in 0..(secs / DT).round() as usize {
        if let Some(e) = energy.as_mut() {
            e.step(DT);
        }
        shots += gun.pull(&RAILGUN, energy.as_mut()) as usize;
        gun.step(DT);
    }
    shots
}

#[test]
fn the_railgun_fires_once_a_reload() {
    let mut gun = Railgun::new(&RAILGUN, 0.0);
    // Pulled all the time: at the start, then every 1.5 s.
    assert_eq!(pull_for(&mut gun, &mut None, 0.1), 1);
    assert_eq!(pull_for(&mut gun, &mut None, 1.3), 0);
    assert_eq!(pull_for(&mut gun, &mut None, 0.2), 1);
    // The first one waits a random part of the reload.
    let mut late = Railgun::new(&RAILGUN, 0.9);
    assert_eq!(pull_for(&mut late, &mut None, 1.0), 0);
    assert_eq!(pull_for(&mut late, &mut None, 0.5), 1);
}

#[test]
fn a_railgun_shot_costs_over_half_the_energy() {
    let mut gun = Railgun::new(&RAILGUN, 0.0);
    let mut energy = Some(Energy::default());
    assert_eq!(pull_for(&mut gun, &mut energy, 0.1), 1);
    assert!((energy.as_ref().unwrap().current - 0.4).abs() < 0.03);
    // The store refills 0.5 a second after a second's wait: by the end of
    // the reload there is enough for a second shot...
    assert_eq!(pull_for(&mut gun, &mut energy, 1.5), 1);
    // ...but not for a third, which overloads the store: no shot (and no
    // click).
    assert_eq!(pull_for(&mut gun, &mut energy, 1.5), 0);
    assert!(energy.as_ref().unwrap().overloaded);
    // Refilled past half it fires again.
    assert_eq!(pull_for(&mut gun, &mut energy, 2.0), 1);
}

#[test]
fn the_aim_line_shows_the_reload_then_the_energy() {
    let mut gun = Railgun::new(&RAILGUN, 0.0);
    assert_eq!(gun.look(&RAILGUN, None), AimLook::Normal);
    let mut energy = Energy::default();
    assert!(gun.pull(&RAILGUN, Some(&mut energy)));
    // Just fired: all of the reload left, then less and less.
    let AimLook::Reload(full) = gun.look(&RAILGUN, Some(&mut energy)) else {
        panic!("reloading");
    };
    assert!(full > 0.99, "{full}");
    gun.step(0.75);
    let AimLook::Reload(half) = gun.look(&RAILGUN, Some(&mut energy)) else {
        panic!("reloading");
    };
    assert!((half - 0.5).abs() < 0.01, "{half}");
    // Reloaded but 0.4 of the energy is not enough.
    gun.step(1.0);
    assert_eq!(gun.look(&RAILGUN, Some(&mut energy)), AimLook::Low);
    // Asking sets the bar's last-asked mark to the cost.
    assert_eq!(energy.last_check, RAILGUN.energy);
    energy.current = 1.0;
    assert_eq!(gun.look(&RAILGUN, Some(&mut energy)), AimLook::Normal);
}

#[test]
fn a_bullet_bounces_off_a_surface_like_light_off_a_mirror() {
    // Flying right at a slope facing up and left (45° off its normal):
    // leaves straight up.
    let out = reflect(0.0, Vec2::new(-1.0, 1.0).normalize()).unwrap();
    assert!((out - PI / 2.0).abs() < 1e-5, "{out}");
    // Against a wall facing left, coming in 30° up from the right: it
    // leaves 30° the other way, still going left.
    let out = reflect(PI / 6.0, Vec2::NEG_X).unwrap();
    assert!((out - 5.0 * PI / 6.0).abs() < 1e-5 || (out + 7.0 * PI / 6.0).abs() < 1e-5, "{out}");
}

#[test]
fn a_bullet_stops_where_there_is_nothing_to_bounce_off() {
    // No surface (it hit nothing, or began inside something).
    assert_eq!(reflect(0.0, Vec2::ZERO), None);
    // Head-on, or within 15° of it.
    assert_eq!(reflect(0.0, Vec2::NEG_X), None);
    let near = REFLECTION_MIN_ANGLE * 0.9;
    assert_eq!(reflect(near, Vec2::NEG_X), None);
    let far = REFLECTION_MIN_ANGLE * 1.1;
    assert!(reflect(far, Vec2::NEG_X).is_some());
}

#[test]
fn the_bounce_does_half_the_damage_rounded_down() {
    assert_eq!(reflected_damage(25), 12);
    assert_eq!(reflected_damage(1), 0);
}

#[test]
fn a_held_trigger_fires_about_ten_a_second() {
    let mut gun = Minigun::new(&MINIGUN_PLAYER, 0.0);
    let shots = hold(&mut gun, &mut None, 1.0, 0.5);
    // A bullet every sixth step, from the first.
    assert_eq!(bullets(&shots), 10);
}

#[test]
fn letting_go_stops_within_two_bullets() {
    let mut gun = Minigun::new(&MINIGUN_PLAYER, 0.0);
    hold(&mut gun, &mut None, 0.5, 0.5);
    let mut after = 0;
    for _ in 0..60 {
        if let Some(Shot::Bullet { .. }) = gun.step(DT, &MINIGUN_PLAYER, None, || 0.5) {
            after += 1;
        }
    }
    assert!(after <= 2, "{after}");
    assert!(!gun.active);
}

#[test]
fn the_energy_lasts_twenty_five_bullets_then_overloads() {
    let mut gun = Minigun::new(&MINIGUN_PLAYER, 0.0);
    let mut energy = Some(Energy::default());
    let shots = hold(&mut gun, &mut energy, 4.0, 0.5);
    let e = energy.unwrap();
    // 1 / 0.04 (give or take the float sum), then dry.
    assert!((24..=25).contains(&bullets(&shots)), "{}", bullets(&shots));
    assert!(e.overloaded);
    assert!(shots.iter().any(|s| matches!(s, Shot::Misfire { .. })));
}

#[test]
fn it_refills_after_a_second_and_cools_down_over_half() {
    let mut e = Energy::default();
    assert!(e.try_spend(0.9));
    for _ in 0..59 {
        e.step(DT);
    }
    assert!((e.current - 0.1).abs() < 1e-4, "still waiting: {}", e.current);
    for _ in 0..61 {
        e.step(DT);
    }
    // A second at half a unit a second.
    assert!(e.current > 0.55 && e.current < 0.62, "{}", e.current);
    // Dry: overloaded until over half (a failed spend does not delay
    // the refill).
    assert!(e.try_spend(0.5));
    assert!(!e.try_spend(0.5));
    assert!(e.overloaded && !e.check(0.0));
    while e.current < 0.49 {
        e.step(DT);
        assert!(e.overloaded, "{}", e.current);
    }
    for _ in 0..3 {
        e.step(DT);
    }
    assert!(!e.overloaded, "{}", e.current);
}

#[test]
fn one_bullet_in_five_and_the_last_push_hard() {
    let rate = |random: f32| {
        let mut gun = Minigun::new(&MINIGUN_PLAYER, 0.0);
        match hold(&mut gun, &mut None, 0.2, random).first() {
            Some(Shot::Bullet { impulse_rate, .. }) => *impulse_rate,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(rate(0.1), STRONG_IMPULSE);
    assert_eq!(rate(0.5), WEAK_IMPULSE);
    // The last bullet the energy allows.
    let mut gun = Minigun::new(&MINIGUN_PLAYER, 0.0);
    let mut energy = Some(Energy::default());
    energy.as_mut().unwrap().current = 0.05;
    let shots = hold(&mut gun, &mut energy, 0.2, 0.9);
    assert!(matches!(shots.first(), Some(Shot::Bullet { impulse_rate, .. }) if *impulse_rate == STRONG_IMPULSE));
}

#[test]
fn the_players_aim_turns_all_round_and_swings_back() {
    let mut aim = Aim::new(PI / 3.0, true);
    aim.aim(PI, 0.0);
    for _ in 0..60 {
        aim.step(DT);
    }
    assert!((aim.angle.current.abs() - PI).abs() < 0.05, "{}", aim.angle.current);
    // Unused for 4 s: back ahead.
    for _ in 0..(5.0 / DT) as usize {
        aim.step(DT);
    }
    assert!(aim.angle.current.abs() < 0.05, "{}", aim.angle.current);
    // Others' guns turn only so far.
    let mut ai = Aim::new(PI / 3.0, false);
    ai.aim(PI / 2.0, 0.0);
    assert_eq!(ai.angle.destination, PI / 3.0);
}

/// A 1.5 × 1 car, as the saw's sprite is square.
fn saw() -> Hammer {
    Hammer::new(Vec2::new(1.5, 1.0), 1.0)
}

#[test]
fn the_hammer_has_one_preset_for_everyone() {
    assert_eq!(for_type("Hammer", true), Some((Kind::Hammer, HAMMER)));
    assert_eq!(for_type("Hammer", false), Some((Kind::Hammer, HAMMER)));
    assert_eq!((HAMMER.damage, HAMMER.damage_max, HAMMER.impulse), (4, 8, 48.0));
    assert!(Kind::Hammer.auto_fire() && !Kind::Minigun.auto_fire() && !Kind::Railgun.auto_fire());
}

#[test]
fn the_saw_sits_on_the_nose_and_is_a_fifth_wider_than_the_car() {
    let saw = saw();
    // From the middle of the car to its nose (0.75), less 0.2 × 1.2 × its width.
    assert!((saw.mount.x - (0.75 - 0.24)).abs() < 1e-6 && saw.mount.y == 0.0);
    assert!((saw.diameter - 1.2).abs() < 1e-6 && (saw.radius - 0.6).abs() < 1e-6);
    // It sticks out 0.36 past the nose.
    assert!((saw.mount.x + saw.radius - (0.75 + 0.36)).abs() < 1e-6);
    // A wider sprite is wider.
    assert!((Hammer::new(Vec2::new(1.5, 1.0), 2.0).radius - 1.2).abs() < 1e-6);
}

#[test]
fn the_saw_reaches_by_a_square_of_the_radii() {
    let saw = saw();
    let at = Vec2::new(5.0, 5.0);
    // A box of radius 0.5: both axes within 1.1.
    assert!(saw.reaches(at, at + Vec2::new(1.0, 1.0), 0.5));
    // Its corner is further than 1.1 in a circle, but a square takes it.
    assert!(saw.reaches(at, at + Vec2::new(1.09, 1.09), 0.5));
    assert!(!saw.reaches(at, at + Vec2::new(1.11, 0.0), 0.5));
    assert!(!saw.reaches(at, at + Vec2::new(0.0, -1.11), 0.5));
    // Strictly closer: just inside the sum reaches, just outside not.
    assert!(saw.reaches(at, at + Vec2::new(1.099, 0.0), 0.5));
    assert!(!saw.reaches(at, at + Vec2::new(1.101, 0.0), 0.5));
}

#[test]
fn a_blow_hurts_by_the_cars_speed() {
    // Standing: the least. At 16 and over: the most. Between: by the square.
    assert_eq!(hammer_damage(&HAMMER, 0.0, false), 4);
    assert_eq!(hammer_damage(&HAMMER, 16.0 * 16.0, false), 8);
    assert_eq!(hammer_damage(&HAMMER, 40.0 * 40.0, false), 8);
    // 8 units a second: a quarter of the way, 4 + 4 × ¼ = 5.
    assert_eq!(hammer_damage(&HAMMER, 64.0, false), 5);
    // Cut, not rounded: 4 + 4 × 0.9 = 7.6.
    assert_eq!(hammer_damage(&HAMMER, 0.9 * 256.0, false), 7);
    // A car that stands static does the most.
    assert_eq!(hammer_damage(&HAMMER, 0.0, true), 8);
}

#[test]
fn the_working_saw_shivers_a_radian_every_tenth_of_a_second() {
    assert_eq!(saw_turn(0.0), 0.0);
    assert!((saw_turn(0.05) - 0.5).abs() < 1e-4);
    assert!(saw_turn(0.0999) > 0.99);
    assert!(saw_turn(0.1001) < 0.02);
}

#[test]
fn tracked_enemies_are_hurt_again_every_third_of_a_second_while_near() {
    let mut tracking: Tracking<u32> = Tracking::default();
    // Not working: nothing, and the list stays.
    tracking.add(1);
    assert!(tracking.due(false, |_| true).is_empty());
    assert_eq!(tracking.len(), 1);
    // The first round is at once (`_invalidatedTime` 0).
    assert_eq!(tracking.due(true, |_| true), vec![1]);
    // Then every 0.33 s.
    tracking.tick(0.3);
    assert!(tracking.due(true, |_| true).is_empty());
    tracking.tick(HAMMER_TRACKING_PERIOD - 0.3);
    assert_eq!(tracking.due(true, |_| true), vec![1]);
}

#[test]
fn tracking_drops_the_far_and_the_dead_and_counts_each_blow() {
    let mut tracking: Tracking<u32> = Tracking::default();
    // Hit twice before the round: hurt twice (`Add` did not look).
    for enemy in [1, 2, 2, 3] {
        tracking.add(enemy);
    }
    assert_eq!(tracking.due(true, |e| e != 3), vec![1, 2, 2]);
    assert_eq!(tracking.len(), 3);
    // 3 is forgotten for good, even if it comes back near.
    tracking.tick(1.0);
    assert_eq!(tracking.due(true, |_| true), vec![1, 2, 2]);
    // The timer ran out with nobody tracked, as `NeedUpdate` was asked first.
    let mut idle: Tracking<u32> = Tracking::default();
    assert!(idle.due(true, |_| true).is_empty());
    idle.add(7);
    assert!(idle.due(true, |_| true).is_empty(), "the round was spent");
    idle.tick(HAMMER_TRACKING_PERIOD);
    assert_eq!(idle.due(true, |_| true), vec![7]);
}

#[test]
fn an_enemy_fires_when_the_barrel_points_at_the_target_within_range() {
    use tilerider::weapon::model::target_in_fire_range as in_range;
    let (range, spread) = (MINIGUN.range, MINIGUN.dispersion);
    let at = |x: f32, y: f32| Vec2::new(x, y);
    // Straight along the barrel, near and at the edge of the range.
    assert!(in_range(Vec2::ZERO, 0.0, range, spread, at(10.0, 0.0), 1.0));
    assert!(!in_range(Vec2::ZERO, 0.0, range, spread, at(range + 0.1, 0.0), 1.0));
    // Off the line by more than the spread plus what the radius covers.
    assert!(!in_range(Vec2::ZERO, 0.0, range, spread, at(10.0, 4.0), 1.0));
    // A bigger radius (a worse `FireAccuracy`) covers more.
    assert!(in_range(Vec2::ZERO, 0.0, range, spread, at(10.0, 4.0), 3.0));
    // It is the barrel's direction now, the short way round.
    assert!(in_range(Vec2::ZERO, 2.0 * PI - 0.05, range, spread, at(10.0, 0.0), 1.0));
    assert!(!in_range(Vec2::ZERO, PI, range, spread, at(10.0, 0.0), 1.0));
    // From the muzzle, not the car's middle; right on it counts.
    assert!(in_range(at(5.0, 5.0), 0.0, range, spread, at(5.0, 5.0), 1.0));
}

#[test]
fn an_enemys_aim_line_shows_by_the_c_sharp_rules() {
    use tilerider::weapon::model::aim_line_on;
    // (local, team, can_move, aiming, alive, active, gadget)
    // A turret that cannot move always shows it, even not aiming.
    assert!(aim_line_on(false, 1, false, false, true, true, false));
    // A car that moves shows it only while aiming.
    assert!(!aim_line_on(false, 1, true, false, true, true, false));
    assert!(aim_line_on(false, 1, true, true, true, true, false));
    // The friends' (team 0, not the player) do not, not even turrets.
    assert!(!aim_line_on(false, 0, false, true, true, true, false));
    assert!(aim_line_on(true, 0, true, true, true, true, false));
    // Dead, inactive or charging a gadget: no line.
    assert!(!aim_line_on(false, 1, false, true, false, true, false));
    assert!(!aim_line_on(false, 1, false, true, true, false, false));
    assert!(!aim_line_on(true, 0, true, true, true, true, true));
}
