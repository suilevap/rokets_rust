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
    #[serde(default)]
    pub outland_color_l_t: Option<XColor>,
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
    /// `Box`, `Coin`, `Tile`, … (`EntityType` in C#); `Box` when absent.
    #[serde(default = "default_entity_type", rename = "Type")]
    pub kind: String,
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
fn default_entity_type() -> String {
    "Box".into()
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<XnaContent>
  <Asset Type="GameLibrary.MapData">
    <Name>Sample</Name>
    <Image>Maps\Singleplayer\Sample</Image>
    <Scale>64</Scale>
    <ImageWallTreshold>96</ImageWallTreshold>
    <AmbientLight>0C0C0C0C</AmbientLight>
    <Lights>
      <Item><Position>8.5 3.5</Position><Color>FFFFFF00</Color><R>12</R></Item>
    </Lights>
    <Players>
      <Item><Id>4001</Id><Name>Player1</Name><Position>9.5 3.5</Position>
        <DirectionDeg>90</DirectionDeg><Control>Local</Control><CarName>SandBuggy</CarName></Item>
    </Players>
    <Entities>
      <Item><Id>1001</Id><Position>5.5 3.5</Position><Sprite>Entities\Coin</Sprite>
        <Animated>false</Animated><Scale>1</Scale><Type>Coin</Type>
        <ActionOnCreate Null="true" /></Item>
    </Entities>
    <Triggers>
      <Item><Id>3</Id><Area>32 17 3 2</Area><Amount>999</Amount><Active>true</Active>
        <Actions><Item>NextMap()</Item></Actions><Subscribers>4001 1004</Subscribers></Item>
    </Triggers>
    <Actions>
      <Item Type="GameLibrary.ShowSuccess"><Name>ShowSuccess</Name><RunOnMapStart>false</RunOnMapStart></Item>
    </Actions>
  </Asset>
</XnaContent>"#;

    #[test]
    fn parses_sample() {
        let map = MapData::parse(SAMPLE).unwrap();
        assert_eq!(map.name, "Sample");
        assert_eq!(map.scale, 64.0);
        assert_eq!(map.ambient_light, XColor::rgba(12, 12, 12, 12));
        assert_eq!(map.lights.items[0].color, XColor::rgba(255, 255, 0, 255));
        assert_eq!(map.players.items[0].direction_deg, 90.0);
        assert_eq!(map.entities.items[0].kind, "Coin");
        let trigger = &map.triggers.items[0];
        assert_eq!(trigger.actions.items, vec!["NextMap()".to_string()]);
        assert_eq!(trigger.subscribers, vec![4001, 1004]);
        assert_eq!(map.actions.items[0].short_kind(), "ShowSuccess");
    }
}
