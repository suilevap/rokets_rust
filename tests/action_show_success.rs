//! `ShowSuccess` stamps the screen and plays the stamp sound; it does not
//! end the level. `MoveToNextMap` does.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, run};
use tilerider::content::actions::defs::SuccessIndicator;
use tilerider::content::map::MapData;
use tilerider::sound::{PlaySound, SoundActionsPlugin};
use tilerider::stamp::{Half, Stamp};
use tilerider::triggers::LevelEnd;

fn app(actions: &str) -> App {
    let map = MapData::parse(&format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>{actions}</Actions></Asset></XnaContent>"#
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, SoundActionsPlugin))
        .init_resource::<LevelEnd>()
        .insert_resource(CurrentActions(map.actions));
    app
}

fn stamps(app: &mut App) -> Vec<Stamp> {
    app.world_mut().query::<&Stamp>().iter(app.world()).cloned().collect()
}

#[test]
fn show_success_does_not_end_the_level() {
    let mut app = app("");
    assert!(run(app.world_mut(), "ShowSuccess()", None, 0));
    assert!(app.world().resource::<LevelEnd>().timer.is_none());
}

#[test]
fn each_kind_makes_two_stamps_of_its_own_and_one_stamp_sound() {
    let mut app = app(
        r#"<Item Type="GameLibrary.ShowSuccess"><Name>Door</Name><SuccessType>DoorOpen</SuccessType></Item>
           <Item Type="GameLibrary.ShowSuccess"><Name>Wrong</Name><SuccessType>AntiSuccess</SuccessType></Item>"#,
    );
    for (name, kind) in [
        ("ShowSuccess()", SuccessIndicator::Standard),
        ("Door", SuccessIndicator::DoorOpen),
        ("Wrong", SuccessIndicator::AntiSuccess),
    ] {
        assert!(run(app.world_mut(), name, None, 0));
        let made = stamps(&mut app);
        assert_eq!(made.len(), 2, "{name}: a long stamp and a swelling one");
        assert!(made.iter().all(|s| s.kind == kind), "{name}");
        assert!(made.iter().any(|s| s.half == Half::Hold) && made.iter().any(|s| s.half == Half::Swell));
        // Both halves land at the same place, tilted the same.
        assert_eq!((made[0].offset_x, made[0].tilt), (made[1].offset_x, made[1].tilt));
        assert!((0.0..64.0).contains(&made[0].offset_x));
        assert!(made[0].tilt.abs() <= std::f32::consts::PI / 8.0);
        let cues: Vec<_> = app.world_mut().resource_mut::<Messages<PlaySound>>().drain().collect();
        assert_eq!(cues.len(), 1, "{name}");
        assert_eq!((cues[0].name, cues[0].at, cues[0].pitch, cues[0].volume), ("Stamp", None, 0.0, 1.0));
        let all: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<Stamp>>().iter(app.world()).collect();
        for e in all {
            app.world_mut().despawn(e);
        }
    }
}

#[test]
fn the_stamp_fades_out_in_three_seconds_and_its_second_half_in_half_a_second() {
    let mut app = app("");
    run(app.world_mut(), "ShowSuccess()", None, 0);
    for s in stamps(&mut app) {
        let style = s.style();
        let (life, grown) = match s.half {
            Half::Hold => (3.0, 1.0),
            Half::Swell => (0.5, 1.5),
        };
        assert_eq!(style.lifetime, life);
        assert_eq!(style.state_at(life).scale.x, grown);
        assert_eq!(style.state_at(0.0).color, [1.0; 4]);
        assert_eq!(style.state_at(life).color[3], 0.0);
    }
}

#[test]
fn move_to_next_map_still_ends_the_level() {
    let mut app = app("");
    assert!(run(app.world_mut(), "NextMap()", None, 0));
    assert!(app.world().resource::<LevelEnd>().timer.is_some());
}
