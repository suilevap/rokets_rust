//! The action types, as `GameLibrary/ActionData.cs` declares them: fields
//! in C#'s order (XNA writes them so), descriptions, references,
//! defaults, and the templates of `ActionTemplates.cs` that make them.
//! See [`actions!`](crate::actions!) for the syntax.

use super::def::Lookup;
use super::def::Ref::*;
use crate::content::xna::XVec2;
use crate::{actions, xna_enum};

xna_enum! {
    /// How "Level complete" looks (`ShowSuccessInternal.SuccessIndicator`).
    SuccessIndicator { Standard, DoorOpen, AntiSuccess }
}

actions! {
    /// Arms or disarms triggers.
    ActivateTrigger("ActivateTriggerActionData") {
        trigger_ids "TriggerIds": Vec<i64> = Vec::new(), refs(&[Trigger]);
        deactivate "Deactivate": bool = false, optional;
    } => templates {
        "ActivateTrigger(triggerId)" => |a| Some(ActivateTrigger { trigger_ids: vec![a.int(0)?], deactivate: false }),
        "DeactivateTrigger(triggerId)" => |a| Some(ActivateTrigger { trigger_ids: vec![a.int(0)?], deactivate: true }),
    }

    /// Changes an entity's speed by `Impulse` (C# units, y down).
    ApplyImpulse("ApplyImpulseActionData") {
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
        impulse "Impulse": XVec2 = XVec2::ZERO;
        /// If not zero, this vector times a random 0..1 is added to `Impulse`.
        random_impulse "RandomImpulse": XVec2 = XVec2::ZERO, optional;
    } => templates {
        "Impulse(itemId,impulseX,impulseY)" => |a| Some(ApplyImpulse {
            item_id: a.int(0)?,
            impulse: XVec2 { x: a.num(1)?, y: a.num(2)? },
            ..ApplyImpulse::default()
        }),
    }

    /// Takes an entity out of the game or brings it back.
    ActivateEntity("ActivateEntityActionData") {
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity, Hint]);
        deactivate "Deactivate": bool = false, optional;
    } => templates {
        "ActivateEntity(itemId)" => |a| Some(ActivateEntity { item_id: a.int(0)?, deactivate: false }),
        "DeactivateEntity(itemId)" => |a| Some(ActivateEntity { item_id: a.int(0)?, deactivate: true }),
    }

    /// Wins the level and moves on to the next map.
    MoveToNextMap("MoveToNextMap", name = "NextMap") {
        /// Explicit stars (0-3). If not given, they are counted from the
        /// coins collected.
        stars "Stars": Option<String> = None, optional, lookup(Lookup::Variables);
    } => templates {
        "NextMap()" => |_| Some(MoveToNextMap::default()),
        "NextMap(stars)" => |a| Some(MoveToNextMap { stars: Some(a.text(0)?.to_string()) }),
    }

    /// Runs actions in order, at once or `TimeBetweenInSec` apart; `na`
    /// steps are skipped.
    ActionSequence("ActionSequence", name = "seqActionSequence") {
        actions "Actions": Vec<String> = Vec::new(), refs(&[Action]);
        time_between_in_sec "TimeBetweenInSec": f32 = 0.0, optional;
    } => templates {
        "Seq(actionsSplitBySpace,timeBetween)" => |a| Some(ActionSequence {
            actions: a.words(0),
            time_between_in_sec: a.num(1).unwrap_or(0.0),
        }),
    }

    /// "Level complete".
    ShowSuccess("ShowSuccess", name = "ShowSuccess") {
        success_type "SuccessType": SuccessIndicator = SuccessIndicator::Standard, optional;
    } => templates {
        "ShowSuccess()" => |_| Some(ShowSuccess::default()),
    }

    /// Destroys an entity: bombs blow up, cars wreck.
    DestroyEntity("DestroyEntityActionData", name = "DestroyEntity") {
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
    } => templates {
        "DestroyEntity(itemId)" => |a| Some(DestroyEntity { item_id: a.int(0)? }),
    }

    /// Hurts or heals an entity.
    DamageEntity("DamageEntity", name = "DamageEntity") {
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
        /// More than 0: damage; less: heal.
        damage "Damage": i64 = 0;
        /// Who dealt it.
        who "Who": i64 = 0, optional, refs(&[Player, SimpleCar, Entity]);
        /// Where the blow comes from, degrees.
        direction "Direction": f32 = 0.0, optional;
    } => templates {
        "Damage(itemId,damage)" => |a| Some(DamageEntity { item_id: a.int(0)?, damage: a.int(1)?, who: -1, direction: 0.0 }),
    }

    /// Starts the map again.
    RestartMap("RestartMap", name = "RestartMap") {}
    => templates {
        "RestartMap()" => |_| Some(RestartMap::default()),
    }

    /// Locks a body's movement along x, y or its turning.
    SetBodyRestrictions("SetBodyRestrictions", name = "SetBodyRestrictions") {
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
        restrict_x "RestrictX": bool = false;
        restrict_y "RestrictY": bool = false;
        restrict_rotation "RestrictRotation": bool = false;
    }

    /// Saves progress: the player respawns here.
    SaveAction("SaveAction", name = "Save") {
        item_id "ItemId": i64 = 0, refs(&[Player]);
        action_on_respawn "ActionOnRespawn": Option<String> = None, optional, refs(&[Action]);
    } => templates {
        "Save()" => |_| Some(SaveAction::default()),
        "SaveWithActionOnRespawn(actionOnRespawn)" => |a| Some(SaveAction {
            action_on_respawn: Some(a.text(0)?.to_string()),
            ..SaveAction::default()
        }),
    }

    /// Opens or closes a door (`AddDoorComponent`); pushes an entity that
    /// is not one.
    OpenCloseDoor("OpenCloseDoor") {
        item_id "ItemId": i64 = 0, refs(&[Entity]);
        open "Open": bool = true;
    } => templates {
        "OpenDoor(itemId)" => |a| Some(OpenCloseDoor { item_id: a.int(0)?, open: true }),
        "CloseDoor(itemId)" => |a| Some(OpenCloseDoor { item_id: a.int(0)?, open: false }),
    }

    /// Makes an entity a door that slides along its facing.
    AddDoorComponent("AddDoorComponent", name = "DoorComponent", on_map_start = true) {
        /// Target entity.
        item_id "ItemId": i64 = 0, refs(&[Entity]);
        /// How far it opens: the entity's length times this.
        open_distance_scale "OpenDistanceScale": f32 = 1.0;
        /// Draw a placeholder where it opens to.
        is_draw_hint "IsDrawHint": bool = true;
        /// Draw an animated arrow hint.
        is_draw_arrow_hint "IsDrawArrowHint": bool = false, optional;
        /// Make the door static once open: for doors opened by pushing,
        /// not only by an action. Not needed with `IsStatic`.
        is_fix_on_open "IsFixOnOpen": bool = true;
        /// Run when the door opens.
        action_on_open "ActionOnOpen": Option<String> = None, refs(&[Action]);
        /// Run when the door closes.
        action_on_close "ActionOnClose": Option<String> = None, optional, refs(&[Action]);
        /// Make the door static when open and when closed.
        is_static "IsStatic": bool = false, optional;
        /// Make a navigation link (for the AI).
        is_generate_nav_link "IsGenerateNavLink": bool = false, optional;
    }

    /// Takes a screenshot for the level select.
    CaptureScreen("CaptureScreen", name = "CaptureScreen") {
        /// The entity to show.
        item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
        timer_sec "TimerSec": f32 = 0.0;
        forced "Forced": bool = false, optional;
    } => templates {
        "CaptureScreen(itemId,timer)" => |a| Some(CaptureScreen { item_id: a.int(0)?, timer_sec: a.num(1)?, forced: false }),
        "CaptureScreenForced(itemId,timer)" => |a| Some(CaptureScreen { item_id: a.int(0)?, timer_sec: a.num(1)?, forced: true }),
    }

    /// Unlocks an achievement.
    UnlockAchievement("UnlockAchievement") {
        achievement_name "AchievementName": Option<String> = None;
    } => templates {
        "UnlockAch(name)" => |a| Some(UnlockAchievement { achievement_name: Some(a.text(0)?.to_string()) }),
    }

    /// Adds to an achievement's stat.
    SetAchievementStat("SetAchievementStat") {
        stat_name "StatName": Option<String> = None;
        value "Value": i64 = 0;
    } => templates {
        "SetStat(name,value)" => |a| Some(SetAchievementStat { stat_name: Some(a.text(0)?.to_string()), value: a.int(1)? }),
    }
}

/// The definition of an action type, by its C# class.
pub fn find(kind: &str) -> Option<&'static super::def::DefInfo> {
    ALL.iter().find(|d| d.kind == kind)
}
