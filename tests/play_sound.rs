//! The `PlaySound` map action: a cue by name, at an entity or the listener.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, run};
use tilerider::game::MapId;
use tilerider::sound::{PlaySound, SoundActionsPlugin};

fn cues(app: &mut App) -> Vec<PlaySound> {
    app.world_mut().resource_mut::<Messages<PlaySound>>().drain().collect()
}

#[test]
fn play_sound_cues_a_named_sound_where_it_comes_from() {
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, SoundActionsPlugin));
    app.world_mut().spawn((MapId(1001), Transform::from_xyz(3.0, 4.0, 0.0)));
    let map = tilerider::content::map::MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.PlaySound"><Name>Boing</Name><Sound>PongBounce</Sound>
               <EmmiterEntityId>1001</EmmiterEntityId><Volume>0.5</Volume><Pitch>0.25</Pitch></Item>
             <Item Type="GameLibrary.PlaySound"><Name>Nowhere</Name><Sound>Stamp</Sound>
               <EmmiterEntityId>9999</EmmiterEntityId></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    app.insert_resource(tilerider::actions::CurrentActions(map.actions));
    let w = app.world_mut();
    run(w, "Boing", None, 0);
    run(w, "Nowhere", None, 0);
    run(w, "PlaySound(ScaryMoment)", None, 0);
    let got: Vec<_> = cues(&mut app).into_iter().map(|c| (c.name, c.at, c.pitch, c.volume)).collect();
    assert_eq!(
        got,
        [("PongBounce", Some(Vec2::new(3.0, 4.0)), 0.25, 0.5), ("ScaryMoment", None, 0.0, 1.0)],
        "nothing from an emitter that is not there; the template at the listener"
    );
}
