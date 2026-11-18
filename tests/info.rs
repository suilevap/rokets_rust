//! `ShowText`: texts by key over the game.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, run};
use tilerider::content::strings::Strings;
use tilerider::info::{InfoPlugin, Shown, Texts};

#[test]
fn texts_are_found_by_key_and_a_newer_one_replaces_the_shown_one() {
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, InfoPlugin));
    let strings = Strings::parse(r#"<root><data name="Be_careful"><value>Be Careful</value></data></root>"#).unwrap();
    app.insert_resource(Texts(strings));
    let w = app.world_mut();
    assert!(run(w, "ShowText(Be_careful,3)", None, 0));
    assert_eq!(w.resource::<Shown>().text.as_deref(), Some("Be Careful"));
    assert!(run(w, "ShowText(Unknown)", None, 0));
    assert_eq!(w.resource::<Shown>().text.as_deref(), Some("#Unknown"));
}
