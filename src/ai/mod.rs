//! Enemy controls: what writes the `PlayerIntent` of the map's simple cars.
//!
//! C# origin: `AiControlFactory` (which `ControlType` makes which control),
//! `BaseControl`, `AIBaseControl`, `AIBTBaseControl`, `AiSimpleControl`,
//! `PeriodicFireControl`, `BTActionFactory` (2010–2016). Every control wrote
//! an `IControl` (`CurrentAxisControl`, `WeaponAxisControl`, `Fire`) that
//! `ControlableEntity.ControlHandle` read; here they write the car's
//! [`PlayerIntent`] (`move_dir`, `aim_dir`, `fire` and `fire_accurate`, the
//! two flags of `FireType.Shoot`), the one surface the player's devices write
//! too, so the car, the gun and its rhythm stay the same code. They run every
//! fixed step before the cars drive and the guns fire, as `Control.Update`
//! ran before `ControlHandle`.
//!
//! | Module | What it is | C# |
//! |---|---|---|
//! | [`orders`] | [`Orders`]: what a tree makes the car do (flatbt's act) | `IControl` |
//! | [`blackboard`] | [`AiBlackboard`]: what the trees read and write | `AiBlackboard` |
//! | [`nodes`] | the shared nodes: `DecoratorCondition`, `Timer`, `All`, `FindNearestEnemy`, `FireToTarget`, … | `BTLib`, `BTActions/*` |
//! | [`trees`] | the trees put together from them: `TurretAi` and its subtrees | `BTActionFactory` |
//! | [`fire`] | `AlwaysShoot`, `ShootPattern`: plain controls, not trees | `AiSimpleControl`, `PeriodicFireControl` |
//!
//! The controls that are live:
//!
//! | `ControlType` | Control |
//! |---|---|
//! | `AiSimpleTurret`, `AiSimpleTurretFriend` | a flatbt `Behavior` running `TurretAi` over an [`AiBlackboard`]: finds an enemy within `ScanDistance` in sight, turns the gun on it, fires when it points there; scans otherwise |
//! | `AlwaysShoot` | [`AlwaysShoot`]: the trigger always down, the gun where the car points |
//! | `ShootPattern` | [`ShootPattern`]: a random pattern of quarter-second silences and bursts |
//! | `NoneLocal`, `NoneEnemy`, `NoneFriend` | nothing (`AINone`; see `car::control`) |
//!
//! `AiSimpleTurret` is a behaviour tree in C# (`AIBTBaseControl` with
//! `TurretAi`), not the `AITurret` class (commented out in the factory). What
//! the turret does, and how it is run (`AIBTBaseControl.UpdateControl`,
//! `BT.Context.Update`):
//!
//! - Every step the running action is ticked: the attack aims at the target
//!   (its position 0.2 s ahead), pulls the trigger when the target is within
//!   the weapon's fire range *as the gun points now* (`TargetInFireRange`,
//!   with the radius scaled by `FireAccuracy`: more than 1 fires at things
//!   further off the line), and steers the car at the target (a car that can
//!   move chases, a static one only turns its gun, as the aim is the same).
//! - The tree is thought over again every 0.25 s (a `Sparse` timer, so cars
//!   are out of step), at once when the action ends, and every step while
//!   nothing runs. A running attack is not left for a nearer enemy; scanning
//!   is dropped for an attack at once (see below).
//! - The attack starts only when the target is in sight now or in 0.5 s
//!   (`SeeTarget | WillSeeTarget`, `MapManager.LineCollide`), and goes on
//!   while that holds and the target lives; then the turret scans.
//! - Scanning: stand 1 s (the gun stays where it was), then pick a random
//!   direction within 110 % of the gun's `MaxAngle` of the car's heading (a
//!   gun with no angle: any direction) and turn the gun there for 1 s; then
//!   the tree ends and is started over.
//!
//! The control writes only what the tree writes: the aim and the stick stay
//! as they were between writes (the stick of a car that can move keeps its
//! last direction after the target is lost, as in C#), and the trigger is
//! pulled for the step it was ticked.
//!
//! # `BTLib`'s running rules on flatbt-bevy
//!
//! The trees are `flatbt` trees (the migration plan's "BTLib → flatbt"),
//! built once by [`flatbt_bevy::BehaviorPlugin`] and run by its tick over each
//! car's [`flatbt_bevy::Behavior`], the [`AiBlackboard`] being the tree's
//! context and [`Orders`] its act (a component on the car while its tree
//! runs: flatbt-bevy puts it on and takes it off). The plugin ticks in
//! `FixedUpdate`, in the [`BehaviorSystems`] set, which runs after the gather
//! (`gather_blackboards`) and before `carry_out` and the cars drive.
//!
//! `AIBTBaseControl.UpdateControl` called `Context.Update(time, force)` every
//! step, `force` being the 0.25 s `PeriodicUpdate.Sparse`. flatbt-bevy enters
//! a tree once per tick, as `Resume` or `Evaluate` as the tick mode answers;
//! the gather writes the answer into the blackboard
//! ([`AiBlackboard::tick`]) and the tick mode only reads it:
//!
//! | BTLib | flatbt |
//! |---|---|
//! | `Context.Run()`: the running action (`LastRunningNode`) is asked `IsInProgress` and, if so, `Tick`ed; nothing else in the tree is visited | `Resume`: follows the saved path to the action, which flatbt's `action` asks and ticks; its act is what the car does |
//! | the action is no longer in progress | the same `Resume` completes it (`Complete` → `Ok`/`Fail`) and its parents go on, as BTLib's thinking pass over the old running path did. A tree that ended is left until the next step; a resumed `Fail` is `Evaluate`d at once (flatbt-bevy's `Behavior::tick` does) |
//! | `Think()`, after `Blackboard.Update`: every `forceUpdate` (0.25 s), and every step while nothing runs (`LastRunningNode == null`) | `Evaluate`, after `AiBlackboard::update` (in `gather`); with nothing running (no [`Orders`] on the car) a fresh entry is `Evaluate` anyway. The 0.25 s timer is the blackboard's own (`evaluate_every` of flatbt-bevy staggers by entity and keeps no per-car phase, so cars would not be out of step by a random part of the period as `Sparse` made them) |
//! | `Selector`: tries the children from the first, the one that ran last time resumed (not started again) when reached | `select` under `Evaluate`: scans from child zero, the saved child updated in place; a candidate that fails keeps the old branch, one that runs replaces it: a running scan is dropped for an attack the moment `FindNearestEnemy` succeeds and the attack starts |
//! | `Sequence`: the running child is updated first and the children before it are not asked again | `seq` under `Evaluate` continues the active child: a running attack does not ask `FindNearestEnemy` again, so a nearer enemy does not take it over |
//! | `ActionNode.OnUpdate`: `Start` unless running, then `IsInProgress`, `Complete` when it ends (`Ok`/`Fail`), `Fail` when `Start` says no | `action(..)` over a `BtAction` (`start`, `is_in_progress`, `tick`, `complete`) |
//! | `ParallelAllActionNode` (`All`) | [`nodes::all`]: a `BtAction` over its children, `start`/`complete` of all (short-circuiting, as `Enumerable.All`), `is_in_progress` of all, every `tick` |
//! | `DecoratorCondtion` (`While`): `init` when not running, then the condition on each thinking pass; `Context.Run()` never visits it | [`nodes::DecoratorCondition`]: `Evaluate` runs `init` (when fresh) and the condition; `Resume` of a running one passes straight to the child |
//! | `Think` starts an action without ticking it | [`nodes::quiet_start`]: the tick in the update that started it gives no orders |
//!
//! What flatbt-bevy cannot say, and how it is handled: its tick is one pass,
//! where C# on a thinking step ticked the running action (`Run`) *and then*
//! thought. A forced step here is the thinking pass alone: a running action
//! that is still in progress is ticked in it (the same orders, the blackboard
//! just updated), but an action that the think ends or replaces misses its
//! last tick of that step (C# ticked it once more before dropping it): at
//! most one step (1/60 s) of the old orders. The gather also decides the
//! tick mode before the tick runs (a `TickFn` sees only the blackboard and
//! `TickAt`), which is why [`AiBlackboard::tick`] exists.
//!
//! Two consequences carried over from C#: a `Timer` ends at a thinking pass,
//! not at its time (`While`'s condition is not asked while its child runs), so
//! a 1 s timer ends at the first thought at or after the second (1 to
//! 1.25 s); and an action started in a thinking pass gives orders from the
//! next step only.
//!
//! Not ported of `BTLib`: `Context.ToString` and `RunningActionChanged` (the
//! AI log: the running path is logged on change, `RUST_LOG=tilerider::ai=debug`),
//! `OptionalActionNode`, `RepeatUntilSuccessDecorator`, `DynamicNode`,
//! `Condition` (with the trees that use them), `BTBuilder`'s single-child
//! shortcut (a `Selector` or `Sequence` of one child is the child).
//!
//! Everything else that is an `AIBTBaseControl` waits for the trees that use
//! the nodes not yet ported: `Ai`, `AiFriend`,
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
//! ([`remember_damagers`]) comes from the [`Damaged`] messages, the `by` of a
//! blow being whoever dealt it.
//!
//! Not ported: `UIManager.IsActive` and `World.ControlEnable` stopping every
//! control (there is no UI that does so; a paused game stops them with the
//! physics); `BaseControl`'s networking (`ContolUpdated`, `IsChanged`,
//! `ForceFeedback`, `CameraOffset`, `WorldPosition`: the camera and the
//! network of C#); `Draw` (the `DEBUG && AI` overlay of the control's
//! state: the running path is logged on change instead, with
//! `RUST_LOG=tilerider::ai=debug`); `TurretAiFireToAnyone`.

pub mod blackboard;
pub mod fire;
pub mod nodes;
pub mod orders;
pub mod trees;

use avian2d::prelude::*;
use bevy::prelude::*;
use flatbt_bevy::prelude::*;

use crate::actions::Inactive;
use crate::car::CarRadius;
use crate::car::control::Team;
use crate::controls::PlayerIntent;
use crate::explosion::{Damaged, ExplosionSystems, Health};
use crate::game::{GameState, LoadedMap, MapId};
use crate::weapon::Weapon;
use crate::weapon::model::Kind;
pub use fire::{AlwaysShoot, ShootPattern};
pub use blackboard::{AiBlackboard, Body, Foe, Gun};
pub use orders::Orders;

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            BehaviorPlugin::for_tree(trees::turret_ai)
                .in_schedule(FixedUpdate)
                .tick_mode(tick_mode),
        )
        .configure_sets(
            FixedUpdate,
            BehaviorSystems.before(crate::car::drive_cars).run_if(in_state(GameState::Playing)).run_if(stepping),
        )
        .add_systems(
            FixedUpdate,
            (
                gather_blackboards.before(BehaviorSystems),
                carry_out.after(BehaviorSystems).run_if(stepping),
                (run_always_shoot, run_shoot_patterns),
            )
                .before(crate::car::drive_cars)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            remember_damagers
                .after(ExplosionSystems)
                .run_if(in_state(GameState::Playing)),
        );
        #[cfg(debug_assertions)]
        app.add_systems(FixedUpdate, log_paths.after(BehaviorSystems));
    }
}

/// Gives a car the control a `ControlType` names (`AiControlFactory`),
/// taking the old one off. `seed` starts its random stream (the car's map
/// id). Controls that are not live (the behaviour-tree ones, `AINone`) leave
/// the car with none.
pub fn set_control(car: &mut EntityCommands, control: &str, seed: u64) {
    car.remove::<(AiBlackboard, AlwaysShoot, ShootPattern)>().stop_behavior(trees::turret_ai);
    #[cfg(debug_assertions)]
    car.remove::<DebugBehavior>();
    match control.trim() {
        "AiSimpleTurret" | "AiSimpleTurretFriend" => {
            car.insert((AiBlackboard::new(seed), Behavior::for_tree(trees::turret_ai)));
            #[cfg(debug_assertions)]
            car.insert(DebugBehavior::default());
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

/// How a car's tree is entered this step: what the gather decided
/// ([`AiBlackboard::tick`]).
pub fn tick_mode(bb: &AiBlackboard, _: TickAt) -> Tick {
    bb.tick
}

/// Whether the game steps: not paused, and time moves.
fn stepping(time: Res<Time>, physics: Res<Time<Physics>>) -> bool {
    !physics.is_paused() && time.delta_secs() > 0.0
}

/// `AIBTBaseControl.UpdateControl`, its first half: copies what each AI car's
/// blackboard reads of the world into it, and decides how its tree is
/// entered ([`AiBlackboard::gather`]).
#[allow(clippy::type_complexity)]
fn gather_blackboards(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    map: Res<LoadedMap>,
    cars: Query<(Entity, &Team, &Position, Option<&LinearVelocity>, &CarRadius, &Health), Without<Inactive>>,
    mut brains: Query<(&mut AiBlackboard, Has<Orders>, &Position, &Rotation, &Team, &Health, Option<&Weapon>, Has<Inactive>)>,
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
    for (mut blackboard, running, position, rotation, team, health, weapon, inactive) in &mut brains {
        // `ControlableEntity.Update`: nothing for a car that is inactive
        // or dead.
        if health.hp <= 0 || inactive {
            blackboard.tick = Tick::Skip;
            continue;
        }
        let body = Body {
            pos: position.0,
            heading: rotation.as_radians(),
            team: team.0,
            gun: weapon.map(|w| Gun { complex: w.kind != Kind::Hammer, max_angle: w.aim.max_angle, range: w.fire_range() }),
        };
        blackboard.gather(dt, running, body, &foes, &blocked);
    }
}

/// `AIBTBaseControl.UpdateControl`, its second half: writes the act of each
/// tree into the car's intent. A car whose tree ended this step has no act and
/// writes nothing (its trigger is let go).
#[allow(clippy::type_complexity)]
fn carry_out(mut brains: Query<(&mut PlayerIntent, Option<&Orders>, &Health, Has<Inactive>), With<AiBlackboard>>) {
    for (mut intent, orders, health, inactive) in &mut brains {
        // `ControlableEntity.Update`: nothing for a car that is inactive
        // or dead.
        let orders = match orders {
            Some(orders) if health.hp > 0 && !inactive => *orders,
            _ => Orders::default(),
        };
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

/// `RUST_LOG=tilerider::ai=debug`: the running path when it changes
/// (`RunningActionChanged`).
#[cfg(debug_assertions)]
fn log_paths(brains: Query<(Entity, Option<&MapId>, &DebugBehavior), Changed<DebugBehavior>>) {
    for (entity, id, path) in &brains {
        debug!("brain {entity} (map id {:?}): {}", id.map(|i| i.0), path.path());
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
fn remember_damagers(mut damaged: MessageReader<Damaged>, mut blackboards: Query<&mut AiBlackboard>, cars: Query<(), With<Team>>) {
    for blow in damaged.read() {
        if blow.damage > 0
            && let Some(by) = blow.by
            && cars.contains(by)
            && let Ok(mut blackboard) = blackboards.get_mut(blow.target)
        {
            blackboard.hurt_by(by);
        }
    }
}
