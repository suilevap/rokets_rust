//! `MoveEntity`: an entity put at a map point, turned, at rest.

use avian2d::prelude::*;
use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::ai::AiBlackboard;
use tilerider::content::map::MapData;
use tilerider::game::{LoadedMap, MapId};
use tilerider::map::grid::MapGrid;
use tilerider::units::CellSize;

/// Cells are 2 world units wide (a 64 px map scale).
const CELL: f32 = 2.0;

/// Garage's `MoveEntity3007` (the actor, 180 degrees) and a named one.
const ACTIONS: &str = r#"
<Item Type="GameLibrary.MoveEntity"><Name>ToPost</Name><RunOnMapStart>false</RunOnMapStart>
  <Position>44.5 2.5</Position><DirectionDeg>180</DirectionDeg><ItemId>0</ItemId></Item>
<Item Type="GameLibrary.MoveEntity"><Name>Named</Name><RunOnMapStart>false</RunOnMapStart>
  <Position>1 2</Position><DirectionDeg>90</DirectionDeg><ItemId>7</ItemId></Item>"#;

fn app() -> App {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>
.
</CharMap><Actions>{ACTIONS}</Actions></Asset></XnaContent>"#
    );
    let data = MapData::parse(&xml).unwrap();
    let grid = MapGrid::build(&data, None);
    let light = tilerider::light::bake::LightMap::bake(&data, &grid, &tilerider::light::raster::Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let actions = data.actions.clone();
    let map = LoadedMap { data, grid, light, cell: CellSize(CELL), cars: default(), list: default(), sprites: default() };
    let mut app = App::new();
    app.add_plugins(ActionsPlugin).insert_resource(CurrentActions(actions)).insert_resource(map);
    app
}

/// A moving, spinning body somewhere else.
fn body(app: &mut App, id: i64) -> Entity {
    app.world_mut()
        .spawn((
            MapId(id),
            Position(Vec2::new(9.0, 9.0)),
            Rotation::radians(0.3),
            LinearVelocity(Vec2::new(3.0, -4.0)),
            AngularVelocity(2.0),
        ))
        .id()
}

#[test]
fn it_puts_the_actor_at_the_map_point_turned_and_at_rest() {
    let mut app = app();
    let turret = body(&mut app, 3007);
    let other = body(&mut app, 3009);
    assert!(run(app.world_mut(), "ToPost", Some(3007), 0));
    let w = app.world();
    // Map y points down: row 2.5 is below zero in the world.
    assert_eq!(w.get::<Position>(turret).unwrap().0, Vec2::new(44.5 * CELL, -2.5 * CELL));
    // 180 degrees: facing left, whichever way the sign goes.
    let angle = w.get::<Rotation>(turret).unwrap().as_radians();
    assert!((angle.abs() - std::f32::consts::PI).abs() < 1e-5, "{angle}");
    assert_eq!(w.get::<LinearVelocity>(turret).unwrap().0, Vec2::ZERO);
    assert_eq!(w.get::<AngularVelocity>(turret).unwrap().0, 0.0);
    // Nobody else moves.
    assert_eq!(w.get::<Position>(other).unwrap().0, Vec2::new(9.0, 9.0));
    assert_eq!(w.get::<LinearVelocity>(other).unwrap().0, Vec2::new(3.0, -4.0));
}

#[test]
fn a_direction_turns_counter_clockwise_on_screen_as_everywhere_in_the_map() {
    let mut app = app();
    let e = body(&mut app, 7);
    assert!(run(app.world_mut(), "Named", Some(1), 0));
    let w = app.world();
    assert_eq!(w.get::<Position>(e).unwrap().0, Vec2::new(1.0 * CELL, -2.0 * CELL));
    // 90 degrees is up on screen: +y in the world.
    let heading = w.get::<Rotation>(e).unwrap() * Vec2::X;
    assert!(heading.distance(Vec2::Y) < 1e-5, "{heading}");
}

#[test]
fn an_item_id_names_the_entity_whoever_set_it_off() {
    let mut app = app();
    let named = body(&mut app, 7);
    let actor = body(&mut app, 1);
    run(app.world_mut(), "Named", Some(1), 0);
    assert_eq!(app.world().get::<Position>(named).unwrap().0, Vec2::new(2.0, -4.0));
    assert_eq!(app.world().get::<Position>(actor).unwrap().0, Vec2::new(9.0, 9.0));
}

#[test]
fn a_behaviour_tree_car_forgets_its_target() {
    let mut app = app();
    let car = body(&mut app, 3007);
    let foe = body(&mut app, 4001);
    let mut brain = AiBlackboard::new(1);
    brain.set_target(foe);
    app.world_mut().entity_mut(car).insert(brain);
    run(app.world_mut(), "ToPost", Some(3007), 0);
    assert_eq!(app.world().get::<AiBlackboard>(car).unwrap().target(), None);
}

#[test]
fn it_succeeds_without_the_entity_or_a_body_so_next_action_still_follows() {
    let mut app = app();
    // C#'s void handlers are always true.
    assert!(run(app.world_mut(), "Named", Some(1), 0), "no entity 7");
    app.world_mut().spawn(MapId(7));
    assert!(run(app.world_mut(), "Named", Some(1), 0), "no body");
}

#[test]
fn a_body_with_no_speeds_is_still_moved() {
    let mut app = app();
    let e = app.world_mut().spawn((MapId(7), Position(Vec2::ZERO), Rotation::IDENTITY)).id();
    run(app.world_mut(), "Named", None, 0);
    assert_eq!(app.world().get::<Position>(e).unwrap().0, Vec2::new(2.0, -4.0));
}
