//! `MapEffect(amount)` (`ChangeMapEffectAmount`): the level effect's new
//! amount, handed on as a message.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, MapEffectAmount, run};
use tilerider::content::map::MapData;

fn app() -> App {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.ChangeMapEffectAmount"><Name>Half</Name><Amount>0.5</Amount></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin)).insert_resource(CurrentActions(map.actions));
    app
}

fn sent(app: &mut App) -> Vec<f32> {
    app.world_mut().resource_mut::<Messages<MapEffectAmount>>().drain().map(|m| m.0).collect()
}

#[test]
fn the_template_hands_on_its_amount() {
    let mut app = app();
    assert!(run(app.world_mut(), "MapEffect(-5)", None, 0));
    assert!(run(app.world_mut(), "MapEffect(1)", None, 0));
    assert_eq!(sent(&mut app), vec![-5.0, 1.0]);
}

#[test]
fn a_map_action_hands_on_its_amount_too() {
    let mut app = app();
    assert!(run(app.world_mut(), "Half", None, 0));
    assert_eq!(sent(&mut app), vec![0.5]);
}

#[test]
fn a_template_without_a_number_does_nothing() {
    let mut app = app();
    assert!(!run(app.world_mut(), "MapEffect(warm)", None, 0));
    assert!(sent(&mut app).is_empty());
}
