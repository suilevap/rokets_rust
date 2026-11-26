//! The saved settings: defaults, the stored text, what a bad file gives,
//! where the file is, command-line overrides, and the plugin's saving and
//! music switch.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use tilerider::info::Language;
use tilerider::settings::{
    ControlScheme, FileStore, LANGUAGES, Overrides, Settings, SettingsPlugin, Store, config_file,
};
use tilerider::sound::music::Jukebox;

/// A store in memory that can be looked into and made to fail.
#[derive(Clone, Default)]
struct Memory {
    text: Arc<Mutex<Option<String>>>,
    fail_writes: bool,
    writes: Arc<Mutex<u32>>,
}

impl Store for Memory {
    fn read(&self) -> io::Result<Option<String>> {
        Ok(self.text.lock().unwrap().clone())
    }

    fn write(&self, text: &str) -> io::Result<()> {
        if self.fail_writes {
            return Err(io::Error::other("read-only"));
        }
        *self.writes.lock().unwrap() += 1;
        *self.text.lock().unwrap() = Some(text.to_string());
        Ok(())
    }
}

fn app_with(store: &Memory, overrides: Overrides) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Jukebox>()
        .add_plugins(SettingsPlugin::with_store(overrides, Box::new(store.clone())));
    app.update();
    app
}

fn stored(store: &Memory) -> Settings {
    Settings::from_json(store.text.lock().unwrap().as_deref().expect("something was saved"))
}

#[test]
fn the_defaults_are_classic_controls_with_the_smart_options_and_music_on() {
    let s = Settings::default();
    assert_eq!(s.control_scheme, ControlScheme::Classic);
    assert!(s.auto_fire && s.auto_aim && s.tap_to_go && s.music);
    assert_eq!(s.language, None);
}

#[test]
fn the_scheme_in_effect_is_the_one_chosen() {
    assert_eq!(ControlScheme::Smart.in_effect(), ControlScheme::Smart);
    assert_eq!(ControlScheme::Classic.in_effect(), ControlScheme::Classic);
}

#[test]
fn the_controls_option_wins_for_the_run_and_is_not_saved() {
    let store = Memory::default();
    let overrides = Overrides { controls: Some(ControlScheme::Smart), ..Overrides::default() };
    let mut app = app_with(&store, overrides);
    assert_eq!(app.world().resource::<Settings>().control_scheme, ControlScheme::Smart);
    app.world_mut().resource_mut::<Settings>().auto_aim = false;
    app.update();
    assert_eq!(stored(&store).control_scheme, ControlScheme::Classic, "the run's choice is not kept");
    // The player picks one in the menu: that is saved.
    app.world_mut().resource_mut::<Settings>().set_control_scheme(ControlScheme::Smart);
    app.update();
    assert_eq!(stored(&store).control_scheme, ControlScheme::Smart);
}

#[test]
fn settings_survive_a_round_trip_through_text() {
    let mut s = Settings::default();
    s.control_scheme = ControlScheme::Smart;
    s.auto_fire = false;
    s.tap_to_go = false;
    s.set_music(false);
    s.set_language(Some("ru".into()));
    assert_eq!(Settings::from_json(&s.to_json()), s);
}

#[test]
fn a_corrupt_or_unknown_file_gives_the_defaults() {
    for text in ["", "not json", "[1,2]", r#"{"control_scheme":"Telepathy"}"#, r#"{"music":"loud"}"#] {
        assert_eq!(Settings::from_json(text), Settings::default(), "{text:?}");
    }
}

#[test]
fn a_file_with_missing_or_extra_fields_keeps_what_it_has() {
    let s = Settings::from_json(r#"{"auto_fire":false,"from_the_future":1}"#);
    assert!(!s.auto_fire);
    assert!(s.auto_aim && s.music);
}

#[test]
fn the_language_cycles_through_the_list_and_wraps() {
    let mut s = Settings::default();
    s.cycle_language(1);
    assert_eq!(s.language.as_deref(), Some("ru"));
    s.cycle_language(-1);
    s.cycle_language(-1);
    assert_eq!(s.language.as_deref(), LANGUAGES.last().unwrap().0);
    s.cycle_language(1);
    assert_eq!(s.language, None);
    s.language = Some("xx".into());
    s.cycle_language(1);
    assert_eq!(s.language, None, "an unlisted language goes to the first");
}

#[test]
fn a_file_store_reads_nothing_before_the_first_write_and_makes_its_folder() {
    let dir = std::env::temp_dir().join(format!("tilerider-settings-{}", std::process::id()));
    let store = FileStore(dir.join("deeper").join("settings.json"));
    assert_eq!(Settings::load(&store), Settings::default());
    let mut s = Settings::default();
    s.auto_aim = false;
    store.write(&s.to_json()).unwrap();
    assert_eq!(Settings::load(&store), s);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_unreadable_file_gives_the_defaults() {
    // A folder where the file should be: reading fails, and that is no crash.
    let dir = std::env::temp_dir().join(format!("tilerider-settings-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(Settings::load(&FileStore(dir.clone())), Settings::default());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_config_file_is_in_the_xdg_folder_else_under_home() {
    let xdg = config_file(|n| (n == "XDG_CONFIG_HOME").then(|| "/x".to_string()));
    assert_eq!(xdg, Some(PathBuf::from("/x/tilerider/settings.json")));
    let home = config_file(|n| (n == "HOME").then(|| "/h".to_string())).unwrap();
    assert!(home.starts_with("/h") && home.ends_with("tilerider/settings.json"));
    assert_eq!(config_file(|_| None), None);
    assert_eq!(config_file(|_| Some(String::new())), None, "empty is unset");
}

#[test]
fn the_plugin_loads_the_stored_settings_and_gives_the_language() {
    let store = Memory::default();
    *store.text.lock().unwrap() = Some(r#"{"auto_fire":false,"language":"de"}"#.into());
    let app = app_with(&store, Overrides::default());
    assert!(!app.world().resource::<Settings>().auto_fire);
    assert_eq!(app.world().resource::<Language>().0.as_deref(), Some("de"));
    assert_eq!(*store.writes.lock().unwrap(), 0, "starting writes nothing");
}

#[test]
fn a_changed_setting_is_saved() {
    let store = Memory::default();
    let mut app = app_with(&store, Overrides::default());
    app.world_mut().resource_mut::<Settings>().tap_to_go = false;
    app.update();
    assert!(!stored(&store).tap_to_go);
    // Nothing changed: nothing written.
    let writes = *store.writes.lock().unwrap();
    app.update();
    app.update();
    assert_eq!(*store.writes.lock().unwrap(), writes);
}

#[test]
fn a_store_that_fails_to_write_does_not_stop_the_game() {
    let store = Memory { fail_writes: true, ..default() };
    let mut app = app_with(&store, Overrides::default());
    app.world_mut().resource_mut::<Settings>().auto_fire = false;
    app.update();
    app.update();
    assert!(!app.world().resource::<Settings>().auto_fire);
}

#[test]
fn the_command_line_wins_for_the_run_and_is_not_saved() {
    let store = Memory::default();
    *store.text.lock().unwrap() = Some(r#"{"music":true,"language":"ru"}"#.into());
    let overrides = Overrides { music: Some(false), language: Some(None), controls: None };
    let mut app = app_with(&store, overrides);
    let s = app.world().resource::<Settings>().clone();
    assert!(!s.music && s.language.is_none());
    assert_eq!(app.world().resource::<Language>().0, None);
    app.update();
    assert!(!app.world().resource::<Jukebox>().is_on(), "the music is off for this run");

    // Another setting changes: the stored music and language stay.
    app.world_mut().resource_mut::<Settings>().auto_aim = false;
    app.update();
    let saved = stored(&store);
    assert!(!saved.auto_aim);
    assert!(saved.music);
    assert_eq!(saved.language.as_deref(), Some("ru"));

    // The player then changes the music: that is saved.
    app.world_mut().resource_mut::<Settings>().set_music(false);
    app.update();
    assert!(!stored(&store).music);
}

#[test]
fn the_settings_switch_the_music_and_follow_the_m_key() {
    let store = Memory::default();
    let mut app = app_with(&store, Overrides::default());
    app.world_mut().resource_mut::<Settings>().set_music(false);
    app.update();
    assert!(!app.world().resource::<Jukebox>().is_on());

    // The jukebox is switched on (what `M` does): the settings follow, saved.
    app.world_mut().resource_mut::<Jukebox>().switch_on();
    app.update();
    app.update();
    assert!(app.world().resource::<Settings>().music);
    assert!(stored(&store).music);
}
