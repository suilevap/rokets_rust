//! Action definitions: every action in the maps reads, writes back and
//! reads again the same, and the game carries out only defined types.

use std::path::Path;

use bevy::prelude::*;
use tilerider::actions::{ActionRegistry, ActionsPlugin, run};
use tilerider::content::actions::def::{Kind, Ref};
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
fn every_defined_action_in_the_maps_round_trips() {
    let (mut checked, mut failed) = (0, Vec::new());
    for (file, map) in maps() {
        for a in &map.actions.0 {
            if let Some(def) = defs::find(&a.kind) {
                match (def.check)(&a.node) {
                    Ok(()) => checked += 1,
                    Err(e) => failed.push(format!("{file}: {}: {e}", a.name())),
                }
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(checked > 400, "{checked}");
   
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
        (on_open.kind, on_open.refs, on_open.optional),
        (Kind::Text, &[Ref::Action][..], false)
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
fn the_game_carries_out_only_defined_types() {
    let mut app = App::new();
    app.add_plugins((
        ActionsPlugin,
        tilerider::triggers::TriggersPlugin,
        tilerider::door::DoorPlugin,
    ));
    let registry = app.world().resource::<ActionRegistry>();
    let defined: Vec<_> = defs::ALL.iter().filter(|d| registry.has(d.kind)).collect();
    assert_eq!(
        defined.len(),
        defs::ALL.len(),
        "every type defined so far is ported"
    );
    // Templates are picked by name and number of arguments.
    assert!(run(app.world_mut(), "NextMap()", None, 0));
    assert!(run(app.world_mut(), "NextMap(3)", None, 0));
}
