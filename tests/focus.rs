//! Camera focus actions: what the camera frames.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, run};
use tilerider::game::MapId;
use tilerider::render::camera::{CameraTarget, Unfocused};

fn framed(app: &mut App) -> Vec<i64> {
    let mut q = app.world_mut().query_filtered::<&MapId, (With<CameraTarget>, Without<Unfocused>)>();
    let mut ids: Vec<i64> = q.iter(app.world()).map(|id| id.0).collect();
    ids.sort();
    ids
}

#[test]
fn focus_moves_to_an_entity_and_back_keeping_the_cars_settings() {
    let mut app = App::new();
    app.add_plugins(ActionsPlugin);
    let w = app.world_mut();
    w.spawn((MapId(4001), CameraTarget { radius: 1.0, max_speed: 20.0 }));
    w.spawn(MapId(1005));
    run(w, "AddFocusOnEntity(1005)", None, 0);
    assert_eq!(framed(&mut app), [1005, 4001]);
    run(app.world_mut(), "FocusOnEntity(1005)", None, 0);
    assert_eq!(framed(&mut app), [1005]);
    run(app.world_mut(), "FocusOnEntity(0)", Some(4001), 0);
    assert_eq!(framed(&mut app), [4001], "0 is the one that set it off");
    let mut q = app.world_mut().query::<(&MapId, &CameraTarget)>();
    let car = q.iter(app.world()).find(|(id, _)| id.0 == 4001).unwrap().1;
    assert_eq!(car.max_speed, 20.0);
}
