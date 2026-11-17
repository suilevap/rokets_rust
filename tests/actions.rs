//! Map actions: the runner, and how little a new action type takes.

use bevy::prelude::*;
use serde::Deserialize;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, MapAction, Run, Templates, run};
use tilerider::content::map::MapData;

/// A new action type, as a map or a template would use it: this struct,
/// its `MapAction` impl and one `add_map_action` line are all it takes.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Note {
    #[serde(default)]
    text: String,
}

impl MapAction for Note {
    const TYPE: &'static str = "Note";

    fn templates() -> Templates<Self> {
        vec![("Note", |args| Some(Note { text: args.text(0)?.to_string() }))]
    }
}

#[derive(Resource, Default)]
struct Notes(Vec<(String, Option<i64>)>);

fn note(In(run): In<Run<Note>>, mut notes: ResMut<Notes>) -> bool {
    let failed = run.data.text == "fail";
    notes.0.push((run.data.text, run.actor));
    !failed
}

fn app(actions: &str) -> App {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins(ActionsPlugin)
        .init_resource::<Notes>()
        .add_map_action::<Note, _>(note)
        .insert_resource(CurrentActions(map.actions));
    app
}

fn notes(app: &App) -> Vec<String> {
    app.world().resource::<Notes>().0.iter().map(|(t, _)| t.clone()).collect()
}

#[test]
fn a_new_action_runs_from_the_map_and_from_its_template() {
    let mut app = app(r#"<Item Type="GameLibrary.Note"><Name>Hello</Name><Text>hi</Text></Item>"#);
    assert!(run(app.world_mut(), "Hello", Some(7), 0));
    assert!(run(app.world_mut(), "Note(there)", None, 0));
    assert_eq!(app.world().resource::<Notes>().0, vec![("hi".into(), Some(7)), ("there".into(), None)]);
}

#[test]
fn next_actions_follow_success_only() {
    let mut app = app(
        r#"<Item Type="GameLibrary.Note"><Name>A</Name><NextAction>B</NextAction><Text>a</Text></Item>
           <Item Type="GameLibrary.Note"><Name>B</Name><Text>b</Text></Item>
           <Item Type="GameLibrary.Note"><Name>F</Name><NextAction>B</NextAction><Text>fail</Text></Item>"#,
    );
    run(app.world_mut(), "A", None, 0);
    run(app.world_mut(), "F", None, 0);
    assert_eq!(notes(&app), ["a", "b", "fail"]);
}

#[test]
fn sequences_run_in_order_and_skip_na() {
    let mut app = app(
        r#"<Item Type="GameLibrary.ActionSequence"><Name>Seq</Name>
             <Actions><Item>Note(1)</Item><Item>na</Item><Item>Inner</Item></Actions></Item>
           <Item Type="GameLibrary.ActionSequence"><Name>Inner</Name>
             <Actions><Item>Note(2)</Item><Item>Note(3)</Item></Actions></Item>"#,
    );
    run(app.world_mut(), "Seq", None, 0);
    run(app.world_mut(), "Seq(Note(4) Note(5), 0)", None, 0);
    assert_eq!(notes(&app), ["1", "2", "3", "4", "5"]);
}

#[test]
fn unknown_actions_fail_and_loops_stop() {
    let mut app = app(
        r#"<Item Type="GameLibrary.CloneObject"><Name>Clone</Name></Item>
           <Item Type="GameLibrary.ActionSequence"><Name>Loop</Name><Actions><Item>Loop</Item></Actions></Item>"#,
    );
    assert!(!run(app.world_mut(), "Clone", None, 0));
    assert!(!run(app.world_mut(), "Nobody", None, 0));
    assert!(!run(app.world_mut(), "OpenDoor(3)", None, 0));
    assert!(run(app.world_mut(), "Loop", None, 0), "the guard ends it");
}

#[test]
fn triggers_default_to_armed_and_many_uses() {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name>
        <Triggers><Item><Id>1</Id><Area>0 0 2 2</Area></Item></Triggers></Asset></XnaContent>"#,
    )
    .unwrap();
    let t = &map.triggers.items[0];
    assert!(t.active);
    assert_eq!(t.amount, 999);
}

#[test]
fn every_map_action_of_a_ported_type_reads() {
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, tilerider::triggers::TriggersPlugin));
    let registry = app.world().resource::<tilerider::actions::ActionRegistry>();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/Maps/Singleplayer");
    let (mut read, mut failed) = (0, Vec::new());
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let map = MapData::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for a in &map.actions.0 {
            match registry.read(&a.kind, &a.xml) {
                Some(Ok(())) => read += 1,
                Some(Err(e)) => failed.push(format!("{}: {} {}: {e}", path.display(), a.kind, a.name)),
                None => {}
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(read > 100, "{read}");
}
