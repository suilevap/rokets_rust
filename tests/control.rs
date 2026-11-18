//! `SetControl` on the player's car: given back, taken away.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, run};
use tilerider::controls::{LocalPlayer, NoInput};
use tilerider::game::MapId;

#[test]
fn set_control_gives_the_car_back_to_the_player_and_takes_it_away() {
    let mut app = App::new();
    app.add_plugins(ActionsPlugin);
    let car = app.world_mut().spawn((MapId(4001), LocalPlayer, NoInput)).id();
    let w = app.world_mut();
    assert!(run(w, "SetControl(4001,local)", None, 0), "names in any case, as Enum.Parse");
    assert!(!w.entity(car).contains::<NoInput>());
    assert!(run(w, "SetControl(0,NoneLocal)", Some(4001), 0));
    assert!(w.entity(car).contains::<NoInput>());
    assert!(!run(w, "SetControl(4001,Nobody)", None, 0), "not a control type");
}
