//! A small XML tree, for content that is read and written back as it is
//! (map actions, for the game and a level editor). C# had XNA's
//! `IntermediateSerializer` for both; this is the part of it the port
//! needs.
//!
//! An element is its name, attributes, text and child elements; text
//! between child elements (indentation) is dropped.

use quick_xml::Reader;
use quick_xml::escape::escape;
use quick_xml::events::{BytesStart, Event};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    /// The element's own text (not its children's).
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(name: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            ..Node::default()
        }
    }

    pub fn with_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Node {
            text: text.into(),
            ..Node::new(name)
        }
    }

    /// Reads the document's root element.
    pub fn parse(xml: &str) -> Result<Node, String> {
        let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
        let mut stack: Vec<Node> = Vec::new();
        loop {
            match reader.read_event().map_err(|e| e.to_string())? {
                Event::Start(e) => stack.push(open(&e)?),
                Event::Empty(e) => {
                    let node = open(&e)?;
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => return Ok(node),
                    }
                }
                Event::End(_) => {
                    let mut node = stack.pop().ok_or("unbalanced XML")?;
                    if !node.children.is_empty() && node.text.trim().is_empty() {
                        node.text.clear();
                    }
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => return Ok(node),
                    }
                }
                Event::Text(t) => {
                    if let Some(node) = stack.last_mut() {
                        node.text.push_str(&t.xml10_content());
                    }
                }
                Event::GeneralRef(r) => {
                    if let Some(node) = stack.last_mut() {
                        let name: &str = r.as_ref();
                        let resolved = match name {
                            "lt" => "<".to_string(),
                            "gt" => ">".to_string(),
                            "amp" => "&".to_string(),
                            "apos" => "'".to_string(),
                            "quot" => "\"".to_string(),
                            _ => r
                                .resolve_char_ref()
                                .ok()
                                .flatten()
                                .map(String::from)
                                .ok_or("unknown entity")?,
                        };
                        node.text.push_str(&resolved);
                    }
                }
                Event::CData(t) => {
                    if let Some(node) = stack.last_mut() {
                        node.text.push_str(&t.xml10_content());
                    }
                }
                Event::Eof => return Err("no root element".into()),
                _ => {}
            }
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => *v = value,
            None => self.attrs.push((name.to_string(), value)),
        }
    }

    /// `Null="true"`: XNA's null reference.
    pub fn is_null(&self) -> bool {
        self.attr("Null") == Some("true")
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// The child elements of a name, in order.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The element as XML, indented two spaces a level as XNA writes it.
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        out.push_str(&indent);
        out.push('<');
        out.push_str(&self.name);
        for (k, v) in &self.attrs {
            out.push_str(&format!(" {k}=\"{}\"", escape(v.as_str())));
        }
        if self.children.is_empty() && self.text.is_empty() {
            out.push_str(" />\n");
            return;
        }
        out.push('>');
        if self.children.is_empty() {
            out.push_str(&escape(self.text.as_str()));
        } else {
            out.push('\n');
            for c in &self.children {
                c.write(out, depth + 1);
            }
            out.push_str(&indent);
        }
        out.push_str(&format!("</{}>\n", self.name));
    }
}

fn open(e: &BytesStart) -> Result<Node, String> {
    let mut node = Node::new(e.name().as_ref());
    for a in e.attributes() {
        let a = a.map_err(|e| e.to_string())?;
        let value = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?;
        node.attrs
            .push((a.key.as_ref().to_string(), value.into_owned()));
    }
    Ok(node)
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
