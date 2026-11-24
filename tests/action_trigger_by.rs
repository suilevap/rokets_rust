//! `TriggerActionBy` and `TriggerActionByFirstSuccess`: who the actions run
//! for, in what order, and what the action returns.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, Run, run};
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

fn app(actions: &str) -> App {
    let data = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins(ActionsPlugin)
        .init_resource::<Notes>()
        .add_map_action::<Note, _>(note)
        .insert_resource(CurrentActions(data.actions));
    for id in [1, 2, 3] {
        app.world_mut().spawn(MapId(id));
    }
    app
}

fn notes(app: &mut App) -> Vec<(String, Option<i64>)> {
    std::mem::take(&mut app.world_mut().resource_mut::<Notes>().0)
}

fn n(text: &str, id: i64) -> (String, Option<i64>) {
    (text.to_string(), Some(id))
}

fn by(ids: &[&str], actions: &[&str], next: &str) -> String {
    let list = |v: &[&str]| v.iter().map(|i| format!("<Item>{i}</Item>")).collect::<String>();
    format!(
        r#"<Item Type="GameLibrary.TriggerActionBy"><Name>By</Name>{next}<ItemIds>{}</ItemIds><Actions>{}</Actions></Item>"#,
        list(ids),
        list(actions)
    )
}

#[test]
fn every_action_runs_for_each_id_in_the_lists_order_with_that_id_as_the_actor() {
    let mut app = app(&by(&["3", "1", "7"], &["Note(a)", "Note(b)"], ""));
    assert!(run(app.world_mut(), "By", Some(99), 0));
    assert_eq!(
        notes(&mut app),
        [n("a", 3), n("b", 3), n("a", 1), n("b", 1), n("a", 7), n("b", 7)],
        "7 has no entity and is still given to the actions"
    );
}

#[test]
fn a_failing_action_neither_stops_the_rest_nor_the_action_and_next_action_follows() {
    let mut app = app(&format!(
        r#"{}<Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#,
        by(&["1", "2"], &["Fail(x)", "Note(y)", " "], "<NextAction>After</NextAction>")
    ));
    assert!(run(app.world_mut(), "By", Some(5), 0));
    assert_eq!(notes(&mut app), [n("x", 1), n("y", 1), n("x", 2), n("y", 2), n("after", 5)]);
}

#[test]
fn an_empty_list_does_nothing_and_succeeds() {
    let mut app = app(&by(&[], &["Note(a)"], ""));
    assert!(run(app.world_mut(), "By", None, 0));
    assert!(notes(&mut app).is_empty());
}

#[test]
fn an_id_may_name_a_variable_global_first_then_the_actors_and_unset_is_zero() {
    // Targets' `var_ReferencedCoin`.
    let mut app = app(&by(&["coin", "mine", "nobody"], &["Note(a)"], ""));
    let w = app.world_mut();
    run(w, "SetGlobalVar(coin,2)", Some(1), 0);
    run(w, "SetVar(mine,3)", Some(1), 0);
    run(w, "SetVar(coin,9)", Some(1), 0);
    run(w, "By", Some(1), 0);
    assert_eq!(notes(&mut app), [n("a", 2), n("a", 3), n("a", 0)]);
}

fn first(ids: &[&str], action: &str, otherwise: &str, next: &str) -> String {
    let items: String = ids.iter().map(|i| format!("<Item>{i}</Item>")).collect();
    format!(
        r#"<Item Type="GameLibrary.TriggerActionByFirstSuccess"><Name>First</Name>{next}<ItemIds>{items}</ItemIds>
        <Action>{action}</Action>{otherwise}</Item>"#
    )
}

#[test]
fn the_first_id_that_succeeds_stops_the_walk() {
    let mut app = app(&format!(
        r#"{}<Item Type="GameLibrary.Note"><Name>Try</Name><Text>try</Text></Item>"#,
        first(&["1", "2", "3"], "Try", "", "")
    ));
    assert!(run(app.world_mut(), "First", Some(9), 0));
    assert_eq!(notes(&mut app), [n("try", 1)]);
}

#[test]
fn ids_without_an_entity_are_skipped_without_running_the_action() {
    let mut app = app(&first(&["40", "2", "3"], "Note(t)", "", ""));
    assert!(run(app.world_mut(), "First", None, 0));
    assert_eq!(notes(&mut app), [n("t", 2)]);
}

#[test]
fn when_none_succeeds_else_action_runs_for_the_original_actor_and_the_action_fails() {
    let mut app = app(&format!(
        r#"{}<Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#,
        first(&["1", "2"], "Fail(t)", "<ElseAction>Note(else)</ElseAction>", "<NextAction>After</NextAction>")
    ));
    assert!(!run(app.world_mut(), "First", Some(9), 0), "ElseAction's own success is not the result");
    assert_eq!(notes(&mut app), [n("t", 1), n("t", 2), n("else", 9)], "and NextAction does not follow");
}

#[test]
fn without_an_else_action_a_failed_walk_just_fails() {
    let mut app = app(&first(&["1", "40"], "Fail(t)", "", ""));
    assert!(!run(app.world_mut(), "First", Some(9), 0));
    assert_eq!(notes(&mut app), [n("t", 1)]);
}

#[test]
fn success_makes_next_action_follow_and_skips_else() {
    let mut app = app(&format!(
        r#"{}<Item Type="GameLibrary.Note"><Name>After</Name><Text>after</Text></Item>"#,
        first(&["2"], "Note(t)", "<ElseAction>Note(else)</ElseAction>", "<NextAction>After</NextAction>")
    ));
    assert!(run(app.world_mut(), "First", Some(9), 0));
    assert_eq!(notes(&mut app), [n("t", 2), n("after", 9)]);
}

#[test]
fn swarms_list_reads_as_the_map_writes_it() {
    use tilerider::content::actions::def::ActionDef;
    use tilerider::content::actions::defs::TriggerActionByFirstSuccess;
    use tilerider::content::node::Node;
    let node = Node::parse(
        r#"<Item Type="GameLibrary.TriggerActionByFirstSuccess"><Name>SetControlForFriend</Name>
        <RunOnMapStart>false</RunOnMapStart><ItemIds><Item>3002</Item><Item>30027</Item></ItemIds>
        <Action>TryToChangeToLocalControl</Action><ElseAction>RestartMap()</ElseAction></Item>"#,
    )
    .unwrap();
    let a = TriggerActionByFirstSuccess::read(&node).unwrap();
    assert_eq!(a.item_ids, ["3002", "30027"]);
    assert_eq!(a.action.as_deref(), Some("TryToChangeToLocalControl"));
    assert_eq!(a.else_action.as_deref(), Some("RestartMap()"));
}

fn success_sequence(actions: &[&str]) -> String {
    let items: String = actions.iter().map(|a| format!("<Item>{a}</Item>")).collect();
    format!(r#"<Item Type="GameLibrary.SuccessSequence"><Name>Seq</Name><Actions>{items}</Actions></Item>"#)
}

#[test]
fn a_success_sequence_runs_its_actions_for_the_actor_until_one_fails() {
    let mut app = app(&success_sequence(&["Note(a)", "Fail(b)", "Note(c)"]));
    assert!(!run(app.world_mut(), "Seq", Some(2), 0));
    assert_eq!(notes(&mut app), [n("a", 2), n("b", 2)]);
    let mut app = app_all();
    assert!(run(app.world_mut(), "Seq", Some(3), 0), "all succeeded");
    assert_eq!(notes(&mut app), [n("a", 3), n("c", 3)]);
}

fn app_all() -> App {
    app(&success_sequence(&["Note(a)", "Note(c)"]))
}

#[test]
fn the_first_car_whose_success_sequence_passes_is_taken() {
    // Swarm: `TryToChangeToLocalControl` is a `SuccessSequence` offered to
    // each car in turn.
    let mut app = app(&format!("{}{}", first(&["1", "2"], "Seq", "", ""), success_sequence(&["Note(take)"])));
    assert!(run(app.world_mut(), "First", None, 0));
    assert_eq!(notes(&mut app), [n("take", 1)]);
}
