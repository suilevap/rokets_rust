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
//!   bt.While("Timer_ts", b => b.StartTimer(),       (decorator_condition: init, check, child)
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
//!     FindNearestEnemy,                               action(FindNearestEnemy),
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
//! Every action is wrapped in `quiet_start` (not shown above): C#'s thinking
//! pass starts an action without ticking it (see the `ai` module docs).
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
//!       FindNearestEnemy,                               action(FindNearestEnemy),
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
//!     bt.Sequence("Danger",                           seq((action(FindNearestEnemy),
//!       FindNearestEnemy, Runaway),                           action(Runaway))),
//!     bt.All("Fire", TryFireToAnyone(3.0f),           action(all((TryFireToAnyone(3),
//!       MoveBase("MoveByPath")))                                  MoveBase))),
//!                                                   ))
//! GoToEnemySimplePredictionPosition =             go_to_enemy_simple_prediction_position() =
//!   bt.Sequence("FindAndFollow",                    seq((action(FindNearestEnemy),
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
//!     bt.Sequence("FindAndFollow",                    seq((action(FindNearestEnemy),
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
//! StopAction =                                    stop() = action(Stop)
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
//!   bt.Sequence("Attack",                           seq((action(FindNearestEnemy),
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
//!     FindNearestEnemy,                               action(FindNearestEnemy),
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
//!     bt.Sequence("Attack",                           seq((action(FindNearestEnemy),
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
//!       bt.Action("Fire!", Fire = Shoot)),              action(FireNow))),
//!     bt.Sequence("Attack",                           seq((
//!       bt.Condition("Agr", TakeDamage &&               check(took damage, no live target),
//!         (Target == null || !Target.Alive)),
//!       bt.Action("Uii", … return false)),              action(AlertFriends))),
//!     bt.Sequence("ChangeTarget",                     seq((
//!       bt.Condition("Agr", TakeDamage &&               check(took damage, target is the damager),
//!         Target == LastDamager),
//!       FindNearestEnemy),                              action(FindNearestEnemy))),
//!     bt.Sequence("Attack",                           seq((
//!       FindNearestEnemy,                               action(FindNearestEnemy),
//!       WhileTargetAlive(                               while_target_alive(
//!         Flocking(1, 8, 1, LastKnowTargetPosition))),    action(Flocking(1, 8, 1, last known)))))),
//!     bt.Sequence("FollowLeader",                     seq((
//!       FindNearestLeader,                              action(FindNearestLeader),
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
//!         bt.While("LeaderAlive", StartTimer,             decorator_condition(start timer,
//!           Leader.Alive && Timer < 5 ? Running : Fail,     leader alive and under 5 s,
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
    AimToTarget, AlertFriends, FindNearestEnemy, FindNearestLeader, FireNow, FireToTarget, Flocking, FollowBy, Idle,
    MoveBase, MoveInInitDirection, MoveTowardsTargetNoPath, MoveToPoint, MoveToRandomVisiblePosition, Power, Runaway,
    Scanning, SimpleFollowByTarget, Sleep, Stop, TriggerMapAction, TryFireToAnyone, all, decorator_condition, optional,
    quiet_start, timer, while_target_alive,
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

/// `BTActionFactory.SimpleAttackTarget`: fires at the target while steering
/// at it, with the gun's aim `predict_sec` ahead of it.
pub fn simple_attack_target(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("A", select((
        named("Fire", action(quiet_start(all((FireToTarget::new(predict_sec), SimpleFollowByTarget::new(predict_sec)))))),
        action(quiet_start(FireToTarget::new(predict_sec))),
    )))
}

/// `BTActionFactory.SimpleAttack`: find an enemy, then attack it.
pub fn simple_attack() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Attack", seq((action(FindNearestEnemy), simple_attack_target(AIM_AHEAD))))
}

/// `BTActionFactory.SimpleScanning`: stand still, then turn the gun to a
/// random direction.
pub fn simple_scanning() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Idle", seq((
        named("Timer_1s", timer(IDLE_TIME, named("Idle", action(quiet_start(Idle))))),
        named("Timer_1s", timer(SCAN_TIME, named("Scanning", action(quiet_start(Scanning))))),
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
        named("MoveToLastKnownPos", action(quiet_start(MoveToPoint::new(|bb| bb.last_known_target_position)))),
    )))
}

/// `BTActionFactory.Chase` (`ChaserAi`): find an enemy and chase it, firing
/// when the gun points at it; failing that, go to where it was last seen;
/// failing that, sleep two seconds.
pub fn chase() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ChaseAI", select((
        named("Chase", seq((
            action(FindNearestEnemy),
            while_target_alive(
                named("Chase and fire", action(quiet_start(all((
                    optional(FireToTarget::new(0.0)),
                    FollowBy::new(|bb| bb.target_position_safe()),
                ))))),
            ),
        ))),
        move_to_last_known_target_position(),
        named("Sleep", seq((named("Sleep2s", action(Sleep { secs: SLEEP_TIME })), named("Idle", action(quiet_start(Idle)))))),
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
        named("Fire", action(quiet_start(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default()))))),
        named("Trigger", action(quiet_start(TriggerMapAction::new(true)))),
        action(quiet_start(TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD))),
    )))
}

/// `BTActionFactory.FollowPathAiSafe`: follow the path, but run from an enemy
/// in sight.
pub fn follow_path_ai_safe() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Brain", select((
        named("Danger", seq((action(FindNearestEnemy), named("Runaway", action(quiet_start(Runaway)))))),
        named("Fire", action(quiet_start(all((TryFireToAnyone::new(FIRE_TO_ANYONE_PERIOD), MoveBase::default()))))),
    )))
}

/// `BTActionFactory.GoToEnemySimplePredictionPosition`: find an enemy and go
/// straight at where it will be.
pub fn go_to_enemy_simple_prediction_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("FindAndFollow", seq((action(FindNearestEnemy), action(quiet_start(MoveTowardsTargetNoPath::new(GO_TO_ENEMY_PREDICT))))))
}

/// `BTActionFactory.SimpleAttackNearestEnemy`: attack, else idle.
pub fn simple_attack_nearest_enemy() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("SimpleAttackNearestEnemy", select((simple_attack(), named("Idle", action(quiet_start(Idle))))))
}

/// `BTActionFactory.SimpleFollowByEnemyAi`: find an enemy and follow it (no
/// path, no fire), else idle.
pub fn simple_follow_by_enemy_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("SimpleFollowByEnemy", select((
        named("FindAndFollow", seq((action(FindNearestEnemy), action(quiet_start(SimpleFollowByTarget::new(AIM_AHEAD)))))),
        named("Idle", action(quiet_start(Idle))),
    )))
}

/// `BTActionFactory.MoveInOneDirectionAi`: the way the car faced at the start,
/// the stick and the gun that way and the trigger down.
pub fn move_in_one_direction_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToInitDirection", action(quiet_start(MoveInInitDirection)))
}

/// `BTActionFactory.StopAction`.
pub fn stop() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Stop", action(Stop))
}

/// `BTActionFactory.Maneur(predictionSec)`: go to a random place in sight of
/// where the target was last seen, within 0.8 gun ranges of it, firing at the
/// target meanwhile if it can be seen.
pub fn maneur(predict_sec: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Maneur", action(quiet_start(all((
        MoveToRandomVisiblePosition::new(
            |bb| bb.last_known_target_position,
            |bb| bb.gun.map_or(0.0, |gun| gun.range.range * MANEUR_RANGE_RATE),
            1.0,
            2.0,
        ),
        optional(FireToTarget::new(predict_sec)),
    )))))
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
            named("AimToTarget", action(quiet_start(AimToTarget::new(0.0, 0.0)))),
            named("AimToTarget", action(quiet_start(AimToTarget::new(0.0, 0.5)))),
            named("Timer_1s", timer(1.0, named("FireToTarget", action(quiet_start(FireToTarget::new(-0.5)))))),
            named("Timer_0.5s", timer(0.5, named("FireToTarget", action(quiet_start(FireToTarget::new(0.0)))))),
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
            named("Timer_1s", timer(1.0, named("FireToTarget", action(quiet_start(FireToTarget::new(-0.5)))))),
            named("Timer_0.5s", timer(0.5, named("FireToTarget", action(quiet_start(FireToTarget::new(0.0)))))),
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
        named("Follow", action(quiet_start(FollowBy::new(|bb| bb.last_known_target_position)))),
    )))
}

/// `BTActionFactory.SmartAttackVer2`: find an enemy, then attack it smartly.
pub fn smart_attack_ver2() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Attack", seq((action(FindNearestEnemy), smart_attack_target_ver2())))
}

/// `BTActionFactory.HideMove`: go to a free place out of the target's sight,
/// or out of reach of its gun (1.25 ranges).
pub fn hide_move() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Hide", action(quiet_start(MoveToPoint::new(|bb| {
        let Some(target) = bb.live_target().copied() else {
            return bevy::math::Vec2::ZERO;
        };
        let range = target.weapon_range.map_or(HIDE_RANGE_NO_GUN, |range| range * HIDE_RANGE_RATE);
        let own = bb.pos;
        bb.random_free_non_visible_point(own, target.pos, range)
    }))))
}

/// `BTActionFactory.PanicBehaviour` (`ChickenAi` is this alone): find an
/// enemy and, if it can see the car (or will in 0.5 s), hide from it.
pub fn panic_behaviour() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("PanicBehaviour", seq((
        action(FindNearestEnemy),
        named("EnemyVisible", check(|bb: &AiBlackboard| bb.live_target().is_some() && bb.can_see_target())),
        hide_move(),
        stop(),
    )))
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
        named("MoveToRandom", seq((
            named("Rnd", action(quiet_start(MoveToPoint::with_params(
                |bb| {
                    let (own, range) = (bb.pos, bb.scan_distance * RANDOM_MOVE_RANGE_RATE);
                    bb.random_node_position(own, range)
                },
                RANDOM_MOVE_SPEED_RATE,
                RANDOM_MOVE_MIN_DIST,
            )))),
            named("Timer_2s", timer(RANDOM_MOVE_REST, named("Idle", action(quiet_start(Idle))))),
        ))),
    )))
}

/// `BTActionFactory.SimpleFlocking` (`SimpleFlocking`): flock at half speed
/// while there is an enemy within `ScanDistance` (the flock not steered at
/// it), else at a tenth.
pub fn simple_flocking() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("FlockingBrain", select((
        named("Attack", seq((action(FindNearestEnemy), named("Flocking", action(quiet_start(Flocking::new(0.5, 4.0))))))),
        named("Flocking", action(quiet_start(Flocking::new(0.1, 4.0)))),
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
            named("Timer_2s", timer(SUPER_ATTACK_POWER_TIME, named("Power", action(quiet_start(Power))))),
            named("Fire!", action(FireNow)),
        ))),
        named("Attack", seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.live_target().is_none())),
            named("Uii", action(AlertFriends)),
        ))),
        named("ChangeTarget", seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.target() == bb.damager_entity())),
            action(FindNearestEnemy),
        ))),
        named("Attack", seq((
            action(FindNearestEnemy),
            while_target_alive(named("Flocking", action(quiet_start(Flocking::toward(1.0, 8.0, 1.0, |bb| {
                bb.last_known_target_position
            }))))),
        ))),
        named("FollowLeader", seq((
            action(FindNearestLeader),
            named("TriggerPlayerLeader", select((
                named("CheckLeader", check(|bb: &AiBlackboard| !bb.player_leads() || bb.map_action_on_trigger.is_none())),
                named("Trigger", action(quiet_start(optional(TriggerMapAction::new(true))))),
            ))),
            named("Follow", select((
                named("FollowPlayerLeader", seq((
                    named("CheckLeader", check(|bb: &AiBlackboard| bb.player_leads() && !bb.leader_visible())),
                    named("FollowBy_Follow leader", action(quiet_start(FollowBy::with_params(
                        |bb| bb.leader_prediction(FOLLOW_LEADER_AHEAD),
                        1.0,
                        FOLLOW_LEADER_MIN_DIST,
                    )))),
                ))),
                named("LeaderAlive", decorator_condition(
                    |bb: &mut AiBlackboard| bb.start_timer(),
                    |bb: &mut AiBlackboard| {
                        if bb.leader().is_some() && bb.timer() < LEADER_FLOCK_TIME - 1e-6 {
                            NodeResult::RUNNING
                        } else {
                            NodeResult::Failure
                        }
                    },
                    named("Flocking", action(quiet_start(Flocking::toward(1.0, 8.0, 4.0, |bb| {
                        if bb.leader().is_some() { bb.leader_prediction(LEADER_FLOCK_AHEAD) } else { bb.pos }
                    })))),
                )),
            ))),
        ))),
        named("Flocking", action(quiet_start(Flocking::new(0.25, 4.0)))),
    )))
}
