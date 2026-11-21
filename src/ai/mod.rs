//! Enemy controls: what writes the `PlayerIntent` of the map's simple cars
//! and of its `Players` cars other than the player's (`car::Role::AiCar`:
//! a `Car` the AI drives as the devices drive the player's).
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
//! | [`nodes`] | the shared nodes: `DecoratorCondition`, `Timer`, `All`, `Optional`, `FindNearestEnemy`, `FireToTarget`, `MoveBase`, `MoveToPoint`, `FollowBy`, … | `BTLib`, `BTActions/*` |
//! | [`trees`] | the trees put together from them: `TurretAi`, `ChaserAi`, `FollowPathAi`, `AttackerAiVer2`, `ZombieFlocking`, `HammerAi`, … and their subtrees | `BTActionFactory` |
//! | [`order_queue`] | [`OrderQueue`]: the orders a map gives a car (`MoveOrder`, `AIClearOrders`) | `OrdersManager`, `ActionRunner.MoveOrderRun`, `AIClearOrdersRun` |
//! | [`navigation`] | [`Navigation`]: the map's navigation graph ([`crate::map::nav`]) as a resource, `SetPath`, `--nav` | `MapManager._navigation`, `ActionRunner.SetPathRun` |
//! | [`fire`] | `AlwaysShoot`, `ShootPattern`: plain controls, not trees | `AiSimpleControl`, `PeriodicFireControl` |
//!
//! The controls that are live:
//!
//! | `ControlType` | Control |
//! |---|---|
//! | `AiSimpleTurret`, `AiSimpleTurretFriend` | a flatbt `Behavior` running `TurretAi` over an [`AiBlackboard`]: finds an enemy within `ScanDistance` in sight, turns the gun on it, fires when it points there; scans otherwise |
//! | `AiChaser` | `BTActionFactory.Chase`: finds the nearest enemy anywhere on the map (`ScanDistance` = the room's width and height), follows it by a path from the navigation graph (`FollowBy`: the path is extended while the target stays near its end and made again when it is old or the target has moved on), fires when the gun points at it, goes to where it was last seen when it is lost, and sleeps two seconds when there is nobody |
//! | `MoveByPath`, `MoveByPathFriend` | `BTActionFactory.FollowPathAi`: follows the `Path` that `SetPath` gave it (`MoveBase`), firing at anyone in the gun's line at most every 3 s (`TryFireToAnyone`); with no path (or when an open one is done) it runs the action `AIAdjustment` gave it (`ActionOnBTTrigger`) once |
//! | `MoveByPathSafelyFriend` | `FollowPathAiSafe`: the same, but runs from an enemy in sight |
//! | `GoToEnemySimplePredictionPosition` | finds an enemy and goes straight at where it will be (`MoveTowardsTargetNoPath`) |
//! | `AttackAiVer2` | `BTActionFactory.AttackerAiVer2`: under half health and hit, hides from the enemy (`PanicBehaviour`); else finds an enemy within `ScanDistance` and, in range and in sight, stands, turns to it (`AimToTarget`), fires 1.5 s and goes to a random place in sight of where it was last seen (`Maneur`; a hit car does so at once), or follows to that place when out of range; with nobody near, walks to a random navigation node and rests 2 s |
//! | `SimpleAttackNearestEnemy` | `SimpleAttack` (the turret's attack, `ScanDistance` 20), else idle |
//! | `SimpleFollowByEnemyAi` | finds an enemy (`ScanDistance` 20) and steers at where it will be, no firing; else idle |
//! | `MoveAndFireInInitDirection` | stick and gun along the way the car faced when its tree started, the trigger down |
//! | `ZombieFlocking`, `ZombieFlockingFriend` | `BTActionFactory.ZombieFlocking` (`ScanDistance` 12): alone near its target with its wave ready, stands 2 s and fires it; hurt with nobody to fight, tells its idle friends who did it; flocks at the nearest enemy; else follows a leader (the player first: runs `ActionOnBTTrigger` once, follows by a path when out of sight) or wanders with its flock ([`trees::zombie_flocking`]) |
//! | `SimpleFlocking` | `BTActionFactory.SimpleFlocking`: flocks at half speed with an enemy near, else at a tenth |
//! | `HammerAi`, `HammerAiFriend` | `BTActionFactory.HammerAi`: chases the nearest enemy within `ScanDistance` for a second, charges at full stick by a path to where it was then (up to 3 s), and, while it lives, backs off at half the stick to a random place in sight for a second before the next round; with nobody, sleeps 2 s and wanders ([`trees::hammer_ai`]) |
//! | `OrderExecutorFriend`, `OrderExecutorAi` | `BTActionFactory.OrderExecutorAi` (`ExecuteOrders`): carries out the orders the map gives it (`MoveOrder`: drives by a path to a point), first to last, each dropped as its `OrderRemoveCondition` says; with none it does nothing ([`trees::order_executor_ai`], [`order_queue`]) |
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
//! `OptionalActionNode`, `RepeatUntilSuccessDecorator`,
//! `Condition` (with the trees that use them), `DynamicNode` as a node of
//! its own (its one use, `ExecuteOrders`, is an action over the order queue:
//! [`nodes::ExecuteOrders`]), `BTBuilder`'s single-child
//! shortcut (a `Selector` or `Sequence` of one child is the child).
//!
//! # Paths
//!
//! The trees that drive (`ChaserAi`, `FollowPathAi`) steer at a *carrot* on
//! the blackboard's `Path` (see [`nodes`]' `MoveBase` and
//! [`crate::map::nav::NavPath`]); the path is the map's (`SetPath`: closed or
//! open, `Path` or `Patrol`) or made by the navigation graph
//! ([`crate::map::nav`], the resource is [`Navigation`], handed to each
//! blackboard by the gather). A car that is stuck (two seconds on an open path
//! without having moved half a unit while pushing) gives the path up. What
//! `Control.ClearAxis` did is [`AiBlackboard::clear_stick`], which
//! `carry_out` turns into a zero stick; `MapManager.Triggers.RunAction` is
//! queued by the tree ([`AiBlackboard::run_action`]) and sent by
//! `send_trigger_actions`.
//!
//! Everything else that is an `AIBTBaseControl` waits for the trees that use
//! the nodes not yet ported (`docs/BACKLOG.md` says which node each needs):
//! `Ai`, `AiFriend`, `AiSimpleTurretFriendFireAvailableTarget`, `GuardAi`,
//! `SupportAi*`, `CaptureTheBox*`,
//! `ChaserTargetFromLocalVariable`. Cars with those stand idle, as
//! before, and `AIAdjustment` sets only their team. `AITurret` and
//! `AISimpleCarControl` are not reached by any `ControlType`;
//! `AIPathCarControl` (the base of `AISimpleCarControl`,
//! `AITouchAndRunCarControl` and the player's `PlayerCarControlPath`: they
//! call `MapManager.GetPath`, which [`Navigation::get_path`] now is) only by
//! game modes the port does not have: not ported.
//!
//! [`Foe`]s are the cars alive (`Health` above 0) and not deactivated; the
//! team decides who is an enemy. For the flocking trees the same cars are
//! also gathered as [`Mate`]s (with the player's car marked as
//! `LocalDevice`: [`LocalPlayer`], and what each AI car's blackboard says, a
//! [`Mind`]), in the map's order (map id, as `ControlableEntities` kept
//! them). What those trees do to others is applied after the trees run:
//! `Uii`'s alarms ([`alert_friends`]: a friend still idle takes the damager,
//! with the red ping of `CreateBip2` when the camera sees it) and the
//! `Power` streaks ([`draw_effects`]); the trigger pulled in a thinking pass
//! (`Fire!`) is pulled by [`carry_out`] for that step. What a turret remembers of who hurt it
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
pub mod navigation;
pub mod nodes;
pub mod order_queue;
pub mod orders;
pub mod trees;

use avian2d::prelude::*;
use bevy::prelude::*;
use flatbt_bevy::prelude::*;

use crate::actions::{Inactive, RunAction};
use crate::car::CarRadius;
use crate::car::control::Team;
use crate::car::shield::Shield;
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::particles::styles::{BOMB_BEEP, TELEPORT_STREAK};
use crate::render::camera::MainCamera;
use crate::render::particles::Particles;
use crate::weapon::aim_help::in_view;
use crate::explosion::{Damaged, ExplosionSystems, Health};
use crate::game::{GameState, LoadedMap, MapId};
use crate::weapon::Weapon;
pub use fire::{AlwaysShoot, ShootPattern};
pub use blackboard::{AiBlackboard, Alert, Body, Effect, Foe, Gun, Mate, Mind};
pub use navigation::{Navigation, SharedTerrain};
pub use order_queue::{Order, OrderKind, OrderQueue};
pub use orders::Orders;

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        // Every tree `set_control` gives a car needs its plugin here, or the
        // car holds the tree and it never ticks (and its `stop_behavior` in
        // `set_control`).
        app.add_plugins((
            BehaviorPlugin::for_tree(trees::turret_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::chaser_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::follow_path_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::follow_path_ai_safe).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::go_to_enemy_simple_prediction_position)
                .in_schedule(FixedUpdate)
                .tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::attacker_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::attacker_ai_ver2).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::panic_behaviour).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::simple_attack_nearest_enemy).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::simple_follow_by_enemy_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::move_in_one_direction_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::zombie_flocking).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::simple_flocking).in_schedule(FixedUpdate).tick_mode(tick_mode),
            BehaviorPlugin::for_tree(trees::hammer_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            navigation::NavigationPlugin,
        ))
        .add_plugins((
            BehaviorPlugin::for_tree(trees::order_executor_ai).in_schedule(FixedUpdate).tick_mode(tick_mode),
            order_queue::OrdersPlugin,
        ))
        .configure_sets(
            FixedUpdate,
            BehaviorSystems.before(crate::car::drive_cars).run_if(in_state(GameState::Playing)).run_if(stepping),
        )
        .add_systems(
            FixedUpdate,
            (
                gather_blackboards.before(BehaviorSystems),
                (carry_out.in_set(crate::controls::IntentSystems), send_trigger_actions, (alert_friends, draw_effects).chain())
                    .after(BehaviorSystems)
                    .run_if(stepping),
                (run_always_shoot, run_shoot_patterns).in_set(crate::controls::IntentSystems),
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
    car.remove::<(AiBlackboard, AlwaysShoot, ShootPattern)>()
        .stop_behavior(trees::turret_ai)
        .stop_behavior(trees::chaser_ai)
        .stop_behavior(trees::follow_path_ai)
        .stop_behavior(trees::follow_path_ai_safe)
        .stop_behavior(trees::go_to_enemy_simple_prediction_position)
        .stop_behavior(trees::attacker_ai)
        .stop_behavior(trees::attacker_ai_ver2)
        .stop_behavior(trees::panic_behaviour)
        .stop_behavior(trees::simple_attack_nearest_enemy)
        .stop_behavior(trees::simple_follow_by_enemy_ai)
        .stop_behavior(trees::move_in_one_direction_ai)
        .stop_behavior(trees::zombie_flocking)
        .stop_behavior(trees::simple_flocking)
        .stop_behavior(trees::hammer_ai)
        .stop_behavior(trees::order_executor_ai);
    #[cfg(debug_assertions)]
    car.remove::<DebugBehavior>();
    let mut brain = AiBlackboard::new(seed);
    match control.trim() {
        "AiSimpleTurret" | "AiSimpleTurretFriend" => {
            car.insert((brain, Behavior::for_tree(trees::turret_ai)));
        }
        // `ScanDistance` = the room's width and height.
        "AiChaser" => {
            brain.scan_distance = trees::CHASER_SCAN_DISTANCE;
            car.insert((brain, Behavior::for_tree(trees::chaser_ai)));
        }
        // `TryFireToAnyone` asks `GetEnemies` as it ticks.
        "MoveByPath" | "MoveByPathFriend" => {
            brain.watch_enemies = true;
            car.insert((brain, Behavior::for_tree(trees::follow_path_ai)));
        }
        "MoveByPathSafelyFriend" => {
            brain.watch_enemies = true;
            car.insert((brain, Behavior::for_tree(trees::follow_path_ai_safe)));
        }
        "GoToEnemySimplePredictionPosition" => {
            car.insert((brain, Behavior::for_tree(trees::go_to_enemy_simple_prediction_position)));
        }
        "AttackAi" | "AttackAiFriend" => {
            car.insert((brain, Behavior::for_tree(trees::attacker_ai)));
        }
        "AttackAiVer2" => {
            car.insert((brain, Behavior::for_tree(trees::attacker_ai_ver2)));
        }
        // `ChickenAi` is `PanicBehaviour` alone: hide from an enemy in sight.
        "ChickenAi" => {
            car.insert((brain, Behavior::for_tree(trees::panic_behaviour)));
        }
        "SimpleAttackNearestEnemy" => {
            brain.scan_distance = trees::SIMPLE_SCAN_DISTANCE;
            car.insert((brain, Behavior::for_tree(trees::simple_attack_nearest_enemy)));
        }
        "SimpleFollowByEnemyAi" => {
            brain.scan_distance = trees::SIMPLE_SCAN_DISTANCE;
            car.insert((brain, Behavior::for_tree(trees::simple_follow_by_enemy_ai)));
        }
        "MoveAndFireInInitDirection" => {
            car.insert((brain, Behavior::for_tree(trees::move_in_one_direction_ai)));
        }
        // `ScanDistance = 12`; the team is the control's (`car::control`).
        "ZombieFlocking" | "ZombieFlockingFriend" => {
            brain.scan_distance = trees::ZOMBIE_SCAN_DISTANCE;
            brain.watch_friends = true;
            car.insert((brain, Behavior::for_tree(trees::zombie_flocking)));
        }
        "SimpleFlocking" => {
            brain.watch_friends = true;
            car.insert((brain, Behavior::for_tree(trees::simple_flocking)));
        }
        // The default `ScanDistance`; the team is the control's (`car::control`).
        "HammerAi" | "HammerAiFriend" => {
            car.insert((brain, Behavior::for_tree(trees::hammer_ai)));
        }
        // The default `ScanDistance`; the team is the control's (`car::control`).
        "OrderExecutorFriend" | "OrderExecutorAi" => {
            car.insert((brain, Behavior::for_tree(trees::order_executor_ai)));
        }
        "AlwaysShoot" => {
            car.insert(AlwaysShoot);
            return;
        }
        "ShootPattern" => {
            let mut rng = <rand::rngs::SmallRng as rand::SeedableRng>::seed_from_u64(seed);
            car.insert(ShootPattern::random(&mut rng));
            return;
        }
        _ => return,
    }
    #[cfg(debug_assertions)]
    car.insert(DebugBehavior::default());
}

/// Whether the port has a live control for a `ControlType` (see the table
/// above).
pub fn is_live(control: &str) -> bool {
    matches!(
        control.trim(),
        "AiSimpleTurret"
            | "AiSimpleTurretFriend"
            | "AiChaser"
            | "MoveByPath"
            | "MoveByPathFriend"
            | "MoveByPathSafelyFriend"
            | "GoToEnemySimplePredictionPosition"
            | "AttackAi"
            | "AttackAiFriend"
            | "AttackAiVer2"
            | "ChickenAi"
            | "SimpleAttackNearestEnemy"
            | "SimpleFollowByEnemyAi"
            | "MoveAndFireInInitDirection"
            | "ZombieFlocking"
            | "ZombieFlockingFriend"
            | "SimpleFlocking"
            | "HammerAi"
            | "HammerAiFriend"
            | "OrderExecutorFriend"
            | "OrderExecutorAi"
            | "AlwaysShoot"
            | "ShootPattern"
    )
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn gather_blackboards(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    map: Res<LoadedMap>,
    cars: Query<
        (Entity, &Team, &Position, Option<&LinearVelocity>, &CarRadius, &Health, Option<&Weapon>, &PlayerIntent, Has<LocalPlayer>, Option<&MapId>),
        Without<Inactive>,
    >,
    mut brains: Query<(
        Entity,
        &mut AiBlackboard,
        Has<Orders>,
        &Position,
        &Rotation,
        &Team,
        &Health,
        Option<&Weapon>,
        Has<Inactive>,
        Option<&LinearVelocity>,
        &CarRadius,
        &RigidBody,
        &PlayerIntent,
        Option<&Shield>,
    )>,
    navigation: Option<Res<Navigation>>,
    terrain: Option<Res<SharedTerrain>>,
    mut foes: Local<Vec<Foe>>,
    mut mates: Local<Vec<Mate>>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    foes.clear();
    foes.extend(cars.iter().filter(|c| c.5.hp > 0).map(|(entity, team, pos, vel, radius, _, weapon, ..)| Foe {
        entity,
        team: team.0,
        pos: pos.0,
        vel: vel.map_or(Vec2::ZERO, |v| v.0),
        radius: radius.0,
        weapon_range: weapon.map(|w| w.options.range),
    }));
    // The flocking trees' friends (`GetFriends`), with what their
    // blackboards say, in the map's order (`EntitiesManager.ControlableEntities`).
    mates.clear();
    if brains.iter().any(|b| b.1.watch_friends) {
        let local = |e: Entity| cars.get(e).is_ok_and(|c| c.8);
        let minds: Vec<(Entity, Mind)> = brains
            .iter()
            .map(|b| {
                let mind = b.1.mind();
                (b.0, Mind { leader_local: mind.leader.is_some_and(local), ..mind })
            })
            .collect();
        let mut keyed: Vec<(u64, Mate)> = cars
            .iter()
            .filter(|c| c.5.hp > 0)
            .map(|(entity, team, pos, vel, _, _, _, intent, local, id)| {
                let key = id.map_or(u64::MAX, |i| i.0 as u64);
                let mate = Mate {
                    entity,
                    team: team.0,
                    pos: pos.0,
                    vel: vel.map_or(Vec2::ZERO, |v| v.0),
                    stick: intent.move_dir,
                    local,
                    mind: minds.iter().find(|m| m.0 == entity).map(|m| m.1),
                };
                (key, mate)
            })
            .collect();
        keyed.sort_by_key(|(key, mate)| (*key, mate.entity));
        mates.extend(keyed.into_iter().map(|k| k.1));
    }
    let blocked = |a: Vec2, b: Vec2| map.line_collides(a, b);
    for (own, mut blackboard, running, position, rotation, team, health, weapon, inactive, velocity, radius, rigid_body, intent, shield) in
        &mut brains
    {
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
            gun: weapon.map(|w| Gun {
                complex: w.kind.is_complex(),
                max_angle: w.aim.max_angle,
                range: w.fire_range(),
                immediate: w.kind.is_immediate(),
            }),
            velocity: velocity.map_or(Vec2::ZERO, |v| v.0),
            radius: radius.0,
            // `!IsStatic`: a simple car's or an AI `Car`'s.
            can_move: *rigid_body != RigidBody::Static,
            stick: intent.move_dir,
            health: health.hp,
            max_health: health.max,
            shield: shield.map_or(0, |s| s.points() as i32),
        };
        if blackboard.nav.is_none() {
            blackboard.nav = navigation.as_deref().cloned();
        }
        if blackboard.terrain.is_none() {
            blackboard.terrain = terrain.as_deref().map(|t| t.0.clone());
        }
        blackboard.gather(dt, running, body, &foes, &blocked);
        blackboard.gun_ready = weapon.is_some_and(Weapon::can_shoot);
        if blackboard.watch_friends {
            blackboard.gather_mates(own, &mates);
        }
    }
}

/// `AIBTBaseControl.UpdateControl`, its second half: writes the act of each
/// tree into the car's intent. A car whose tree ended this step has no act and
/// writes nothing (its trigger is let go).
#[allow(clippy::type_complexity)]
fn carry_out(mut brains: Query<(&mut PlayerIntent, Option<&Orders>, &Health, Has<Inactive>, &mut AiBlackboard)>) {
    for (mut intent, orders, health, inactive, mut blackboard) in &mut brains {
        // `ControlableEntity.Update`: nothing for a car that is inactive
        // or dead.
        let orders = match orders {
            Some(orders) if health.hp > 0 && !inactive => *orders,
            _ => Orders::default(),
        };
        // The trigger is pulled for the step it was ticked
        // (`Control.Fire &= ~FireType.Shoot` takes it after).
        // `Fire = Shoot` from a thinking pass (`Fire!`).
        let pulled = blackboard.bypass_change_detection().take_trigger() && health.hp > 0 && !inactive;
        set(&mut intent.fire, orders.fire || pulled);
        set(&mut intent.fire_accurate, orders.fire || pulled);
        // An action that ended let the stick go (`Control.ClearAxis`); what a
        // tick wrote after that stands.
        let cleared = blackboard.bypass_change_detection().take_stick_cleared();
        match orders.move_dir {
            Some(m) => set(&mut intent.move_dir, m),
            None if cleared => set(&mut intent.move_dir, Vec2::ZERO),
            None => {}
        }
        if let Some(a) = orders.aim_dir {
            set(&mut intent.aim_dir, a);
        }
    }
}

/// `MapManager.Triggers.RunAction(name, Entity.CustomId)` for the map actions
/// the trees asked for (`TriggerMapAction`).
fn send_trigger_actions(mut brains: Query<(&mut AiBlackboard, Option<&MapId>)>, mut run: MessageWriter<RunAction>) {
    for (mut blackboard, id) in &mut brains {
        if blackboard.has_actions() {
            for name in blackboard.take_actions() {
                run.write(RunAction { name, actor: id.map(|i| i.0) });
            }
        }
    }
}

/// `ZombieFlocking`'s `Uii`: each friend told of who hurt a car takes the
/// damager if it still has nobody to fight, and a red ping flashes on it
/// (`CreateBip2`, only when the camera sees it).
#[allow(clippy::type_complexity)]
fn alert_friends(
    mut brains: Query<(Entity, &mut AiBlackboard, &Position)>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut particles: Option<ResMut<Particles>>,
    mut alerts: Local<Vec<Alert>>,
) {
    alerts.clear();
    for (_, mut blackboard, _) in &mut brains {
        if blackboard.has_news() {
            alerts.extend(blackboard.take_alerts());
        }
    }
    let camera = cameras.single().ok();
    for alert in alerts.drain(..) {
        let Ok((_, mut friend, at)) = brains.get_mut(alert.friend) else {
            continue;
        };
        if friend.alerted(alert.damager, alert.left)
            && camera.is_none_or(|(c, t)| in_view(c, t, at.0, 0.0))
            && let Some(particles) = particles.as_mut()
        {
            particles.try_add(&BOMB_BEEP, at.0, 0.0, 0.0);
        }
    }
}

/// The effects the trees asked for (`Power`'s streaks).
fn draw_effects(mut brains: Query<&mut AiBlackboard>, mut particles: Option<ResMut<Particles>>) {
    for mut blackboard in &mut brains {
        if !blackboard.has_news() {
            continue;
        }
        for effect in blackboard.take_effects() {
            match (effect, particles.as_mut()) {
                (Effect::Teleport { at, direction }, Some(particles)) => {
                    particles.try_add(&TELEPORT_STREAK, at, direction, direction)
                }
                (_, None) => {}
            }
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
