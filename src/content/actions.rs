//! References to actions, as triggers and other actions write them:
//! `ShowSuccess`, `NextMap()`, `CloseDoor(1001)`, `CaptureScreen(0,0.2)`.
//!
//! A reference names an action from the map's `<Actions>` or a built-in
//! template (`GameLibrary/ActionTemplates.cs`); the arguments fill the
//! template. [`resolve`] turns a reference into what it does ([`Act`]).

use super::map::{ActionData, MapData};

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

/// What an action does, as far as the port knows it. Entity ids are the
/// map's (`CustomId`); 0 or less means the entity that set it off.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// `ActionSequence`, `Seq(a b c, t)`: run these, `step` seconds apart.
    Sequence { actions: Vec<String>, step: f32 },
    /// `ActivateEntityActionData`, `ActivateEntity(id)`, `DeactivateEntity(id)`.
    SetActive { item: i64, active: bool },
    /// `DestroyEntityActionData`, `DestroyEntity(id)`.
    Destroy { item: i64 },
    /// `ActivateTriggerActionData`, `ActivateTrigger(id)`, `DeactivateTrigger(id)`.
    SetTriggers { ids: Vec<i64>, active: bool },
    /// `ApplyImpulseActionData`, `Impulse(id, x, y)`: a change of speed,
    /// in C# units (y down), plus up to `random` of it at random.
    Impulse { item: i64, impulse: [f32; 2], random: [f32; 2] },
    /// `DamageEntity`, `Damage(id, n)`: negative heals; the blow comes
    /// from `direction` C# degrees.
    Damage { item: i64, amount: i64, direction: f32 },
    /// `MoveToNextMap`, `ShowSuccess`, `NextMap()`, `ShowSuccess()`.
    LevelComplete,
    /// `RestartMap`, `RestartMap()`.
    Restart,
    /// Nothing to show here: saves, screenshots, achievements, stats.
    Nothing,
    /// Not ported yet: the action's type or template name.
    Unsupported(String),
}

/// An action and what runs after it (`NextAction`).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub act: Act,
    pub next: Option<String>,
}

/// Templates that do nothing visible in the port.
const SILENT: [&str; 6] = ["Save", "SaveWithActionOnRespawn", "CaptureScreen", "CaptureScreenForced", "UnlockAch", "SetStat"];

/// What a reference runs: a built-in template if it has parentheses,
/// else every map action of that name (`TriggerManager.RunAction`).
pub fn resolve(map: &MapData, reference: &str) -> Vec<Resolved> {
    let r = ActionRef::parse(reference);
    if let Some(args) = &r.args {
        return vec![Resolved {
            act: template(&r.name, args),
            next: None,
        }];
    }
    map.actions
        .iter()
        .filter(|a| a.name.as_deref() == Some(r.name.as_str()))
        .map(|a| Resolved {
            act: of_data(a),
            next: a.next_action.clone().filter(|n| !n.trim().is_empty()),
        })
        .collect()
}

fn of_data(a: &ActionData) -> Act {
    match a.short_kind() {
        "ActionSequence" => Act::Sequence {
            actions: a.actions.as_ref().map(|i| i.items.clone()).unwrap_or_default(),
            step: a.time_between_in_sec,
        },
        "ActivateEntityActionData" => Act::SetActive {
            item: a.item_id,
            active: !a.deactivate,
        },
        "DestroyEntityActionData" => Act::Destroy { item: a.item_id },
        "ActivateTriggerActionData" => Act::SetTriggers {
            ids: a.trigger_ids.clone(),
            active: !a.deactivate,
        },
        "ApplyImpulseActionData" => Act::Impulse {
            item: a.item_id,
            impulse: a.impulse.map_or([0.0; 2], |v| [v.x, v.y]),
            random: a.random_impulse.map_or([0.0; 2], |v| [v.x, v.y]),
        },
        "DamageEntity" => Act::Damage {
            item: a.item_id,
            amount: a.damage,
            direction: a.direction,
        },
        "MoveToNextMap" | "ShowSuccess" => Act::LevelComplete,
        "RestartMap" => Act::Restart,
        "SaveAction" | "CaptureScreen" | "UnlockAchievement" | "SetAchievementStat" => Act::Nothing,
        other => Act::Unsupported(other.to_string()),
    }
}

fn template(name: &str, args: &[String]) -> Act {
    let int = |i: usize| args.get(i).and_then(|a| a.parse::<i64>().ok());
    let num = |i: usize| args.get(i).and_then(|a| a.parse::<f32>().ok());
    let parsed = match name {
        "ActivateEntity" | "DeactivateEntity" => int(0).map(|item| Act::SetActive {
            item,
            active: name == "ActivateEntity",
        }),
        "DestroyEntity" => int(0).map(|item| Act::Destroy { item }),
        "ActivateTrigger" | "DeactivateTrigger" => int(0).map(|id| Act::SetTriggers {
            ids: vec![id],
            active: name == "ActivateTrigger",
        }),
        "Impulse" => match (int(0), num(1), num(2)) {
            (Some(item), Some(x), Some(y)) => Some(Act::Impulse {
                item,
                impulse: [x, y],
                random: [0.0; 2],
            }),
            _ => None,
        },
        "Damage" => match (int(0), int(1)) {
            (Some(item), Some(amount)) => Some(Act::Damage {
                item,
                amount,
                direction: 0.0,
            }),
            _ => None,
        },
        "Seq" => args.first().map(|list| Act::Sequence {
            actions: list.split_whitespace().map(String::from).collect(),
            step: num(1).unwrap_or(0.0),
        }),
        "NextMap" | "ShowSuccess" => Some(Act::LevelComplete),
        "RestartMap" => Some(Act::Restart),
        n if SILENT.contains(&n) => Some(Act::Nothing),
        _ => None,
    };
    parsed.unwrap_or_else(|| Act::Unsupported(name.to_string()))
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
