//! The player's settings, kept between runs.
//!
//! The C# game had no such thing: `Settings.cs` is compile-time constants
//! and a few statics (`SoundMute`, `UseAlternateControl`, `StringsCulture`,
//! `VibroOn`) that were set by the menus and lost on exit. The port keeps
//! what the options menu changes ([`crate::menu`], the *Options* tab) in a
//! small JSON document:
//!
//! - native: `settings.json` in `$XDG_CONFIG_HOME/tilerider/`, else (by
//!   platform) `%APPDATA%\tilerider\`, `~/Library/Application Support/tilerider/`
//!   or `~/.config/tilerider/`;
//! - browser: `localStorage` under the key `tilerider.settings`.
//!
//! It is read once at start and written when a setting changes. Failing to
//! read it (missing, unreadable, corrupt or from a newer version) gives the
//! defaults; failing to write is logged and the game goes on. Command-line
//! options (`--no-music`, `--lang`, or the same as URL parameters) win for
//! the run they are given in ([`Overrides`]) and are not saved: only a
//! setting the player changes afterwards is.
//!
//! [`Settings`] and its storage are plain data (tested in
//! `tests/settings.rs`); [`SettingsPlugin`] is the small Bevy part: the
//! resource, the music switch that follows it, and the saving.

use std::io;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::info::Language;
use crate::sound::music::Jukebox;

/// How the player drives and fires on a touch screen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlScheme {
    /// The zones of `controls.rs`: a floating stick, a gadget circle and a
    /// fire zone.
    #[default]
    Classic,
    /// One finger ([`crate::assist`]): a drag drives, a tap on an enemy
    /// locks it (the car then aims and fires by itself, as the
    /// `auto_fire` and `auto_aim` options say), a tap on the ground lets
    /// the lock go. Tapping a box to grab it, the ground to drop what is
    /// towed, and double-tapping the ground to go there come later
    /// (`docs/plans/SMART_CONTROLS.md`).
    Smart,
}

impl ControlScheme {
    pub const BOTH: [ControlScheme; 2] = [ControlScheme::Classic, ControlScheme::Smart];

    pub fn label(self) -> &'static str {
        match self {
            ControlScheme::Classic => "Classic",
            ControlScheme::Smart => "Smart",
        }
    }

    /// The scheme the controls code is to use. The one place to answer
    /// "is Smart on?": `controls.rs` and `assist` read it (with the smart
    /// options of [`Settings`]). It is `self` now; it stays a function so a
    /// platform that cannot do a scheme can still turn it down here.
    pub fn in_effect(self) -> ControlScheme {
        self
    }
}

/// The languages of the maps' texts: the `--lang` code (none for the
/// English of `Strings.resx`) and its name.
pub const LANGUAGES: [(Option<&str>, &str); 6] = [
    (None, "English"),
    (Some("ru"), "Russian"),
    (Some("de"), "German"),
    (Some("es"), "Spanish"),
    (Some("it"), "Italian"),
    (Some("be"), "Belarusian"),
];

/// Which settings a command-line option set for this run: they are not to
/// be saved over the stored ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Overridden {
    music: bool,
    language: bool,
    controls: bool,
}

/// What the player chose.
#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub control_scheme: ControlScheme,
    /// Smart controls: fire while a target is locked.
    pub auto_fire: bool,
    /// Smart controls: turn the gun on the locked target.
    pub auto_aim: bool,
    /// Smart controls: a double tap on the ground drives there.
    pub tap_to_go: bool,
    /// The level music (`M` switches it while playing).
    pub music: bool,
    /// The maps' texts' language (`--lang`); read at start, so a change
    /// shows from the next run.
    pub language: Option<String>,
    #[serde(skip)]
    overridden: Overridden,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            control_scheme: ControlScheme::Classic,
            auto_fire: true,
            auto_aim: true,
            tap_to_go: true,
            music: true,
            language: None,
            overridden: Overridden::default(),
        }
    }
}

impl Settings {
    /// Settings from stored text; anything unreadable gives the defaults
    /// (and a warning), as does an unknown scheme from a newer version.
    pub fn from_json(text: &str) -> Settings {
        serde_json::from_str(text).unwrap_or_else(|e| {
            warn!("settings: unreadable, using the defaults: {e}");
            Settings::default()
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Reads the stored settings: none stored, or a failing store, is the
    /// defaults.
    pub fn load(store: &dyn Store) -> Settings {
        match store.read() {
            Ok(Some(text)) => Settings::from_json(&text),
            Ok(None) => Settings::default(),
            Err(e) => {
                warn!("settings: could not be read, using the defaults: {e}");
                Settings::default()
            }
        }
    }

    /// The music is switched by the player: no longer a command line's.
    pub fn set_music(&mut self, on: bool) {
        self.music = on;
        self.overridden.music = false;
    }

    /// The language is chosen by the player: no longer a command line's.
    pub fn set_control_scheme(&mut self, scheme: ControlScheme) {
        self.control_scheme = scheme;
        self.overridden.controls = false;
    }

    pub fn set_language(&mut self, language: Option<String>) {
        self.language = language;
        self.overridden.language = false;
    }

    /// The next (`step` 1) or previous (-1) language of [`LANGUAGES`],
    /// wrapping; a language that is not in the list (a hand-edited file)
    /// goes to the first.
    pub fn cycle_language(&mut self, step: i32) {
        let n = LANGUAGES.len() as i32;
        let at = LANGUAGES.iter().position(|(code, _)| *code == self.language.as_deref()).map_or(-1, |i| i as i32);
        let next = if at < 0 { 0 } else { (at + step).rem_euclid(n) };
        self.set_language(LANGUAGES[next as usize].0.map(String::from));
    }

    pub fn language_name(&self) -> &str {
        LANGUAGES.iter().find(|(code, _)| *code == self.language.as_deref()).map_or("Other", |(_, name)| name)
    }

    /// What is to be written: these settings, but the ones a command line
    /// set for this run keep the stored value.
    pub fn to_store(&self, stored: &Settings) -> Settings {
        let mut out = self.clone();
        if self.overridden.music {
            out.music = stored.music;
        }
        if self.overridden.language {
            out.language = stored.language.clone();
        }
        if self.overridden.controls {
            out.control_scheme = stored.control_scheme;
        }
        out.overridden = Overridden::default();
        out
    }
}

/// Settings the command line (or the page's URL) gave: they win for this
/// run over the stored ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `--no-music`.
    pub music: Option<bool>,
    /// `--lang`: `Some(None)` is English.
    pub language: Option<Option<String>>,
    /// `--controls classic|smart`.
    pub controls: Option<ControlScheme>,
}

impl Overrides {
    pub fn apply(&self, settings: &mut Settings) {
        if let Some(music) = self.music {
            settings.music = music;
            settings.overridden.music = true;
        }
        if let Some(language) = &self.language {
            settings.language = language.clone();
            settings.overridden.language = true;
        }
        if let Some(scheme) = self.controls {
            settings.control_scheme = scheme;
            settings.overridden.controls = true;
        }
    }
}

/// Where the settings are kept.
pub trait Store: Send + Sync {
    /// The stored text, or `None` when nothing was stored yet.
    fn read(&self) -> io::Result<Option<String>>;
    fn write(&self, text: &str) -> io::Result<()>;
}

/// A file (native).
pub struct FileStore(pub PathBuf);

impl Store for FileStore {
    fn read(&self) -> io::Result<Option<String>> {
        match std::fs::read_to_string(&self.0) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(&self, text: &str) -> io::Result<()> {
        if let Some(dir) = self.0.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.0, text)
    }
}

/// `settings.json` in the platform's config folder, from the environment
/// (`var`: `std::env::var(..).ok()`): `$XDG_CONFIG_HOME`, else `%APPDATA%`
/// on Windows, else under `$HOME` (`Library/Application Support` on macOS,
/// `.config` elsewhere). `None` when none is set.
pub fn config_file(var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = set("XDG_CONFIG_HOME")
        .or_else(|| if cfg!(windows) { set("APPDATA") } else { None })
        .or_else(|| {
            set("HOME").map(|home| {
                if cfg!(target_os = "macos") { home.join("Library/Application Support") } else { home.join(".config") }
            })
        })?;
    Some(base.join("tilerider").join("settings.json"))
}

/// The browser's `localStorage` (a missing or blocked one reads as empty and
/// fails to write).
#[cfg(target_arch = "wasm32")]
pub struct LocalStorage;

#[cfg(target_arch = "wasm32")]
mod local_storage {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function get(k) { try { return window.localStorage.getItem(k); } catch (e) { return null; } }
export function set(k, v) { window.localStorage.setItem(k, v); }")]
    extern "C" {
        pub fn get(key: &str) -> Option<String>;
        #[wasm_bindgen(catch)]
        pub fn set(key: &str, value: &str) -> Result<(), JsValue>;
    }
}

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "tilerider.settings";

#[cfg(target_arch = "wasm32")]
impl Store for LocalStorage {
    fn read(&self) -> io::Result<Option<String>> {
        Ok(local_storage::get(STORAGE_KEY))
    }

    fn write(&self, text: &str) -> io::Result<()> {
        local_storage::set(STORAGE_KEY, text).map_err(|_| io::Error::other("localStorage is not available"))
    }
}

/// The platform's store; native without a config folder gets one that
/// stores nothing.
pub fn platform_store() -> Box<dyn Store> {
    #[cfg(target_arch = "wasm32")]
    {
        Box::new(LocalStorage)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match config_file(|name| std::env::var(name).ok()) {
            Some(path) => Box::new(FileStore(path)),
            None => Box::new(NoStore),
        }
    }
}

/// Nowhere to keep settings (no config folder).
pub struct NoStore;

impl Store for NoStore {
    fn read(&self) -> io::Result<Option<String>> {
        Ok(None)
    }

    fn write(&self, _text: &str) -> io::Result<()> {
        Err(io::Error::other("no folder to keep settings in"))
    }
}

/// Where the settings are written.
#[derive(Resource)]
pub struct SettingsStore(pub Box<dyn Store>);

/// What the store holds now, so a write happens only for a real change.
#[derive(Resource)]
struct Stored(Settings);

/// Loads the settings (with `overrides` on top), saves them when they
/// change, and keeps the level music and `M` in step with them.
pub struct SettingsPlugin {
    pub overrides: Overrides,
    store: std::sync::Mutex<Option<Box<dyn Store>>>,
}

impl SettingsPlugin {
    /// Settings kept in the platform's place.
    pub fn new(overrides: Overrides) -> Self {
        Self::with_store(overrides, platform_store())
    }

    pub fn with_store(overrides: Overrides, store: Box<dyn Store>) -> Self {
        SettingsPlugin { overrides, store: std::sync::Mutex::new(Some(store)) }
    }
}

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let store = self.store.lock().ok().and_then(|mut s| s.take()).unwrap_or_else(|| Box::new(NoStore));
        let stored = Settings::load(store.as_ref());
        let mut settings = stored.clone();
        self.overrides.apply(&mut settings);
        app.insert_resource(Language(settings.language.clone()))
            .insert_resource(settings)
            .insert_resource(Stored(stored))
            .insert_resource(SettingsStore(store))
            .add_systems(Update, (sync_music, save).chain());
    }
}

/// The settings' music and the jukebox agree: a change in the settings (the
/// menu) is pushed to the jukebox; otherwise (`M` was pressed) the settings
/// follow it.
fn sync_music(mut settings: ResMut<Settings>, jukebox: Option<ResMut<Jukebox>>) {
    let Some(mut jukebox) = jukebox else {
        return;
    };
    if jukebox.is_on() == settings.music {
        return;
    }
    if settings.is_changed() {
        if settings.music {
            jukebox.switch_on();
        } else {
            jukebox.switch_off();
        }
    } else {
        let on = jukebox.is_on();
        settings.set_music(on);
    }
}

/// Writes the settings when they differ from what is stored.
fn save(settings: Res<Settings>, mut stored: ResMut<Stored>, store: Res<SettingsStore>) {
    if !settings.is_changed() {
        return;
    }
    let want = settings.to_store(&stored.0);
    if want == stored.0 {
        return;
    }
    match store.0.write(&want.to_json()) {
        Ok(()) => stored.0 = want,
        Err(e) => warn!("settings: could not be saved: {e}"),
    }
}
