//! `SetControl` and `AIAdjustment`: who drives a car, and its team.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::content::map::MapData;
use tilerider::car::control::Team;
use tilerider::controls::{LocalPlayer, NoInput};
use tilerider::game::MapId;

#[test]
fn set_control_gives_the_car_back_to_the_player_and_takes_it_away() {
    let mut app = App::new();
    app.add_plugins(ActionsPlugin);
    let car = app.world_mut().spawn((MapId(4001), LocalPlayer, NoInput, Team(0))).id();
    let w = app.world_mut();
    assert!(run(w, "SetControl(4001,local)", None, 0), "names in any case, as Enum.Parse");
    assert!(!w.entity(car).contains::<NoInput>());
    assert!(run(w, "SetControl(0,NoneLocal)", Some(4001), 0));
    assert!(w.entity(car).contains::<NoInput>());
    assert!(!run(w, "SetControl(4001,Nobody)", None, 0), "not a control type");
}

#[test]
fn set_control_on_a_simple_car_sets_its_team_and_whether_nobody_drives_it() {
    let mut app = App::new();
    app.add_plugins(ActionsPlugin);
    let car = app.world_mut().spawn((MapId(3001), Team(0))).id();
    let w = app.world_mut();
    assert!(run(w, "SetControl(3001,NoneEnemy)", None, 0));
    assert!(w.entity(car).contains::<NoInput>(), "AINone");
    assert_eq!(w.entity(car).get::<Team>(), Some(&Team(1)));
    // An AI type: not undriven (a Hammer's saw works), and the AI's team.
    assert!(run(w, "SetControl(3001,AlwaysShoot)", None, 0));
    assert!(!w.entity(car).contains::<NoInput>());
    assert_eq!(w.entity(car).get::<Team>(), Some(&Team(1)));
    assert!(run(w, "SetControl(3001,NoneFriend)", None, 0));
    assert_eq!(w.entity(car).get::<Team>(), Some(&Team(0)));
    assert!(!run(w, "SetControl(3999,NoneFriend)", None, 0), "no such car");
}

/// An app whose map has an `AIAdjustment` named `Adjust` for `item` with `team`.
fn adjusting(item: i64, team: i64) -> App {
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
        <Item Type="GameLibrary.AIAdjustment"><Name>Adjust</Name><ItemId>{item}</ItemId><Team>{team}</Team>
        <ScanDistance>-1</ScanDistance><WeaponMaxAngleDeg>-1</WeaponMaxAngleDeg></Item>
        </Actions></Asset></XnaContent>"#
    );
    let map = MapData::parse(&xml).unwrap();
    let mut app = App::new();
    app.add_plugins(ActionsPlugin).insert_resource(CurrentActions(map.actions));
    app
}

#[test]
fn ai_adjustment_sets_the_team_unless_255() {
    let mut app = adjusting(3001, 0);
    let enemy = app.world_mut().spawn((MapId(3001), Team(1))).id();
    assert!(run(app.world_mut(), "Adjust", None, 0));
    assert_eq!(app.world().entity(enemy).get::<Team>(), Some(&Team(0)));
    let mut app = adjusting(3001, 255);
    let enemy = app.world_mut().spawn((MapId(3001), Team(1))).id();
    assert!(run(app.world_mut(), "Adjust", None, 0));
    assert_eq!(app.world().entity(enemy).get::<Team>(), Some(&Team(1)), "255 keeps it");
}

#[test]
fn ai_adjustment_leaves_the_players_own_control_alone() {
    // The player's composite control is not a `BaseControl`; once it is
    // `AINone` (`NoInput`) it is.
    let mut app = adjusting(4001, 1);
    let player = app.world_mut().spawn((MapId(4001), LocalPlayer, Team(0))).id();
    assert!(run(app.world_mut(), "Adjust", None, 0));
    assert_eq!(app.world().entity(player).get::<Team>(), Some(&Team(0)));
    app.world_mut().entity_mut(player).insert(NoInput);
    assert!(run(app.world_mut(), "Adjust", None, 0));
    assert_eq!(app.world().entity(player).get::<Team>(), Some(&Team(1)));
}
