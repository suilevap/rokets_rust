//! The XML tree.

use super::*;

#[test]
fn reads_and_writes_back() {
    let xml = "\u{feff}<Item Type=\"GameLibrary.X\">\n  <Name>a &amp; b</Name>\n  <Path Null=\"true\" />\n  <List>\n    <Item>1</Item>\n  </List>\n</Item>\n";
    let node = Node::parse(xml).unwrap();
    assert_eq!(node.attr("Type"), Some("GameLibrary.X"));
    assert_eq!(node.child("Name").unwrap().text, "a & b");
    assert!(node.child("Path").unwrap().is_null());
    assert_eq!(
        node.child("List").unwrap().children_named("Item").count(),
        1
    );
    assert_eq!(node.to_xml(), xml.trim_start_matches('\u{feff}'));
}
