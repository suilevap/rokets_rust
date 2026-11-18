//! Action references.

use super::*;

#[test]
fn parses_refs() {
    assert_eq!(
        ActionRef::parse("ShowSuccess"),
        ActionRef {
            name: "ShowSuccess".into(),
            args: None
        }
    );
    assert_eq!(ActionRef::parse("NextMap()").args, Some(vec![]));
    assert_eq!(
        ActionRef::parse("CaptureScreen(0, 0.2)").args,
        Some(vec!["0".into(), "0.2".into()])
    );
}
