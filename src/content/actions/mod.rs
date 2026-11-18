//! A map's actions as data, for the action runner (`crate::actions`).
//!
//! - [`ActionRef`]: how triggers and other actions name an action: a map
//!   action (`ShowSuccess`) or a built-in template with arguments
//!   (`NextMap()`, `CloseDoor(1001)`, `CaptureScreen(0,0.2)`,
//!   `GameLibrary/ActionTemplates.cs`).
//! - [`MapActions`]: the map's `<Actions>`, each kept with its own element
//!   so that every action type reads its own fields when it runs; the
//!   runner only needs the common ones ([`def::Base`]).
//! - [`TemplateArgs`]: a template's arguments.
//! - [`def`]: how an action type is defined, with what an editor needs of
//!   its fields; [`defs`]: the types; [`source`]: where a field's values
//!   come from.

pub mod def;
pub mod defs;
pub mod source;

use super::ContentError;
use super::node::Node;
use def::Base;
pub use def::{ActionDef, DefInfo, Templates};

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
        self.text(i)
            .map(|t| t.split_whitespace().map(String::from).collect())
            .unwrap_or_default()
    }
}

/// One map action: what every action type has, and its element.
#[derive(Clone, Debug, PartialEq)]
pub struct RawAction {
    /// The C# class without `GameLibrary.`: `ActivateEntityActionData`.
    pub kind: String,
    pub base: Base,
    /// The whole `<Item Type="…">…</Item>`, for the type to read.
    pub node: Node,
}

impl RawAction {
    pub fn read(node: Node) -> Result<RawAction, String> {
        let kind = node.attr("Type").unwrap_or_default();
        // `GameLibrary.X`, or `GameLibrary:X` with a namespace prefix.
        let kind = kind.rsplit(['.', ':']).next().unwrap_or_default().to_string();
        let base = Base::read(&node, defs::find(&kind))?;
        Ok(RawAction { kind, base, node })
    }

    pub fn name(&self) -> &str {
        &self.base.name
    }
}

/// The map's `<Actions>`, in order.
#[derive(Clone, Debug, Default)]
pub struct MapActions(pub Vec<RawAction>);

impl MapActions {
    /// Reads the actions of a map's XML (`XnaContent/Asset/Actions/Item`).
    pub fn parse(xml: &str) -> Result<MapActions, ContentError> {
        let root = Node::parse(xml).map_err(ContentError::Action)?;
        let items = root
            .child("Asset")
            .and_then(|a| a.child("Actions"))
            .map(|a| a.children_named("Item").cloned().collect())
            .unwrap_or_else(Vec::new);
        let actions = items
            .into_iter()
            .map(RawAction::read)
            .collect::<Result<_, _>>();
        Ok(MapActions(actions.map_err(ContentError::Action)?))
    }

    /// The actions of a name, in order (several may share one).
    pub fn named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a RawAction> + 'a {
        self.0.iter().filter(move |a| a.base.name == name)
    }
}

#[cfg(test)]
mod tests;
