//! The weapon's numbers and timing (`weapon::model`), as `MinigunWeapon`,
//! `EnergyComponent` and `BaseComplexWeapon` behaved.

use std::f32::consts::PI;

use bevy::math::Vec2;
use tilerider::weapon::model::{
    Aim, AimLook, Energy, Kind, MINIGUN, MINIGUN_PLAYER, Minigun, RAILGUN, REFLECTION_MIN_ANGLE, Railgun, STRONG_IMPULSE,
    Shot, WEAK_IMPULSE, for_type, reflect, reflected_damage,
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
