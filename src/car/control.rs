//! Who drives a car, as far as it matters before there is an AI: its team
//! and what kind of control it has.
//!
//! C# origin: `AiControlFactory` (each `ControlType` made a control with a
//! team: 0 for the player and friends, 1 for enemies), `IControl.Team`,
//! `IControl.LocalDevice`, `AINone`. A car's [`Team`] decides whom its
//! bullets, blasts and saw spare (`WeaponHelper.SimulateBullet`,
//! `CreateExplosion`, `HammerWeapon`) and, later, whom the AI attacks.
//!
//! The controls themselves are [`crate::ai`]'s; the ones that need
//! behaviour trees are not ported yet. `Ai` and `AiFriend` are
//! `GameMode.GetAi(team)` in C#, an `AIBTBaseControl` with team 1 and 0.

use bevy::prelude::*;

/// A car's team (`Control.Team`): bullets, blasts and the saw do not hurt
/// the same team. 0 is the player's and its friends', 1 the enemies'.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Team(pub u8);

/// `IControl.IsVisibleForAutoAim`: for an enemy, whether the player's aim
/// help may pull the aim towards it. `AINone` (`NoneLocal`, `NoneEnemy`,
/// `NoneFriend`) is not, every other control is until `AIAdjustment` says
/// otherwise.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoAimVisible(pub bool);

/// What a `ControlType` makes of a car.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlKind {
    /// `Control.Team`.
    pub team: u8,
    /// Nobody drives it at all (`AINone`: `NoneLocal`, `NoneEnemy`,
    /// `NoneFriend`): the Hammer's saw stays still then (`_isWorking`).
    pub ai_none: bool,
    /// `Control.LocalDevice`: the player's own devices drive it (`Local`,
    /// the hot-seat ones) or it is held for them (`NoneLocal`). Every AI is
    /// not: its health bar shows when it is hurt.
    pub local_device: bool,
}

/// The `ControlType` named `name`, as `AiControlFactory` made it. An
/// unknown name is a friend nobody drives; so is none at all (the
/// enumeration's first value, `NoneLocal`).
pub fn control_kind(name: &str) -> ControlKind {
    let name = name.trim();
    let ai_none = matches!(name, "" | "NoneLocal" | "NoneEnemy" | "NoneFriend");
    let local_device = matches!(name, "" | "NoneLocal" | "Local" | "LocalHotSeat1" | "LocalHotSeat2");
    let enemy = matches!(
        name,
        "NoneEnemy"
            | "Ai"
            | "AiSimpleTurret"
            | "AiChaser"
            | "MoveByPath"
            | "SimpleFlocking"
            | "ZombieFlocking"
            | "SimpleAttackNearestEnemy"
            | "SimpleFollowByEnemyAi"
            | "MoveAndFireInInitDirection"
            | "AlwaysShoot"
            | "ShootPattern"
            | "GuardAi"
            | "SupportAi"
            | "OrderExecutorAi"
            | "CaptureTheBox"
            | "AttackAi"
            | "AttackAiVer2"
            | "HammerAi"
            | "ChickenAi"
            | "GoToEnemySimplePredictionPosition"
            | "ChaserTargetFromLocalVariable"
    );
    ControlKind {
        team: u8::from(enemy),
        ai_none,
        local_device,
    }
}

/// Whether a blow from team `from` spares team `to` (the same team:
/// `ignoreDamage`, `isFriend`, the explosion's check).
pub fn spares(from: Option<Team>, to: Option<Team>) -> bool {
    matches!((from, to), (Some(a), Some(b)) if a == b)
}
