//! Map actions: the runner, and how little a new action type takes.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, Run, run};
use tilerider::content::map::MapData;

// A new action type, as a map or a template would use it: this
// definition, a system and one `add_map_action` line are all it takes.
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
    app.world()
        .resource::<Notes>()
        .0
        .iter()
        .map(|(t, _)| t.clone())
        .collect()
}

#[test]
fn a_new_action_runs_from_the_map_and_from_its_template() {
    let mut app = app(r#"<Item Type="GameLibrary.Note"><Name>Hello</Name><Text>hi</Text></Item>"#);
    assert!(run(app.world_mut(), "Hello", Some(7), 0));
    assert!(run(app.world_mut(), "Note(there)", None, 0));
    assert_eq!(
        app.world().resource::<Notes>().0,
        vec![("hi".into(), Some(7)), ("there".into(), None)]
    );
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
    let mut app = app(r#"<Item Type="GameLibrary.ActionSequence"><Name>Seq</Name>
             <Actions><Item>Note(1)</Item><Item>na</Item><Item>Inner</Item></Actions></Item>
           <Item Type="GameLibrary.ActionSequence"><Name>Inner</Name>
             <Actions><Item>Note(2)</Item><Item>Note(3)</Item></Actions></Item>"#);
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
fn of_actions_sharing_a_name_the_first_that_succeeds_runs() {
    // C#'s `result = result || action.Run()`: the maps were tuned with it.
    let mut app = app(
        r#"<Item Type="GameLibrary.Note"><Name>Twice</Name><Text>first</Text></Item>
           <Item Type="GameLibrary.Note"><Name>Twice</Name><Text>second</Text></Item>
           <Item Type="GameLibrary.Note"><Name>Else</Name><Text>fail</Text></Item>
           <Item Type="GameLibrary.Note"><Name>Else</Name><Text>fallback</Text></Item>"#,
    );
    assert!(run(app.world_mut(), "Twice", None, 0));
    assert!(run(app.world_mut(), "Else", None, 0));
    assert_eq!(notes(&app), ["first", "fail", "fallback"]);
}
