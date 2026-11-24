//! Level music: `SoundManager`'s one song at a time with fades, and the
//! `PlayLevelMusic` action that picks which one.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, run};
use tilerider::content::actions::defs::LevelMusicType;
use tilerider::content::map::MapData;
use tilerider::sound::music::{Jukebox, MASTER_VOLUME, MusicActionsPlugin, MusicCue};

fn play(track: &str, looped: bool) -> MusicCue {
    MusicCue::Play { track: track.into(), looped }
}

/// Steps the jukebox `seconds` in 1/60 s.
fn step(j: &mut Jukebox, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        j.update(1.0 / 60.0);
    }
}

#[test]
fn the_ambient_song_starts_with_the_map_looped_and_fades_in() {
    let mut j = Jukebox::default();
    j.set_level_music("7", "Short2", "");
    assert_eq!(j.take_cues(), [play("7", true)]);
    assert_eq!(j.volume(), 0.0, "it starts from silence");
    step(&mut j, 0.5);
    let half = j.volume();
    assert!(half > 0.1 && half < MASTER_VOLUME, "fading in: {half}");
    step(&mut j, 4.0);
    assert!((j.volume() - MASTER_VOLUME).abs() < 0.001);
    assert_eq!(j.playing(), Some(("7", true)));
}

#[test]
fn another_song_waits_for_the_one_playing_to_fade_out() {
    let mut j = Jukebox::default();
    j.set_level_music("7", "Short2", "");
    step(&mut j, 5.0);
    j.take_cues();
    j.play_level_music(LevelMusicType::Dynamic1, false);
    assert!(j.take_cues().is_empty(), "the old song goes on while it fades");
    assert_eq!(j.playing(), Some(("7", true)));
    step(&mut j, 0.5);
    assert!(j.take_cues().is_empty());
    assert!(j.volume() < MASTER_VOLUME && j.volume() > 0.01);
    // Faded (0.4 · e^(-3t) < 0.01 at 1.23 s): the new song starts, from silence.
    let mut cues = Vec::new();
    while cues.is_empty() {
        j.update(1.0 / 60.0);
        cues = j.take_cues();
    }
    assert_eq!(cues, [MusicCue::Stop, play("Short2", false)]);
    assert_eq!(j.playing(), Some(("Short2", false)));
    assert_eq!(j.volume(), 0.0);
    step(&mut j, 5.0);
    assert!((j.volume() - MASTER_VOLUME).abs() < 0.001, "and fades in");
}

#[test]
fn the_song_playing_goes_on_when_asked_for_again() {
    let mut j = Jukebox::default();
    j.set_level_music("7", "Short2", "");
    step(&mut j, 5.0);
    j.take_cues();
    // The same ambient on the next map, and the same song by `PlayLevelMusic`.
    j.set_level_music("7", "", "");
    j.play_level_music(LevelMusicType::Ambient, false);
    assert!(j.take_cues().is_empty());
    assert_eq!(j.playing(), Some(("7", true)), "`IsRepeating` stays as it was");
    assert!((j.volume() - MASTER_VOLUME).abs() < 0.001);
}

#[test]
fn a_song_fading_out_is_called_back_by_asking_for_it() {
    let mut j = Jukebox::default();
    j.set_level_music("7", "", "");
    step(&mut j, 5.0);
    j.take_cues();
    j.play_level_music(LevelMusicType::None, true);
    step(&mut j, 0.5);
    let low = j.volume();
    assert!(low < MASTER_VOLUME - 0.1);
    j.play_level_music(LevelMusicType::Ambient, true);
    step(&mut j, 5.0);
    assert!((j.volume() - MASTER_VOLUME).abs() < 0.001, "back at full volume");
    assert!(j.take_cues().is_empty(), "never stopped");
}

#[test]
fn none_and_a_kind_the_map_lacks_fade_the_music_out_and_stop_it() {
    for kind in [LevelMusicType::None, LevelMusicType::Dynamic2] {
        let mut j = Jukebox::default();
        j.set_level_music("5", "", "");
        step(&mut j, 5.0);
        j.take_cues();
        j.play_level_music(kind, true);
        assert!(j.take_cues().is_empty());
        step(&mut j, 3.0);
        assert_eq!(j.take_cues(), [MusicCue::Stop], "{kind:?}");
        assert_eq!(j.playing(), None);
    }
}

#[test]
fn a_map_without_an_ambient_song_fades_the_music_out() {
    let mut j = Jukebox::default();
    j.set_level_music("5", "MidFast", "2_1");
    step(&mut j, 5.0);
    j.take_cues();
    j.set_level_music("", "2_10", "");
    step(&mut j, 3.0);
    assert_eq!(j.take_cues(), [MusicCue::Stop]);
    // Its dynamic song is there for `PlayLevelMusic`; the third is the last map's.
    assert_eq!(j.level_track(LevelMusicType::Dynamic1), Some("2_10"));
    assert_eq!(j.level_track(LevelMusicType::Dynamic2), Some("2_1"));
}

#[test]
fn a_song_that_ended_plays_again_from_silence() {
    let mut j = Jukebox::default();
    j.set_level_music("5", "Short2", "");
    step(&mut j, 5.0);
    j.play_level_music(LevelMusicType::Dynamic1, false);
    step(&mut j, 2.0);
    j.take_cues();
    step(&mut j, 5.0);
    j.song_ended();
    assert_eq!(j.playing(), None);
    j.play_level_music(LevelMusicType::Dynamic1, false);
    assert_eq!(j.take_cues(), [play("Short2", false)], "no fade out of what is not playing");
    assert_eq!(j.volume(), 0.0);
}

#[test]
fn music_off_plays_nothing_and_switching_it_off_stops_the_song() {
    let mut j = Jukebox::new(false);
    j.set_level_music("5", "MidFast", "");
    j.play_level_music(LevelMusicType::Dynamic1, true);
    step(&mut j, 2.0);
    assert!(j.take_cues().is_empty());
    assert_eq!(j.level_track(LevelMusicType::Dynamic1), Some("MidFast"), "the songs are still known");

    let mut j = Jukebox::default();
    j.set_level_music("5", "", "");
    step(&mut j, 1.0);
    j.take_cues();
    j.switch_off();
    assert_eq!(j.take_cues(), [MusicCue::Stop], "at once, no fade");
    j.play_level_music(LevelMusicType::Ambient, true);
    assert!(j.take_cues().is_empty());
    j.switch_on();
    assert!(j.take_cues().is_empty(), "nothing plays until a song is asked for");
    j.play_level_music(LevelMusicType::Ambient, true);
    assert_eq!(j.take_cues(), [play("5", true)]);
}

#[test]
fn the_map_names_its_songs() {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name>
             <MusicAmbient>2_12</MusicAmbient><MusicDynamic1> Short2 </MusicDynamic1>
             <MusicDynamic2></MusicDynamic2></Asset></XnaContent>"#,
    )
    .unwrap();
    let name = |s: &Option<String>| s.clone().unwrap_or_default();
    let mut j = Jukebox::default();
    j.set_level_music(&name(&map.music_ambient), &name(&map.music_dynamic1), &name(&map.music_dynamic2));
    assert_eq!(j.take_cues(), [play("2_12", true)]);
    assert_eq!(j.level_track(LevelMusicType::Dynamic1), Some("Short2"));
    assert_eq!(j.level_track(LevelMusicType::Dynamic2), None, "an empty element names nothing");

    let none = MapData::parse(r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name></Asset></XnaContent>"#).unwrap();
    assert!(none.music_ambient.is_none() && none.music_dynamic1.is_none());
}

#[test]
fn the_play_level_music_action_and_its_templates() {
    let mut app = App::new();
    app.add_plugins((ActionsPlugin, MusicActionsPlugin));
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.PlayLevelMusic"><Name>Fight</Name>
               <LevelMusicType>Dynamic1</LevelMusicType><Looped>false</Looped></Item>
             <Item Type="GameLibrary.PlayLevelMusic"><Name>Quiet</Name>
               <LevelMusicType>None</LevelMusicType></Item>
             <Item Type="GameLibrary.PlayLevelMusic"><Name>Calm</Name>
               <LevelMusicType>Ambient</LevelMusicType></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    app.insert_resource(tilerider::actions::CurrentActions(map.actions));
    app.world_mut().resource_mut::<Jukebox>().set_level_music("5", "MidFast", "2_1");
    let step_all = |app: &mut App, seconds: f32| step(&mut app.world_mut().resource_mut::<Jukebox>(), seconds);
    let state = |app: &App| {
        let j = app.world().resource::<Jukebox>();
        (j.playing().map(|(t, l)| (t.to_string(), l)), j.volume() > 0.3)
    };
    step_all(&mut app, 5.0);
    assert_eq!(state(&app), (Some(("5".into(), true)), true));

    run(app.world_mut(), "Fight", None, 0);
    step_all(&mut app, 8.0);
    assert_eq!(state(&app), (Some(("MidFast".into(), false)), true), "a song the action does not loop");

    run(app.world_mut(), "Quiet", None, 0);
    step_all(&mut app, 3.0);
    assert_eq!(state(&app).0, None);

    run(app.world_mut(), "Calm", None, 0);
    step_all(&mut app, 8.0);
    assert_eq!(state(&app), (Some(("5".into(), true)), true), "`Looped` is on unless the map says");

    run(app.world_mut(), "PlayMusic(Dynamic2)", None, 0);
    step_all(&mut app, 8.0);
    assert_eq!(state(&app), (Some(("2_1".into(), false)), true), "`PlayMusic` plays once");

    run(app.world_mut(), "PlayMusicLooped(dynamic1)", None, 0);
    step_all(&mut app, 8.0);
    assert_eq!(state(&app), (Some(("MidFast".into(), true)), true), "`PlayMusicLooped` loops, in any case");
}
