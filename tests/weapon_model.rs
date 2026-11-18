//! The weapon's numbers and timing (`weapon::model`), as `MinigunWeapon`,
//! `EnergyComponent` and `BaseComplexWeapon` behaved.

use std::f32::consts::PI;

use tilerider::weapon::model::{
    Aim, Energy, Kind, MINIGUN, MINIGUN_PLAYER, Minigun, STRONG_IMPULSE, Shot, WEAK_IMPULSE, for_type,
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
    assert_eq!(for_type("Railgun", true), None);
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
