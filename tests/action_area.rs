//! `TriggerActionByEntitiesInArea`: which entities an area picks, and what
//! its actions run with.

use avian2d::prelude::Position;
use bevy::prelude::*;
use tilerider::actions::area::in_area;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, Inactive, Run, run};
use tilerider::content::map::MapData;
use tilerider::content::xna::XRect;
use tilerider::game::{LoadedMap, MapId};
use tilerider::map::grid::MapGrid;
use tilerider::units::CellSize;

tilerider::actions! {
    Note("Note") {
        text "Text": String = String::new();
    } => templates {
        "Note(text)" => |a| Some(Note { text: a.text(0)?.to_string() }),
    }
}

#[derive(Resource, Default)]
struct Notes(Vec<(String, Option<i64>)>);

fn note(In(run): In<Run<Note>>, mut notes: ResMut<Notes>) -> bool {
    notes.0.push((run.data.text, run.actor));
    true
}

/// Cells are 2 world units wide (a 64 px map scale).
const CELL: f32 = 2.0;

/// The world point at the middle of map cell `(x, y)` plus an offset in cells.
fn at(x: f32, y: f32) -> Vec2 {
    CellSize(CELL).to_world(x, y)
}

fn app(actions: &str) -> App {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>64</Scale>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>
.
</CharMap><Actions>{actions}</Actions></Asset></XnaContent>"#
    );
    let data = MapData::parse(&xml).unwrap();
    let grid = MapGrid::build(&data, None);
    let light = tilerider::light::bake::LightMap::bake(&data, &grid, &tilerider::light::raster::Canvas::from_rgba8(1, 1, &[255; 4]), &[]);
    let actions = data.actions.clone();
    let map = LoadedMap { data, grid, light, cell: CellSize(CELL), cars: default(), list: default(), sprites: default() };
    let mut app = App::new();
    app.add_plugins(ActionsPlugin)
        .init_resource::<Notes>()
        .add_map_action::<Note, _>(note)
        .insert_resource(CurrentActions(actions))
        .insert_resource(map);
    app
}

const AREA: &str = r#"<Item Type="GameLibrary.TriggerActionByEntitiesInArea"><Name>Area</Name>
    <Area>1 1 2 2</Area><Actions><Item>Note(a)</Item><Item>Note(b)</Item></Actions></Item>"#;

fn notes(app: &mut App) -> Vec<(String, Option<i64>)> {
    std::mem::take(&mut app.world_mut().resource_mut::<Notes>().0)
}

#[test]
fn the_cell_of_an_entitys_middle_decides_and_the_right_and_bottom_edges_are_out() {
    let area = XRect { x: 1, y: 1, w: 2, h: 2 };
    let cell = CellSize(CELL);
    assert!(in_area(area, cell, at(1.0, 1.0)), "the left and top edges are in");
    assert!(in_area(area, cell, at(2.99, 2.99)));
    assert!(!in_area(area, cell, at(3.0, 2.0)), "the right edge is out");
    assert!(!in_area(area, cell, at(2.0, 3.0)), "the bottom edge is out");
    assert!(!in_area(area, cell, at(0.99, 1.5)));
    // Map y points down: the same cell is below zero in the world.
    assert!(at(1.5, 1.5).y < 0.0);
}

#[test]
fn a_point_just_left_of_the_map_counts_as_cell_zero_as_in_c_sharp() {
    // `(int)(-0.5)` is 0, not -1.
    let area = XRect { x: 0, y: 0, w: 1, h: 1 };
    assert!(in_area(area, CellSize(CELL), at(-0.5, 0.5)));
    assert!(!in_area(area, CellSize(CELL), at(-1.5, 0.5)));
}

#[test]
fn each_entity_with_an_id_in_the_area_runs_every_action_with_itself_as_the_actor() {
    let mut app = app(AREA);
    let w = app.world_mut();
    w.spawn((MapId(12), Position(at(2.5, 1.5))));
    w.spawn((MapId(10), Position(at(1.5, 1.5))));
    w.spawn((MapId(11), Position(at(5.5, 5.5))));
    w.spawn(Position(at(1.5, 1.5)));
    w.spawn((MapId(0), Position(at(1.5, 1.5))));
    assert!(run(app.world_mut(), "Area", Some(99), 0));
    let some = |t: &str, id| (t.to_string(), Some(id));
    assert_eq!(
        notes(&mut app),
        [some("a", 10), some("b", 10), some("a", 12), some("b", 12)],
        "by id, each one's actions in a row; none for 11 (outside), the unnamed and id 0"
    );
}

#[test]
fn inactive_entities_count_and_a_body_is_where_its_position_is_not_where_it_is_drawn() {
    let mut app = app(AREA);
    let w = app.world_mut();
    // Hidden by `DeactivateEntity`, still in the world.
    w.spawn((MapId(1), Inactive, Position(at(1.5, 1.5))));
    // Eased in `Update` somewhere else than the physics has it.
    w.spawn((MapId(2), Position(at(1.5, 2.5)), Transform::from_translation(at(8.5, 8.5).extend(0.0))));
    // No body: the transform is all it has.
    w.spawn((MapId(3), Transform::from_translation(at(2.5, 2.5).extend(0.0))));
    w.spawn((MapId(4), Transform::from_translation(at(8.5, 8.5).extend(0.0))));
    run(app.world_mut(), "Area", None, 0);
    let ids: Vec<i64> = notes(&mut app).into_iter().filter(|(t, _)| t == "a").filter_map(|(_, id)| id).collect();
    assert_eq!(ids, [1, 2, 3]);
}

#[test]
fn escapes_room_switch_deactivates_what_is_in_the_area_and_only_that() {
    // Escape's `TriggerActionByEntitiesInArea1`: `DeactivateEntity(0)`, the
    // 0 being whichever entity the area is running for.
    let mut app = app(
        r#"<Item Type="GameLibrary.TriggerActionByEntitiesInArea"><Name>Room</Name>
        <Area>1 1 2 2</Area><Actions><Item>DeactivateEntity(0)</Item></Actions></Item>"#,
    );
    let w = app.world_mut();
    let inside = [w.spawn((MapId(1), Position(at(1.5, 1.5)))).id(), w.spawn((MapId(2), Position(at(2.5, 2.5)))).id()];
    let outside = w.spawn((MapId(3), Position(at(4.5, 1.5)))).id();
    run(app.world_mut(), "Room", None, 0);
    for e in inside {
        assert!(app.world().entity(e).contains::<Inactive>());
    }
    assert!(!app.world().entity(outside).contains::<Inactive>());
}

#[test]
fn it_succeeds_even_with_nobody_there_so_next_action_follows() {
    let mut app = app(&format!(
        r#"{AREA}<Item Type="GameLibrary.TriggerActionByEntitiesInArea"><Name>Empty</Name>
        <NextAction>After</NextAction><Area>50 50 1 1</Area><Actions><Item>Note(x)</Item></Actions></Item>
        <Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#
    ));
    assert!(run(app.world_mut(), "Empty", None, 0));
    assert_eq!(notes(&mut app), [("after".to_string(), None)]);
}

#[test]
fn escapes_areas_read_as_the_maps_write_them() {
    use tilerider::content::actions::def::ActionDef;
    use tilerider::content::actions::defs::TriggerActionByEntitiesInArea;
    use tilerider::content::node::Node;
    let node = Node::parse(
        r#"<Item Type="GameLibrary.TriggerActionByEntitiesInArea"><Name>TriggerActionByEntitiesInArea1</Name>
        <RunOnMapStart>false</RunOnMapStart><Area>1 17 9 7</Area><Actions><Item>DeactivateEntity(0)</Item></Actions></Item>"#,
    )
    .unwrap();
    let a = TriggerActionByEntitiesInArea::read(&node).unwrap();
    assert_eq!((a.area, a.actions), (XRect { x: 1, y: 17, w: 9, h: 7 }, vec!["DeactivateEntity(0)".to_string()]));
}
