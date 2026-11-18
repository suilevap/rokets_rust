//! Variables: set and checked by actions, per item or global.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, AddMapAction, CurrentActions, Owner, Run, Variables, run};
use tilerider::content::map::MapData;

tilerider::actions! {
    Note("Note") {
        text "Text": String = String::new();
    } => templates {
        "Note(text)" => |a| Some(Note { text: a.text(0)?.to_string() }),
    }
}

#[derive(Resource, Default)]
struct Notes(Vec<String>);

fn note(In(run): In<Run<Note>>, mut notes: ResMut<Notes>) -> bool {
    notes.0.push(run.data.text);
    true
}

fn app(actions: &str) -> App {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin))
        .init_resource::<Notes>()
        .add_map_action::<Note, _>(note)
        .insert_resource(CurrentActions(map.actions));
    app
}

fn notes(app: &mut App) -> Vec<String> {
    std::mem::take(&mut app.world_mut().resource_mut::<Notes>().0)
}

#[test]
fn templates_set_change_and_compare_global_variables() {
    let mut app = app("");
    let w = app.world_mut();
    run(w, "SetGlobalVar(doors,2)", None, 0);
    run(w, "SetVarEx(-1,doors,-,1)", None, 0);
    assert_eq!(w.resource::<Variables>().get(Owner::Global, "doors"), Some(1));
    run(w, "IfEx(-1,doors,==,1,Note(one))", None, 0);
    run(w, "IfEx(-1,doors,>,1,Note(more))", None, 0);
    run(w, "IfEx(-1,doors,<=,1,Note(at most one))", None, 0);
    assert_eq!(notes(&mut app), ["one", "at most one"]);
}

#[test]
fn an_items_variables_are_its_own_and_hold_its_id() {
    // Garage: a trigger checks who drove in by `Id`.
    let mut app = app(
        r#"<Item Type="GameLibrary.CheckIntVariable"><Name>Who</Name>
             <VariableName>Id</VariableName><Global>false</Global><ItemId>0</ItemId><Value>1005</Value>
             <ActionOnLess>Note(less)</ActionOnLess><ActionOnEqual Null="true" /><ActionOnGreater>Note(more)</ActionOnGreater>
           </Item>"#,
    );
    let w = app.world_mut();
    run(w, "SetVar(Target,4001)", Some(1001), 0);
    assert!(run(w, "Who", Some(1001), 0));
    assert!(run(w, "Who", Some(4001), 0));
    assert!(!run(w, "Who", Some(1005), 0), "nothing to run on equal: it fails");
    let vars = w.resource::<Variables>();
    assert_eq!(vars.get(Owner::Item(1001), "Target"), Some(4001));
    assert_eq!(vars.get(Owner::Item(4001), "Target"), None);
    assert_eq!(notes(&mut app), ["less", "more"]);
}

#[test]
fn if_is_a_condition_for_the_next_action() {
    let mut app = app(
        r#"<Item Type="GameLibrary.ActionSequence"><Name>Gate</Name><NextAction>Note(passed)</NextAction>
             <Actions><Item>na</Item></Actions></Item>
           <Item Type="GameLibrary.Note"><Name>Open</Name><NextAction>if(keys,>=,2)</NextAction><Text>try</Text></Item>"#,
    );
    let w = app.world_mut();
    run(w, "SetVar(keys,1)", Some(7), 0);
    assert!(!run(w, "Open", Some(7), 0));
    run(w, "SetVarEx(0,keys,+,1)", Some(7), 0);
    assert!(run(w, "Open", Some(7), 0));
    // The value side may name a variable.
    run(w, "SetGlobalVar(need,2)", None, 0);
    assert!(run(w, "if(keys,==,need)", Some(7), 0));
    assert_eq!(notes(&mut app), ["try", "try"]);
}

#[test]
fn variables_start_before_the_other_map_start_actions() {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.ActionSequence"><Name>Start</Name><RunOnMapStart>true</RunOnMapStart></Item>
             <Item Type="GameLibrary.ActionSequence"><Name>Later</Name></Item>
             <Item Type="GameLibrary.ChangeIntVariable"><Name>Init</Name><RunOnMapStart>true</RunOnMapStart></Item>
             <Item Type="GameLibrary.AddDoorComponent"><Name>Door</Name></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    // `Door` runs on map start by its type's default.
    assert_eq!(tilerider::actions::start_order(&map.actions), [2, 0, 3]);
}
