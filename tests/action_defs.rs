//! Action definitions: every action in the maps has one, and reads,
//! writes back and reads again the same.

use std::path::Path;

use bevy::prelude::*;
use tilerider::actions::{ActionRegistry, ActionsPlugin, run};
use tilerider::content::actions::def::Kind;
use tilerider::content::actions::source::{self, Source};
use tilerider::content::actions::defs;
use tilerider::content::map::MapData;
use tilerider::content::node::Node;

fn maps() -> impl Iterator<Item = (String, MapData)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Maps/Singleplayer");
    std::fs::read_dir(dir).unwrap().map(|e| {
        let path = e.unwrap().path();
        let map = MapData::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        (
            path.file_name().unwrap().to_string_lossy().into_owned(),
            map,
        )
    })
}

#[test]
fn every_action_in_the_maps_is_defined_and_round_trips() {
    let (mut checked, mut failed) = (0, Vec::new());
    for (file, map) in maps() {
        for a in &map.actions.0 {
            let Some(def) = defs::find(&a.kind) else {
                failed.push(format!("{file}: {}: no definition of {}", a.name(), a.kind));
                continue;
            };
            match (def.check)(&a.node) {
                Ok(()) => checked += 1,
                Err(e) => failed.push(format!("{file}: {}: {e}", a.name())),
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(checked > 1000, "{checked}");
   
}

#[test]
fn a_type_left_out_takes_its_defaults() {
    // `AddDoorComponent` runs on map start unless it says otherwise.
    let node =
        Node::parse(r#"<Item Type="GameLibrary.AddDoorComponent"><ItemId>3</ItemId></Item>"#)
            .unwrap();
    let a = tilerider::content::actions::RawAction::read(node).unwrap();
    assert!(a.base.run_on_map_start);
    assert_eq!(a.name(), "DoorComponent");
}

#[test]
fn fields_carry_what_an_editor_needs() {
    let door = defs::find("AddDoorComponent").unwrap();
    let names: Vec<_> = door.all_fields().map(|f| f.name).collect();
    assert_eq!(
        &names[..5],
        ["Name", "Groups", "RunOnMapStart", "NextAction", "ItemId"]
    );
    let on_open = door
        .fields
        .iter()
        .find(|f| f.name == "ActionOnOpen")
        .unwrap();
    assert_eq!(
        (on_open.kind, on_open.sources, on_open.optional),
        (Kind::Text, source::ACTION, false)
    );
    assert_eq!(on_open.description(), "Run when the door opens.");
    let scale = door
        .fields
        .iter()
        .find(|f| f.name == "OpenDistanceScale")
        .unwrap();
    assert_eq!((scale.default)().text, "1");
    let success = defs::find("ShowSuccess").unwrap().fields[0];
    assert_eq!(
        success.kind,
        Kind::Enum(&["Standard", "DoorOpen", "AntiSuccess"])
    );
}

#[test]
fn templates_are_picked_by_name_and_number_of_arguments() {
    let mut app = App::new();
    app.add_plugins((
        ActionsPlugin,
        tilerider::triggers::TriggersPlugin,
        tilerider::door::DoorPlugin,
    ));
    assert!(app.world().resource::<ActionRegistry>().has("MoveToNextMap"));
    // Templates are picked by name and number of arguments.
    assert!(run(app.world_mut(), "NextMap()", None, 0));
    assert!(run(app.world_mut(), "NextMap(3)", None, 0));
}

#[test]
fn an_editor_finds_what_a_field_may_take_without_knowing_the_game() {
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let xml = std::fs::read_to_string(content.join("Maps/Singleplayer/Garage.xml")).unwrap();
    let map = Node::parse(&xml).unwrap();
    let door = defs::find("AddDoorComponent").unwrap();
    let values = |name: &str| -> Vec<String> {
        let field = door.all_fields().find(|f| f.name == name).unwrap();
        field.sources.iter().flat_map(|s| s.values(&map, &content)).collect()
    };
    assert!(values("ItemId").contains(&"1029".to_string()));
    let actions = values("ActionOnOpen");
    assert!(actions.iter().any(|a| a == "DoorComponent1029"), "the map's actions");
    assert!(actions.iter().any(|a| a == "OpenDoor(itemId)"), "and the templates");
    assert!(source::SOUNDS.values(&map, &content).len() > 10);
    let custom = Source::Xml { file: None, path: "XnaContent/Asset/Players|Entities/Item/Id" };
    assert!(custom.values(&map, &content).len() > 1);
}

#[test]
fn records_inside_actions_round_trip() {
    // `ParticleStateData` in `SetParticleStyleActionData`; no map uses it.
    let node = Node::parse(
        r#"<Item Type="GameLibrary.SetParticleStyleActionData">
             <Name>Smoke</Name><StyleName>smokeStyle</StyleName><LifeTime>2</LifeTime>
             <BeginState><Scale>1 2</Scale><Color>80FF0000</Color></BeginState>
           </Item>"#,
    )
    .unwrap();
    (defs::find("SetParticleStyleActionData").unwrap().check)(&node).unwrap();
    use tilerider::content::actions::ActionDef;
    let style = defs::SetParticleStyle::read(&node).unwrap();
    let begin = style.begin_state.unwrap();
    assert_eq!(begin.scale.map(|s| (s.x, s.y)), Some((1.0, 2.0)));
    assert_eq!(begin.speed, None);
    assert!(style.end_state.is_none());
}
