//! The Zombie's weapon (`weapon::model::Zombie`, `ZombieWeapon`) and the
//! stun it deals (`stun`): the presets, the look for a target every half
//! second, the bite, the drag, the wave's reload and the held control.

use bevy::math::Vec2;
use bevy::prelude::Entity;
use tilerider::controls::PlayerIntent;
use tilerider::stun::{Stunned, hold};
use tilerider::weapon::model::{
    Energy, Kind, Options, ZOMBIE, ZOMBIE_PLAYER, Zombie, bite_reach, for_type, zombie_circle_rate, zombie_pull,
    zombie_reaches,
};

const DT: f32 = 1.0 / 60.0;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

/// A zombie that has just spawned (first wave and first look due now).
fn fresh(options: &Options) -> Zombie<Entity> {
    Zombie::new(options, 0.0)
}

#[test]
fn the_type_is_the_zombie_with_the_players_faster_reload() {
    assert_eq!(for_type("ZombieWeapon", false), Some((Kind::Zombie, ZOMBIE)));
    assert_eq!(for_type("ZombieWeapon", true), Some((Kind::Zombie, ZOMBIE_PLAYER)));
    assert_eq!((ZOMBIE.reload, ZOMBIE_PLAYER.reload), (10.0, 2.0));
    // A drag towards the zombie, a bite of 2 every half second, 4 units.
    assert_eq!((ZOMBIE.impulse, ZOMBIE.damage, ZOMBIE.shoot_time, ZOMBIE.range), (-36.0, 2, 0.5, 4.0));
    // It spends the whole store, has no barrel, and the player fires it.
    assert_eq!(ZOMBIE.energy, 1.0);
    assert!(!Kind::Zombie.is_complex());
    assert!(!Kind::Zombie.auto_fire());
}

#[test]
fn it_looks_at_once_and_then_every_half_second() {
    let mut zombie = fresh(&ZOMBIE);
    let mut looks = Vec::new();
    for step in 0..(5.0 / DT) as usize {
        zombie.tick(DT);
        if zombie.needs_check() {
            looks.push(step as f32 * DT);
        }
    }
    assert_eq!(looks[0], 0.0);
    // Ten seconds of looks in five: one at the start and one every 30 steps.
    assert!((10..=11).contains(&looks.len()), "{looks:?}");
    for pair in looks.windows(2) {
        assert!((pair[1] - pair[0] - 0.5).abs() < 0.02, "{looks:?}");
    }
}

#[test]
fn a_look_at_the_same_target_near_bites_at_every_look_for_the_presets() {
    assert_eq!(Zombie::<Entity>::looks_per_bite(&ZOMBIE), 1);
    let mut zombie = fresh(&ZOMBIE);
    // The first look finds it: no bite yet (it was not the target).
    assert!(!zombie.look(&ZOMBIE, Some(id(1)), |_| true));
    assert_eq!(zombie.target, Some(id(1)));
    // Every look after bites, while it stays near.
    assert!(zombie.look(&ZOMBIE, Some(id(1)), |_| true));
    assert!(zombie.look(&ZOMBIE, Some(id(1)), |_| true));
}

#[test]
fn a_target_in_range_but_out_of_the_bites_reach_is_dragged_not_bitten() {
    let mut zombie = fresh(&ZOMBIE);
    zombie.look(&ZOMBIE, Some(id(1)), |_| false);
    assert!(!zombie.look(&ZOMBIE, Some(id(1)), |_| false));
    assert_eq!(zombie.target, Some(id(1)));
    assert!(zombie.look(&ZOMBIE, Some(id(1)), |_| true));
}

#[test]
fn a_new_target_starts_over_and_nobody_clears_the_target() {
    let mut zombie = fresh(&ZOMBIE);
    zombie.look(&ZOMBIE, Some(id(1)), |_| true);
    // Another one is first in the list: no bite for it at its first look.
    assert!(!zombie.look(&ZOMBIE, Some(id(2)), |_| true));
    assert_eq!(zombie.target, Some(id(2)));
    // Nobody in range: no target (so nothing is dragged), and the one who
    // comes back has to be found first, as a new target.
    assert!(!zombie.look(&ZOMBIE, None, |_| true));
    assert_eq!(zombie.target, None);
    assert!(!zombie.look(&ZOMBIE, Some(id(2)), |_| true));
}

#[test]
fn a_slower_bite_takes_several_looks_and_the_count_survives_a_miss() {
    let slow = Options { shoot_time: 1.5, ..ZOMBIE };
    assert_eq!(Zombie::<Entity>::looks_per_bite(&slow), 3);
    let mut zombie = fresh(&slow);
    let near = |_| true;
    assert!(!zombie.look(&slow, Some(id(1)), near)); // found
    assert!(!zombie.look(&slow, Some(id(1)), near)); // 1
    assert!(!zombie.look(&slow, Some(id(1)), |_| false)); // too far: not counted
    assert!(!zombie.look(&slow, Some(id(1)), near)); // 2
    assert!(zombie.look(&slow, Some(id(1)), near)); // 3: bites
    assert!(!zombie.look(&slow, Some(id(1)), near)); // starts again
}

#[test]
fn the_wave_waits_for_its_reload_and_the_players_energy() {
    let mut zombie = fresh(&ZOMBIE_PLAYER);
    let mut energy = Energy::default();
    // The first pull works (a Sparse offset of 0 here) and costs the store.
    assert!(zombie.pull(&ZOMBIE_PLAYER, Some(&mut energy)));
    assert!(energy.current < 1e-6);
    // Pulled again at once: the reload is not over, and nothing is spent.
    energy.current = 1.0;
    assert!(!zombie.pull(&ZOMBIE_PLAYER, Some(&mut energy)));
    assert_eq!(energy.current, 1.0);
    // After the 2 s the player's reload is over, but with the store not full
    // the pull overloads it and sends nothing.
    for _ in 0..(2.0 / DT) as usize + 3 {
        zombie.tick(DT);
    }
    energy.current = 0.5;
    assert!(!zombie.pull(&ZOMBIE_PLAYER, Some(&mut energy)));
    assert!(energy.overloaded);
}

#[test]
fn an_enemy_zombie_waves_every_ten_seconds_for_nothing() {
    let mut zombie = fresh(&ZOMBIE);
    assert!(zombie.pull(&ZOMBIE, None));
    for _ in 0..(9.0 / DT) as usize {
        zombie.tick(DT);
    }
    assert!(!zombie.pull(&ZOMBIE, None));
    for _ in 0..(1.1 / DT) as usize {
        zombie.tick(DT);
    }
    assert!(zombie.pull(&ZOMBIE, None));
}

#[test]
fn the_first_wave_waits_a_random_part_of_the_reload() {
    let mut zombie = Zombie::<Entity>::new(&ZOMBIE, 0.5);
    assert!(!zombie.pull(&ZOMBIE, None));
    for _ in 0..(5.1 / DT) as usize {
        zombie.tick(DT);
    }
    assert!(zombie.pull(&ZOMBIE, None));
}

#[test]
fn the_range_is_a_square_for_the_choice_and_a_circle_for_the_bite() {
    let zombie = Vec2::new(10.0, 10.0);
    // Within 4 on both axes: a corner is 5.6 away, still in.
    assert!(zombie_reaches(zombie, Vec2::new(13.9, 13.9), &ZOMBIE, 1.0));
    assert!(!zombie_reaches(zombie, Vec2::new(14.1, 10.0), &ZOMBIE, 1.0));
    // The wave reaches twice as far.
    assert!(zombie_reaches(zombie, Vec2::new(17.9, 10.0), &ZOMBIE, 2.0));
    assert!(!zombie_reaches(zombie, Vec2::new(18.1, 10.0), &ZOMBIE, 2.0));
    // The bite: the two radii and one.
    assert!(bite_reach(zombie, Vec2::new(11.9, 10.0), 0.5, 0.5));
    assert!(!bite_reach(zombie, Vec2::new(12.1, 10.0), 0.5, 0.5));
    // A corner at 1.5 on both axes is 2.12 away: out of the bite.
    assert!(!bite_reach(zombie, Vec2::new(11.5, 11.5), 0.5, 0.5));
}

#[test]
fn the_drag_points_at_the_zombie_with_a_force_of_36() {
    let pull = zombie_pull(Vec2::ZERO, Vec2::new(3.0, 4.0), &ZOMBIE);
    assert!((pull - Vec2::new(-21.6, -28.8)).length() < 1e-4, "{pull}");
    // At one point there is no direction (`SafeNormalize`).
    assert_eq!(zombie_pull(Vec2::ONE, Vec2::ONE, &ZOMBIE), Vec2::ZERO);
}

#[test]
fn the_waves_circle_covers_twice_the_range() {
    // A 204 px circle texture is 3.19 units in radius: 8 units is 2.51 times.
    let rate = zombie_circle_rate(ZOMBIE.range * 2.0, 204.0);
    assert!((rate - 8.0 / 3.1875).abs() < 1e-5, "{rate}");
}

#[test]
fn a_stun_clears_the_control_but_not_the_gadget_and_lasts_its_time() {
    let mut intent = PlayerIntent {
        move_dir: Vec2::X,
        aim_dir: Vec2::Y,
        look: Vec2::Y,
        fire: true,
        fire_accurate: true,
        gadget: true,
        ..Default::default()
    };
    hold(&mut intent);
    assert_eq!((intent.move_dir, intent.aim_dir, intent.look), (Vec2::ZERO, Vec2::ZERO, Vec2::ZERO));
    assert!(!intent.fire && !intent.fire_accurate);
    assert!(intent.gadget);

    let mut stun = Stunned { left: 2.0 };
    let mut steps = 0;
    while !stun.step(DT) {
        steps += 1;
    }
    // Two seconds, to a step.
    assert!(((steps + 1) as f32 * DT - 2.0).abs() < 2.0 * DT, "{steps}");
}
