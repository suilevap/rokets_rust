//! The player's aim help (`weapon::aim_help`): `UpdateAimMagnet` and the
//! pull in `BaseComplexWeapon.Aim`.

use bevy::math::Vec2;
use bevy::prelude::Entity;
use tilerider::weapon::aim_help::{AimMagnet, COS_MIN, PERIOD, PULL, pick, pulled};

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

#[test]
fn the_nearest_enemy_the_aim_points_at_is_the_candidate() {
    let aim = Vec2::X;
    let near_line = (id(1), Vec2::new(10.0, 1.0));
    let nearer = (id(2), Vec2::new(5.0, 0.0));
    let far = (id(3), Vec2::new(25.0, 0.0));
    assert_eq!(pick([near_line, far], Vec2::ZERO, aim, 32.0), Some(id(1)));
    // Distance × (1 − cosine / 2): the nearer wins.
    assert_eq!(pick([near_line, nearer, far], Vec2::ZERO, aim, 32.0), Some(id(2)));
}

#[test]
fn only_enemies_within_a_cosine_of_three_quarters_of_the_aim_count() {
    // 45° off is a cosine of 0.707: outside.
    let off = (id(1), Vec2::new(5.0, 5.0));
    assert_eq!(COS_MIN, 0.75);
    assert_eq!(pick([off], Vec2::ZERO, Vec2::X, 32.0), None);
    // 30° off is 0.866: inside.
    let inside = (id(2), Vec2::new(8.66, 5.0));
    assert_eq!(pick([off, inside], Vec2::ZERO, Vec2::X, 32.0), Some(id(2)));
    // Behind: never.
    assert_eq!(pick([(id(3), Vec2::new(-5.0, 0.0))], Vec2::ZERO, Vec2::X, 32.0), None);
}

#[test]
fn a_short_aim_weighs_less_and_may_pick_nobody() {
    // The aim is dotted as given, so half a stick is a cosine of at most
    // 0.5: below the minimum.
    assert_eq!(pick([(id(1), Vec2::new(5.0, 0.0))], Vec2::ZERO, Vec2::new(0.5, 0.0), 32.0), None);
    assert_eq!(pick([(id(1), Vec2::new(5.0, 0.0))], Vec2::ZERO, Vec2::new(0.9, 0.0), 32.0), Some(id(1)));
}

#[test]
fn a_score_has_to_be_under_the_range() {
    // Score 20 × (1 − ½) = 10 is under a range of 32 but not of 8.
    let enemy = (id(1), Vec2::new(20.0, 0.0));
    assert_eq!(pick([enemy], Vec2::ZERO, Vec2::X, 32.0), Some(id(1)));
    assert_eq!(pick([enemy], Vec2::ZERO, Vec2::X, 8.0), None);
    // One on top of the car has no direction.
    assert_eq!(pick([(id(2), Vec2::ZERO)], Vec2::ZERO, Vec2::X, 32.0), None);
}

#[test]
fn the_aim_is_pulled_three_quarters_of_the_way_to_the_candidate() {
    assert_eq!(PULL, 0.75);
    // Aiming right, the candidate straight up from the gun.
    let aim = pulled(Vec2::X, Vec2::new(3.0, 3.0), Vec2::new(3.0, 9.0));
    assert!((aim - Vec2::new(0.25, 0.75)).length() < 1e-6, "{aim}");
    // No direction to a candidate under the gun: the aim stays.
    assert_eq!(pulled(Vec2::X, Vec2::ONE, Vec2::ONE), Vec2::X);
}

#[test]
fn the_look_comes_at_once_every_half_second_and_when_the_candidate_dies() {
    let mut magnet = AimMagnet::default();
    let dt = 1.0 / 60.0;
    assert!(magnet.due(dt, true), "the first look is at once");
    let mut looks = 0;
    for _ in 0..(2.0 / dt).round() as usize {
        if magnet.due(dt, true) {
            looks += 1;
        }
    }
    assert_eq!(looks, 4, "every {PERIOD} s");
    // A dead candidate is looked at again straight away.
    magnet.candidate = Some(id(1));
    assert!(magnet.due(dt, false));
    assert!(!magnet.due(dt, true));
}
