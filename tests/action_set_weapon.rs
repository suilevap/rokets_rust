//! `SetWeapon`: hands a car another gun or gadget while the map runs.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::actions::{ActionRegistry, ActionsPlugin, CurrentActions, run};
use tilerider::car::control::LocalDevice;
use tilerider::car::drive::{DriveParams, DriveState};
use tilerider::car::{Car, Mounts};
use tilerider::content::map::MapData;
use tilerider::controls::LocalPlayer;
use tilerider::explosion::Health;
use tilerider::game::MapId;
use tilerider::gadget::Magnet;
use tilerider::gadget::accelerator::Accelerator;
use tilerider::sound::{CarSounds, PlaySound};
use tilerider::weapon::model::{Kind, for_type};
use tilerider::weapon::swap::{SwapPlugin, WeaponCollected};
use tilerider::weapon::{PlayerEnergy, Weapon};

const MOUNTS: Mounts = Mounts { weapon: Vec2::new(0.25, 0.0), gadget: Vec2::new(-0.5, 0.0), size: Vec2::new(3.0, 1.5) };

/// An app with the action, and a map whose `Actions` are `items`.
fn app(items: &str) -> App {
    let xml = format!(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{items}</Actions></Asset></XnaContent>"#);
    let map = MapData::parse(&xml).unwrap();
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, SwapPlugin)).add_message::<PlaySound>().insert_resource(CurrentActions(map.actions));
    app
}

/// A living car of the map, with a place to carry things.
fn car(app: &mut App, id: i64) -> Entity {
    app.world_mut().spawn((MapId(id), Health::full(10), MOUNTS, Position(Vec2::new(5.0, 6.0)), Rotation::radians(1.0))).id()
}

/// A car the player's devices drive.
fn players_car(app: &mut App, id: i64) -> Entity {
    let e = car(app, id);
    app.world_mut().entity_mut(e).insert((LocalDevice, LocalPlayer));
    e
}

fn gun(app: &App, e: Entity) -> Option<&Weapon> {
    app.world().entity(e).get::<Weapon>()
}

fn sounds(app: &mut App) -> Vec<&'static str> {
    app.world_mut().resource_mut::<Messages<PlaySound>>().drain().map(|c| c.name).collect()
}

fn rings(app: &mut App) -> Vec<WeaponCollected> {
    app.world_mut().resource_mut::<Messages<WeaponCollected>>().drain().collect()
}

fn set(weapon: &str, id: i64, extra: &str) -> String {
    format!(r#"<Item Type="GameLibrary.SetWeapon"><Name>Set</Name><ItemId>{id}</ItemId><NewWeapon>{weapon}</NewWeapon>{extra}</Item>"#)
}

#[test]
fn a_new_gun_is_made_for_the_car_with_the_preset_of_whoever_drives_it() {
    let mut app = app(&set("Minigun", 0, ""));
    let enemy = car(&mut app, 3001);
    let player = players_car(&mut app, 4001);
    assert!(run(app.world_mut(), "Set", Some(3001), 0), "the actor when the item is 0");
    assert!(run(app.world_mut(), "Set", Some(4001), 0));
    let (kind, others) = for_type("Minigun", false).unwrap();
    let (_, players) = for_type("Minigun", true).unwrap();
    assert_ne!(others, players, "the two presets differ, or this test says nothing");
    let (g, p) = (gun(&app, enemy).unwrap(), gun(&app, player).unwrap());
    assert_eq!((g.kind, g.options), (kind, others));
    assert_eq!((p.kind, p.options), (kind, players), "held for the devices: the player's preset");
    assert_eq!(g.mount, MOUNTS.weapon, "at the car's WeaponPositon");
    assert_ne!(g.at, Vec2::ZERO, "placed on the car at once");
}

#[test]
fn it_replaces_the_old_gun_and_all_its_state() {
    let mut app = app(&set("Hammer", 3001, ""));
    let e = car(&mut app, 3001);
    let (kind, options) = for_type("Tesla", false).unwrap();
    let mut old = Weapon::new(kind, options, MOUNTS.weapon, MOUNTS.size, false, &mut SmallRng::seed_from_u64(1));
    old.tesla.armed = true;
    old.working = true;
    app.world_mut().entity_mut(e).insert(old);
    assert!(run(app.world_mut(), "Set", None, 0));
    let g = gun(&app, e).unwrap();
    assert_eq!(g.kind, Kind::Hammer);
    assert!(!g.tesla.armed && !g.working, "nothing of the Tesla is left");
    assert_eq!(g.mount, g.hammer.mount, "the saw sits by the car's nose, not at WeaponPositon");
    assert!(!g.kind.is_complex() && g.kind.auto_fire());
}

#[test]
fn the_new_guns_range_and_shot_are_what_the_ai_reads() {
    // The attacking AI reads `Weapon` every step: its range through
    // `fire_range`, how the shot flies through `Kind::is_immediate`.
    let mut app = app(&format!("{}{}", set("Minigun", 3001, ""), set("PlasmaGunSlow", 3001, "").replace(">Set<", ">Slow<")));
    let e = car(&mut app, 3001);
    run(app.world_mut(), "Set", None, 0);
    let fast = gun(&app, e).map(|g| (g.kind.is_immediate(), g.fire_range().range)).unwrap();
    run(app.world_mut(), "Slow", None, 0);
    let slow = gun(&app, e).map(|g| (g.kind.is_immediate(), g.fire_range().range)).unwrap();
    assert!(fast.0 && !slow.0, "a slow weapon picks the slow weapon's attack style");
    assert_eq!(slow.1, for_type("PlasmaGunSlow", false).unwrap().1.range);
}

#[test]
fn default_keeps_the_gun_and_none_takes_it_away() {
    let mut app = app(&format!(
        "{}{}{}",
        set("Default", 3001, ""),
        set("None", 3001, "").replace(">Set<", ">Take<"),
        set("Shotgun", 3001, "").replace(">Set<", ">Give<")
    ));
    let e = car(&mut app, 3001);
    assert!(run(app.world_mut(), "Give", None, 0));
    let before = gun(&app, e).unwrap().options;
    assert!(run(app.world_mut(), "Set", None, 0));
    assert_eq!(gun(&app, e).unwrap().options, before, "Default: do not change");
    assert!(run(app.world_mut(), "Take", None, 0));
    assert!(gun(&app, e).is_none(), "None: no gun");
}

#[test]
fn the_template_gives_the_actor_a_gun() {
    let mut app = app("");
    let e = car(&mut app, 4003);
    assert!(run(app.world_mut(), "SetWeapon(Railgun)", Some(4003), 0));
    assert_eq!(gun(&app, e).unwrap().kind, Kind::Railgun);
    assert!(!run(app.world_mut(), "SetWeapon(Nonsense)", Some(4003), 0), "not a weapon type");
}

#[test]
fn only_a_living_car_is_handed_anything() {
    let mut app = app(&format!("{}{}", set("Railgun", 3001, ""), set("Railgun", 3002, "").replace(">Set<", ">Box<")));
    assert!(!run(app.world_mut(), "Set", None, 0), "no such car");
    let wreck = app.world_mut().spawn((MapId(3001), MOUNTS)).id();
    assert!(run(app.world_mut(), "Set", None, 0));
    assert!(gun(&app, wreck).is_none(), "a wreck has no health, and takes no gun");
    let not_a_car = app.world_mut().spawn((MapId(3002), Health::full(1))).id();
    assert!(run(app.world_mut(), "Box", None, 0), "nothing to do is not a failure");
    assert!(gun(&app, not_a_car).is_none(), "nothing that carries a gun");
}

#[test]
fn the_players_car_gets_an_energy_store_and_a_simple_car_its_sounds() {
    let mut app = app(&set("Minigun", 0, ""));
    let player = players_car(&mut app, 4001);
    let simple = car(&mut app, 3001);
    run(app.world_mut(), "Set", Some(4001), 0);
    run(app.world_mut(), "Set", Some(3001), 0);
    assert!(app.world().entity(player).contains::<PlayerEnergy>(), "the player's weapon spends it");
    assert!(!app.world().entity(simple).contains::<PlayerEnergy>());
    assert!(app.world().entity(simple).contains::<CarSounds>(), "the Minigun's loop sounds on the car");
    // A store the car has is not refilled by a new gun.
    let mut energy = PlayerEnergy::default();
    energy.0.current = 0.25;
    app.world_mut().entity_mut(player).insert(energy);
    run(app.world_mut(), "Set", Some(4001), 0);
    assert_eq!(app.world().entity(player).get::<PlayerEnergy>().unwrap().0.current, 0.25);
}

#[test]
fn the_player_is_told_unless_it_is_silent_or_nothing_was_given() {
    let mut app = app(&format!(
        "{}{}{}{}",
        set("Railgun", 4001, ""),
        set("Railgun", 4001, "<ChangeSilently>true</ChangeSilently>").replace(">Set<", ">Quiet<"),
        set("None", 4001, "").replace(">Set<", ">Take<"),
        set("Railgun", 3001, "").replace(">Set<", ">Enemy<")
    ));
    players_car(&mut app, 4001);
    car(&mut app, 3001);
    run(app.world_mut(), "Set", None, 0);
    assert_eq!(sounds(&mut app), ["WeaponReload"]);
    assert_eq!(rings(&mut app), [WeaponCollected { gadget: false }]);
    run(app.world_mut(), "Quiet", None, 0);
    run(app.world_mut(), "Take", None, 0);
    run(app.world_mut(), "Enemy", None, 0);
    assert!(sounds(&mut app).is_empty(), "silent, none, and a car the devices do not drive");
    assert!(rings(&mut app).is_empty());
}

fn driving() -> Car {
    Car {
        params: DriveParams { acceleration_force: 1.0, max_speed: 1.0, rotation_acceleration: 1.0, mass: 1.0, radius: 1.0, map_friction: 1.0 },
        drive: DriveState::default(),
    }
}

#[test]
fn the_magnet_and_the_accelerator_are_given_to_a_car_and_taken_away() {
    let gadget = |name: &str, g: &str, extra: &str| {
        format!(
            r#"<Item Type="GameLibrary.SetWeapon"><Name>{name}</Name><ItemId>4001</ItemId><NewWeapon>Default</NewWeapon><NewGadget>{g}</NewGadget>{extra}</Item>"#
        )
    };
    let mut app = app(&format!(
        "{}{}{}{}",
        gadget("Link", "Link", ""),
        gadget("Quiet", "Link", "<ChangeSilently>true</ChangeSilently>"),
        gadget("Boost", "Accelerator", ""),
        gadget("Drop", "None", "")
    ));
    let player = players_car(&mut app, 4001);
    app.world_mut().entity_mut(player).insert(driving());
    assert!(run(app.world_mut(), "Link", None, 0));
    assert!(app.world().entity(player).contains::<Magnet>());
    assert!(gun(&app, player).is_none(), "NewWeapon Default: no gun made");
    assert_eq!(sounds(&mut app), ["ChainBrake"]);
    assert_eq!(rings(&mut app), [WeaponCollected { gadget: true }]);
    run(app.world_mut(), "Quiet", None, 0);
    assert!(sounds(&mut app).is_empty());
    // One gadget a car: the accelerator takes the magnet's place, quietly,
    // and the player's car gets the energy it spends.
    assert!(run(app.world_mut(), "Boost", None, 0));
    assert!(app.world().entity(player).contains::<Accelerator>());
    assert!(!app.world().entity(player).contains::<Magnet>());
    assert!(app.world().entity(player).contains::<PlayerEnergy>());
    assert!(sounds(&mut app).is_empty() && rings(&mut app).is_empty(), "only the magnet was announced");
    run(app.world_mut(), "Link", None, 0);
    assert!(app.world().entity(player).contains::<Magnet>());
    assert!(!app.world().entity(player).contains::<Accelerator>());
    run(app.world_mut(), "Boost", None, 0);
    run(app.world_mut(), "Drop", None, 0);
    assert!(!app.world().entity(player).contains::<Magnet>() && !app.world().entity(player).contains::<Accelerator>());
}

#[test]
fn a_simple_car_takes_no_gadget() {
    let mut app = app(
        r#"<Item Type="GameLibrary.SetWeapon"><Name>Link</Name><ItemId>3001</ItemId><NewGadget>Link</NewGadget></Item>"#,
    );
    let simple = car(&mut app, 3001);
    app.world_mut().entity_mut(simple).insert(LocalDevice);
    assert!(run(app.world_mut(), "Link", None, 0));
    assert!(!app.world().entity(simple).contains::<Magnet>(), "`ent is Car`");
    assert!(sounds(&mut app).is_empty() && rings(&mut app).is_empty());
}

#[test]
fn the_maps_load_the_textures_of_the_guns_they_hand_out() {
    let app = app(&format!(
        "{}{}",
        set("Tesla", 4001, ""),
        set("Railgun", 4001, "<ChangeSilently>true</ChangeSilently>").replace(">Set<", ">Quiet<")
    ));
    let registry = app.world().resource::<ActionRegistry>();
    let actions = app.world().resource::<CurrentActions>().0.0.clone();
    let tesla = registry.sprites_of(&actions[0]);
    assert!(tesla.iter().any(|s| s == "UI/BigCircleGun"), "the ring of the effect");
    assert!(tesla.iter().any(|s| s.contains("Tesla")), "the Tesla's discs");
    let quiet = registry.sprites_of(&actions[1]);
    assert!(!quiet.iter().any(|s| s.starts_with("UI")), "no ring for a silent one");
    assert!(quiet.iter().any(|s| s == "Weapons/Railgun"));
    // The template, as an `ActionOnCreate` or a sequence's step.
    assert!(registry.sprites_of_text("SetWeapon(Hammer)").iter().any(|s| s == "Weapons/Saw"));
    assert!(registry.sprites_of_text("NextMap()").is_empty());
}

#[test]
fn the_maps_load_the_textures_of_the_accelerator_they_hand_out() {
    let app = app(
        r#"<Item Type="GameLibrary.SetWeapon"><Name>Boost</Name><ItemId>4001</ItemId><NewGadget>Accelerator</NewGadget></Item>"#,
    );
    let registry = app.world().resource::<ActionRegistry>();
    let actions = app.world().resource::<CurrentActions>().0.0.clone();
    let sprites = registry.sprites_of(&actions[0]);
    assert!(sprites.iter().any(|s| s == "Particles/flash") && sprites.iter().any(|s| s == "UI/GadgetUsage"), "{sprites:?}");
}
