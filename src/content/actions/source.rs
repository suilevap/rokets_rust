//! Where the values a field may take come from, for an editor to offer
//! them: C#'s `[FieldReference(File, Path)]` (an XPath into the map or
//! another content file) and `[ReferenceTo(typeof(…))]` (the keys of the
//! map's items of a type, which the editor found by reflection).
//!
//! Both are one thing here, a [`Source`]: mostly a path into XML, so an
//! editor resolves any field the same way ([`Source::values`]) without
//! knowing what an entity or a trigger is. The common ones are named
//! below; a definition may also write its own path.

use std::path::Path;

use super::defs;
use crate::content::node::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The text of the elements at `path` in the map, or in `file` (from
    /// the content root). The path is a simplified XPath from the root:
    /// element names joined by `/`, `*` for any name, `A|B` for either.
    Xml {
        file: Option<&'static str>,
        path: &'static str,
    },
    /// Names of the files in a content folder with this extension, without
    /// it (the content project's sounds and music in C#).
    Files {
        dir: &'static str,
        ext: &'static str,
    },
    /// Action templates, by signature (`ActionTemplates.Templates`, which
    /// C# pointed at with `FieldReferenceToField`).
    Templates,
}

const fn map(path: &'static str) -> Source {
    Source::Xml { file: None, path }
}

/// Map actions, by name.
pub const ACTIONS: Source = map("XnaContent/Asset/Actions/Item/Name");
/// Triggers, by id.
pub const TRIGGERS: Source = map("XnaContent/Asset/Triggers/Item/Id");
/// Players' cars, by id.
pub const PLAYERS: Source = map("XnaContent/Asset/Players/Item/Id");
/// Other cars, by id.
pub const SIMPLE_CARS: Source = map("XnaContent/Asset/SimpleCars/Item/Id");
/// Entities, by id.
pub const ENTITIES: Source = map("XnaContent/Asset/Entities/Item/Id");
/// Hints, by id.
pub const HINTS: Source = map("XnaContent/Asset/Hints/Item/Id");
/// Groups used anywhere in the map (`MapData.GroupPath`).
pub const GROUPS: Source = map("XnaContent/Asset/*/Item/Groups/Item");
/// The map's variables (`MapData.VariablePath`).
pub const VARIABLES: Source = map("XnaContent/Asset/Actions/Item/VariableName");
/// Splashes (`MapData.SplashPath`).
pub const SPLASHES: Source = Source::Xml {
    file: Some("Splashes/Splashes.xml"),
    path: "XnaContent/Asset/Splashes/Item/Name",
};
/// Sounds (`MapData.SoundPath`).
pub const SOUNDS: Source = Source::Files { dir: "Sounds", ext: "wav" };

/// An action: one of the map's, or a template.
pub const ACTION: &[Source] = &[ACTIONS, Source::Templates];
/// Something that moves or can be hit: a player, a car or an entity.
pub const ITEM: &[Source] = &[PLAYERS, SIMPLE_CARS, ENTITIES];

impl Source {
    /// The values found: in `map` (the map's root element), or in content
    /// under `content`.
    pub fn values(&self, map: &Node, content: &Path) -> Vec<String> {
        match *self {
            Source::Xml { file: None, path } => select(map, path),
            Source::Xml { file: Some(file), path } => std::fs::read_to_string(content.join(file))
                .ok()
                .and_then(|xml| Node::parse(&xml).ok())
                .map(|root| select(&root, path))
                .unwrap_or_default(),
            Source::Files { dir, ext } => {
                let mut names: Vec<String> = std::fs::read_dir(content.join(dir))
                    .into_iter()
                    .flatten()
                    .filter_map(|e| {
                        let path = e.ok()?.path();
                        if path.extension()? != ext {
                            return None;
                        }
                        Some(path.file_stem()?.to_string_lossy().into_owned())
                    })
                    .collect();
                names.sort();
                names
            }
            Source::Templates => defs::ALL.iter().flat_map(|d| (d.templates)()).map(String::from).collect(),
        }
    }
}

/// The text of the elements at `path` under `root` (whose own name is the
/// path's first step), in document order, each value once.
pub fn select(root: &Node, path: &str) -> Vec<String> {
    let mut steps = path.split('/').filter(|s| !s.is_empty());
    let matches = |step: &str, name: &str| step == "*" || step.split('|').any(|s| s == name);
    let mut found = match steps.next() {
        Some(first) if matches(first, &root.name) => vec![root],
        _ => return Vec::new(),
    };
    for step in steps {
        found = found.iter().flat_map(|n| n.children.iter().filter(|c| matches(step, &c.name))).collect();
    }
    let mut out: Vec<String> = Vec::new();
    for n in found {
        let text = n.text.trim();
        if !text.is_empty() && !out.iter().any(|o| o == text) {
            out.push(text.to_string());
        }
    }
    out
}
