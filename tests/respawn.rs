//! Respawn points and lives.

use bevy::math::Vec2;
use tilerider::game_mode::Respawn;

#[test]
fn without_a_point_death_restarts_and_with_one_three_lives_bring_it_back() {
    let mut r = Respawn::default();
    assert!(!r.on_death(), "no respawn point yet: restart");
    assert_eq!(r.lives, 3, "a restart costs no life");
    r.point = Some((Vec2::ZERO, 0.0));
    assert!(r.on_death() && r.on_death() && r.on_death());
    assert_eq!(r.lives, 0);
    assert!(!r.on_death(), "the fourth death restarts the level");
}

mod comes_back_with_what_it_carried {
    use bevy::ecs::world::CommandQueue;
    use bevy::prelude::*;
    use tilerider::car::Mounts;
    use tilerider::gadget::Magnet;
    use tilerider::gadget::accelerator::Accelerator;
    use tilerider::game_mode::{Kept, KeptGadget, give_back};
    use tilerider::weapon::Weapon;
    use tilerider::weapon::model::{Kind, for_type};

    const MOUNTS: Mounts = Mounts { weapon: Vec2::new(0.5, 0.0), gadget: Vec2::new(-0.5, 0.0), size: Vec2::new(2.0, 1.0) };

    fn respawn(world: &mut World, car: Entity, kept: Kept) {
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        give_back(&mut commands, car, kept, &MOUNTS, Some(4001));
        queue.apply(world);
    }

    #[test]
    fn a_car_the_map_gave_the_accelerator_gets_it_back_instead_of_its_magnet() {
        // Derby: the buggy starts with a magnet, `SetWeapon` gives it the
        // accelerator; the map's car would come back with the magnet.
        let mut world = World::new();
        let car = world.spawn(Magnet::new(MOUNTS.gadget)).id();
        respawn(&mut world, car, Kept { gun: None, gadget: KeptGadget::Accelerator });
        assert!(world.get::<Accelerator>(car).is_some());
        assert!(world.get::<Magnet>(car).is_none());
        assert!(world.get::<Weapon>(car).is_none(), "it died with no gun");
    }

    #[test]
    fn the_gun_it_died_with_replaces_the_maps() {
        let mut world = World::new();
        let car = world.spawn(Accelerator::new()).id();
        let railgun = for_type("Railgun", true).unwrap();
        respawn(&mut world, car, Kept { gun: Some(railgun), gadget: KeptGadget::Magnet });
        assert_eq!(world.get::<Weapon>(car).map(|w| w.kind), Some(Kind::Railgun));
        assert!(world.get::<Magnet>(car).is_some());
        assert!(world.get::<Accelerator>(car).is_none());
    }

    #[test]
    fn a_car_that_lost_its_gadget_comes_back_without_one() {
        let mut world = World::new();
        let car = world.spawn((Magnet::new(MOUNTS.gadget), Accelerator::new())).id();
        respawn(&mut world, car, Kept::default());
        assert!(world.get::<Magnet>(car).is_none() && world.get::<Accelerator>(car).is_none());
    }
}
