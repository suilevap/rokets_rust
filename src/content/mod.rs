//! The C# game's content formats, parsed without Bevy: map XML, the car
//! catalogue, the campaign list and action references.

pub mod actions;
pub mod cars;
pub mod map;
pub mod map_list;
pub mod node;
pub mod splashes;
pub mod strings;
pub mod xna;

use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("XML: {0}")]
    Xml(#[from] quick_xml::DeError),
    #[error("action: {0}")]
    Action(String),
}

#[derive(Deserialize)]
struct XnaContent<T> {
    #[serde(rename = "Asset")]
    asset: T,
}

/// Reads `<XnaContent><Asset Type="...">…</Asset></XnaContent>`.
pub fn parse_asset<T: DeserializeOwned>(xml: &str) -> Result<T, ContentError> {
    let content: XnaContent<T> = quick_xml::de::from_str(xml.trim_start_matches('\u{feff}'))?;
    Ok(content.asset)
}
