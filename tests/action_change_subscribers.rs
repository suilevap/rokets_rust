//! `ChangeSubscribers`: who sets a trigger off after Add and Remove.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::content::map::MapData;
use tilerider::controls::LocalPlayer;
use tilerider::game::MapId;
use tilerider::trigger_subscribers::TriggerSubscribersPlugin;
use tilerider::triggers::Triggers;

const PLAYER: i64 = 4001;

/// Triggers: 1 lists nobody useful (Race's `0`), 2 lists 4002 and 4003,
/// 3 lists none (the player's alone).
fn app() -> App {
    let action = |name: &str, trigger: i64, item: i64, op: &str| {
        format!(
            r#"<Item Type="GameLibrary.ChangeSubscribers"><Name>{name}</Name><TriggerId>{trigger}</TriggerId><ItemId>{item}</ItemId><Operation>{op}</Operation></Item>"#
        )
    };
    let actions: String = [
        action("Add1", 1, 0, "Add"),
        action("Remove1", 1, 0, "Remove"),
        action("Add2", 2, 0, "Add"),
        action("Remove2", 2, 4002, "Remove"),
        action("Add3", 3, 0, "Add"),
        action("Remove3", 3, 0, "Remove"),
        action("AddPlayer3", 3, PLAYER, "Add"),
        action("Add9", 9, 0, "Add"),
        action("AddGhost", 2, 99, "Add"),
    ]
    .concat();
    let data = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name>
        <Triggers>
          <Item><Id>1</Id><Area>0 0 1 1</Area><Subscribers>0</Subscribers></Item>
          <Item><Id>2</Id><Area>1 0 1 1</Area><Subscribers>4002 4003</Subscribers></Item>
          <Item><Id>3</Id><Area>2 0 1 1</Area></Item>
        </Triggers>
        <Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, TriggerSubscribersPlugin))
        .insert_resource(Triggers::new(4, 1, &data.triggers.items))
        .insert_resource(CurrentActions(data.actions));
    app.world_mut().spawn((MapId(PLAYER), LocalPlayer));
    for id in [4002, 4003] {
        app.world_mut().spawn(MapId(id));
    }
    app
}

fn who(app: &App, trigger: i64) -> Option<Vec<i64>> {
    app.world().resource::<Triggers>().subscribers_of(trigger).unwrap().map(<[i64]>::to_vec)
}

#[test]
fn add_puts_the_actor_in_a_list_once_and_remove_takes_it_out() {
    let mut app = app();
    assert!(run(app.world_mut(), "Add1", Some(4002), 0));
    assert!(run(app.world_mut(), "Add1", Some(4002), 0));
    assert_eq!(who(&app, 1), Some(vec![0, 4002]));
    assert!(run(app.world_mut(), "Add1", Some(4003), 0));
    assert!(run(app.world_mut(), "Remove1", Some(4002), 0));
    assert_eq!(who(&app, 1), Some(vec![0, 4003]));
    assert!(run(app.world_mut(), "Remove1", Some(4002), 0), "not in the list: no change, still success");
    assert_eq!(who(&app, 1), Some(vec![0, 4003]));
}

#[test]
fn item_id_names_the_entity_unless_it_is_zero() {
    let mut app = app();
    run(app.world_mut(), "Remove2", Some(4003), 0);
    assert_eq!(who(&app, 2), Some(vec![4003]), "4002 went, whoever set the action off");
    run(app.world_mut(), "Add2", Some(4002), 0);
    assert_eq!(who(&app, 2), Some(vec![4003, 4002]));
}

#[test]
fn a_trigger_of_the_player_alone_takes_the_player_and_the_newcomer() {
    let mut app = app();
    run(app.world_mut(), "AddPlayer3", Some(4002), 0);
    assert_eq!(who(&app, 3), None, "the player already sets it off");
    run(app.world_mut(), "Add3", Some(4002), 0);
    assert_eq!(who(&app, 3), Some(vec![PLAYER, 4002]));
}

#[test]
fn removing_the_player_from_a_trigger_of_the_player_alone_leaves_nobody() {
    let mut app = app();
    run(app.world_mut(), "Remove3", Some(4002), 0);
    assert_eq!(who(&app, 3), None, "another entity: no change");
    run(app.world_mut(), "Remove3", Some(PLAYER), 0);
    assert_eq!(who(&app, 3), Some(vec![]));
    run(app.world_mut(), "Add3", Some(4002), 0);
    assert_eq!(who(&app, 3), Some(vec![4002]));
}

#[test]
fn a_missing_trigger_or_entity_changes_nothing_and_fails() {
    let mut app = app();
    assert!(!run(app.world_mut(), "Add9", Some(4002), 0), "no trigger 9");
    assert!(!run(app.world_mut(), "AddGhost", Some(4002), 0), "no entity 99");
    assert!(!run(app.world_mut(), "Add2", Some(55), 0), "the actor is no entity");
    assert!(!run(app.world_mut(), "Add2", None, 0), "no actor");
    assert_eq!(who(&app, 2), Some(vec![4002, 4003]));
}
