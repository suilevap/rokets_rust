//! A map's actions as data, for the action runner (`crate::actions`).
//!
//! - [`ActionRef`]: how triggers and other actions name an action: a map
//!   action (`ShowSuccess`) or a built-in template with arguments
//!   (`NextMap()`, `CloseDoor(1001)`, `CaptureScreen(0,0.2)`,
//!   `GameLibrary/ActionTemplates.cs`).
//! - [`MapActions`]: the map's `<Actions>`, each kept with its own XML so
//!   that every action type reads its own fields when it runs; the runner
//!   only needs the common ones (type, name, `NextAction`,
//!   `RunOnMapStart`).
//! - [`TemplateArgs`]: a template's arguments.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Deserialize;

use super::ContentError;
use super::xna::boolean;

#[derive(Clone, Debug, PartialEq)]
pub struct ActionRef {
    pub name: String,
    /// `None` without parentheses; `Some(vec![])` for `Name()`.
    pub args: Option<Vec<String>>,
}

impl ActionRef {
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        if let Some(open) = s.find('(')
            && s.ends_with(')')
        {
            let name = s[..open].trim().to_string();
            let inner = &s[open + 1..s.len() - 1];
            let args = if inner.trim().is_empty() {
                vec![]
            } else {
                inner.split(',').map(|a| a.trim().to_string()).collect()
            };
            return ActionRef {
                name,
                args: Some(args),
            };
        }
        ActionRef {
            name: s.to_string(),
            args: None,
        }
    }
}

/// A template's arguments, read by position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateArgs(pub Vec<String>);

impl TemplateArgs {
    pub fn text(&self, i: usize) -> Option<&str> {
        self.0.get(i).map(String::as_str)
    }

    pub fn int(&self, i: usize) -> Option<i64> {
        self.text(i)?.parse().ok()
    }

    pub fn num(&self, i: usize) -> Option<f32> {
        self.text(i)?.parse().ok()
    }

    /// A list written with spaces (`Seq(A B C, 1)`).
    pub fn words(&self, i: usize) -> Vec<String> {
        self.text(i).map(|t| t.split_whitespace().map(String::from).collect()).unwrap_or_default()
    }
}

/// One map action: what every action type has, and its XML.
#[derive(Clone, Debug, PartialEq)]
pub struct RawAction {
    /// The C# class without `GameLibrary.`: `ActivateEntityActionData`.
    pub kind: String,
    pub name: String,
    pub next: Option<String>,
    pub run_on_map_start: bool,
    /// The whole `<Item Type="…">…</Item>`, for the type to read.
    pub xml: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Common {
    #[serde(rename = "@Type", default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    next_action: Option<String>,
    #[serde(default, deserialize_with = "boolean")]
    run_on_map_start: bool,
}

/// The map's `<Actions>`, in order.
#[derive(Clone, Debug, Default)]
pub struct MapActions(pub Vec<RawAction>);

impl MapActions {
    /// Reads the actions of a map's XML (`XnaContent/Asset/Actions/Item`).
    pub fn parse(xml: &str) -> Result<MapActions, ContentError> {
        // Offsets are into the text after a byte-order mark.
        let xml = xml.trim_start_matches('\u{feff}');
        let mut reader = Reader::from_str(xml);
        let mut depth = 0;
        let mut in_actions = false;
        let mut item_start = None;
        let mut out = Vec::new();
        let err = |e: quick_xml::Error| ContentError::Xml(e.into());
        loop {
            let before = reader.buffer_position() as usize;
            match reader.read_event().map_err(err)? {
                Event::Start(e) => {
                    depth += 1;
                    if depth == 3 && e.name().as_ref() == "Actions" {
                        in_actions = true;
                    } else if in_actions && depth == 4 && e.name().as_ref() == "Item" {
                        item_start = Some(before);
                    }
                }
                Event::Empty(e) if in_actions && depth == 3 && e.name().as_ref() == "Item" => {
                    out.push(&xml[before..reader.buffer_position() as usize]);
                }
                Event::End(e) => {
                    if in_actions && depth == 4 && e.name().as_ref() == "Item"
                        && let Some(start) = item_start.take()
                    {
                        out.push(&xml[start..reader.buffer_position() as usize]);
                    }
                    if depth == 3 && e.name().as_ref() == "Actions" {
                        in_actions = false;
                    }
                    depth -= 1;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let actions = out
            .into_iter()
            .map(|xml| {
                let c: Common = quick_xml::de::from_str(xml)?;
                Ok(RawAction {
                    kind: c.kind.rsplit('.').next().unwrap_or_default().to_string(),
                    name: c.name,
                    next: c.next_action.filter(|n| !n.trim().is_empty()),
                    run_on_map_start: c.run_on_map_start,
                    xml: xml.to_string(),
                })
            })
            .collect::<Result<_, ContentError>>()?;
        Ok(MapActions(actions))
    }

    /// The actions of a name, in order (several may share one).
    pub fn named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a RawAction> + 'a {
        self.0.iter().filter(move |a| a.name == name)
    }
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
