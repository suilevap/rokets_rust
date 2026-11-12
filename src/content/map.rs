//! `GameLibrary.MapData`: one map XML (`assets/Maps/Singleplayer/*.xml`).
//!
//! Every section is read, including those the game does not run yet, so
//! that unsupported content can be reported instead of silently dropped.
//! Positions are in map cells (tile-centred: cell 24,30 is `24.5 30.5`);
//! directions are degrees, 0 = right, 90 = down (the XML's y axis points
//! down).

use serde::Deserialize;

use super::xna::{Items, XColor, XRect, XVec2, boolean, int_list};
use super::{ContentError, parse_asset};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MapData {
    #[serde(default)]
    pub name: String,
    /// Content path of the map image (`Maps\Singleplayer\Middleware`), under
    /// `Sprites/`. Empty for maps made only of a `CharMap`.
    #[serde(default)]
    pub image: Option<String>,
    /// Pixels per map cell (the C# physics works in 1/32 of that).
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Map image alpha at or above which a cell is a wall.
    #[serde(default = "default_wall_threshold")]
    pub image_wall_treshold: u8,
    #[serde(default, deserialize_with = "boolean")]
    pub have_outland_vizualization: bool,
    /// `MapEffectFactory`: dirty glass is the default; snow has its own
    /// weather overlay rather than the dirty-lens effect.
    #[serde(default)]
    pub level_effect: LevelEffect,
    /// The outland's colours at its outer corners (left-top, right-top,
    /// right-bottom, left-bottom); C# defaults when a map leaves them out.
    #[serde(default = "default_outland_lt")]
    pub outland_color_l_t: XColor,
    #[serde(default = "default_outland_rt")]
    pub outland_color_r_t: XColor,
    #[serde(default = "default_outland_rb")]
    pub outland_color_r_b: XColor,
    #[serde(default = "default_outland_lb")]
    pub outland_color_l_b: XColor,
    #[serde(default)]
    pub friction: Option<f32>,
    #[serde(default = "default_ambient")]
    pub ambient_light: XColor,
    #[serde(default)]
    pub lights: Items<LightData>,
    #[serde(default)]
    pub players: Items<PlayerData>,
    #[serde(default)]
    pub simple_cars: Items<SimpleCarData>,
    #[serde(default)]
    pub entities: Items<EntityData>,
    #[serde(default)]
    pub triggers: Items<TriggerData>,
    #[serde(default)]
    pub actions: Items<ActionData>,
    #[serde(default)]
    pub char_map_dictionary: Option<String>,
    #[serde(default)]
    pub char_map: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum LevelEffect {
    #[default]
    None,
    Snow,
    SnowWind,
    #[serde(other)]
    Other,
}

/// `MapData.OutlandColor*` defaults: Black, DeepSkyBlue, Orange, Yellow.
fn default_outland_lt() -> XColor {
    XColor::BLACK
}
fn default_outland_rt() -> XColor {
    XColor::rgba(0, 191, 255, 255)
}
fn default_outland_rb() -> XColor {
    XColor::rgba(255, 165, 0, 255)
}
fn default_outland_lb() -> XColor {
    XColor::rgba(255, 255, 0, 255)
}

fn default_scale() -> f32 {
    48.0
}
fn default_wall_threshold() -> u8 {
    96
}
/// `Color.White * 0.05f` in XNA scales every channel, alpha included.
fn default_ambient() -> XColor {
    XColor::rgba(13, 13, 13, 13)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LightData {
    pub position: XVec2,
    pub color: XColor,
    /// Radius in cells.
    pub r: f32,
    #[serde(default, deserialize_with = "boolean")]
    pub use_for_dynamic_shadow: bool,
    #[serde(default)]
    pub additive_rate: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayerData {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub position: XVec2,
    #[serde(default)]
    pub direction_deg: f32,
    /// `Local`, `NoneLocal`, `Ai…`: who drives it.
    #[serde(default)]
    pub control: String,
    #[serde(default = "default_player_car")]
    pub car_name: String,
    #[serde(default, deserialize_with = "boolean")]
    pub is_prototype: bool,
}

fn default_player_car() -> String {
    "SandBuggyNoGadget".into()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SimpleCarData {
    pub id: i64,
    pub position: XVec2,
    #[serde(default)]
    pub control: String,
    #[serde(default)]
    pub car_name: String,
    #[serde(default)]
    pub direction_deg: f32,
    #[serde(default, deserialize_with = "boolean")]
    pub is_prototype: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EntityData {
    pub id: i64,
    pub position: XVec2,
    /// Content path under `Sprites/` (`Entities\CargoQuest`).
    #[serde(default)]
    pub sprite: String,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub direction_deg: f32,
    /// What the entity is; `Box` when absent.
    #[serde(default, rename = "Type")]
    pub kind: EntityType,
    #[serde(default = "one")]
    pub probability: f32,
    #[serde(default, deserialize_with = "boolean")]
    pub try_cast_static_shadow: bool,
    /// Lock the entity's x / y position or its rotation; all three fix it.
    #[serde(default, deserialize_with = "boolean")]
    pub restrict_x: bool,
    #[serde(default, deserialize_with = "boolean")]
    pub restrict_y: bool,
    #[serde(default, deserialize_with = "boolean")]
    pub restrict_rotation: bool,
}

fn one() -> f32 {
    1.0
}
/// `GameLibrary.MapDataInternal.EntityType`: what a map entity is.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityType {
    /// A pushable box that can be towed.
    #[default]
    Box,
    NonLinkableBox,
    /// A box 64 times heavier while nothing tows it.
    HeavyBox,
    Bomb,
    /// Decor drawn flat on the floor (finish markings, signs).
    Tile,
    /// A gear to pick up.
    Coin,
    /// A box that blows up when hit.
    DamagableBox,
    /// A box bullets pass over.
    IgnoreWeaponBox,
    /// A pad that throws whatever touches it.
    Accelerator,
    /// A gear that moves (a physics body).
    CoinDynamic,
    /// A type this version does not know.
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TriggerData {
    pub id: i64,
    /// Area in cells.
    pub area: XRect,
    /// How many times it fires.
    #[serde(default)]
    pub amount: i64,
    #[serde(default, deserialize_with = "boolean")]
    pub active: bool,
    /// Action references run on enter: a name, or a template like `NextMap()`.
    #[serde(default)]
    pub actions: Items<String>,
    #[serde(default)]
    pub leave_actions: Items<String>,
    /// Ids of the entities that set it off; empty means the player.
    #[serde(default, deserialize_with = "int_list")]
    pub subscribers: Vec<i64>,
}

/// One entry of the map's `<Actions>`: an `ActionData` subclass. Only the
/// common fields are read for now; the type says what it is.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActionData {
    /// `GameLibrary.ShowSuccess`, …
    #[serde(rename = "@Type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "boolean")]
    pub run_on_map_start: bool,
    /// For `ActionSequence`: the names of the actions it runs.
    #[serde(default)]
    pub actions: Option<Items<String>>,
}

impl ActionData {
    /// The type name without the `GameLibrary.` namespace.
    pub fn short_kind(&self) -> &str {
        self.kind.rsplit('.').next().unwrap_or(&self.kind)
    }
}

impl MapData {
    pub fn parse(xml: &str) -> Result<Self, ContentError> {
        parse_asset(xml)
    }
}
