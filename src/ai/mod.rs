//! Enemy controls: what writes the `PlayerIntent` of the map's simple cars.
//!
//! C# origin: `AiControlFactory` (which `ControlType` makes which control),
//! `BaseControl`, `AIBaseControl`, `AIBTBaseControl`, `AiSimpleControl`,
//! `PeriodicFireControl`, `BTActionFactory.TurretAi` (2010–2016). Every
//! control wrote an `IControl` (`CurrentAxisControl`, `WeaponAxisControl`,
//! `Fire`) that `ControlableEntity.ControlHandle` read; here they write the
//! car's [`PlayerIntent`] (`move_dir`, `aim_dir`, `fire` and
//! `fire_accurate`, the two flags of `FireType.Shoot`), the one surface the
//! player's devices write too, so the car, the gun and its rhythm stay the
//! same code. They run every fixed step before the cars drive and the guns
//! fire, as `Control.Update` ran before `ControlHandle`.
//!
//! The controls that are live:
//!
//! | `ControlType` | Control | Module |
//! |---|---|---|
//! | `AiSimpleTurret`, `AiSimpleTurretFriend` | [`Turret`]: finds an enemy within `ScanDistance` in sight, turns the gun on it, fires when it points there; scans otherwise | [`turret`] |
//! | `AlwaysShoot` | [`AlwaysShoot`]: the trigger always down, the gun where the car points | [`fire`] |
//! | `ShootPattern` | [`ShootPattern`]: a random pattern of quarter-second silences and bursts | [`fire`] |
//! | `NoneLocal`, `NoneEnemy`, `NoneFriend` | nothing (`AINone`; see `car::control`) | |
//!
//! `AiSimpleTurret` is a behaviour tree in C# (`AIBTBaseControl` with
//! `TurretAi`), not the `AITurret` class (commented out in the factory);
//! its tree is ported flat, in [`turret`]. Everything else that is an
//! `AIBTBaseControl` waits for the behaviour-tree slice: `Ai`, `AiFriend`,
//! `AiSimpleTurretFriendFireAvailableTarget`, `AiChaser`, `MoveByPath*`,
//! `SimpleFlocking`, `ZombieFlocking*`, `SimpleAttackNearestEnemy`,
//! `SimpleFollowByEnemyAi`, `MoveAndFireInInitDirection`, `GuardAi`,
//! `SupportAi*`, `OrderExecutor*`, `CaptureTheBox*`, `AttackAi*`, `HammerAi*`,
//! `ChickenAi`, `GoToEnemySimplePredictionPosition`,
//! `ChaserTargetFromLocalVariable`. Cars with those stand idle, as before,
//! and `AIAdjustment` sets only their team. `AITurret`, `AISimpleCarControl`,
//! `AITouchAndRunCarControl` and `AIPathCarControl` are not reached by any
//! `ControlType` (the last two only by game modes the port does not have,
//! and they need `NavGraph`): not ported.
//!
//! [`Foe`]s are the cars alive (`Health` above 0) and not deactivated; the
//! team decides who is an enemy. What a turret remembers of who hurt it
//! ([`remember_damagers`]) comes from the [`Damaged`] messages, the `by` of
//! a blow being whoever dealt it.
//!
//! Not ported: `UIManager.IsActive` and `World.ControlEnable` stopping every
//! control (there is no UI that does so; a paused game stops them with the
//! physics); `BaseControl`'s networking (`ContolUpdated`, `IsChanged`,
//! `ForceFeedback`, `CameraOffset`, `WorldPosition`: the camera and the
//! network of C#); `Draw`.

pub mod fire;
pub mod turret;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::actions::Inactive;
use crate::car::CarRadius;
use crate::car::control::Team;
use crate::controls::PlayerIntent;
use crate::explosion::{Damaged, ExplosionSystems, Health};
use crate::game::{GameState, LoadedMap, MapId};
use crate::weapon::Weapon;
use crate::weapon::model::Kind;
pub use fire::{AlwaysShoot, ShootPattern};
pub use turret::{Foe, Turret};

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (run_turrets, run_always_shoot, run_shoot_patterns)
                .before(crate::car::drive_cars)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            remember_damagers
                .after(ExplosionSystems)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Gives a car the control a `ControlType` names (`AiControlFactory`),
/// taking the old one off. `seed` starts its random stream (the car's map
/// id). Controls that are not live (the behaviour-tree ones, `AINone`) leave
/// the car with none.
pub fn set_control(car: &mut EntityCommands, control: &str, seed: u64) {
    car.remove::<(Turret, AlwaysShoot, ShootPattern)>();
    match control.trim() {
        "AiSimpleTurret" | "AiSimpleTurretFriend" => {
            car.insert(Turret::new(seed));
        }
        "AlwaysShoot" => {
            car.insert(AlwaysShoot);
        }
        "ShootPattern" => {
            let mut rng = <rand::rngs::SmallRng as rand::SeedableRng>::seed_from_u64(seed);
            car.insert(ShootPattern::random(&mut rng));
        }
        _ => {}
    }
}

/// Whether the port has a live control for a `ControlType` (see the table
/// above).
pub fn is_live(control: &str) -> bool {
    matches!(control.trim(), "AiSimpleTurret" | "AiSimpleTurretFriend" | "AlwaysShoot" | "ShootPattern")
}

/// Writes `value` only when it differs, so the intent is not marked
/// changed every step.
fn set<T: PartialEq>(slot: &mut T, value: T) {
    if *slot != value {
        *slot = value;
    }
}

/// `AIBTBaseControl.UpdateControl` for the turrets.
#[allow(clippy::type_complexity)]
fn run_turrets(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    map: Res<LoadedMap>,
    cars: Query<(Entity, &Team, &Position, Option<&LinearVelocity>, &CarRadius, &Health), Without<Inactive>>,
    mut turrets: Query<(Entity, Option<&MapId>, &mut Turret, &mut PlayerIntent, &Position, &Rotation, &Team, &Health, Option<&Weapon>, Has<Inactive>)>,
    mut foes: Local<Vec<Foe>>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    foes.clear();
    foes.extend(cars.iter().filter(|c| c.5.hp > 0).map(|(entity, team, pos, vel, radius, _)| Foe {
        entity,
        team: team.0,
        pos: pos.0,
        vel: vel.map_or(Vec2::ZERO, |v| v.0),
        radius: radius.0,
    }));
    let blocked = |a: Vec2, b: Vec2| map.line_collides(a, b);
    for (entity, id, mut turret, mut intent, position, rotation, team, health, weapon, inactive) in &mut turrets {
        // `ControlableEntity.Update`: nothing for a car that is inactive
        // or dead.
        if health.hp <= 0 || inactive {
            set(&mut intent.fire, false);
            continue;
        }
        let in_range = |target: Vec2, radius: f32| weapon.is_some_and(|w| w.in_fire_range(target, radius));
        let scene = turret::Scene {
            pos: position.0,
            heading: rotation.as_radians(),
            team: team.0,
            gun: weapon.map(|w| turret::Gun {
                complex: w.kind != Kind::Hammer,
                max_angle: w.aim.max_angle,
                in_range: &in_range,
            }),
            cars: &foes,
            blocked: &blocked,
        };
        let phase = turret.phase();
        let orders = turret.step(dt, &scene);
        // `RUST_LOG=tilerider::ai=debug`.
        if turret.phase() != phase {
            debug!("turret {entity} (map id {:?}) {phase:?} -> {:?}, target {:?}", id.map(|i| i.0), turret.phase(), turret.target());
        }
        // The trigger is pulled for the step it was ticked
        // (`Control.Fire &= ~FireType.Shoot` takes it after).
        set(&mut intent.fire, orders.fire);
        set(&mut intent.fire_accurate, orders.fire);
        if let Some(m) = orders.move_dir {
            set(&mut intent.move_dir, m);
        }
        if let Some(a) = orders.aim_dir {
            set(&mut intent.aim_dir, a);
        }
    }
}

/// `AlwaysShoot`: `Fire = Shoot; WeaponAxisControl = DirectionVector`.
#[allow(clippy::type_complexity)]
fn run_always_shoot(
    physics: Res<Time<Physics>>,
    mut cars: Query<(&mut PlayerIntent, &Rotation, &Health, Has<Inactive>), (With<AlwaysShoot>, Without<ShootPattern>)>,
) {
    if physics.is_paused() {
        return;
    }
    for (mut intent, rotation, health, inactive) in &mut cars {
        if health.hp <= 0 || inactive {
            continue;
        }
        set(&mut intent.fire, true);
        set(&mut intent.fire_accurate, true);
        set(&mut intent.aim_dir, Vec2::from_angle(rotation.as_radians()));
    }
}

/// `PeriodicFireControl.UpdateControl`.
#[allow(clippy::type_complexity)]
fn run_shoot_patterns(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    mut cars: Query<(&mut ShootPattern, &mut PlayerIntent, &Rotation, &Health, Has<Inactive>), Without<AlwaysShoot>>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    for (mut pattern, mut intent, rotation, health, inactive) in &mut cars {
        if health.hp <= 0 || inactive {
            continue;
        }
        let firing = pattern.step(dt);
        set(&mut intent.fire, firing);
        set(&mut intent.fire_accurate, firing);
        if firing {
            set(&mut intent.aim_dir, Vec2::from_angle(rotation.as_radians()));
        }
    }
}

/// `AiBlackboard.Entity_OnTakeDamage`: a turret remembers the car that hurt
/// it.
fn remember_damagers(mut damaged: MessageReader<Damaged>, mut turrets: Query<&mut Turret>, cars: Query<(), With<Team>>) {
    for blow in damaged.read() {
        if blow.damage > 0
            && let Some(by) = blow.by
            && cars.contains(by)
            && let Ok(mut turret) = turrets.get_mut(blow.target)
        {
            turret.hurt_by(by);
        }
    }
}
