//! `GameLibrary.SplashesData`: `assets/Splashes/Splashes.xml`, the layouts
//! of the full-screen overlays `ShowSplash` names (C#'s
//! `Content/Splashes/Splashes.xml`). A splash is a stack of layers, each a
//! sprite, a text by key, or a bound text slot, placed by an offset in
//! the 800 × 480 screen of the C# game.
//!
//! Not ported: nothing is dropped from the format; every field of
//! `SplashData` and `LayerData` is read (the editor-only `SplashFakeData`
//! wrappers are not). What the game does with a field is in `splash`.

use serde::Deserialize;

use super::xna::{Items, Tint, XVec2, boolean, premultiplied_tint};
use super::{ContentError, parse_asset};

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

fn unset() -> i32 {
    -1
}

fn minus_one() -> f32 {
    -1.0
}

fn translation() -> f32 {
    2.5
}

fn clickable_after() -> f32 {
    3.0
}

/// A file of splashes.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SplashesData {
    #[serde(default)]
    pub splashes: Items<SplashData>,
}

/// `SplashesDataInternal.SplashData`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SplashData {
    pub name: String,
    /// The white panel with the menu's foreground picture behind the layers.
    #[serde(default = "yes", deserialize_with = "boolean")]
    pub draw_background: bool,
    /// Appears and hides through black (the menus' look) instead of fading.
    #[serde(default = "yes", deserialize_with = "boolean")]
    pub appear_fade_to_black: bool,
    /// Seconds to fade in, and again to fade out.
    #[serde(default = "translation")]
    pub translation_time: f32,
    #[serde(default, deserialize_with = "boolean")]
    pub auto_hide: bool,
    /// Keeps its clock running while the game is paused.
    #[serde(default = "yes", deserialize_with = "boolean")]
    pub ignore_pause: bool,
    /// How far the layers sway (16 px × this, at layer height 1).
    #[serde(default = "one")]
    pub parallax_amount: f32,
    #[serde(default = "clickable_after")]
    pub show_clickable_indicator_after_sec: f32,
    /// Layers' sizes and offsets grow with the screen.
    #[serde(default, deserialize_with = "boolean")]
    pub scale_up_when_needed: bool,
    /// Only the offsets grow, the sprites and texts keep their size.
    #[serde(default, deserialize_with = "boolean")]
    pub scale_up_only_offset_when_needed: bool,
    #[serde(default)]
    pub layers: Items<LayerData>,
}

/// `SplashesDataInternal.LayerData`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayerData {
    pub name: Option<String>,
    /// A sprite under `Sprites/` (empty: none).
    pub sprite: Option<String>,
    /// A key of the game's texts.
    pub text: Option<String>,
    /// A slot of the bound texts (`SplashBindText`), -1 for none.
    #[serde(default = "unset")]
    pub bind_text_id: i32,
    /// Top left corner in the 800 × 480 screen.
    #[serde(default)]
    pub offset: XVec2,
    #[serde(default, deserialize_with = "boolean")]
    pub stretch_to_screen: bool,
    /// Straight; a premultiplied colour of C# is converted when read.
    #[serde(default, deserialize_with = "premultiplied_tint")]
    pub color: Tint,
    /// Does not sway.
    #[serde(default, deserialize_with = "boolean", rename = "Static")]
    pub is_static: bool,
    /// Drawn over the layers that are not.
    #[serde(default, deserialize_with = "boolean")]
    pub on_foreground: bool,
    /// Seconds after the splash starts to show.
    #[serde(default)]
    pub delay_sec: f32,
    pub sound: Option<String>,
    /// Sway and draw order: more is nearer; -1: by position in the list.
    #[serde(default = "minus_one")]
    pub height: f32,
    #[serde(default = "one")]
    pub scale: f32,
    /// Counter-clockwise on the screen.
    #[serde(default)]
    pub direction_rad: f32,
}

impl SplashesData {
    pub fn parse(xml: &str) -> Result<Self, ContentError> {
        parse_asset(xml)
    }

    /// The splash of exactly this name (`First(x => x.Name == name)`).
    pub fn get(&self, name: &str) -> Option<&SplashData> {
        self.splashes.iter().find(|s| s.name == name)
    }

    /// The splash `ShowSplash` shows (`SplashScreen.GetSplashData`): the
    /// variant for the last control used (`_Win` for keyboard and mouse,
    /// `_GamePad` for a gamepad, none for touch), else the plain one.
    pub fn variant(&self, name: &str, suffix: &str) -> Option<&SplashData> {
        self.get(&format!("{name}{suffix}")).or_else(|| self.get(name))
    }
}

impl LayerData {
    /// The sprite's content name, if it names one.
    pub fn sprite_name(&self) -> Option<&str> {
        self.sprite.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }

    /// The text key, if it names one.
    pub fn text_key(&self) -> Option<&str> {
        self.text.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }
}
