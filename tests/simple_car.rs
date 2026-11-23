//! How the map's simple cars drive and which team they are on.

use bevy::math::Vec2;
use tilerider::car::control::{Team, control_kind, spares};
use tilerider::car::simple::SimpleDrive;

fn drive() -> SimpleDrive {
    // A buggy's numbers (`SimpleBuggy`: Speed 0.92, Acceleration 1).
    SimpleDrive::new(0.92, 1.0, 0.0, 1.0)
}

#[test]
fn a_stick_pushes_along_itself_with_acceleration_force_over_two() {
    let d = drive();
    assert!((d.max_speed - 1500.0 / 32.0 * 0.5 * 0.92).abs() < 1e-5);
    let f = d.force(Vec2::ZERO, Vec2::new(1.0, 0.0)).expect("a push");
    assert!((f.x - 64.0 / 2.0).abs() < 1e-4 && f.y == 0.0, "{f}");
    // A half stick pushes half as hard.
    let half = d.force(Vec2::ZERO, Vec2::new(0.0, 0.5)).expect("a push");
    assert!((half.y - 16.0).abs() < 1e-4 && half.x == 0.0, "{half}");
}

#[test]
fn no_stick_or_top_speed_means_no_push() {
    let d = drive();
    assert_eq!(d.force(Vec2::new(3.0, 0.0), Vec2::ZERO), None);
    // At top speed along the stick, no more; slower, still.
    assert_eq!(d.force(Vec2::new(d.max_speed, 0.0), Vec2::X), None);
    assert!(d.force(Vec2::new(d.max_speed * 0.9, 0.0), Vec2::X).is_some());
    // A shorter stick has a lower top speed.
    assert_eq!(d.force(Vec2::new(d.max_speed * 0.5, 0.0), Vec2::new(0.5, 0.0)), None);
}

#[test]
fn controllability_pushes_against_the_sideways_speed() {
    let mut d = drive();
    d.controllability = 1.0;
    // Going up while the stick says right: the sideways part (up, here
    // all of it) is pushed against, so the push leans down.
    let f = d.force(Vec2::new(0.0, 2.0), Vec2::X).expect("a push");
    assert!(f.y < 0.0, "{f}");
    // Straight along the stick it makes no difference.
    let straight = d.force(Vec2::new(2.0, 0.0), Vec2::X).expect("a push");
    assert!((straight - Vec2::new(32.0, 0.0)).length() < 1e-4, "{straight}");
}

#[test]
fn enemies_are_team_one_friends_and_the_player_team_zero() {
    for (name, team) in [
        ("Local", 0),
        ("NoneLocal", 0),
        ("NoneFriend", 0),
        ("NoneEnemy", 1),
        ("AiSimpleTurret", 1),
        ("AiSimpleTurretFriend", 0),
        ("AlwaysShoot", 1),
        ("ZombieFlocking", 1),
        ("ZombieFlockingFriend", 0),
        ("HammerAiFriend", 0),
        ("ShootPattern", 1),
    ] {
        assert_eq!(control_kind(name).team, team, "{name}");
    }
}

#[test]
fn only_the_none_controls_leave_a_car_undriven() {
    for name in ["NoneLocal", "NoneEnemy", "NoneFriend"] {
        assert!(control_kind(name).ai_none, "{name}");
    }
    for name in ["Local", "AlwaysShoot", "AiSimpleTurret", "HammerAi"] {
        assert!(!control_kind(name).ai_none, "{name}");
    }
    // The player's own devices, and `NoneLocal` held for them.
    assert!(control_kind("Local").local_device && control_kind("NoneLocal").local_device);
    assert!(!control_kind("NoneEnemy").local_device && !control_kind("AlwaysShoot").local_device);
}

#[test]
fn a_blow_spares_its_own_team_only() {
    assert!(spares(Some(Team(1)), Some(Team(1))));
    assert!(!spares(Some(Team(1)), Some(Team(0))));
    // Something with no team (a box, a blast with no owner) hurts and is hurt.
    assert!(!spares(None, Some(Team(1))));
    assert!(!spares(Some(Team(0)), None));
}

mod shield {
    use tilerider::car::shield::{QUIET, Shield};

    #[test]
    fn a_shield_takes_whole_points_and_passes_on_the_rest() {
        let mut s = Shield::new(4);
        assert_eq!(s.spend(1), 0);
        assert_eq!(s.points(), 3);
        assert_eq!(s.spend(5), 2, "three points soaked, two through");
        assert_eq!(s.points(), 0);
        assert_eq!(s.spend(3), 3, "a shield that is down passes all");
    }

    #[test]
    fn it_grows_back_only_after_the_quiet_spell() {
        let mut s = Shield::new(8);
        s.spend(8);
        s.step(QUIET - 0.1);
        assert_eq!(s.points(), 0, "still quiet");
        s.step(0.2);
        s.step(1.0);
        assert!((4..=5).contains(&s.points()), "{}", s.points());
        s.step(5.0);
        assert!(s.points() >= 8 && s.part() >= 1.0);
        // A blow restarts the wait, even on a down shield.
        let mut s = Shield::new(4);
        s.spend(10);
        s.step(3.0);
        s.spend(1);
        s.step(3.0);
        assert_eq!(s.points(), 0);
    }

    #[test]
    fn an_immortal_shield_takes_blows_whole_and_loses_nothing() {
        let mut s = Shield::new(4);
        s.set_immortal(true);
        assert_eq!(s.spend(3), 0);
        assert_eq!(s.spend(100), 0, "however hard");
        assert_eq!((s.points(), s.is_immortal()), (4, true));
        // Mortal again, it takes damage as any shield does.
        s.set_immortal(false);
        assert_eq!(s.spend(5), 1);
        assert_eq!(s.points(), 0);
    }

    #[test]
    fn an_immortal_shield_that_is_down_passes_the_blow_on() {
        // `Spend` guards on `Value > 0`: with nothing left, immortal or not,
        // the health takes it.
        let mut s = Shield::new(4);
        s.spend(4);
        s.set_immortal(true);
        assert_eq!(s.spend(3), 3);
    }

    #[test]
    fn a_blow_restarts_the_quiet_spell_even_on_an_immortal_shield() {
        let mut s = Shield::new(4);
        s.spend(2);
        s.set_immortal(true);
        s.step(3.0);
        s.spend(1);
        s.step(3.0);
        assert_eq!(s.points(), 2, "no growing back yet");
    }

    #[test]
    fn a_new_shield_may_grow_at_once_but_has_nothing_to_grow() {
        let mut s = Shield::new(4);
        s.step(1.0);
        assert_eq!((s.points(), s.max()), (4, 4));
    }
}

mod catalogue {
    use std::fs;
    use std::path::Path;

    use tilerider::car::weapon_type;
    use tilerider::content::cars::CarCatalog;
    use tilerider::content::map::MapData;

    fn cars() -> CarCatalog {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Cars/Cars.xml");
        CarCatalog::parse(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn simple_cars_are_a_catalogue_of_their_own() {
        let cars = cars();
        assert!(cars.get_simple("SimpleTarget8HP").is_some());
        // The players' cars are not simple cars, nor the other way round.
        assert!(cars.get("SimpleTarget8HP").is_none());
        assert!(cars.get_simple("SandBuggyMinigun").is_none());
        let target = cars.get_simple("SimpleTarget8HP").unwrap();
        assert_eq!((target.max_health, target.mass, target.can_move), (8, -1.0, true));
        assert_eq!(target.weapon, None);
    }

    #[test]
    fn a_simple_car_takes_its_base_and_only_its_own_weapon() {
        let cars = cars();
        let hammer = cars.get_simple("SimpleBuggyHammer").unwrap();
        assert_eq!(hammer.weapon.as_deref(), Some("Hammer"));
        assert_eq!(hammer.max_health, 10);
        assert_eq!(hammer.sprites_path.as_deref(), Some("Cars\\SimpleBuggy"));
        let turret = cars.get_simple("TurretStatHammer").unwrap();
        assert!(!turret.can_move && !turret.can_link && turret.shield == 4);
        assert_eq!(turret.weapon.as_deref(), Some("Hammer"));
        let minigun = cars.get_simple("TurretStatMinigun").unwrap();
        assert_eq!(minigun.weapon.as_deref(), Some("Minigun"));
        let generator = cars.get_simple("EnemyGenerator").unwrap();
        assert!(generator.use_circle_for_geometry && (generator.geometry_scale - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_base_of_a_base_is_resolved_too() {
        let xml = r#"<XnaContent><Asset Type="GameLibrary.CarsData"><Cars/><SimpleCars>
            <Item><Name>A</Name><SpritesPath>Cars\A</SpritesPath><MaxHealth>7</MaxHealth><Weapon>Minigun</Weapon></Item>
            <Item><Name>B</Name><BaseCar>A</BaseCar><Weapon>Hammer</Weapon></Item>
            <Item><Name>C</Name><BaseCar>B</BaseCar></Item>
            </SimpleCars></Asset></XnaContent>"#;
        let cars = CarCatalog::parse(xml).unwrap();
        let (b, c) = (cars.get_simple("B").unwrap(), cars.get_simple("C").unwrap());
        assert_eq!((b.max_health, b.weapon.as_deref()), (7, Some("Hammer")));
        // C names a base and its own (empty) weapon: it has none, as `MergeSimpleCar`.
        assert_eq!((c.max_health, c.weapon.as_deref(), c.name.as_str()), (7, None, "C"));
        assert_eq!(c.sprites_path.as_deref(), Some("Cars\\A"));
    }

    #[test]
    fn the_maps_weapon_overrides_the_cars_but_none_leaves_it() {
        assert_eq!(weapon_type(None, Some("Minigun")), Some("Minigun"));
        assert_eq!(weapon_type(Some("Railgun"), Some("Minigun")), Some("Railgun"));
        assert_eq!(weapon_type(Some("Railgun"), None), Some("Railgun"));
        // The factory makes nothing for `None`, and the car's own is made after.
        assert_eq!(weapon_type(Some("None"), Some("Minigun")), Some("Minigun"));
        assert_eq!(weapon_type(None, None), None);
    }

    #[test]
    fn a_maps_simple_cars_are_read_with_their_actions_and_weapons() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Maps/Singleplayer/Targets.xml");
        let map = MapData::parse(&fs::read_to_string(path).unwrap()).unwrap();
        let first = map.simple_cars.iter().find(|c| c.id == 3001).expect("target 3001");
        assert_eq!((first.car_name.as_str(), first.control.as_str()), ("SimpleTarget8HP", "NoneEnemy"));
        assert_eq!(first.weapon_override(), None, "`Default` keeps the car's weapon");
        assert_eq!(first.action_on_destroy.as_deref(), Some("Seq_DestroyTarget1"));
        let turret = map.simple_cars.iter().find(|c| c.id == 3003).expect("turret 3003");
        assert_eq!(turret.weapon_override(), Some("PlasmaGunSlow"));
        assert!(!first.is_prototype);
    }
}

mod health_bar {
    use bevy::math::Vec2;
    use tilerider::car::health_bar::{Look, SHOWN_FOR};

    #[test]
    fn the_bar_sits_above_the_car_and_shrinks_with_its_health() {
        let full = Look::of(Vec2::new(10.0, -4.0), 1.0, 1.0, None, SHOWN_FOR, Vec2::Y);
        assert_eq!(full.centre, Vec2::new(10.0, -4.0 + 1.1));
        assert!((full.back.x - 3.0).abs() < 1e-6 && (full.back.y - 0.15).abs() < 1e-6);
        assert!((full.fill.x - 2.875).abs() < 1e-6 && (full.fill.y - 0.1).abs() < 1e-6);
        let half = Look::of(Vec2::ZERO, 2.0, 0.5, None, SHOWN_FOR, Vec2::Y);
        // Turned to portrait, it sits at the screen's top: the world's x.
        let turned = Look::of(Vec2::ZERO, 1.0, 1.0, None, SHOWN_FOR, Vec2::X);
        assert!((turned.centre - Vec2::new(1.1, 0.0)).length() < 1e-6, "{:?}", turned.centre);
        assert!((half.fill.x - 2.875).abs() < 1e-6, "{:?}", half.fill);
        assert!((half.back.x - 6.0).abs() < 1e-6);
    }

    #[test]
    fn it_runs_from_red_when_whole_to_yellow_and_fades_over_two_seconds() {
        assert_eq!(Look::of(Vec2::ZERO, 1.0, 1.0, None, 2.0, Vec2::Y).fill_colour, [1.0, 0.0, 0.0]);
        assert_eq!(Look::of(Vec2::ZERO, 1.0, 0.0, None, 2.0, Vec2::Y).fill_colour, [1.0, 1.0, 0.0]);
        assert_eq!(Look::of(Vec2::ZERO, 1.0, 1.0, None, 2.0, Vec2::Y).alpha, 1.0);
        assert_eq!(Look::of(Vec2::ZERO, 1.0, 1.0, None, 1.0, Vec2::Y).alpha, 0.5);
        assert_eq!(Look::of(Vec2::ZERO, 1.0, 1.0, None, 0.0, Vec2::Y).alpha, 0.0);
    }

    #[test]
    fn a_shield_shows_instead_of_the_health_in_white() {
        let look = Look::of(Vec2::ZERO, 1.0, 1.0, Some(0.25), 2.0, Vec2::Y);
        assert_eq!(look.fill_colour, [1.0, 1.0, 1.0]);
        assert!((look.fill.x - 2.875 * 0.25).abs() < 1e-6);
    }
}
