//! `CheckEntityAlive`: which branch runs, for whom, and what it returns.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, Inactive, Owner, Run, Variables, run};
use tilerider::content::map::MapData;
use tilerider::game::MapId;

tilerider::actions! {
    Note("Note") {
        text "Text": String = String::new();
        /// Whether the action succeeds.
        ok "Ok": bool = true;
    } => templates {
        "Note(text)" => |a| Some(Note { text: a.text(0)?.to_string(), ok: true }),
        "Fail(text)" => |a| Some(Note { text: a.text(0)?.to_string(), ok: false }),
    }
}

#[derive(Resource, Default)]
struct Notes(Vec<(String, Option<i64>)>);

fn note(In(run): In<Run<Note>>, mut notes: ResMut<Notes>) -> bool {
    notes.0.push((run.data.text, run.actor));
    run.data.ok
}

const CHECK: &str = r#"<Item Type="GameLibrary.CheckEntityAlive"><Name>Check</Name>
    <NextAction>After</NextAction><ItemId>ID</ItemId>
    <ActionOnAlive>Note(alive)</ActionOnAlive><ActionOnNotAlive>Fail(gone)</ActionOnNotAlive></Item>
    <Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#;

/// An app with entities 7 (active), 8 (inactive) and a check of `item`.
fn app(check: &str) -> App {
    let data = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{check}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins(ActionsPlugin)
        .init_resource::<Notes>()
        .add_map_action::<Note, _>(note)
        .insert_resource(CurrentActions(data.actions));
    app.world_mut().spawn(MapId(7));
    app.world_mut().spawn((MapId(8), Inactive));
    app
}

fn check(item: &str) -> App {
    app(&CHECK.replace("ID", item))
}

fn notes(app: &mut App) -> Vec<(String, Option<i64>)> {
    std::mem::take(&mut app.world_mut().resource_mut::<Notes>().0)
}

fn n(text: &str, id: i64) -> (String, Option<i64>) {
    (text.to_string(), Some(id))
}

#[test]
fn an_entity_that_exists_runs_the_alive_action_for_the_same_actor() {
    let mut app = check("7");
    assert!(run(app.world_mut(), "Check", Some(3), 0));
    assert_eq!(notes(&mut app), [n("alive", 3), n("after", 3)]);
}

#[test]
fn an_entity_that_does_not_exist_runs_the_not_alive_action() {
    let mut app = check("9");
    assert!(run(app.world_mut(), "Check", Some(3), 0));
    assert_eq!(notes(&mut app), [n("gone", 3), n("after", 3)]);
}

#[test]
fn a_deactivated_entity_is_alive() {
    let mut app = check("8");
    run(app.world_mut(), "Check", None, 0);
    assert_eq!(notes(&mut app)[0].0, "alive", "inactive is still in the world");
}

#[test]
fn a_destroyed_entity_is_not_alive() {
    let mut app = check("7");
    let entity = app.world_mut().query::<(Entity, &MapId)>().iter(app.world()).find(|(_, m)| m.0 == 7).unwrap().0;
    app.world_mut().despawn(entity);
    run(app.world_mut(), "Check", None, 0);
    assert_eq!(notes(&mut app)[0].0, "gone");
}

#[test]
fn an_item_id_of_zero_means_the_actor() {
    let mut app = check("0");
    run(app.world_mut(), "Check", Some(7), 0);
    assert_eq!(notes(&mut app)[0], n("alive", 7));
    run(app.world_mut(), "Check", Some(5), 0);
    assert_eq!(notes(&mut app)[0], n("gone", 5));
    run(app.world_mut(), "Check", None, 0);
    assert_eq!(notes(&mut app)[0].0, "gone", "no actor, no entity");
}

#[test]
fn the_item_id_may_name_a_variable_that_holds_the_id() {
    let mut app = check("target");
    app.world_mut().resource_mut::<Variables>().set(Owner::Global, "target", 7);
    run(app.world_mut(), "Check", Some(1), 0);
    assert_eq!(notes(&mut app)[0].0, "alive");
    app.world_mut().resource_mut::<Variables>().set(Owner::Global, "target", 42);
    run(app.world_mut(), "Check", Some(1), 0);
    assert_eq!(notes(&mut app)[0].0, "gone");
}

#[test]
fn the_action_succeeds_and_runs_next_action_even_when_its_branch_fails() {
    // "Fail(gone)" fails, `After` still follows and the action succeeds.
    let mut app = check("9");
    assert!(run(app.world_mut(), "Check", Some(1), 0));
    assert_eq!(notes(&mut app).len(), 2);
}

#[test]
fn a_missing_or_empty_branch_runs_nothing_and_still_succeeds() {
    let mut app = app(
        r#"<Item Type="GameLibrary.CheckEntityAlive"><Name>C</Name><NextAction>After</NextAction><ItemId>9</ItemId>
        <ActionOnAlive>Note(alive)</ActionOnAlive><ActionOnNotAlive Null="true" /></Item>
        <Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#,
    );
    assert!(run(app.world_mut(), "C", Some(1), 0));
    assert_eq!(notes(&mut app), [n("after", 1)]);
}

#[test]
fn an_action_without_an_item_id_checks_the_actor() {
    let mut app = app(&CHECK.replace("<ItemId>ID</ItemId>", r#"<ItemId Null="true" />"#));
    assert!(run(app.world_mut(), "Check", Some(7), 0));
    assert_eq!(notes(&mut app)[0], n("alive", 7));
    run(app.world_mut(), "Check", Some(5), 0);
    assert_eq!(notes(&mut app)[0], n("gone", 5));
}
