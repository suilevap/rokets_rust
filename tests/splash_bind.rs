//! `SplashSetBindVariable`: a variable's value, as text, in a splash slot.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, SplashBindText, run};
use tilerider::content::map::MapData;

fn app() -> App {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.SplashSetBindVariable"><Name>Bind1</Name>
               <BindVariableNumber>First</BindVariableNumber><Value>Score</Value></Item>
             <Item Type="GameLibrary.SplashSetBindVariable"><Name>Bind2</Name>
               <BindVariableNumber>Second</BindVariableNumber><Value>Mine</Value></Item>
             <Item Type="GameLibrary.SplashSetBindVariable"><Name>BindNumber</Name>
               <BindVariableNumber>Third</BindVariableNumber><Value>42</Value></Item>
             <Item Type="GameLibrary.SplashSetBindVariable"><Name>BindFifth</Name>
               <BindVariableNumber>Fifth</BindVariableNumber><Value>1</Value></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin)).insert_resource(CurrentActions(map.actions));
    app
}

fn slot(app: &App, i: usize) -> Option<String> {
    app.world().resource::<SplashBindText>().get(i).map(str::to_string)
}

#[test]
fn a_global_variable_goes_to_its_slot_as_text() {
    let mut app = app();
    let w = app.world_mut();
    run(w, "SetGlobalVar(Score,7)", None, 0);
    assert!(run(w, "Bind1", None, 0));
    assert_eq!(slot(&app, 1).as_deref(), Some("7"));
    // The value is read again at every run, as a counter changes.
    let w = app.world_mut();
    run(w, "SetVarEx(-1,Score,+,2)", None, 0);
    run(w, "Bind1", None, 0);
    assert_eq!(slot(&app, 1).as_deref(), Some("9"));
}

#[test]
fn the_actors_variable_is_read_when_there_is_no_global_one() {
    let mut app = app();
    let w = app.world_mut();
    run(w, "SetVar(Mine,5)", Some(1001), 0);
    run(w, "Bind2", Some(1001), 0);
    assert_eq!(slot(&app, 2).as_deref(), Some("5"));
}

#[test]
fn an_unknown_variable_is_zero_and_a_number_is_itself() {
    let mut app = app();
    let w = app.world_mut();
    run(w, "Bind2", None, 0);
    run(w, "BindNumber", None, 0);
    assert_eq!(slot(&app, 2).as_deref(), Some("0"));
    assert_eq!(slot(&app, 3).as_deref(), Some("42"));
    assert_eq!(slot(&app, 1), None, "slots no action set stay empty");
    assert_eq!(slot(&app, 0), None, "slot 0 is the splash's countdown");
}

#[test]
fn the_fifth_slot_does_not_exist() {
    let mut app = app();
    assert!(run(app.world_mut(), "BindFifth", None, 0), "it still counts as run");
    assert_eq!(app.world().resource::<SplashBindText>().set_slots().count(), 0);
}
