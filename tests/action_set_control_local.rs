//! `SetControl(id, Local)` on a car other than the player's hands it the
//! player's devices (Revenge, when its Scout dies): the intent, the camera,
//! the energy store and the death rule go with it.

use std::time::Duration;

use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tilerider::actions::{ActionsPlugin, OnDestroy, CurrentActions, run};
use tilerider::car::control::{LocalDevice, Team};
use tilerider::car::Mounts;
use tilerider::car::death::DeathPlugin;
use tilerider::content::map::MapData;
use tilerider::controls::{LocalPlayer, NoInput, PlayerIntent};
use tilerider::explosion::{Destroyed, Health};
use tilerider::game::{ChangeMap, GameState, LoadedMap, MapId};
use tilerider::game_mode::{Abandoned, GameModePlugin, Respawn, Wreck, Wreckable};
use tilerider::map::grid::MapGrid;
use tilerider::render::camera::CameraTarget;
use tilerider::render::layers::Layer;
use tilerider::render::particles::Particles;
use tilerider::render::sprite::UnitQuad;
use tilerider::units::CellSize;
use tilerider::weapon::model::for_type;
use tilerider::weapon::{PlayerEnergy, Weapon};

const MOUNTS: Mounts = Mounts { weapon: Vec2::new(0.25, 0.0), gadget: Vec2::new(-0.5, 0.0), size: Vec2::new(3.0, 1.5) };

const ACTIONS: &str = r#"<Item Type="GameLibrary.SetControl"><Name>Hand</Name><RunOnMapStart>false</RunOnMapStart><ItemId>4001</ItemId><Control>Local</Control></Item>
<Item Type="GameLibrary.SetControl"><Name>Back</Name><RunOnMapStart>false</RunOnMapStart><ItemId>4003</ItemId><Control>Local</Control></Item>"#;

fn app() -> App {
    let xml = format!(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>
.
</CharMap><Actions>{ACTIONS}</Actions></Asset></XnaContent>"#);
    let data = MapData::parse(&xml).unwrap();
    let grid = MapGrid::build(&data, None);
    let light = tilerider::light::bake::LightMap::bake(&data, &grid, &tilerider::light::raster::Mask::from_alpha(1, 1, &[255; 4]), &[]);
    let actions = data.actions.clone();
    let map = LoadedMap { data, grid, light, cell: CellSize(2.0), cars: default(), list: default(), sprites: default() };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, ActionsPlugin, GameModePlugin, DeathPlugin))
        .init_state::<GameState>()
        .add_message::<Destroyed>()
        .add_message::<ChangeMap>()
        .add_message::<tilerider::explosion::Explosion>()
        .add_message::<tilerider::explosion::Hit>()
        .add_message::<tilerider::actions::MapEffectAmount>()
        .add_message::<avian2d::prelude::CollisionStart>()
        .add_message::<tilerider::sound::PlaySound>()
        .init_resource::<Particles>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(UnitQuad(default()))
        .insert_resource(CurrentActions(actions))
        .insert_resource(map);
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
    app.world_mut().run_schedule(StateTransition);
    app
}

/// Any car of the map: what the hand-over and a death look at.
fn car(app: &mut App, id: i64, team: u8) -> Entity {
    app.world_mut()
        .spawn((
            MapId(id),
            Team(team),
            Health::full(10),
            MOUNTS,
            avian2d::prelude::RigidBody::Dynamic,
            PlayerIntent::default(),
            Transform::default(),
            Wreckable { sprites: default(), scale: 1.0, layer: Layer::CarBody },
        ))
        .id()
}

/// The player's car (the Scout) and the car that will get the devices.
fn scout_and_buggy(app: &mut App) -> (Entity, Entity) {
    let scout = car(app, 4003, 0);
    app.world_mut().entity_mut(scout).insert((LocalPlayer, LocalDevice, CameraTarget { radius: 1.0, max_speed: 5.0 }, PlayerEnergy::default()));
    let buggy = car(app, 4001, 0);
    app.world_mut().entity_mut(buggy).insert((NoInput, LocalDevice, CameraTarget { radius: 1.0, max_speed: 5.0 }));
    (scout, buggy)
}

/// Fixed steps of `secs` seconds in all.
fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs * 10.0) as usize {
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(0.1));
        app.world_mut().run_schedule(FixedUpdate);
    }
}

fn has<C: Component>(app: &App, e: Entity) -> bool {
    app.world().entity(e).contains::<C>()
}

fn changes(app: &mut App) -> usize {
    app.world_mut().resource_mut::<Messages<ChangeMap>>().drain().count()
}

#[test]
fn the_devices_go_to_the_new_car_and_not_to_the_old() {
    let mut app = app();
    let (scout, buggy) = scout_and_buggy(&mut app);
    assert!(run(app.world_mut(), "Hand", None, 0));
    assert!(has::<LocalPlayer>(&app, buggy) && has::<LocalDevice>(&app, buggy), "the intent writers' query now finds the new car");
    assert!(!has::<NoInput>(&app, buggy));
    assert!(!has::<LocalPlayer>(&app, scout) && !has::<LocalDevice>(&app, scout), "and no longer the old one");
    assert!(has::<NoInput>(&app, scout), "which stands");
    let local: Vec<_> = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().iter(app.world()).collect();
    assert_eq!(local, [buggy]);
}

#[test]
fn the_camera_and_the_energy_store_follow() {
    let mut app = app();
    let (scout, buggy) = scout_and_buggy(&mut app);
    // The new car has a gun and no store, no camera target.
    app.world_mut().entity_mut(buggy).remove::<CameraTarget>();
    let (kind, options) = for_type("Minigun", false).unwrap();
    let gun = Weapon::new(kind, options, Vec2::ZERO, MOUNTS.size, false, &mut SmallRng::seed_from_u64(1));
    assert!(!gun.aim.local);
    app.world_mut().entity_mut(buggy).insert(gun);
    assert!(run(app.world_mut(), "Hand", None, 0));
    assert!(has::<CameraTarget>(&app, buggy), "the camera frames the new car");
    assert!(!has::<CameraTarget>(&app, scout), "and lets the old one go");
    assert!(has::<PlayerEnergy>(&app, buggy) && !has::<PlayerEnergy>(&app, scout));
    assert!(app.world().entity(buggy).get::<Weapon>().unwrap().aim.local, "its gun aims as the player's");
}

#[test]
fn giving_the_player_s_own_car_the_control_changes_nothing_else() {
    let mut app = app();
    let (scout, buggy) = scout_and_buggy(&mut app);
    assert!(run(app.world_mut(), "Back", None, 0));
    assert!(has::<LocalPlayer>(&app, scout) && !has::<NoInput>(&app, scout));
    assert!(!has::<LocalPlayer>(&app, buggy), "the NoneLocal car is not the player's");
}

#[test]
fn the_new_cars_death_is_the_players_and_the_old_wreck_neither_restarts_nor_returns() {
    let mut app = app();
    let (scout, buggy) = scout_and_buggy(&mut app);
    // The Scout is destroyed first: the player's death (a wreck).
    app.world_mut().write_message(Destroyed(scout));
    step(&mut app, 0.1);
    assert!(has::<Wreck>(&app, scout));
    // A second later the map hands the devices over.
    step(&mut app, 1.0);
    assert!(run(app.world_mut(), "Hand", None, 0));
    assert!(has::<Abandoned>(&app, scout));
    step(&mut app, 3.0);
    assert_eq!(changes(&mut app), 0, "the level did not restart for the Scout");
    assert!(has::<Wreck>(&app, scout) && !has::<LocalPlayer>(&app, scout));

    // The new car dies the player's way: a wreck, then (no respawn point) the level starts again.
    app.world_mut().write_message(Destroyed(buggy));
    step(&mut app, 0.1);
    assert!(has::<Wreck>(&app, buggy), "wrecked as the player's, not a plain AI corpse");
    assert!(!has::<LocalPlayer>(&app, buggy));
    step(&mut app, 2.5);
    assert_eq!(changes(&mut app), 1);
}

#[test]
fn with_a_respawn_point_the_new_cars_death_costs_a_life() {
    let mut app = app();
    let (_, buggy) = scout_and_buggy(&mut app);
    app.world_mut().resource_mut::<Respawn>().point = Some((Vec2::ZERO, 0.0));
    assert!(run(app.world_mut(), "Hand", None, 0));
    app.world_mut().write_message(Destroyed(buggy));
    step(&mut app, 2.5);
    assert_eq!(app.world().resource::<Respawn>().lives, 2);
    assert_eq!(changes(&mut app), 0, "it comes back instead");
}

#[test]
fn a_player_whose_devices_were_taken_dies_as_an_ai_car_and_its_action_hands_the_next_one_over() {
    // Revenge: `RemovePlayerControl` (`NoneFriend`), then the Scout dies; its
    // `ActionOnDestroy` runs (here the hand-over itself) and the level goes on.
    let mut app = app();
    let (scout, buggy) = scout_and_buggy(&mut app);
    app.world_mut().entity_mut(scout).remove::<LocalDevice>().insert((NoInput, OnDestroy("Hand".into())));
    app.world_mut().write_message(Destroyed(scout));
    step(&mut app, 0.5);
    assert!(!has::<Wreck>(&app, scout), "not the player's death: no wreck to restart from");
    assert!(!has::<LocalPlayer>(&app, scout));
    // (This is a plain body: its wreck lies two seconds before the action runs.)
    step(&mut app, 2.0);
    assert!(has::<LocalPlayer>(&app, buggy), "its ActionOnDestroy gave the devices to the next car");
    step(&mut app, 2.0);
    assert_eq!(changes(&mut app), 0, "and the level did not restart");
}
