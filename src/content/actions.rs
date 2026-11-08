//! References to actions, as triggers and other actions write them:
//! `ShowSuccess`, `NextMap()`, `CloseDoor(1001)`, `CaptureScreen(0,0.2)`.
//!
//! A reference names an action from the map's `<Actions>` or a built-in
//! template (`GameLibrary/ActionTemplates.cs`); the arguments fill the
//! template.

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

#[cfg(test)]
mod tests {
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
}
