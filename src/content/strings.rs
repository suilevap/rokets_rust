//! The game's texts by key (`Globalization/Strings*.resx`, imported to
//! `assets/Strings/`): what `ShowText` and the like name.

use std::collections::HashMap;

use super::node::Node;

#[derive(Clone, Debug, Default)]
pub struct Strings(HashMap<String, String>);

impl Strings {
    /// Reads a `.resx`: `<data name="Key"><value>Text</value></data>`.
    pub fn parse(xml: &str) -> Result<Strings, String> {
        let root = Node::parse(xml)?;
        let pairs = root.children_named("data").filter_map(|d| {
            let value = d.child("value")?.text.clone();
            Some((d.attr("name")?.to_string(), value))
        });
        Ok(Strings(pairs.collect()))
    }

    /// These texts over `base`: a language falls back to the default
    /// texts for keys it lacks (.NET resource fallback).
    pub fn over(self, mut base: Strings) -> Strings {
        base.0.extend(self.0);
        base
    }

    /// The text of a key, or the key marked as missing (`StringsByStringKeys`
    /// wrote its warning prefix).
    pub fn get(&self, key: &str) -> String {
        self.0.get(key).cloned().unwrap_or_else(|| format!("#{key}"))
    }
}
