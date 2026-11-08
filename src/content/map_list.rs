//! `GameLibrary.MapListData`: `assets/Maps/MapList.xml`, the campaign order.

use serde::Deserialize;

use super::xna::Items;
use super::{ContentError, parse_asset};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MapList {
    #[serde(default)]
    pub singleplayer: Items<String>,
}

impl MapList {
    pub fn parse(xml: &str) -> Result<Self, ContentError> {
        parse_asset(xml)
    }

    /// The map after `name` in the campaign, if any.
    pub fn next_after(&self, name: &str) -> Option<&str> {
        let maps = &self.singleplayer.items;
        let i = maps.iter().position(|m| m.eq_ignore_ascii_case(name))?;
        maps.get(i + 1).map(String::as_str)
    }
}
