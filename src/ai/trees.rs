//! The behaviour trees: `BTActionFactory`, built from [`super::nodes`].
//!
//! C# origin: `BTActionFactory` (2013–2016) put its trees together with
//! `BTBuilder` (`bt.Selector`, `bt.Sequence`, `bt.All`, `bt.While`, …) from
//! the nodes in `BTActions/`. Each function here is one of its trees or
//! subtrees, written beside its C#:
//!
//! ```text
//! C#                                              Rust
//! Timer(t, child) =                               timer(t, child)
//!   bt.While("Timer_ts", b => b.StartTimer(),       (a node keeping its start; asked at a think)
//!     b => b.Timer.TotalSeconds < t ? Running : Ok, child)
//!
//! SimpleAttackTarget(p) =                         simple_attack_target(p) =
//!   bt.Selector("A",                                select((
//!     bt.All("Fire",                                  action(all((FireToTarget(p),
//!       FireToTarget(p),                                          SimpleFollowByTarget(p)))),
//!       SimpleFollowByTarget(p)),                     action(FireToTarget(p)),
//!     FireToTarget(p))                              ))
//!
//! SimpleAttack =                                  simple_attack() =
//!   bt.Sequence("Attack",                           seq((
//!     FindNearestEnemy,                               leaf(find_nearest_enemy),
//!     SimpleAttackTarget(0.2f))                       simple_attack_target(0.2),
//!                                                   ))
//! SimpleScanning =                                simple_scanning() =
//!   bt.Sequence("Idle",                             seq((
//!     Timer(1.0f, bt.Idle),                           timer(1.0, action(Idle)),
//!     Timer(1.0f, bt.Action("Scanning", ..)))         timer(1.0, action(Scanning)),
//!                                                   ))
//! TurretAi =                                      turret_ai() =
//!   bt.Selector("TurretBrain",                      select((
//!     SimpleAttack, SimpleScanning)                   simple_attack(), simple_scanning(),
//!                                                   ))
//! ```
//!
//! Every action is written `act(..)` (not shown above): named after its own
//! type in debug paths.
//!
//! `SimpleAttackTarget`'s second branch (`FireToTarget` alone) is kept so the
//! tree is C#'s, though it can never run: the first branch starts exactly
//! when it would (`All` starts when `FireToTarget` and `SimpleFollowByTarget`
//! do, and the second needs a target, which `FireToTarget` needs too) and is
//! left under the same conditions, so whenever the first is not running the
//! second fails at its start. (With no weapon the first starts, is at once
//! not in progress and completes `Fail` (the target lives); the second does
//! the same.) It costs a failed start; `tests/ai_turret.rs` checks that it never runs.
//!
//! The trees that drive along paths (`MoveBase`, `MoveToPoint`, `FollowBy`):
//!
//! ```text
//! C#                                              Rust
//! Chase =                                         chase() =
//!   bt.Selector("ChaseAI",                          select((
//!     bt.Sequence("Chase",                            seq((
//!       FindNearestEnemy,                               leaf(find_nearest_enemy),
//!       WhileTargetAlive(                               while_target_alive(
//!         bt.All("Chase and fire",                        action(all((
//!           bt.Optional(FireToTarget()),                    optional(FireToTarget(0)),
//!           FollowBy("Target",                              FollowBy(target's safe position)
//!             b => b.Target.SafePosition)))),             )))))),
//!     MoveToLastKnownTargetPosition =                 seq((
//!       bt.Sequence(                                    check(has a live target),
//!         bt.Condition("HastTarget", …),                action(MoveToPoint(last known position)),
//!         MoveToPoint("MoveToLastKnownPos",           )),
//!           b => b.LastKnowTargetPosition)),          seq((action(Sleep(2)), action(Idle))),
//!     bt.Sequence("Sleep", Sleep2Sec, bt.Idle))     ))
//! ChaserAi = Chase                                chaser_ai() = chase()
//!
//! FollowPathAi =                                  follow_path_ai() =
//!   bt.Selector("Brain",                            select((
//!     bt.All("Fire",                                  action(all((TryFireToAnyone(3),
//!       TryFireToAnyone(3.0f),                                    MoveBase))),
//!       MoveBase("MoveByPath")),                      action(TriggerMapAction(once)),
//!     TriggerMapAction(true),                         action(TryFireToAnyone(3)),
//!     TryFireToAnyone(3.0f))                        ))
//! FollowPathAiSafe =                              follow_path_ai_safe() =
//!   bt.Selector("Brain",                            select((
//!     bt.Sequence("Danger",                           seq((leaf(find_nearest_enemy),
//!       FindNearestEnemy, Runaway),                           action(Runaway))),
//!     bt.All("Fire", TryFireToAnyone(3.0f),           action(all((TryFireToAnyone(3),
//!       MoveBase("MoveByPath")))                                  MoveBase))),
//!                                                   ))
//! GoToEnemySimplePredictionPosition =             go_to_enemy_simple_prediction_position() =
//!   bt.Sequence("FindAndFollow",                    seq((leaf(find_nearest_enemy),
//!     FindNearestEnemy,                                   action(MoveTowardsTargetNoPath(0.6))))
//!     MoveTowardsTargetNoPath(0.6f))
//! ```
//!
//! `FollowPathAi`'s third branch (`TryFireToAnyone` alone) is kept as C#'s
//! though it never runs: `TriggerMapAction` (optional) always succeeds, and
//! the selector stops at the first success. A car with no path therefore does
//! nothing but run its trigger action (once), and does not even fire.
//!
//! The small trees of `AiControlFactory`'s other simple controls:
//!
//! ```text
//! C#                                              Rust
//! SimpleAttackNearestEnemy =                      simple_attack_nearest_enemy() =
//!   bt.Selector(SimpleAttack, bt.Idle)              select((simple_attack(), action(Idle)))
//!   (ScanDistance = 20)
//! SimpleFollowByEnemyAi =                         simple_follow_by_enemy_ai() =
//!   bt.Selector("SimpleFollowByEnemy",              select((
//!     bt.Sequence("FindAndFollow",                    seq((leaf(find_nearest_enemy),
//!       FindNearestEnemy,                                   action(SimpleFollowByTarget(0.2)))),
//!       SimpleFollowByTarget(0.2f)),                  action(Idle),
//!     bt.Idle)                                      ))
//!   (ScanDistance = 20)
//! MoveInOneDirectionAi =                          move_in_one_direction_ai() =
//!   bt.Action("MoveToInitDirection",                action(MoveInInitDirection)
//!     b => { ActionData = DirectionVector; true },    (start: the heading; tick: stick and
//!     b => true,                                       gun that way, trigger down)
//!     b => { axis = weapon axis = ActionData;
//!            Fire = Shoot; })
//! ```
//!
//! `AttackerAiVer2` (`AttackAiVer2`), with the subtrees it is made of:
//!
//! ```text
//! C#                                              Rust
//! StopAction =                                    stop() = leaf(stop_moving)
//!   bt.Action("Stop", b => { b.Path = null;         (the path dropped, the stick let go;
//!     CurrentAxisControl = Zero; return true; })     over at once)
//!
//! Maneur(p) =                                     maneur(p) =
//!   bt.All("Maneur",                                action(all((
//!     MoveToRandomVisiblePosition(                    MoveToRandomVisiblePosition(
//!       b => b.LastKnowTargetPosition,                  last known target position,
//!       b => b.Entity.Weapon.Range * 0.8f,              gun range * 0.8,
//!       1.0f, 2.0f),                                    1.0, 2.0),
//!     bt.Optional(FireToTarget(p)))                   optional(FireToTarget(p)))))
//! ManeurOnDamage(p) =                             maneur_on_damage(p) =
//!   bt.Sequence(                                    seq((
//!     bt.Condition(b => CheckState(TakeDamage)),      check(took damage),
//!     Maneur(p))                                      maneur(p)))
//!
//! SmartAttackTargetImmidiateWeapon =              smart_attack_target_immediate_weapon() =
//!   bt.Selector("ImmidiateWeapon",                  select((
//!     ManeurOnDamage(-1f),                            maneur_on_damage(-1),
//!     bt.Sequence(                                    seq((
//!       bt.Condition(Weapon.IsImmidiate),               check(gun is immediate),
//!       StopAction,                                     stop(),
//!       AimToTarget(0, 0),                              action(AimToTarget(0, 0)),
//!       AimToTarget(0, 0.5f),                           action(AimToTarget(0, 0.5)),
//!       Timer(1.0f, FireToTarget(-0.5f)),               timer(1, action(FireToTarget(-0.5))),
//!       Timer(0.5f, FireToTarget(0.0f)),                timer(0.5, action(FireToTarget(0))),
//!       Maneur(-1))))                                   maneur(-1),
//! SmartAttackTargetSlowWeapon =                   smart_attack_target_slow_weapon() =
//!   bt.Selector("SlowWeapon",                       select((
//!     ManeurOnDamage(-0.5f),                          maneur_on_damage(-0.5),
//!     bt.Sequence(                                    seq((
//!       bt.Condition(!Weapon.IsImmidiate),              check(gun is not immediate),
//!       StopAction,                                     stop(),
//!       Timer(1.0f, FireToTarget(-0.5f)),               timer(1, action(FireToTarget(-0.5))),
//!       Timer(0.5f, FireToTarget(0.0f)),                timer(0.5, action(FireToTarget(0))),
//!       Maneur(-0.5f))))                                maneur(-0.5),
//! SmartAttackTargetVer2 =                         smart_attack_target_ver2() =
//!   bt.Selector("A",                                select((
//!     bt.Sequence("Fire",                             seq((
//!       bt.Condition("DistAndVis",                      check(target alive and in sight now and
//!         Target != null && Weapon != null &&                 within the gun's range),
//!         Target.Alive && SeeTarget &&                  select((immediate, slow)),
//!         CloserThanByChebyshev(Target.Position,      )),
//!           Entity.Position, Weapon.Range)),          action(FollowBy(last known target position)),
//!       bt.Selector(Immidiate, Slow)),                ))
//!     bt.Sequence("Follow",
//!       FollowBy(b => b.LastKnowTargetPosition)))     (a Sequence of one child is the child)
//! SmartAttackVer2 =                               smart_attack_ver2() =
//!   bt.Sequence("Attack",                           seq((leaf(find_nearest_enemy),
//!     FindNearestEnemy, SmartAttackTargetVer2)        smart_attack_target_ver2()))
//!
//! HideMove =                                      hide_move() =
//!   MoveToPoint("Hide", b =>                        action(MoveToPoint(
//!     GetRandomFreeNonVisiblePoint(                   random free non visible point(
//!       Entity.Position, Target.Position,               own position, target's position,
//!       Target.Weapon != null                           target's gun range * 1.25, or 32
//!         ? Target.Weapon.Range * 1.25f : 32))          without a gun)))
//! PanicBehaviour =                                panic_behaviour() =
//!   bt.Sequence("PanicBehaviour",                   seq((
//!     FindNearestEnemy,                               leaf(find_nearest_enemy),
//!     bt.Condition("EnemyVisible", Target != null     check(target alive, in sight now or in 0.5 s),
//!       && Target.Alive &&                            hide_move(),
//!       CheckState(SeeTarget | WillSeeTarget)),       stop(),
//!     HideMove, StopAction)                         ))
//!
//! AttackerAiVer2 =                                attacker_ai_ver2() =
//!   bt.Selector("AttackerAiVer2",                   select((
//!     bt.Sequence("Panic",                            seq((
//!       bt.Condition("CheckHelath",                     check(health < max health / 2 (whole
//!         Health < MaxHealth / 2 &&                           numbers) and took damage and
//!         CheckState(TakeDamage) &&                           the target is alive),
//!         Target != null && Target.Alive),              panic_behaviour(),
//!       PanicBehaviour),                              )),
//!     SmartAttackVer2,                                smart_attack_ver2(),
//!     bt.Sequence("MoveToRandom",                     seq((
//!       MoveToPoint("Rnd",                              action(MoveToPoint(
//!         GetRandomNodePosition(                          random node position(own position,
//!           Entity.Position, ScanDistance * 4),             ScanDistance * 4), 0.5, 4)),
//!         0.5f, 4),                                     timer(2, action(Idle)),
//!       Timer(2.0f, bt.Idle)))                        )),
//!                                                   ))
//! ```
//!
//! Where a `Sequence` or `Selector` has one child C#'s `BTBuilder` returns the
//! child: `SmartAttackTargetVer2`'s `Follow` is the `FollowBy` itself.
//!
//! The flocking trees (`SimpleFlocking`; `ZombieFlocking`, for
//! `ZombieFlocking` and `ZombieFlockingFriend`), over [`Flocking`] and
//! [`FindNearestLeader`]:
//!
//! ```text
//! C#                                              Rust
//! SimpleFlocking =                                simple_flocking() =
//!   bt.Selector("FlockingBrain",                    select((
//!     bt.Sequence("Attack",                           seq((leaf(find_nearest_enemy),
//!       FindNearestEnemy, Flocking(.5f, 4.0f)),             action(Flocking(0.5, 4)))),
//!     Flocking(0.1f, 4.0f))                           action(Flocking(0.1, 4)),
//!                                                   ))
//! ZombieFlocking =                                zombie_flocking() =
//!   bt.Selector("ZombieFlocking",                   select((
//!     bt.Sequence("Super attack",                     seq((
//!       bt.Condition("Check", Target alive &&           check(target alive, in sight now, gun
//!         SeeTarget && Weapon.CanShoot &&                     ready, within 4 gun ranges on both
//!         CloserThanByChebyshev(.., Range * 4) &&             axes, no friend within 2 ranges),
//!         !GetFriends(Range * 2).Any()),
//!       Timer(2.0f, bt.Action("Power", …)),             timer(2, action(Power)),
//!       bt.Action("Fire!", Fire = Shoot)),              leaf(fire_now))),
//!     bt.Sequence("Attack",                           seq((
//!       bt.Condition("Agr", TakeDamage &&               check(took damage, no live target),
//!         (Target == null || !Target.Alive)),
//!       bt.Action("Uii", … return false)),              leaf(alert_friends))),
//!     bt.Sequence("ChangeTarget",                     seq((
//!       bt.Condition("Agr", TakeDamage &&               check(took damage, target is the damager),
//!         Target == LastDamager),
//!       FindNearestEnemy),                              leaf(find_nearest_enemy))),
//!     bt.Sequence("Attack",                           seq((
//!       FindNearestEnemy,                               leaf(find_nearest_enemy),
//!       WhileTargetAlive(                               while_target_alive(
//!         Flocking(1, 8, 1, LastKnowTargetPosition))),    action(Flocking(1, 8, 1, last known)))))),
//!     bt.Sequence("FollowLeader",                     seq((
//!       FindNearestLeader,                              leaf(find_nearest_leader),
//!       bt.Selector("TriggerPlayerLeader",              select((
//!         bt.Condition("CheckLeader", Leader            check(no player leader or no action),
//!           == null || !Alive || !LocalDevice ||
//!           MapActionOnTirgger == null),
//!         bt.Optional(TriggerMapAction(true))),           action(optional(TriggerMapAction(once))))),
//!       bt.Selector("Follow",                           select((
//!         bt.Sequence("FollowPlayerLeader",               seq((
//!           bt.Condition("CheckLeader", player              check(player leads, out of sight),
//!             leads && !IsVisible(Leader.Position)),
//!           FollowBy("Follow leader",                       action(FollowBy(leader in 1 s, 1, 10)))),
//!             Leader.GetPredictionPosition(1), 1, 10)),
//!         bt.While("LeaderAlive", StartTimer,             if_else(leader alive,
//!           Leader.Alive && Timer < 5 ? Running : Fail,     time_limit(5,
//!           Flocking(1, 8, 4, Leader alive ?                action(Flocking(1, 8, 4, leader in 4 s,
//!             Leader.GetPredictionPosition(4) :               or its own position)))),
//!             Entity.Position)))),                        )),
//!     Flocking(0.25f, 4.0f))                          action(Flocking(0.25, 4)),
//!                                                   ))
//! ```
//!
//! What it comes to: a zombie that has its target in sight and near (16
//! for the Zombie's 4-unit range), its wave ready and no friend within 8,
//! stands 2 s gathering it (the streaks) and lets it go (it stuns every
//! enemy within 8). A zombie hurt with nobody to fight alarms its idle
//! friends within 24 with whoever hurt it (they then pick the damager as
//! their target: `FindNearestEnemy` takes the damager first) and goes on;
//! hurt by its own target, it looks again (the damager); otherwise it finds
//! the nearest enemy within 12 and flocks at full speed to where it last saw
//! it while it lives. With no enemy, it finds a leader (the player's car
//! near, or a slow friend) and, when the player leads, runs the map's
//! `ActionOnBTTrigger` once (`Swarm` counts the friends that joined);
//! the player out of sight, it follows by a path to where the player will
//! be in a second (within 10 is near enough); otherwise it flocks towards
//! where the leader will be in 4 s for 5 s at a time. With no leader it
//! wanders with its flock at a quarter speed. `Flocking`'s pull holds the
//! flock together whatever it heads for.
//!
//! As in C#, a target found but never seen leaves `LastKnowTargetPosition`
//! where the last one was seen, or at the origin (the map's top-left corner)
//! if none ever was: the attack flocks there. The `Fire!` trigger is pulled
//! in the thinking pass that starts it (`ai::carry_out` takes it for that
//! step), as `Control.Fire = FireType.Shoot` in C#'s `Start` was.
//!
//! Differences from C# in `ZombieFlocking`: `Weapon.CanShoot` asks the
//! reload only (the AI cars have no energy store); `LeaderAlive` with no
//! leader fails (C# would throw), and `Follow leader` with its leader gone
//! heads for where it was last gathered (`SafePosition`); `Uii`'s alarms
//! reach the friends after every tree of the step has run (C# set them at
//! once, so a friend ticked later in the same frame saw it a step sooner),
//! and the ping's view check is the camera's picture with no margin
//! (`IsInView(pos, 0)`). The commented-out `RandomHelper.RandomEvent(0.2f)`
//! of `Check` is not there in C# either.
//!
//! `HammerAi` (`HammerAi`, team 1; `HammerAiFriend`, team 0; the default
//! `ScanDistance` 16), for the cars with the Hammer's saw, which hurts by
//! the car's speed at the blow:
//!
//! ```text
//! C#                                              Rust
//! RemeberTargetPos(p) =                           RememberTargetPos { predict_sec: p }
//!   bt.Action("RemberPos", b => {                   (start: a live target's
//!     Target != null && Target.Alive &&               prediction_position into
//!       (TargetPosition =                             target_position; fails without)
//!        Target.GetPredictionPosition(p)) })
//!
//! HammerAi =                                      hammer_ai() =
//!   bt.Selector("Hammer Ai",                        select((
//!     bt.Sequence("AttackHammer",                     seq((
//!       Timer(1.0f, Chase),                             timer(1, chase()),
//!       RemeberTargetPos(0.0f),                         action(RememberTargetPos(0)),
//!       Timer(3.0f, MoveToPoint("PrevTargetPos",        timer(3, action(MoveToPoint(
//!         b => b.TargetPosition)),                        target_position))),
//!       bt.Condition("CheckTarget",                     check(a live target),
//!         Target != null && Target.Alive),
//!       Timer(1.0f, MoveToPoint("Rnd",                  timer(1, action(MoveToPoint(
//!         b => GetRandomFreeVisiblePoint(                 random free visible point(own
//!           Entity.Position, ScanDistance),                 position, ScanDistance),
//!         0.5f, 1))),                                     0.5, 1)))),
//!                                                     )),
//!     bt.Sequence("Idle",                             seq((
//!       Sleep2Sec,                                      action(Sleep(2)),
//!       MoveToRandomVisiblePosition(                    action(MoveToRandomVisiblePosition(
//!         b => b.ScanDistance, 0.5f)))                    own position, ScanDistance, 0.5, 1)),
//!                                                     )),
//!                                                   ))
//! ```
//!
//! What it comes to (its history: a chase of 3 s and a run-up, January
//! 2014; two days later the chase cut to 1 s and the charge at where the
//! target was added; the `Idle` branch with `Sleep2Sec` a few days after):
//! the hammer car chases the nearest enemy (as `ChaserAi`, but within
//! `ScanDistance`; its trigger pulls change nothing, the saw cuts by
//! contact) for a second, then
//! drives at full stick by a path to where the enemy was then, without
//! following it, so it rams through at speed (the saw's blow grows with
//! the car's speed); the charge ends on arrival (within its radius plus 1),
//! when stuck, or after 3 s. If the enemy still lives it backs off at half
//! the stick to a random free place in sight within `ScanDistance` for a
//! second, a run-up for the next charge, and starts again. With no live
//! target, or when the charge finds no path, it sleeps 2 s and wanders
//! at half the stick to a random free place in sight.
//!
//! Its branches, as `BTLib` runs them (each a consequence of the nodes,
//! not written out): every `Timer` ends at the first thought at or after its
//! time, and with its child's result if the child ends first (a charge that
//! arrives goes straight on; a chase that ends `Ok` on reaching the target
//! too); with no enemy `Chase` reaches its `Sleep` (no think for 2 s and a
//! period), the timer then ends `Ok`, `RemberPos` fails and `Idle`'s
//! `Sleep2s` puts the next think 2.25 s off while its wander starts at
//! once: the car stands 2.25 s, then wanders, and at that next think the
//! selector tries the attack again, so with nobody near it alternates
//! standing and wandering; a run-up spot within the car's radius and 1 is
//! reached at once and the round starts over; a charge whose `MoveToPoint`
//! finds no path, or a blocked one, or the origin, fails the attack for
//! `Idle`; the run-up's random point is the origin (no point) without a
//! floor plan, failing it likewise; `CheckTarget` failing (the enemy died
//! during the charge) goes to `Idle` too.
//!
//! Differences from C# in `HammerAi`: none of its own. `RemeberTargetPos`'s
//! `GetPredictionPosition` has no `Removed` branch (a removed target is no
//! live target, so the action fails; C# would have charged at its
//! `SafePosition`), and the nodes it shares keep their own differences
//! (`Chase`, `MoveToPoint`, the `Timer`s ending at a think).
//!
//! `OrderExecutorAi` (`OrderExecutorFriend`, team 0; `OrderExecutorAi`,
//! team 1; the default `ScanDistance` 16, which nothing reads), for the
//! cars a map scripts with orders (`super::order_queue`):
//!
//! ```text
//! C#                                              Rust
//! OrderExecutorAi = new ExecuteOrders()           order_executor_ai() =
//!   (DynamicNode "Order": the first order's         action(ExecuteOrders)
//!    tree, Fail with none; EndOrder(status)         (start: the first order's action,
//!    when it ends)                                   fails with none; complete: the
//!                                                    queue told)
//! MoveOrderRun's order =                          OrderKind::Move { to } =
//!   MoveToPoint("MoveOrder", b => p)                MoveToPoint(to), speed 1, within 1
//! ```
//!
//! What it comes to: with no order the car does nothing at all (its tree
//! fails every step, so it is thought over every step; the stick stays
//! where it was, zero for a car that never had an order). With orders it
//! drives by a path to the first order's point, and when it is there (its
//! radius plus 1 from the point, or at the path's end) the order is done
//! and the next one starts a step later. An order that fails (no path, a
//! blocked one, stuck two seconds on the way) is tried again at the next
//! step unless it is `RemoveOnTry`. The truck of Escort is such a car: the
//! map orders it to the next stretch of road as the player clears it.
//!
//! Differences from C# in `OrderExecutorAi`: an order cleared and replaced
//! within a quarter second starts afresh (see [`ExecuteOrders`]); the
//! `DEBUG` build's `SmartMoveToPos` for a `MoveOrder` is not ported.
//!
//! Not ported here: every other tree of `BTActionFactory` (`SimpleAttackDelayPos`,
//! `Attack`, `SimpleAi`, `GuardAi`, `SmartAttack` (`AttackAi`'s: the
//! first version of the smart attack, which `AttackAiVer2` replaced), … the
//! ones with the nodes not yet ported: see `docs/BACKLOG.md`), and
//! `TurretAiFireToAnyone` (`AiSimpleTurretFriendFireAvailableTarget`:
//! `SimpleAttackByAngle`, `FindAngleNearestEnemy`, `TryFireToAnyone`), which no
//! map uses.

use flatbt_bevy::prelude::*;

use super::blackboard::AiBlackboard;
use super::nodes::{
    AimToTarget, ExecuteOrders, FireToTarget, Flocking, FollowBy, Idle, alert_friends, find_nearest_enemy, find_nearest_leader, fire_now,
    MoveBase, MoveInInitDirection, MoveTowardsTargetNoPath, MoveToPoint, MoveToRandomVisiblePosition, Power,
    remember_target_pos, Runaway, Scanning, SimpleFollowByTarget, sleep, stop_moving, TriggerMapAction, TryFireToAnyone, all, optional,
    act, never, time_limit, timer, while_target_alive,
};
use crate::weapon::model::closer_by_chebyshev;
use super::orders::Orders;

/// `SimpleAttackTarget(0.2f)`: how far ahead of the target the turret aims, s.
pub const AIM_AHEAD: f32 = 0.2;
/// `SimpleScanning`: `Timer(1.0f, …)` twice, s.
pub const IDLE_TIME: f32 = 1.0;
pub const SCAN_TIME: f32 = 1.0;
/// `Sleep2Sec`: how long a chaser with nobody to chase waits, s.
pub const SLEEP_TIME: f32 = 2.0;
/// `TryFireToAnyone(3.0f)`: at most a shot every this many seconds, s.
pub const FIRE_TO_ANYONE_PERIOD: f32 = 3.0;
/// `GoToEnemySimplePredictionPosition`: how far ahead it steers, s.
pub const GO_TO_ENEMY_PREDICT: f32 = 0.6;
/// `AiControlFactory`'s `AiChaser`: `ScanDistance = World.RoomSize.Width +
/// World.RoomSize.Height` (800 × 4 and 480 × 4 sim units): the whole map.
pub const CHASER_SCAN_DISTANCE: f32 = 800.0 * 4.0 + 480.0 * 4.0;

/// `AiControlFactory`'s `SimpleAttackNearestEnemy` and `SimpleFollowByEnemyAi`:
/// `ScanDistance = 20`.
pub const SIMPLE_SCAN_DISTANCE: f32 = 20.0;
/// `Maneur`: how far from where the target was last seen the car goes, in
/// gun ranges (`Weapon.Range * 0.8f`).
pub const MANEUR_RANGE_RATE: f32 = 0.8;
/// `HideMove`: how far from the target's gun the car hides, in its ranges
/// (`Target.Weapon.Range * 1.25f`), and how far when it has none.
pub const HIDE_RANGE_RATE: f32 = 1.25;
pub const HIDE_RANGE_NO_GUN: f32 = 32.0;
/// `AttackerAiVer2`'s `MoveToPoint("Rnd", …, 0.5f, 4)`: the speed rate, how
/// near is near enough, and how far a random node may be, in `ScanDistance`s.
pub const RANDOM_MOVE_SPEED_RATE: f32 = 0.5;
pub const RANDOM_MOVE_MIN_DIST: f32 = 4.0;
pub const RANDOM_MOVE_RANGE_RATE: f32 = 4.0;
/// `AttackerAiVer2`'s `Timer(2.0f, bt.Idle)`, s.
pub const RANDOM_MOVE_REST: f32 = 2.0;
/// `AiControlFactory`'s `ZombieFlocking` and `ZombieFlockingFriend`:
/// `ScanDistance = 12`.
pub const ZOMBIE_SCAN_DISTANCE: f32 = 12.0;
/// `ZombieFlocking`'s super attack: the target within this many gun ranges
/// on both axes (`Weapon.Range * 4`), no friend within this many
/// (`Weapon.Range * 2`), and the wave gathered this long (`Timer(2.0f,
/// Power)`), s.
pub const SUPER_ATTACK_RANGE_RATE: f32 = 4.0;
pub const SUPER_ATTACK_ALONE_RATE: f32 = 2.0;
pub const SUPER_ATTACK_POWER_TIME: f32 = 2.0;
/// `FollowPlayerLeader`'s `FollowBy(…, b => Leader.GetPredictionPosition(1.0f),
/// 1.0f, 10)`: how far ahead of the leader it heads, s, its speed rate, and
/// how near is near enough.
pub const FOLLOW_LEADER_AHEAD: f32 = 1.0;
pub const FOLLOW_LEADER_MIN_DIST: f32 = 10.0;
/// `LeaderAlive`: flock with the leader this long at a time
/// (`Timer.TotalSeconds < 5`), s, towards where it will be this far ahead
/// (`GetPredictionPosition(4.0f)`), s.
pub const LEADER_FLOCK_TIME: f32 = 5.0;
pub const LEADER_FLOCK_AHEAD: f32 = 4.0;

/// `HammerAi`'s `Timer(1.0f, Chase)`: how long it chases before it charges, s.
pub const HAMMER_CHASE_TIME: f32 = 1.0;
/// `RemeberTargetPos(0.0f)`: the charge goes at where the target is, not
/// ahead of it, s.
pub const HAMMER_CHARGE_PREDICT: f32 = 0.0;
/// `Timer(3.0f, MoveToPoint("PrevTargetPos", …))`: the longest charge, s.
pub const HAMMER_CHARGE_TIME: f32 = 3.0;
/// `Timer(1.0f, MoveToPoint("Rnd", …, 0.5f, 1))`: the run-up after a charge,
/// s, at half the stick, done within 1 of the place.
pub const HAMMER_RUN_UP_TIME: f32 = 1.0;
pub const HAMMER_RUN_UP_SPEED_RATE: f32 = 0.5;
pub const HAMMER_RUN_UP_MIN_DIST: f32 = 1.0;
/// `Idle`'s `MoveToRandomVisiblePosition(b => b.ScanDistance, 0.5f)`: at half
/// the stick, done within 1 (the constructor's `minDistanceToTarget`).
pub const HAMMER_WANDER_SPEED_RATE: f32 = 0.5;
pub const HAMMER_WANDER_MIN_DIST: f32 = 1.0;

/// `BTActionFactory.SimpleAttackTarget`: fires at the target while steering
/// at it, with the gun's aim `predict_sec` ahead of it.
pub fn simple_attack_target(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("A", select((
        named("Fire", act(all((FireToTarget::new(predict_sec), SimpleFollowByTarget::new(predict_sec))))),
        act(FireToTarget::new(predict_sec)),
    )))
}

/// `BTActionFactory.SimpleAttack`: find an enemy, then attack it.
pub fn simple_attack() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Attack", seq((leaf(find_nearest_enemy), simple_attack_target(AIM_AHEAD))))
}

/// `BTActionFactory.SimpleScanning`: stand still, then turn the gun to a
/// random direction.
pub fn simple_scanning() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Idle", seq((
        named("Timer_1s", timer(IDLE_TIME, act(Idle))),
        named("Timer_1s", timer(SCAN_TIME, act(Scanning))),
    )))
}

/// `BTActionFactory.TurretAi`: attack, else scan.
pub fn turret_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("TurretBrain", select((simple_attack(), simple_scanning())))
}

/// `BTActionFactory.Chase`'s `MoveToLastKnownTargetPosition`: with a target
/// (alive), go to where it was last seen.
fn move_to_last_known_target_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToLastKnownTargetPosition", seq((
        named("HastTarget", check(|bb: &AiBlackboard| bb.live_target().is_some())),
        named("MoveToLastKnownPos", act(MoveToPoint::new(|bb| bb.last_known_target_position))),
    )))
}

/// `BTActionFactory.Chase` (`ChaserAi`): find an enemy and chase it, firing
/// when the gun points at it; failing that, go to where it was last seen;
/// failing that, sleep two seconds.
pub fn chase() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ChaseAI", select((
        named("Chase", seq((
            leaf(find_nearest_enemy),
            while_target_alive(
                named("Chase and fire", act(all((
                    optional(FireToTarget::new(0.0)),
                    FollowBy::new(|bb| bb.target_position_safe()),
                )))),
            ),
        ))),
        move_to_last_known_target_position(),
        named("Sleep", seq((named("Sleep2s", leaf(sleep(SLEEP_TIME))), act(Idle)))),
    )))
}

/// `BTActionFactory.ChaserAi`.
pub fn chaser_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    chase()
}

/// `BTActionFactory.FollowPathAi`: follow the path the map gave it, firing at
/// anyone in the gun's line; with no path, run the map action it was given.
pub fn follow_path_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Brain", select((
        named("Fire", act(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default())))),
        named("Trigger", act(TriggerMapAction::new(true))),
        act(TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD)),
    )))
}

/// `BTActionFactory.FollowPathAiSafe`: follow the path, but run from an enemy
/// in sight.
pub fn follow_path_ai_safe() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Brain", select((
        named("Danger", seq((leaf(find_nearest_enemy), act(Runaway)))),
        named("Fire", act(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default())))),
    )))
}

/// `BTActionFactory.GoToEnemySimplePredictionPosition`: find an enemy and go
/// straight at where it will be.
pub fn go_to_enemy_simple_prediction_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("FindAndFollow", seq((leaf(find_nearest_enemy), act(MoveTowardsTargetNoPath::new(GO_TO_ENEMY_PREDICT)))))
}

/// `BTActionFactory.SimpleAttackNearestEnemy`: attack, else idle.
pub fn simple_attack_nearest_enemy() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("SimpleAttackNearestEnemy", select((simple_attack(), act(Idle))))
}

/// `BTActionFactory.SimpleFollowByEnemyAi`: find an enemy and follow it (no
/// path, no fire), else idle.
pub fn simple_follow_by_enemy_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("SimpleFollowByEnemy", select((
        named("FindAndFollow", seq((leaf(find_nearest_enemy), act(SimpleFollowByTarget::new(AIM_AHEAD))))),
        act(Idle),
    )))
}

/// `BTActionFactory.MoveInOneDirectionAi`: the way the car faced at the start,
/// the stick and the gun that way and the trigger down.
pub fn move_in_one_direction_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToInitDirection", act(MoveInInitDirection))
}

/// `BTActionFactory.StopAction`.
pub fn stop() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Stop", leaf(stop_moving))
}

/// `BTActionFactory.Maneur(predictionSec)`: go to a random place in sight of
/// where the target was last seen, within 0.8 gun ranges of it, firing at the
/// target meanwhile if it can be seen.
pub fn maneur(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Maneur", act(all((
        MoveToRandomVisiblePosition::new(
            |bb| bb.last_known_target_position,
            |bb| bb.gun.map_or(0.0, |gun| gun.range.range * MANEUR_RANGE_RATE),
            1.0,
            2.0,
        ),
        optional(FireToTarget::new(predict_sec)),
    ))))
}

/// `BTActionFactory.ManeurOnDamage(predictionSec)`: [`maneur`], but only when
/// the car lost health or shield since it last thought.
pub fn maneur_on_damage(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ManeurOnDamage", seq((named("CheckDamage", check(|bb: &AiBlackboard| bb.took_damage())), maneur(predict_sec))))
}

/// `BTActionFactory.SmartAttackTargetImmidiateWeapon`: for a gun whose shot
/// arrives at once: stand, turn the car and the gun on the target, fire for
/// a second at where the target was half a second ago (the prediction is
/// negative: aimed behind it) and for half a second at where it is, then go
/// to another place.
pub fn smart_attack_target_immediate_weapon() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ImmidiateWeapon", select((
        maneur_on_damage(-1.0),
        named("SmartAttackTargetImmidiateWeapon", seq((
            named("CheckWeaponType", check(|bb: &AiBlackboard| bb.gun.is_some_and(|gun| gun.immediate))),
            stop(),
            act(AimToTarget::new(0.0, 0.0)),
            act(AimToTarget::new(0.0, 0.5)),
            named("Timer_1s", timer(1.0, act(FireToTarget::new(-0.5)))),
            named("Timer_0.5s", timer(0.5, act(FireToTarget::new(0.0)))),
            maneur(-1.0),
        ))),
    )))
}

/// `BTActionFactory.SmartAttackTargetSlowWeapon`: for a gun whose shot flies:
/// stand and fire (no aiming turn), then go to another place.
pub fn smart_attack_target_slow_weapon() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("SlowWeapon", select((
        maneur_on_damage(-0.5),
        named("SmartAttackTargetSlowWeapon", seq((
            named("CheckWeaponType", check(|bb: &AiBlackboard| bb.gun.is_some_and(|gun| !gun.immediate))),
            stop(),
            named("Timer_1s", timer(1.0, act(FireToTarget::new(-0.5)))),
            named("Timer_0.5s", timer(0.5, act(FireToTarget::new(0.0)))),
            maneur(-0.5),
        ))),
    )))
}

/// `DistAndVis`: the target is alive, in sight now, and within the gun's
/// range on both axes (a car with no gun never).
fn target_in_range_and_sight(bb: &AiBlackboard) -> bool {
    match (bb.live_target(), bb.gun) {
        (Some(target), Some(gun)) => bb.sees_target() && closer_by_chebyshev(target.pos, bb.pos, gun.range.range),
        _ => false,
    }
}

/// `BTActionFactory.SmartAttackTargetVer2`: in range and in sight, attack in
/// the style the gun asks for; otherwise go to where the target was last
/// seen.
pub fn smart_attack_target_ver2() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("A", select((
        named("Fire", seq((
            named("DistAndVis", check(target_in_range_and_sight)),
            named("ChooseAttackStyle", select((smart_attack_target_immediate_weapon(), smart_attack_target_slow_weapon()))),
        ))),
        named("Follow", act(FollowBy::new(|bb| bb.last_known_target_position))),
    )))
}

/// `BTActionFactory.SmartAttackVer2`: find an enemy, then attack it smartly.
pub fn smart_attack_ver2() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Attack", seq((leaf(find_nearest_enemy), smart_attack_target_ver2())))
}

/// `BTActionFactory.HideMove`: go to a free place out of the target's sight,
/// or out of reach of its gun (1.25 ranges).
pub fn hide_move() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Hide", act(MoveToPoint::new(|bb| {
        let Some(target) = bb.live_target().copied() else {
            return bevy::math::Vec2::ZERO;
        };
        let range = target.weapon_range.map_or(HIDE_RANGE_NO_GUN, |range| range * HIDE_RANGE_RATE);
        let own = bb.pos;
        bb.random_free_non_visible_point(own, target.pos, range)
    })))
}

/// `BTActionFactory.PanicBehaviour` (`ChickenAi` is this alone): find an
/// enemy and, if it can see the car (or will in 0.5 s), hide from it.
pub fn panic_behaviour() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("PanicBehaviour", seq((
        leaf(find_nearest_enemy),
        named("EnemyVisible", check(|bb: &AiBlackboard| bb.live_target().is_some() && bb.can_see_target())),
        hide_move(),
        stop(),
    )))
}

/// `AttackerAi`'s and `AttackerAiVer2`'s last branch (`MoveToRandom`): walk
/// at half speed to a random node within four scan distances, then rest two
/// seconds.
fn move_to_random() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToRandom", seq((
        named("Rnd", act(MoveToPoint::with_params(
            |bb| {
                let (own, range) = (bb.pos, bb.scan_distance * RANDOM_MOVE_RANGE_RATE);
                bb.random_node_position(own, range)
            },
            RANDOM_MOVE_SPEED_RATE,
            RANDOM_MOVE_MIN_DIST,
        ))),
        named("Timer_2s", timer(RANDOM_MOVE_REST, act(Idle))),
    )))
}

/// `BTActionFactory.SmartAttackTarget`, the first version of
/// [`smart_attack_target_ver2`]: in range and in sight, stop, fire a second
/// aimed half a second behind the target and half a second at it, then go to
/// a random place in sight of where it was ([`maneur`], firing at it);
/// otherwise go to where it was last seen. Unlike the second version, it
/// has no aiming turn, no manoeuvre when hit and no style for slow guns.
pub fn smart_attack_target() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("A", select((
        named("Fire", seq((
            named("DistAndVis", check(target_in_range_and_sight)),
            stop(),
            named("Timer_1s", timer(1.0, act(FireToTarget::new(-0.5)))),
            named("Timer_0.5s", timer(0.5, act(FireToTarget::new(0.0)))),
            maneur(0.0),
        ))),
        named("Follow", act(FollowBy::new(|bb| bb.last_known_target_position))),
    )))
}

/// `BTActionFactory.SmartAttack`: find an enemy, then [`smart_attack_target`].
pub fn smart_attack() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Attack", seq((leaf(find_nearest_enemy), smart_attack_target())))
}

/// `BTActionFactory.AttackerAi` (`AttackAi`, and `AttackAiFriend` for the
/// player's team): attack the nearest enemy; with none, walk to a random
/// place and rest there two seconds.
pub fn attacker_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("AttackerAi", select((smart_attack(), move_to_random())))
}

/// `BTActionFactory.AttackerAiVer2` (`AttackAiVer2`): hide when under half
/// health and being hit; otherwise attack the nearest enemy; with none, walk
/// to a random place and rest there two seconds.
pub fn attacker_ai_ver2() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("AttackerAiVer2", select((
        named("Panic", seq((
            named("CheckHelath", check(|bb: &AiBlackboard| {
                bb.health < bb.max_health / 2 && bb.took_damage() && bb.live_target().is_some()
            })),
            panic_behaviour(),
        ))),
        smart_attack_ver2(),
        move_to_random(),
    )))
}

/// `BTActionFactory.SimpleFlocking` (`SimpleFlocking`): flock at half speed
/// while there is an enemy within `ScanDistance` (the flock not steered at
/// it), else at a tenth.
pub fn simple_flocking() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("FlockingBrain", select((
        named("Attack", seq((leaf(find_nearest_enemy), act(Flocking::new(0.5, 4.0))))),
        act(Flocking::new(0.1, 4.0)),
    )))
}

/// `Super attack`'s `Check`: the target alive and in sight now, the gun
/// ready (`CanShoot`), the target within 4 gun ranges on both axes and no
/// friend within 2.
fn super_attack_ready(bb: &AiBlackboard) -> bool {
    let (Some(target), Some(gun)) = (bb.live_target(), bb.gun) else {
        return false;
    };
    let range = gun.range.range;
    bb.sees_target()
        && bb.gun_ready
        && closer_by_chebyshev(bb.pos, target.pos, range * SUPER_ATTACK_RANGE_RATE)
        && bb.friends(range * SUPER_ATTACK_ALONE_RATE).next().is_none()
}

/// `BTActionFactory.ZombieFlocking` (`ZombieFlocking`, `ZombieFlockingFriend`):
/// see the module docs.
pub fn zombie_flocking() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ZombieFlocking", select((
        named("Super attack", seq((
            named("Check", check(super_attack_ready)),
            named("Timer_2s", timer(SUPER_ATTACK_POWER_TIME, act(Power))),
            named("Fire!", leaf(fire_now)),
        ))),
        named("Attack", seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.live_target().is_none())),
            named("Uii", leaf(alert_friends)),
        ))),
        named("ChangeTarget", seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.target() == bb.damager_entity())),
            leaf(find_nearest_enemy),
        ))),
        named("Attack", seq((
            leaf(find_nearest_enemy),
            while_target_alive(act(Flocking::toward(1.0, 8.0, 1.0, |bb| {
                bb.last_known_target_position
            }))),
        ))),
        named("FollowLeader", seq((
            leaf(find_nearest_leader),
            named("TriggerPlayerLeader", select((
                named("CheckLeader", check(|bb: &AiBlackboard| !bb.player_leads() || bb.map_action_on_trigger.is_none())),
                named("Trigger", act(optional(TriggerMapAction::new(true)))),
            ))),
            named("Follow", select((
                named("FollowPlayerLeader", seq((
                    named("CheckLeader", check(|bb: &AiBlackboard| bb.player_leads() && !bb.leader_visible())),
                    named("FollowBy_Follow leader", act(FollowBy::with_params(
                        |bb| bb.leader_prediction(FOLLOW_LEADER_AHEAD),
                        1.0,
                        FOLLOW_LEADER_MIN_DIST,
                    ))),
                ))),
                // `While("LeaderAlive", StartTimer, Leader.Alive && Timer < 5 ? Running : Fail)`.
                named("LeaderAlive", if_else(
                    |bb: &AiBlackboard| bb.leader().is_some(),
                    time_limit(LEADER_FLOCK_TIME, act(Flocking::toward(1.0, 8.0, 4.0, |bb| {
                        if bb.leader().is_some() { bb.leader_prediction(LEADER_FLOCK_AHEAD) } else { bb.pos }
                    }))),
                    check(never),
                )),
            ))),
        ))),
        act(Flocking::new(0.25, 4.0)),
    )))
}

/// `BTActionFactory.HammerAi` (`HammerAi`, `HammerAiFriend`): chase the
/// nearest enemy for a second, then charge at the place it was (not
/// following it) for up to 3 s; while it lives, pull back at half speed to a
/// random place in sight for a second for the next run; with nobody,
/// sleep 2 s and wander. See the module docs.
pub fn hammer_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Hammer Ai", select((
        named("AttackHammer", seq((
            named("Timer_1s", timer(HAMMER_CHASE_TIME, chase())),
            named("RemberPos", leaf(remember_target_pos(HAMMER_CHARGE_PREDICT))),
            named("Timer_3s", timer(
                HAMMER_CHARGE_TIME,
                named("MoveTo_PrevTargetPos", act(MoveToPoint::new(|bb| bb.target_position))),
            )),
            named("CheckTarget", check(|bb: &AiBlackboard| bb.live_target().is_some())),
            named("Timer_1s", timer(
                HAMMER_RUN_UP_TIME,
                named("MoveTo_Rnd", act(MoveToPoint::with_params(
                    |bb| {
                        let (own, range) = (bb.pos, bb.scan_distance);
                        bb.random_free_visible_point(own, range)
                    },
                    HAMMER_RUN_UP_SPEED_RATE,
                    HAMMER_RUN_UP_MIN_DIST,
                ))),
            )),
        ))),
        named("Idle", seq((
            named("Sleep2s", leaf(sleep(SLEEP_TIME))),
            named("MoveTo_RandomPos", act(MoveToRandomVisiblePosition::new(
                |bb| bb.pos,
                |bb| bb.scan_distance,
                HAMMER_WANDER_SPEED_RATE,
                HAMMER_WANDER_MIN_DIST,
            ))),
        ))),
    )))
}

/// `BTActionFactory.OrderExecutorAi` (`OrderExecutorFriend`,
/// `OrderExecutorAi`): carry out the map's orders, first to last; with none,
/// do nothing. See the module docs.
pub fn order_executor_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Order", act(ExecuteOrders))
}
