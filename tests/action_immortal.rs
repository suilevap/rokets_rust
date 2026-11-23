//! `SetImmortal`: a car's shield made immortal by an action.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::car::shield::Shield;
use tilerider::content::actions::TemplateArgs;
use tilerider::content::actions::def::ActionDef;
use tilerider::content::actions::defs::SetImmortal;
use tilerider::content::map::MapData;
use tilerider::content::node::Node;
use tilerider::game::MapId;

fn app(actions: &str) -> App {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins(ActionsPlugin).insert_resource(CurrentActions(map.actions));
    app
}

fn immortal(app: &App, car: Entity) -> bool {
    app.world().get::<Shield>(car).unwrap().is_immortal()
}

/// Escape's: `ActionOnCreate` names an action with `ItemId` 0, so it hits
/// the car that was created.
const ESCAPE: &str = r#"<Item Type="GameLibrary.SetImmortal"><Name>SetImmortal</Name>
    <RunOnMapStart>false</RunOnMapStart><ItemId>0</ItemId><IsImmortal>true</IsImmortal></Item>"#;

#[test]
fn an_item_id_of_zero_means_the_car_that_set_it_off() {
    let mut app = app(ESCAPE);
    let turret = app.world_mut().spawn((MapId(3015), Shield::new(4))).id();
    let other = app.world_mut().spawn((MapId(3001), Shield::new(4))).id();
    assert!(run(app.world_mut(), "SetImmortal", Some(3015), 0));
    assert!(immortal(&app, turret));
    assert!(!immortal(&app, other));
}

#[test]
fn an_item_id_names_the_car_whoever_set_it_off() {
    let mut app = app(
        r#"<Item Type="GameLibrary.SetImmortal"><Name>Off</Name><ItemId>3001</ItemId><IsImmortal>false</IsImmortal></Item>"#,
    );
    let car = app.world_mut().spawn((MapId(3001), Shield::new(4))).id();
    app.world_mut().get_mut::<Shield>(car).unwrap().set_immortal(true);
    run(app.world_mut(), "Off", Some(4001), 0);
    assert!(!immortal(&app, car));
}

#[test]
fn battlecitys_template_sets_it_by_id_and_value() {
    let mut app = app("");
    let car = app.world_mut().spawn((MapId(3001), Shield::new(4))).id();
    assert!(run(app.world_mut(), "SetImmortal(0,true)", Some(3001), 0));
    assert!(immortal(&app, car));
    assert!(run(app.world_mut(), "SetImmortal(3001,False)", None, 0), "`bool.Parse` ignores case");
    assert!(!immortal(&app, car));
}

#[test]
fn the_template_wants_an_integer_and_a_bool() {
    let t = SetImmortal::templates();
    let (name, make) = t.iter().find(|(n, _)| n.starts_with("SetImmortal")).expect("template");
    assert_eq!(*name, "SetImmortal(id,value)");
    let args = |a: &[&str]| TemplateArgs(a.iter().map(|s| s.to_string()).collect());
    let made = make(&args(&["7", " true "])).expect("made");
    assert_eq!((made.item_id, made.is_immortal), (7, true));
    assert!(make(&args(&["x", "true"])).is_none());
    assert!(make(&args(&["7", "yes"])).is_none());
    assert!(make(&args(&["7"])).is_none());
}

#[test]
fn it_succeeds_with_no_target_or_no_shield_so_next_action_still_follows() {
    // C#'s void handlers are always true.
    let mut app = app(
        r#"<Item Type="GameLibrary.SetImmortal"><Name>Nobody</Name><ItemId>9999</ItemId><IsImmortal>true</IsImmortal></Item>"#,
    );
    let bare = app.world_mut().spawn(MapId(3002)).id();
    assert!(run(app.world_mut(), "Nobody", None, 0));
    assert!(run(app.world_mut(), "SetImmortal(0,true)", Some(3002), 0));
    assert!(app.world().get::<Shield>(bare).is_none());
}

#[test]
fn escapes_action_reads_as_the_map_writes_it() {
    let node = Node::parse(ESCAPE).unwrap();
    let a = SetImmortal::read(&node).unwrap();
    assert_eq!((a.item_id, a.is_immortal), (0, true));
}
