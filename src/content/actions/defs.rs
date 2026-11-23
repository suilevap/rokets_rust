//! The action types, as `GameLibrary/ActionData.cs` declares them: fields
//! in C#'s order (XNA writes them so), descriptions, references,
//! defaults, and the templates of `ActionTemplates.cs` that make them.
//! See [`actions!`](crate::actions!) for the syntax.

use super::source::*;
use crate::content::xna::{XColor, XRect, XVec2};
use crate::{actions, record, xna_enum};

xna_enum! {
    /// How "Level complete" looks (`ShowSuccessInternal.SuccessIndicator`).
    SuccessIndicator { Standard, DoorOpen, AntiSuccess }
}

xna_enum! {
    /// `MapDataInternal.ControlType`.
    ControlType { NoneLocal, NoneEnemy, NoneFriend, Local, Ai, AiFriend, AiSimpleTurret, AiSimpleTurretFriend, AiSimpleTurretFriendFireAvailableTarget, AiChaser, MoveByPath, MoveByPathFriend, MoveByPathSafelyFriend, SimpleFlocking, ZombieFlocking, ZombieFlockingFriend, SimpleAttackNearestEnemy, SimpleFollowByEnemyAi, MoveAndFireInInitDirection, AlwaysShoot, GuardAi, SupportAi, SupportAiFriend, OrderExecutorFriend, OrderExecutorAi, CaptureTheBox, CaptureTheBoxFriend, AttackAi, AttackAiVer2, AttackAiFriend, HammerAi, HammerAiFriend, ChickenAi, GoToEnemySimplePredictionPosition, ChaserTargetFromLocalVariable, LocalHotSeat1, LocalHotSeat2, ShootPattern }
}

xna_enum! {
    /// `MapDataInternal.LevelMusicType`.
    LevelMusicType { None, Ambient, Dynamic1, Dynamic2 }
}

xna_enum! {
    /// `GameLibrary.PathType`.
    PathType { Path, Patrol }
}

xna_enum! {
    /// `ChangeIntVariableInternal.OperationType`.
    ChangeIntVariableOperationType { Set, Increase, Mul, Div, Mod }
}

xna_enum! {
    /// `ChangeSubscribersInternal.OperationType`.
    ChangeSubscribersOperationType { Add, Remove }
}

xna_enum! {
    /// `GameLibrary.BoolSetterType`.
    BoolSetterType { DoNotChange, Yes, No }
}

xna_enum! {
    /// `CreateButtonInternal.ButtonTypeType`.
    ButtonTypeType { Button, LaserCollector }
}

xna_enum! {
    /// `OrderActionInternal.OrderType`.
    OrderType { RemoveOnSuccess, RemoveOnTry, Infinite }
}

xna_enum! {
    /// `CarsDataInternal.WeaponType`.
    WeaponType { None, Default, RocketlauncherDirect, RocketlauncherChasing, RocketlauncherChasing360, Minigun, Minigun180, Minigun360, MinigunSlowInacurate, MinigunLowConsumption, Railgun, RailgunAngle5, Hammer, Tesla, ZombieWeapon, Shotgun, Laser, PlasmaGun, PlasmaGunSlow, RailgunNoReflection, LaserNoReflection, LaserWithDelay }
}

xna_enum! {
    /// `CarsDataInternal.GadgetType`.
    GadgetType { None, Default, Link, Bomb, Accelerator, EditorPointer }
}

xna_enum! {
    /// `SplashSetBindVariableInternal.BindVariableNum`.
    BindVariableNum { First, Second, Third, Fourth, Fifth }
}

xna_enum! {
    /// `CreateParticleEffectInternal.EffectType`.
    EffectType { NewZombieFriend }
}

xna_enum! {
    /// `MapPartSettingsInternal.PartTypeEnum`.
    PartTypeEnum { Corridor, Battle, Quest, Max }
}

record! {
    /// Partial override for a particle state. Null fields keep their current value.
    ParticleStateData {
        speed "Speed": Option<f32> = None, optional;
        scale "Scale": Option<XVec2> = None, optional;
        view_rotation "ViewRotation": Option<f32> = None, optional;
        rotation "Rotation": Option<f32> = None, optional;
        color "Color": Option<XColor> = None, optional;
        height "Height": Option<f32> = None, optional;
    }
}

// New enums and records go above this line (tools/gen_action_defs.py).

actions! {
    /// Arms or disarms triggers.
    ActivateTrigger("ActivateTriggerActionData") {
        trigger_ids "TriggerIds": Vec<i64> = Vec::new(), from(&[TRIGGERS]);
        deactivate "Deactivate": bool = false, optional;
    } => templates {
        "ActivateTrigger(triggerId)" => |a| Some(ActivateTrigger { trigger_ids: vec![a.int(0)?], deactivate: false }),
        "DeactivateTrigger(triggerId)" => |a| Some(ActivateTrigger { trigger_ids: vec![a.int(0)?], deactivate: true }),
    }

    /// Changes an entity's speed by `Impulse` (C# units, y down).
    ApplyImpulse("ApplyImpulseActionData") {
        item_id "ItemId": i64 = 0, from(ITEM);
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
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
        deactivate "Deactivate": bool = false, optional;
    } => templates {
        "ActivateEntity(itemId)" => |a| Some(ActivateEntity { item_id: a.int(0)?, deactivate: false }),
        "DeactivateEntity(itemId)" => |a| Some(ActivateEntity { item_id: a.int(0)?, deactivate: true }),
    }

    /// Wins the level and moves on to the next map.
    MoveToNextMap("MoveToNextMap", name = "NextMap") {
        /// Explicit stars (0-3). If not given, they are counted from the
        /// coins collected.
        stars "Stars": Option<String> = None, optional, from(&[VARIABLES]);
    } => templates {
        "NextMap()" => |_| Some(MoveToNextMap::default()),
        "NextMap(stars)" => |a| Some(MoveToNextMap { stars: Some(a.text(0)?.to_string()) }),
    }

    /// Runs actions in order, at once or `TimeBetweenInSec` apart; `na`
    /// steps are skipped.
    ActionSequence("ActionSequence", name = "seqActionSequence") {
        actions "Actions": Vec<String> = Vec::new(), from(ACTION);
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
        item_id "ItemId": i64 = 0, from(ITEM);
    } => templates {
        "DestroyEntity(itemId)" => |a| Some(DestroyEntity { item_id: a.int(0)? }),
    }

    /// Hurts or heals an entity.
    DamageEntity("DamageEntity", name = "DamageEntity") {
        item_id "ItemId": i64 = 0, from(ITEM);
        /// More than 0: damage; less: heal.
        damage "Damage": i64 = 0;
        /// Who dealt it.
        who "Who": i64 = 0, optional, from(ITEM);
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
        item_id "ItemId": i64 = 0, from(ITEM);
        restrict_x "RestrictX": bool = false;
        restrict_y "RestrictY": bool = false;
        restrict_rotation "RestrictRotation": bool = false;
    }

    /// Saves progress: the player respawns here.
    SaveAction("SaveAction", name = "Save") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS]);
        action_on_respawn "ActionOnRespawn": Option<String> = None, optional, from(ACTION);
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
        item_id "ItemId": i64 = 0, from(&[ENTITIES]);
        open "Open": bool = true;
    } => templates {
        "OpenDoor(itemId)" => |a| Some(OpenCloseDoor { item_id: a.int(0)?, open: true }),
        "CloseDoor(itemId)" => |a| Some(OpenCloseDoor { item_id: a.int(0)?, open: false }),
    }

    /// Makes an entity a door that slides along its facing.
    AddDoorComponent("AddDoorComponent", name = "DoorComponent", on_map_start = true) {
        /// Target entity.
        item_id "ItemId": i64 = 0, from(&[ENTITIES]);
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
        action_on_open "ActionOnOpen": Option<String> = None, from(ACTION);
        /// Run when the door closes.
        action_on_close "ActionOnClose": Option<String> = None, optional, from(ACTION);
        /// Make the door static when open and when closed.
        is_static "IsStatic": bool = false, optional;
        /// Make a navigation link (for the AI).
        is_generate_nav_link "IsGenerateNavLink": bool = false, optional;
    }

    /// Takes a screenshot for the level select.
    CaptureScreen("CaptureScreen", name = "CaptureScreen") {
        /// The entity to show.
        item_id "ItemId": i64 = 0, from(ITEM);
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


    RemoveMarker("RemoveMarkerActionData", name = "RemoveMarker") {
        marker_id "MarkerId": i64 = 0, from(&[TRIGGERS]);
    } => templates {
        "RemoveMarker(markerId)" => |a| Some(RemoveMarker { marker_id: a.int(0)? }),
    }

    AddMarker("AddMarkerActionData", name = "AddMarker") {
        marker_id "MarkerId": i64 = 0, from(&[TRIGGERS]);
        item_id "ItemId": i64 = 0, from(&[TRIGGERS, PLAYERS, SIMPLE_CARS, ENTITIES]);
    } => templates {
        "AddMarker(itemId,markerId)" => |a| Some(AddMarker { item_id: a.int(0)?, marker_id: a.int(1)? }),
    }

    AddEntitiesToFocus("AddEntitiesToFocus") {
        item_ids "ItemIds": Vec<i64> = Vec::new(), from(&[PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
        remove_all_others "RemoveAllOthers": bool = false, optional;
    } => templates {
        "FocusOnEntity(itemId)" => |a| Some(AddEntitiesToFocus { item_ids: vec![a.int(0)?], remove_all_others: true }),
        "AddFocusOnEntity(itemId)" => |a| Some(AddEntitiesToFocus { item_ids: vec![a.int(0)?], remove_all_others: false }),
    }

    RemoveEntitiesFromFocus("RemoveEntitiesFromFocus") {
        item_ids "ItemIds": Vec<i64> = Vec::new(), from(&[PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
    }

    SetRespawnPoint("SetRespawnPoint") {
        position "Position": XVec2 = XVec2::ZERO;
        direction_deg "DirectionDeg": f32 = 0.0, optional;
    }

    SetControl("SetControl", name = "SetControl") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        control "Control": ControlType = ControlType::NoneLocal;
    } => templates {
        "SetControl(itemId,control)" => |a| Some(SetControl { item_id: a.int(0)?, control: ControlType::from_name(a.text(1)?)? }),
    }

    SuccessSequence("SuccessSequence", name = "seqSuccessSequence") {
        actions "Actions": Vec<String> = Vec::new(), from(ACTION);
    }

    TriggerActionBy("TriggerActionBy") {
        item_ids "ItemIds": Vec<String> = Vec::new(), from(&[VARIABLES, PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
        actions "Actions": Vec<String> = Vec::new(), from(ACTION);
    }

    TriggerActionByFirstSuccess("TriggerActionByFirstSuccess") {
        item_ids "ItemIds": Vec<String> = Vec::new(), from(&[VARIABLES, PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
        action "Action": Option<String> = None, from(ACTION);
        else_action "ElseAction": Option<String> = None, optional, from(ACTION);
    }

    PlaySound("PlaySound", name = "PlaySound") {
        /// Only file name without extension
        sound "Sound": Option<String> = None, from(&[SOUNDS]);
        emmiter_entity_id "EmmiterEntityId": i64 = 0;
        use_listener_position "UseListenerPosition": bool = false;
        volume "Volume": f32 = 1.0;
        pitch "Pitch": f32 = 0.0;
        /// Rotated list of pitches will be used
        play_with_next_pitch "PlayWithNextPitch": bool = false, optional;
    } => templates {
        "PlaySound(soundName)" => |a| Some(PlaySound {
            sound: Some(a.text(0)?.to_string()),
            use_listener_position: true,
            ..PlaySound::default()
        }),
    }

    ShowText("ShowText", name = "ShowText") {
        text "Text": Vec<String> = Vec::new();
        /// Default 8 sec
        show_time_sec "ShowTimeSec": f32 = -1.0;
        pause_game "PauseGame": bool = false;
    } => templates {
        // Time and pause may be left out.
        "ShowText(text,time,isPause)" => |a| Some(ShowText {
            text: vec![a.text(0)?.to_string()],
            show_time_sec: a.num(1).unwrap_or(-1.0),
            pause_game: a.text(2).is_some_and(|p| p.trim().eq_ignore_ascii_case("true")),
        }),
    }

    ApplyGadgetToPosition("ApplyGadgetToPosition", name = "ApplyGadgetToPosition") {
        gadget_owner_id "GadgetOwnerId": i64 = 0, from(&[PLAYERS]);
        target_item_id "TargetItemId": i64 = 0, from(ITEM);
        relative_position "RelativePosition": XVec2 = XVec2::ZERO;
    }

    PlayLevelMusic("PlayLevelMusic", name = "PlayLevelMusic") {
        level_music_type "LevelMusicType": LevelMusicType = LevelMusicType::None;
        looped "Looped": bool = true, optional;
    }

    SetPath("SetPath", name = "SetPath") {
        path "Path": Vec<XVec2> = Vec::new();
        item_id "ItemId": i64 = 0, from(ITEM);
        closed "Closed": bool = false, optional;
        kind "Type": PathType = PathType::Path, optional;
    }

    CheckIntVariable("CheckIntVariable", name = "CheckIntVariable") {
        variable_name "VariableName": Option<String> = Some("Var".into()), from(&[VARIABLES]);
        global "Global": bool = true;
        item_id "ItemId": i64 = 0, optional, from(ITEM);
        /// Can be int value or name of variable to compare
        value "Value": Option<String> = None, from(&[VARIABLES]);
        action_on_less "ActionOnLess": Option<String> = None, from(ACTION);
        action_on_equal "ActionOnEqual": Option<String> = None, from(ACTION);
        action_on_greater "ActionOnGreater": Option<String> = None, from(ACTION);
    } => templates {
        "if(variable,check,value)" => |a| compare(0, a.text(0)?, a.text(1)?, a.text(2)?, "NA"),
        "IfEx(itemId,variable,check,value,action)" => |a| {
            compare(a.int(0)?, a.text(1)?, a.text(2)?, a.text(3)?, a.text(4)?)
        },
    }

    ChangeIntVariable("ChangeIntVariable", name = "ChangeIntVariable") {
        variable_name "VariableName": Option<String> = Some("Var".into()), from(&[VARIABLES]);
        global "Global": bool = true;
        item_id "ItemId": i64 = 0, optional, from(ITEM);
        value "Value": i64 = 0;
        operation "Operation": ChangeIntVariableOperationType = ChangeIntVariableOperationType::Set;
    } => templates {
        "SetGlobalVar(Var,Value)" => |a| Some(ChangeIntVariable {
            variable_name: Some(a.text(0)?.to_string()),
            global: true,
            item_id: 0,
            value: a.int(1)?,
            operation: ChangeIntVariableOperationType::Set,
        }),
        "SetVar(Var,Value)" => |a| Some(ChangeIntVariable {
            variable_name: Some(a.text(0)?.to_string()),
            global: false,
            item_id: 0,
            value: a.int(1)?,
            operation: ChangeIntVariableOperationType::Set,
        }),
        "SetVarEx(itemId,variable,operation,value)" => |a| {
            use ChangeIntVariableOperationType::*;
            let (item_id, value) = (a.int(0)?, a.int(3)?);
            let (operation, value) = match a.text(2)? {
                "=" => (Set, value),
                "+" => (Increase, value),
                "-" => (Increase, -value),
                "*" => (Mul, value),
                "/" => (Div, value),
                "%" => (Mod, value),
                _ => return None,
            };
            Some(ChangeIntVariable {
                variable_name: Some(a.text(1)?.to_string()),
                global: item_id < 0,
                item_id,
                value,
                operation,
            })
        },
    }

    ChangeSubscribers("ChangeSubscribers", name = "ChangeSubscribers") {
        trigger_id "TriggerId": i64 = 0, from(&[TRIGGERS]);
        item_id "ItemId": i64 = 0, from(ITEM);
        operation "Operation": ChangeSubscribersOperationType = ChangeSubscribersOperationType::Add;
    }

    AIAdjustment("AIAdjustment", name = "AIAdjustment") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        team "Team": i64 = 255;
        scan_distance "ScanDistance": f32 = -1.0;
        weapon_max_angle_deg "WeaponMaxAngleDeg": f32 = -1.0;
        action_on_bt_trigger "ActionOnBTTrigger": Option<String> = None, optional, from(ACTION);
        /// 1-precise (Default), >1 - less
        fire_accuracy "FireAccuracy": f32 = 1.0, optional;
        /// Switch on or switch off visibility for auto aim helper. No = no autoaim
        is_visible_for_auto_aim "IsVisibleForAutoAim": BoolSetterType = BoolSetterType::DoNotChange, optional;
    }

    CloneObject("CloneObject", name = "CloneObjByArea") {
        area "Area": XRect = XRect::default();
        item_id "ItemId": i64 = 0, from(ITEM);
        target_ids "TargetIds": Vec<i64> = Vec::new();
        count "Count": i64 = 1, optional;
        /// if True, objects are created relative Item with Id==RelativeByItem
        is_use_relative_position "IsUseRelativePosition": bool = false, optional;
        /// if IsUseRelativePosition is True, objects are created relative by this Item
        relative_by_item "RelativeByItem": i64 = 0, optional, from(ITEM);
    } => templates {
        "Clone(id)" => |a| {
            // C#'s template counted its uses: ids `id × 10 + n` for the
            // first ten, then from 6000.
            static USES: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
            let id = a.int(0)?;
            let n = USES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let target = if id > 0 && n < 10 { id * 10 + n } else { 6000 + n };
            Some(CloneObject {
                item_id: id,
                count: 1,
                target_ids: vec![target],
                area: XRect { x: -1, y: -1, w: 2, h: 2 },
                is_use_relative_position: true,
                ..CloneObject::default()
            })
        },
    }

    CloneObjectByPath("CloneObjectByPath", name = "CloneObjByPath") {
        area "Area": XRect = XRect::default();
        item_id "ItemId": i64 = 0, from(ITEM);
        target_ids "TargetIds": Vec<i64> = Vec::new();
        count "Count": i64 = 1, optional;
        /// if True, objects are created relative Item with Id==RelativeByItem
        is_use_relative_position "IsUseRelativePosition": bool = false, optional;
        /// if IsUseRelativePosition is True, objects are created relative by this Item
        relative_by_item "RelativeByItem": i64 = 0, optional, from(ITEM);
        path "Path": Vec<XVec2> = Vec::new();
        delta_angle "DeltaAngle": f32 = 90.0;
    }

    CreateButton("CreateButton", name = "CreateButton", on_map_start = true) {
        id "Id": i64 = 5000, key;
        position "Position": XVec2 = XVec2::ZERO;
        path "Path": Vec<XVec2> = Vec::new();
        scale "Scale": f32 = 1.0;
        direction_deg "DirectionDeg": f32 = 0.0;
        action_press "ActionPress": Option<String> = None, from(ACTION);
        action_release "ActionRelease": Option<String> = None, optional, from(ACTION);
        /// Only for ButtonType.Button
        /// Who may press it; null: the player's car, empty: nobody.
        subscribers "Subscribers": Option<Vec<i64>> = None, from(ITEM);
        button_type "ButtonType": ButtonTypeType = ButtonTypeType::Button, optional;
    }

    ShowSplash("ShowSplash", name = "ShowSplash") {
        splash_name "SplashName": Option<String> = None, from(&[SPLASHES]);
        auto_hide_time_sec "AutoHideTimeSec": f32 = 2.0;
        clickable "Clickable": bool = false;
        action_on_hide "ActionOnHide": Option<String> = None, from(ACTION);
    } => templates {
        "HideSplash()" => |_| Some(ShowSplash { splash_name: None, ..ShowSplash::default() }),
    }

    AddMovingPlatformComponent("AddMovingPlatformComponent", name = "MovingPlatformComponent", on_map_start = true) {
        /// If specified then OpenDistanceScale is ignored
        position "Position": XVec2 = XVec2::ZERO, optional;
        /// Target Entity Id
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS, ENTITIES, HINTS]);
        /// Max offset distance = TargetEntity.Raidus * value
        open_distance_scale "OpenDistanceScale": f32 = 1.0;
        /// Time needed for 2 changes of direction
        period_sec "PeriodSec": f32 = 7.0;
    }

    CheckEntityAlive("CheckEntityAlive", name = "CheckEntityAlive") {
        /// Entity id or variable which contains entity id
        item_id "ItemId": Option<String> = None, from(&[VARIABLES, PLAYERS, SIMPLE_CARS, ENTITIES]);
        action_on_alive "ActionOnAlive": Option<String> = None, from(ACTION);
        action_on_not_alive "ActionOnNotAlive": Option<String> = None, from(ACTION);
    }

    HighlightController("HighlightController", name = "HighlightController") {
        gadget_control "GadgetControl": bool = false;
        weapon_control "WeaponControl": bool = false;
        movement_control "MovementControl": bool = false;
    }

    MoveOrder("MoveOrder", name = "MoveOrder") {
        order_remove_condition "OrderRemoveCondition": OrderType = OrderType::RemoveOnSuccess, optional;
        position "Position": XVec2 = XVec2::ZERO;
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
    }

    PressButtonOrder("PressButtonOrder", name = "PressButtonOrder") {
        order_remove_condition "OrderRemoveCondition": OrderType = OrderType::RemoveOnSuccess, optional;
        button_id "ButtonId": i64 = 0, from(&[Source::Xml { file: None, path: "XnaContent/Asset/Actions/Item[@Type='GameLibrary.CreateButton']/Id" }]);
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
    }

    AIClearOrders("AIClearOrders", name = "AIClearOrders") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
    }

    MoveCargoOrder("MoveCargoOrder", name = "MoveCargoOrder") {
        order_remove_condition "OrderRemoveCondition": OrderType = OrderType::RemoveOnSuccess, optional;
        position "Position": XVec2 = XVec2::ZERO;
        r "R": f32 = 1.0, optional;
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        cargo_item_id "CargoItemId": i64 = 0, from(ITEM);
        /// Use 0 0 for auto finding drop position by Ai
        drop_position "DropPosition": XVec2 = XVec2::ZERO, optional;
    }

    GrabCargoOrder("GrabCargoOrder", name = "GrabCargoOrder") {
        order_remove_condition "OrderRemoveCondition": OrderType = OrderType::RemoveOnSuccess, optional;
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        cargo_item_id "CargoItemId": i64 = 0, from(ITEM);
        /// ActionOnBTTrigger
        action_on_grab "ActionOnGrab": Option<String> = None, optional, from(ACTION);
    }

    AttackTargetOrder("AttackTargetOrder", name = "AttackTargetOrder") {
        order_remove_condition "OrderRemoveCondition": OrderType = OrderType::RemoveOnSuccess, optional;
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        target_id "TargetId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
    }

    ChangeMapEffectAmount("ChangeMapEffectAmount", name = "ChangeMapEffectAmount") {
        /// 0 - disable, 1- full
        amount "Amount": f32 = 0.0;
    }

    AttachAdditionalView("AttachAdditionalView", name = "AttachAdditionalView") {
        position "Position": XVec2 = XVec2::ZERO;
        sprite "Sprite": Option<String> = None;
        scale "Scale": f32 = 1.0;
        direction_deg "DirectionDeg": f32 = 0.0;
        animated "Animated": bool = false;
        color "Color": XColor = XColor::rgba(255, 255, 255, 255);
        item_id "ItemId": i64 = 0, from(ITEM);
        /// In Meters. 0 - floor (or entity depth for relative), <0 - down, >0 - up
        depth "Depth": f32 = 0.0;
        /// If depth is relative to entity depth
        relative_depth "RelativeDepth": bool = false, optional;
    }

    AttachTags("AttachTags", name = "AttachTag") {
        item_id "ItemId": i64 = 0, from(ITEM);
        tags "Tags": Vec<String> = Vec::new();
    }

    MoveEntity("MoveEntity", name = "MoveEntity") {
        position "Position": XVec2 = XVec2::ZERO;
        direction_deg "DirectionDeg": f32 = 0.0;
        item_id "ItemId": i64 = 0, from(ITEM);
    }

    ResetEntityPosition("ResetEntityPosition", name = "ResetEntityPosition") {
        item_id "ItemId": i64 = 0, from(ITEM);
    }

    SetWeapon("SetWeapon", name = "SetWepon") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        /// Default = do not change weapon
        new_weapon "NewWeapon": WeaponType = WeaponType::Default;
        new_gadget "NewGadget": GadgetType = GadgetType::Default, optional;
        /// Show&play effect
        change_silently "ChangeSilently": bool = false, optional;
    } => templates {
        "SetWeapon(weapon)" => |a| Some(SetWeapon { new_weapon: WeaponType::from_name(a.text(0)?)?, ..SetWeapon::default() }),
    }

    TriggerActionByEntitiesInArea("TriggerActionByEntitiesInArea", name = "TriggerActionByEntitiesInArea") {
        area "Area": XRect = XRect::default();
        actions "Actions": Vec<String> = Vec::new(), from(ACTION);
    }

    AddPathComponent("AddPathComponent", name = "PathComponent", on_map_start = true) {
        path "Path": Vec<XVec2> = Vec::new();
        /// Target Entity Id
        item_id "ItemId": i64 = 0, from(&[ENTITIES, PLAYERS, SIMPLE_CARS]);
        color "Color": XColor = XColor::rgba(255, 255, 255, 255);
        /// In Meters. 0 - floor, <0 - down, >0 - up
        depth "Depth": f32 = 0.0;
        /// Width in pixels
        width "Width": f32 = 5.0;
    }

    AddConserveMaxReachedSpeedComponent("AddConserveMaxReachedSpeedComponent", name = "ConserveMaxReachedSpeedComponent", on_map_start = true) {
        /// Target Entity Id
        item_id "ItemId": i64 = 0, from(&[ENTITIES, SIMPLE_CARS, PLAYERS]);
    }

    SplashSetBindVariable("SplashSetBindVariable", name = "SplashSetBindVariable") {
        bind_variable_number "BindVariableNumber": BindVariableNum = BindVariableNum::First;
        value "Value": Option<String> = None, from(&[VARIABLES]);
    }

    CreateParticleEffect("CreateParticleEffect", name = "CreateParticleEffect") {
        /// Target Entity Id
        item_id "ItemId": i64 = 0, from(&[ENTITIES, SIMPLE_CARS, PLAYERS]);
        effect "Effect": EffectType = EffectType::NewZombieFriend;
    }

    CameraShowPath("CameraShowPath", name = "CameraShowPath") {
        path "Path": Vec<XVec2> = Vec::new();
        /// m/s
        speed "Speed": f32 = 0.0;
        action_on_end "ActionOnEnd": Option<String> = None, from(ACTION);
        action_on_reverse_end "ActionOnReverseEnd": Option<String> = None, optional, from(ACTION);
        is_reverse_at_the_end "IsReverseAtTheEnd": bool = false, optional;
        /// Wait sec in the end, until start reverse move
        wait_in_sec "WaitInSec": f32 = 3.0, optional;
        pause_while_move "PauseWhileMove": bool = true, optional;
        /// 0-1; 0 - close to object, 1 - further
        scale_amount "ScaleAmount": f32 = 0.0, optional;
    }

    ChangeAreaWeight("ChangeAreaWeight", name = "ChangeAreaWeight", on_map_start = true) {
        area "Area": XRect = XRect::default();
        /// Weight of navigation node in this area. Default is 1.0. Used as multiplayer for distance
        weight "Weight": f32 = 1.0;
    }

    CreateTeleport("CreateTeleport", name = "CreateTeleport", on_map_start = true) {
        id "Id": i64 = 5000;
        position "Position": XVec2 = XVec2::ZERO;
        target_position "TargetPosition": XVec2 = XVec2::ZERO;
        scale "Scale": f32 = 1.0;
        action "Action": Option<String> = None, from(ACTION);
        /// Who it takes; null: anything, empty: nothing.
        subscribers "Subscribers": Option<Vec<i64>> = None, from(ITEM);
        generate_back_teleport "GenerateBackTeleport": bool = false, optional;
    }

    AttachDestroyOnContact("AttachDestroyOnContact", name = "AttachDestroyOnContact") {
        item_id "ItemId": i64 = 0, from(ITEM);
        subscribers "Subscribers": Vec<i64> = Vec::new(), from(ITEM);
    } => templates {
        "DestroyOnContact(sub)" => |a| Some(AttachDestroyOnContact { item_id: 0, subscribers: vec![a.int(0)?] }),
    }

    InstantCameraMove("InstantCameraMove", name = "InstantCameraMove") {
        position "Position": XVec2 = XVec2::ZERO;
    }

    MapPart("MapPart") {
        part_name "PartName": Option<String> = None, from(&[MAP_PARTS]);
        position "Position": XVec2 = XVec2::ZERO;
        direction_deg "DirectionDeg": f32 = 0.0, optional;
    }

    MapPartPortal("MapPartPortal", name = "Portal") {
        area "Area": XRect = XRect::default();
        allow_in "AllowIn": bool = true;
        allow_out "AllowOut": bool = true;
    }

    MapPartSettings("MapPartSettings", name = "MapPartSettings") {
        part_type "PartType": PartTypeEnum = PartTypeEnum::Corridor;
        /// Estimate length for gameplay in cells. 0-default value(computes as (Width+Height)/2)
        estimated_gameplay_length "EstimatedGameplayLength": f32 = 0.0;
        /// Final porbability for part will be * on this rate
        probability_rank "ProbabilityRank": f32 = 1.0, optional;
        difficulty_level "DifficultyLevel": f32 = 0.0, optional;
        free_point "FreePoint": XVec2 = XVec2::ZERO, optional;
        /// if not empty: this part can be used only once per game.
        /// Else it should replaced by FallbackPart
        /// Fallback part should be with exact size and portals
        fallbackpart_name_for_completed "FallbackpartNameForCompleted": Option<String> = None, optional, from(&[MAP_PARTS]);
    }

    TriggerWithPeriodicCheck("TriggerWithPeriodicCheck", name = "TriggerWithPeriodicCheck", on_map_start = true) {
        actions "Actions": Vec<String> = Vec::new(), from(ACTION);
        area "Area": XRect = XRect::default();
    }

    EnemySpawnArea("EnemySpawnArea", name = "EnemySpawnArea") {
        area "Area": XRect = XRect::default();
    }

    WallWithProbability("WallWithProbability", name = "WallWithProbability") {
        area "Area": XRect = XRect::default();
        color "Color": XColor = XColor::rgba(255, 255, 255, 128);
        /// (0,1]
        probability "Probability": f32 = 0.5;
        /// Walls with same non empty WallGroup place together and use shared probability
        wall_group "WallGroup": Option<String> = None;
    }

    CreateEnemyGenerator("CreateEnemyGenerator", name = "CreateEnemyGenerator") {
        position "Position": XVec2 = XVec2::ZERO;
        direction_deg "DirectionDeg": f32 = 0.0;
    }

    MarkPartAsCompleted("MarkPartAsCompleted", name = "MarkAsCompleted") {
        part_name "PartName": Option<String> = None, from(&[MAP_PARTS]);
    }

    GenerateCorridorByPath("GenerateCorridorByPath", name = "GenerateCorridorByPath") {
        path "Path": Vec<XVec2> = Vec::new();
        width "Width": i64 = 0;
        random_rate "RandomRate": f32 = 0.0;
        inner_color "InnerColor": XColor = XColor::rgba(0, 0, 255, 64);
        outer_color "OuterColor": XColor = XColor::rgba(255, 128, 128, 128);
        /// Split path part on smaller parts if it is longer than define. if <=0 - do not split
        split_if_longer_than "SplitIfLongerThan": f32 = 0.0;
    }

    SetImmortal("SetImmortal", name = "SetImmortal") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        is_immortal "IsImmortal": bool = false;
    } => templates {
        // `bool.Parse` ignores case and surrounding blanks.
        "SetImmortal(id,value)" => |a| Some(SetImmortal {
            item_id: a.int(0)?,
            is_immortal: match a.text(1)?.trim().to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => return None,
            },
        }),
    }

    AdjustWepaonMaxAngle("AdjustWepaonMaxAngle") {
        item_id "ItemId": i64 = 0, from(&[PLAYERS, SIMPLE_CARS]);
        min_distance "MinDistance": f32 = 2.0;
    }

    SetParticleStyle("SetParticleStyleActionData") {
        /// Name of the registered particle style to modify (e.g. 'smokeStyle', 'boomStyle')
        style_name "StyleName": Option<String> = None;
        /// Override lifetime in seconds
        life_time "LifeTime": Option<f32> = None, optional;
        /// Override begin state fields (null fields keep current value)
        begin_state "BeginState": Option<ParticleStateData> = None, optional;
        /// Override end state fields (null fields keep current value)
        end_state "EndState": Option<ParticleStateData> = None, optional;
    }

    AiMaskArea("AiMaskArea", name = "AiMask1") {
        area "Area": XRect = XRect { x: 0, y: 0, w: 4, h: 4 }, optional;
        color "Color": XColor = XColor::rgba(160, 80, 255, 255), optional;
    }

    FakeWallRegion("FakeWallRegionActionData") {
        area "Area": XRect = XRect::default();
        initially_revealed "InitiallyRevealed": bool = false, optional;
        animation_duration "AnimationDuration": f32 = 0.3, optional;
        wave_direction_degrees "WaveDirectionDegrees": f32 = 0.0, optional;
    }

    SetFakeWallRevealed("SetFakeWallRevealedActionData") {
        target_name "TargetName": Option<String> = None, optional, from(&[Source::Xml { file: None, path: "XnaContent/Asset/Actions/Item[@Type='GameLibrary.FakeWallRegionActionData']/Name" }]);
        revealed "Revealed": bool = false;
    }

    TrinagleVertex("TrinagleVertex", name = "Vertex") {
        position "Position": XVec2 = XVec2::ZERO;
        color "Color": XColor = XColor::rgba(255, 255, 255, 255);
    }


    SplashFakeData("SplashFakeData", name = "SplashMainInfo") {
        draw_background "DrawBackground": bool = true, optional;
        appear_fade_to_black "AppearFadeToBlack": bool = true, optional;
        translation_time "TranslationTime": f32 = 2.5, optional;
        auto_hide "AutoHide": bool = false, optional;
        ignore_pause "IgnorePause": bool = true, optional;
        parallax_amount "ParallaxAmount": f32 = 1.0, optional;
        show_clickable_indicator_after_sec "ShowClickableIndicatorAfterSec": f32 = 3.0, optional;
        scale_up_when_needed "ScaleUpWhenNeeded": bool = false, optional;
        scale_up_only_offset_when_needed "ScaleUpOnlyOffsetWhenNeeded": bool = false, optional;
    }

    LayerFakeMapData("LayerFakeMapData", name = "Layer") {
        layer_name "LayerName": Option<String> = None, optional;
        text "Text": Option<String> = None, optional;
        bind_text_id "BindTextId": i64 = -1, optional;
        stretch_to_screen "StretchToScreen": bool = false, optional;
        color "Color": XColor = XColor::rgba(255, 255, 255, 255), optional;
        is_static "Static": bool = false, optional;
        on_foreground "OnForeground": bool = false, optional;
        delay_sec "DelaySec": f32 = 0.0, optional;
        sound "Sound": Option<String> = None, optional;
        height "Height": f32 = -1.0, optional;
        sprite "Sprite": Option<String> = None;
        scale "Scale": f32 = 0.0;
        position "Position": XVec2 = XVec2::ZERO;
        direction_deg "DirectionDeg": f32 = 0.0;
    }

    // New types go above this line (tools/gen_action_defs.py).
}

/// `if` and `IfEx`: a check that runs `action` on the outcomes `check`
/// names (`<`, `>`, `==`, `<=`, `>=`); an item id below 0 is global.
fn compare(item_id: i64, variable: &str, check: &str, value: &str, action: &str) -> Option<CheckIntVariable> {
    let on = |yes: bool| yes.then(|| action.to_string());
    Some(CheckIntVariable {
        variable_name: Some(variable.to_string()),
        global: item_id < 0,
        item_id,
        value: Some(value.to_string()),
        action_on_less: on(check.starts_with('<')),
        action_on_equal: on(check.ends_with('=')),
        action_on_greater: on(check.starts_with('>')),
    })
}

/// The definition of an action type, by its C# class.
pub fn find(kind: &str) -> Option<&'static super::def::DefInfo> {
    ALL.iter().find(|d| d.kind == kind)
}
