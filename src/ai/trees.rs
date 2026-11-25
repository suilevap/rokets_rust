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
//!   bt.Selector("A",                                action(all((FireToTarget(p),
//!     bt.All("Fire",                                          SimpleFollowByTarget(p))))
//!       FireToTarget(p),                              (the second branch is not here)
//!       SimpleFollowByTarget(p)),
//!     FireToTarget(p))
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
//! `SimpleAttackTarget`'s second branch (`FireToTarget` alone) is not there:
//! it could never run. The first branch starts exactly when it would (`All`
//! starts when `FireToTarget` and `SimpleFollowByTarget` do, and the second
//! needs a target, which `FireToTarget` needs too) and is left under the same
//! conditions, so whenever the first was not running the second failed at its
//! start.
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
//!     TriggerMapAction(true),                       ))
//!     TryFireToAnyone(3.0f))        (the third branch is not here)
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
//! `FollowPathAi`'s third branch (`TryFireToAnyone` alone) is not there: it
//! could never run, as `TriggerMapAction` (optional) always succeeds and the
//! selector stops at the first success. A car with no path does nothing but
//! run its trigger action (once), and does not even fire, as in C#.
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
//! `LastKnowTargetPosition` is unset until a target has been seen (C# left
//! the origin, the map's top-left corner, and the attack flocked there; the
//! car's trees that go to it now fail that branch instead, `has_last_known`).
//! The `Fire!` trigger is pulled
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
//! RemeberTargetPos(p) =                           (folded into the charge's point:
//!   bt.Action("RemberPos", b => {                    where_the_target_is, a live
//!     Target != null && Target.Alive &&               target's prediction_position;
//!       (TargetPosition =                             ZERO without one, which
//!        Target.GetPredictionPosition(p)) })          MoveToPoint refuses)
//!
//! HammerAi =                                      hammer_ai() =
//!   bt.Selector("Hammer Ai",                        select((
//!     bt.Sequence("AttackHammer",                     seq((
//!       Timer(1.0f, Chase),                             timer(1, chase()),
//!       RemeberTargetPos(0.0f),
//!       Timer(3.0f, MoveToPoint("PrevTargetPos",        timer(3, act(MoveToPoint(
//!         b => b.TargetPosition)),                        where_the_target_is))),
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
//!   (DynamicNode "Order": the first order's         execute_orders(move_order, carrier,
//!    tree, Fail with none; EndOrder(status)           grab_cargo, press_button)
//!    when it ends)                                  (the first order's tree by its kind,
//!                                                    fails with none; the queue told
//!                                                    when it ends)
//! MoveOrderRun's order =                          move_order() =
//!   MoveToPoint("MoveOrder", b => p)                MoveToPoint(the order's point),
//!                                                   speed 1, within 1
//! ```
//!
//! The orders of the cargo and the button (`ActionRunner.MoveCargoOrderRun`,
//! `GrabCargoOrderRun`, `PressButtonOrderRun`), each the tree C# built when
//! the order was given, here reading its values off the order being carried
//! out ([`AiBlackboard::executing`]):
//!
//! ```text
//! C#                                              Rust
//! CreateCarrierBehaviour(cargo, p, r, dropPos) =  carrier() =
//!   bt.Selector("Carrier",                          select((
//!     dropNode,                                       drop_cargo(),
//!     bt.Sequence("Seq", GrabCargo, dropNode))        seq((grab_cargo(), drop_cargo()))))
//!
//! CreateGrabCargoBehaviour(cargo) =               grab_cargo() =
//!   bt.Sequence("GetCargo",                         seq((
//!     Condition("HasNoCargo",                         check(nothing on the rope),
//!       LinkedSlave == null),
//!     Action("FindCargo", cargo != null &&            check(the cargo is there),
//!       !cargo.Removed; PutCustomData),
//!     MoveUntilPositionNonVisible(                    move_until_position_non_visible(
//!       cargo.Position, null, 4, 7),                    cargo's position, the path's end or
//!                                                       the cargo, 4, 7),
//!     Timer(1.0f, bt.Idle),                           timer(1.0, act(Idle)),
//!     GrabByChainGadget(cargo),                       act(GrabByChainGadget),
//!     TriggerMapAction(true))                         act(TriggerMapAction(once))))
//!
//! CreateDropCargoBehaviour(p, r, dropPos) =       drop_cargo() =
//!   bt.Sequence("DropCargo",                        seq((
//!     Condition("HasCargo", LinkedSlave != null),     check(something on the rope),
//!     Action("FindTargetPos", TargetPosition = p),    leaf(TargetPosition = the order's point),
//!     Selector("WorkaroundForStat",                   select((   (here "SkipDriveWhenStatic")
//!       Condition("IsStat", IsStatic),                  check(the car cannot move),
//!       dropPos given:                                  if_else(a drop point is given,
//!         MoveToPoint("DropPos", dropPos)                 act(MoveToPoint(the drop point)),
//!         else MoveUntilPositionNonVisible(               move_until_position_non_visible(
//!           random free visible point 4..8                  a random free visible point 4..8
//!             of TargetPosition,                              of TargetPosition,
//!           TargetPosition, 4, 7.2)),                       TargetPosition, 4, 7.2)))),
//!     DropByChainGadget(p, r))                        act(DropByChainGadget)))
//!
//! MoveUntilPositionNonVisible(pos, goal, lo, hi) = move_until_position_non_visible(..) =
//!   bt.Sequence("MoveToActiveDist",                 seq((
//!     bt.While("CheckDist", null,                     if_else(not (static, or goal within hi,
//!       IsStatic => Ok;                                 not within lo, no wall between),
//!       goal in (lo, hi) and in sight => Ok;            act(MoveToPoint(pos)),
//!       else Running,                                   success),
//!       MoveToPoint("MoveToDest", pos, 1, 1)),
//!     StopAction)                                     stop()))
//!
//! CreatePressButtonBehaviour(button) =            press_button() =
//!   bt.Sequence("PressButton",                      seq((
//!     Condition(button != null && !Removed),          check(the button is there),
//!     MoveToPoint(button.Position),                   act(MoveToPoint(button's position)),
//!     Condition(CloserThanByChebyshev(pos,            check(Chebyshev-near the button within
//!       button.Position, button.Radius)),               its radius),
//!     Timer(button.ActivationTimeSec + 0.2f, Idle),   timer(1.2, act(Idle)),
//!     Condition(button.IsActivated))                  check(the button is pressed)))
//! ```
//!
//! In a `While`, `Ok` ends it at once without the child, as `Running` runs
//! the child: `if_else`'s second arm is the `Ok` leaf. As every `While`, it
//! asks its condition at a think only.
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
//! With the cargo orders it drives to within 4 to 7 units of the cargo in
//! sight, stands a second, shoots the magnet at it (the button held at least
//! a second, then let go once the magnet has found something) and, with the
//! cargo on the rope, drives (to the drop point if the order gives one, else
//! to a random free place 4 to 7.2 units from the point, in sight of it),
//! then holds the cargo over the point (the button held while it is further
//! from the point than its radius plus the order's `R`, 5 s at most) and
//! lets it go. A `GrabCargoOrder` stops after the grab and runs the
//! `ActionOnGrab`; Assembly's buggy takes the player's car so (the cargo may
//! be a car). A `PressButtonOrder` drives onto the button and waits 1.2 s for
//! it to be pressed (it needs a second and its quarter-second look).
//!
//! Differences from C# in `OrderExecutorAi`: an order cleared and replaced
//! within a quarter second starts afresh (see [`ExecuteOrders`]); the
//! `DEBUG` build's `SmartMoveToPos` for a `MoveOrder` is not ported;
//! `AttackTargetOrder`'s `CreateAttackTargetBehaviour` (no map gives one) is
//! not ported; the cargo's and the button's own state are read as the gather
//! copied it at the start of the step; the magnet takes 0.2 s to charge for
//! the AI as for the player (see `gadget`); the car carries the gadget's
//! button over from tree to tree as its intent holds it, as C#'s `Fire`
//! flags did; the rest are in [`nodes::GrabByChainGadget`] and
//! [`nodes::DropByChainGadget`].
//!
//! `SupportAi` (`SupportAiFriend`, team 0, the default `ScanDistance` 16;
//! `SupportAi`, team 1), the friendly cars The City gives the player in its
//! third mission (its first `SupportAiFriend` car starts there, the second
//! takes the control when the player reaches the gate), with its subtree
//! `FollowLeader`:
//!
//! ```text
//! C#                                              Rust
//! SupportAi =                                     support_ai() =
//!   bt.Selector("Support AI",                       select((
//!     SmartAttack,                                    smart_attack(),
//!     ExecuteOrders.Instance,                         order_executor_ai(),
//!     FollowLeader,                                   follow_leader(),
//!     bt.Idle)                                        action(Idle),
//!                                                   ))
//! FollowLeader =                                  follow_leader() =
//!   bt.Selector("FollowLeader",                     select((
//!     bt.Sequence("Follow",                           seq((
//!       bt.Condition("Leader is far",                   check(a live leader),
//!         Leader != null && Leader.Alive),
//!       bt.Selector("Move",                             select((
//!         bt.While("Close", null,                         if_else(the leader's place in 1 s is
//!           !CloserThanByChebyshev(Position,                not within 10,
//!             Leader.GetPredictionPosition(1), 10)          action(FollowBy(that place,
//!             ? Running : Fail,                               1, 10)),
//!           FollowBy("Follow leader",                       check(never)),
//!             Leader.GetPredictionPosition(1), 1, 10)),
//!         MoveToRandomVisiblePosition(                    action(MoveToRandomVisiblePosition(
//!           b => ScanDistance, 0.1f))),                     own position, ScanDistance, 0.1, 1)),
//!                                                       )),
//!                                                     )),
//!     bt.Sequence("Find Leader",                      leaf(find_nearest_leader),
//!       FindNearestLeader))                         ))
//! ```
//!
//! What it comes to (its history: `SupportAi` and the leader, December
//! 2013; the attack `SimpleAttack` until `SmartAttack` took its place in
//! January 2014; in February 2014 "supporter move in 10m": `Close` was
//! `ScanDistance / 2`, `FollowBy` had no radius and the wander was at half
//! speed): the supporter fights the nearest enemy within `ScanDistance` as
//! `AttackAi` does (`SmartAttack`; the enemy the leader sees first); with
//! none it carries out the orders the map gave it, if any; with none it
//! follows its leader, the player's car or a friend AI with a high enough
//! `Traits[0]` (`find_nearest_leader`, which also keeps the last leader): by
//! a path to where the leader will be in a second while it is not within 10
//! of that place, at a full stick; when it is, it wanders at a tenth of the
//! stick to random places in sight within `ScanDistance`, a new one each time
//! the last is reached, staying about the leader. With no leader it looks for
//! one (the tree ends at once and is thought over again at the next step) and
//! otherwise stands (`Idle`). The leader is looked for only when it is gone
//! or dead, never while a living one is out of range: `Leader is far` asks
//! only that it lives, so a supporter whose leader has left its
//! `ScanDistance` still follows it.
//!
//! Differences from C# in `SupportAi`: none of its own. Its parts keep their
//! own (`SmartAttack`, `ExecuteOrders`, `FindNearestLeader`, `FollowBy`,
//! `MoveToRandomVisiblePosition`, the `Timer`s ending at a think); the
//! leader's `Leader.GetPredictionPosition(1.0f)` has no `Removed` branch (a
//! leader gone is no live leader, and its `Close` is asked of a live one
//! only). `ExecuteOrders.Instance` is one node shared by every `SupportAi`
//! car in C#; the port builds one per tree, each reading its own car's queue,
//! as `order_executor_ai` does for its own controls.
//!
//! Not ported here: every other tree of `BTActionFactory` (`SimpleAttackDelayPos`,
//! `Attack`, `SimpleAi`, `GuardAi`, `SmartAttack` (`AttackAi`'s: the
//! first version of the smart attack, which `AttackAiVer2` replaced), … the
//! ones with the nodes not yet ported: see `docs/BACKLOG.md`), and
//! `TurretAiFireToAnyone` (`AiSimpleTurretFriendFireAvailableTarget`:
//! `SimpleAttackByAngle`, `FindAngleNearestEnemy`, `TryFireToAnyone`), which no
//! map uses.

use bevy::math::Vec2;
use flatbt_bevy::prelude::*;

use super::blackboard::AiBlackboard;
use super::nodes::{
    AimToTarget, DropByChainGadget, FireToTarget, GrabByChainGadget, PositionFn, execute_orders, executing_drop, Flocking, FollowBy, Idle, alert_friends, find_nearest_enemy, find_nearest_leader, fire_now, get_target_from_variables,
    MoveBase, MoveInInitDirection, MoveTowardsTargetNoPath, MoveToPoint, MoveToRandomVisiblePosition, Power,
    Runaway, Scanning, SimpleFollowByTarget, sleep, stop_moving, TriggerMapAction, TryFireToAnyone, all, optional,
    act, never, time_limit, timer, while_target_alive,
};
use super::order_queue::OrderKind;
use crate::button::PRESS_AFTER;
use crate::gadget::REACH;
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

/// `FollowLeader`'s `Close`: the supporter follows while it is not within
/// this many units (on both axes) of where the leader will be in 1 s, and
/// is as near as `FollowBy("Follow leader", …, 1.0f, 10)` needs to be.
pub const SUPPORT_CLOSE_DIST: f32 = 10.0;
/// `FollowLeader`'s `MoveToRandomVisiblePosition(…, 0.1f)`: how slowly a
/// supporter near its leader wanders, as a share of the full stick.
pub const SUPPORT_WANDER_SPEED_RATE: f32 = 0.1;

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
    named("Fire", act(all((FireToTarget::new(predict_sec), SimpleFollowByTarget::new(predict_sec)))))
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

/// Whether the target has ever been seen (`LastKnowTargetPosition` is set).
/// Every tree that goes to where it was last seen asks first, and fails its
/// branch when it was not.
fn has_last_known(bb: &AiBlackboard) -> bool {
    bb.last_known_target_position.is_some()
}

/// Where the target was last seen, for the nodes that take a point: the
/// origin, which they refuse as "no point", when it never was (the trees
/// check [`has_last_known`] before).
fn last_known(bb: &AiBlackboard) -> Vec2 {
    bb.last_known_target_position.unwrap_or_default()
}

/// `BTActionFactory.Chase`'s `MoveToLastKnownTargetPosition`: with a target
/// (alive) that has been seen, go to where it was last seen.
fn move_to_last_known_target_position() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToLastKnownTargetPosition", seq((
        named("HastTarget", check(|bb: &AiBlackboard| bb.live_target().is_some() && has_last_known(bb))),
        named("MoveToLastKnownPos", act(MoveToPoint::new(|bb| last_known(bb)))),
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

/// `BTActionFactory.ChaseCurrentTargetForever`: while the target lives,
/// chase it and fire when the gun points at it, or chase it alone.
fn chase_current_target_forever() -> impl BehaviorNode<AiBlackboard, Orders> {
    while_target_alive(named("FollowAndFire", select((
        named("Chase and fire", act(all((FireToTarget::new(0.0), FollowBy::new(|bb| bb.target_position_safe()))))),
        act(FollowBy::new(|bb| bb.target_position_safe())),
    ))))
}

/// `BTActionFactory.ChaserTargetFromLocalVariable` (2014, "Ai to chase
/// specified target"): chase the car the map named in the car's own
/// `Target` variable (Escape's hover scout after the player), forever.
pub fn chaser_target_from_local_variable() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("ChaserTargetFromLocalVariable", seq((leaf(get_target_from_variables), chase_current_target_forever())))
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
    named("Maneur", seq((
        named("HasLastKnown", check(has_last_known)),
        act(all((
            MoveToRandomVisiblePosition::new(
                last_known,
                |bb| bb.gun.map_or(0.0, |gun| gun.range.range * MANEUR_RANGE_RATE),
                1.0,
                2.0,
            ),
            optional(FireToTarget::new(predict_sec)),
        ))),
    )))
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
            fire_round(),
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
            fire_round(),
            maneur(-0.5),
        ))),
    )))
}

/// The smart attacks' round of fire: a second aimed half a second behind
/// the target, then half a second at it (`Timer(1, FireToTarget(-0.5))`,
/// `Timer(0.5, FireToTarget(0))`).
fn fire_round() -> impl BehaviorNode<AiBlackboard, Orders> {
    seq((
        named("Timer_1s", timer(1.0, act(FireToTarget::new(-0.5)))),
        named("Timer_0.5s", timer(0.5, act(FireToTarget::new(0.0)))),
    ))
}

/// The smart attacks' `Follow`: by a path to where the target was last seen
/// (fails to start when it never was: no point, [`last_known`]).
fn follow_last_known() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Follow", act(FollowBy::new(|bb| last_known(bb))))
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
        follow_last_known(),
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
            return Vec2::ZERO;
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
            fire_round(),
            maneur(0.0),
        ))),
        follow_last_known(),
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
        // C#'s `Uii` returns `false`: the alarm given, the tree goes on.
        named("Attack", force_failure(seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.live_target().is_none())),
            named("Uii", leaf(alert_friends)),
        )))),
        named("ChangeTarget", seq((
            named("Agr", check(|bb: &AiBlackboard| bb.took_damage() && bb.target() == bb.damager_entity())),
            leaf(find_nearest_enemy),
        ))),
        named("Attack", seq((
            leaf(find_nearest_enemy),
            named("HasLastKnown", check(has_last_known)),
            while_target_alive(act(Flocking::toward(1.0, 8.0, 1.0, last_known))),
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
/// `RemeberTargetPos(0)` then `MoveToPoint(b => b.TargetPosition)`: where a
/// live target is now (`GetPredictionPosition`, a place in a wall moved to a
/// free one), asked once as the charge starts; with no live target, none
/// (`ZERO`, which `MoveToPoint` refuses, failing the attack as C#'s
/// `RemberPos` did).
///
/// `GetPredictionPosition`'s `Removed` branch (a removed body answers its
/// `SafePosition`) is not here: a removed or deactivated target is no live
/// target.
fn where_the_target_is(bb: &mut AiBlackboard) -> Vec2 {
    match bb.live_target().copied() {
        Some(target) => bb.prediction_position(target.pos, target.vel, HAMMER_CHARGE_PREDICT),
        None => Vec2::ZERO,
    }
}

pub fn hammer_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Hammer Ai", select((
        named("AttackHammer", seq((
            named("Timer_1s", timer(HAMMER_CHASE_TIME, chase())),
            named("Timer_3s", timer(HAMMER_CHARGE_TIME, named("MoveTo_PrevTargetPos", act(MoveToPoint::new(where_the_target_is))))),
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
    named("Order", execute_orders(move_order(), carrier(), grab_cargo(), press_button()))
}

/// `BTActionFactory.FollowLeader`: see the module docs.
pub fn follow_leader() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("FollowLeader", select((
        named("Follow", seq((
            named("Leader is far", check(|bb: &AiBlackboard| bb.leader().is_some())),
            named("Move", select((
                // `While("Close", null, !CloserThanByChebyshev(..) ? Running : Fail)`.
                named("Close", if_else(
                    |bb: &AiBlackboard| {
                        !closer_by_chebyshev(bb.pos, bb.leader_prediction(FOLLOW_LEADER_AHEAD), SUPPORT_CLOSE_DIST)
                    },
                    named("FollowBy_Follow leader", act(FollowBy::with_params(
                        |bb| bb.leader_prediction(FOLLOW_LEADER_AHEAD),
                        1.0,
                        SUPPORT_CLOSE_DIST,
                    ))),
                    check(never),
                )),
                named("MoveTo_RandomPos", act(MoveToRandomVisiblePosition::new(
                    |bb| bb.pos,
                    |bb| bb.scan_distance,
                    SUPPORT_WANDER_SPEED_RATE,
                    1.0,
                ))),
            ))),
        ))),
        // `bt.Sequence("Find Leader", FindNearestLeader)`: a sequence of one is the leaf.
        leaf(find_nearest_leader),
    )))
}

/// `BTActionFactory.SupportAi` (`SupportAi`, `SupportAiFriend`): fight the
/// nearest enemy, else carry out the map's orders, else follow the leader,
/// else stand. See the module docs.
pub fn support_ai() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Support AI", select((smart_attack(), order_executor_ai(), follow_leader(), act(Idle))))
}

/// Where a `MoveOrder` goes: the order's point, or no point (the origin).
fn move_order_target(bb: &mut AiBlackboard) -> Vec2 {
    match bb.executing.map(|order| order.kind) {
        Some(OrderKind::Move { to }) => to,
        _ => Vec2::ZERO,
    }
}

/// `MoveOrderRun`'s order: `new MoveToPoint("MoveOrder", b => p)`, the
/// defaults (speed rate 1, near enough within 1).
pub fn move_order() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveOrder", act(MoveToPoint::new(move_order_target)))
}

/// What a magnet reaches without the car moving (`ActiveDistance`) over two
/// and `ActiveDistance - 1` (`MoveUntilPositionNonVisible`'s limits for a
/// cargo).
const GRAB_FROM: f32 = REACH * 0.5;
const GRAB_TO: f32 = REACH - 1.0;
/// `CreateDropCargoBehaviour`: where, from the point, the car looks for a
/// place to drop at, and how near it must stand.
const DROP_PLACE_FROM: f32 = REACH * 0.5;
const DROP_PLACE_TO: f32 = REACH;
const DROP_FROM: f32 = REACH * 0.5;
const DROP_TO: f32 = REACH * 0.9;
/// `Timer(1.0f, bt.Idle)` before the grab, s.
pub const GRAB_WAIT: f32 = 1.0;
/// `CreatePressButtonBehaviour`: the wait is the button's time and this, s.
pub const PRESS_EXTRA: f32 = 0.2;

/// `BTActionFactory.MoveUntilPositionNonVisible`'s `While`: whether the car
/// is to go on moving: it can move, and the goal (`goal`) is not yet within
/// `hi` and no nearer than `lo`, and in sight. (C#'s condition said `Ok` when
/// it is, `Running` when not.)
fn keep_moving(bb: &AiBlackboard, goal: fn(&AiBlackboard) -> Vec2, lo: f32, hi: f32) -> bool {
    if !bb.can_move {
        return false;
    }
    let goal = goal(bb);
    let near = |d: f32| goal.distance_squared(bb.pos) < d * d;
    !(near(hi) && !near(lo) && !bb.line_collides(goal, bb.pos))
}

/// `BTActionFactory.MoveUntilPositionNonVisible(posFunc, goalFunc, minL,
/// maxL)`: drive (`MoveToPoint` to `pos`, picked when it starts) while the
/// goal is not yet between `lo` and `hi` away in sight, then stop. `goal` is
/// what the check measures to: C#'s `goalFunc`, else the end of the path, else
/// `posFunc`.
pub fn move_until_position_non_visible(pos: PositionFn, goal: fn(&AiBlackboard) -> Vec2, lo: f32, hi: f32) -> impl BehaviorNode<AiBlackboard, Orders> {
    named("MoveToActiveDist", seq((
        named("CheckDist", if_else(
            move |bb: &AiBlackboard| keep_moving(bb, goal, lo, hi),
            named("MoveToDest", act(MoveToPoint::new(pos))),
            named("Ok", leaf(|_: &mut AiBlackboard| NodeResult::Success)),
        )),
        stop(),
    )))
}

/// The cargo of the order being carried out, as the gather last saw it.
fn cargo_position(bb: &AiBlackboard) -> Vec2 {
    super::nodes::executing_cargo(bb).and_then(|cargo| bb.thing(cargo)).map_or(Vec2::ZERO, |cargo| cargo.pos)
}

fn cargo_position_mut(bb: &mut AiBlackboard) -> Vec2 {
    cargo_position(bb)
}

/// `CreateGrabCargoBehaviour`'s check: C# had no goal function, so the end of
/// the path the car is on, or else the cargo.
fn grab_goal(bb: &AiBlackboard) -> Vec2 {
    bb.path.as_ref().map_or_else(|| cargo_position(bb), |path| path.last_position())
}

/// `CreateGrabCargoBehaviour(findCargoFunc)` (`GrabCargoOrder`): with nothing
/// on the rope, find the cargo, drive to within reach of it in sight, stand a
/// second, shoot the magnet at it and take it, then run the car's
/// `MapActionOnTirgger` once. Fails when the cargo is gone, there is no way
/// to it, or the magnet does not take it in 5 s.
pub fn grab_cargo() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("GetCargo", seq((
        named("HasNoCargo", check(|bb: &AiBlackboard| bb.magnet.is_none_or(|m| m.towing.is_none()))),
        // `findCargoFunc(b)`, `cargo != null && !cargo.Removed`; `PutCustomData`
        // is not needed (the cargo is the order's).
        named("FindCargo", check(|bb: &AiBlackboard| super::nodes::executing_cargo(bb).is_some_and(|c| bb.thing(c).is_some()))),
        move_until_position_non_visible(cargo_position_mut, grab_goal, GRAB_FROM, GRAB_TO),
        named("Timer_1s", timer(GRAB_WAIT, act(Idle))),
        act(GrabByChainGadget),
        named("Trigger", act(TriggerMapAction::new(true))),
    )))
}

/// `CreateDropCargoBehaviour`'s `FindTargetPos`: `TargetPosition` is the
/// order's point.
fn find_target_position(bb: &mut AiBlackboard) -> NodeResult<Orders> {
    match executing_drop(bb) {
        Some((to, _)) => {
            bb.target_position = to;
            NodeResult::Success
        }
        None => NodeResult::Failure,
    }
}

/// The order's `DropPosition`, if it gives one.
fn drop_position(bb: &AiBlackboard) -> Option<Vec2> {
    match bb.executing?.kind {
        OrderKind::MoveCargo { drop_at, .. } => drop_at,
        _ => None,
    }
}

fn drop_position_or_origin(bb: &mut AiBlackboard) -> Vec2 {
    drop_position(bb).unwrap_or(Vec2::ZERO)
}

/// `MapManager.GetRandomFreeVisiblePoint(TargetPosition, ActiveDistance * 0.5,
/// ActiveDistance)`: where the AI finds a place to drop the cargo.
fn random_drop_place(bb: &mut AiBlackboard) -> Vec2 {
    let target = bb.target_position;
    bb.random_free_visible_point_from(target, DROP_PLACE_FROM, DROP_PLACE_TO)
}

fn target_position(bb: &AiBlackboard) -> Vec2 {
    bb.target_position
}

/// `CreateDropCargoBehaviour` (`MoveCargoOrder`'s second half): with cargo on
/// the rope, drive to the drop point, or to a place the AI finds within
/// reach of the order's point in sight of it; then hold the cargo over the
/// point and drop it. A car that cannot move skips the drive.
pub fn drop_cargo() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("DropCargo", seq((
        named("HasCargo", check(|bb: &AiBlackboard| bb.magnet.is_some_and(|m| m.towing.is_some()))),
        named("FindTargetPos", leaf(find_target_position)),
        named("SkipDriveWhenStatic", select((
            named("IsStatic", check(|bb: &AiBlackboard| !bb.can_move)),
            if_else(
                |bb: &AiBlackboard| drop_position(bb).is_some(),
                named("DropPos", act(MoveToPoint::new(drop_position_or_origin))),
                move_until_position_non_visible(random_drop_place, target_position, DROP_FROM, DROP_TO),
            ),
        ))),
        act(DropByChainGadget),
    )))
}

/// `CreateCarrierBehaviour` (`MoveCargoOrder`): drop the cargo the car has on
/// the rope, else grab the cargo and then drop it.
pub fn carrier() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("Carrier", select((drop_cargo(), named("Seq", seq((grab_cargo(), drop_cargo()))))))
}

/// The button of the order being carried out, as the gather last saw it.
fn button(bb: &AiBlackboard) -> Option<super::blackboard::Thing> {
    let OrderKind::PressButton { button } = bb.executing?.kind else {
        return None;
    };
    bb.thing(button).copied()
}

fn button_position(bb: &mut AiBlackboard) -> Vec2 {
    button(bb).map_or(Vec2::ZERO, |b| b.pos)
}

/// `CreatePressButtonBehaviour(button)` (`PressButtonOrder`): drive onto the
/// button, check the car is on it, stand for its time and a fifth of a
/// second, and check it is pressed.
pub fn press_button() -> impl BehaviorNode<AiBlackboard, Orders> {
    named("PressButton", seq((
        named("Check", check(|bb: &AiBlackboard| button(bb).is_some())),
        named("MoveTo", act(MoveToPoint::new(button_position))),
        named("Check that pressing", check(|bb: &AiBlackboard| button(bb).is_some_and(|b| closer_by_chebyshev(bb.pos, b.pos, b.radius)))),
        named("Timer", timer(PRESS_AFTER + PRESS_EXTRA, act(Idle))),
        named("Check state", check(|bb: &AiBlackboard| button(bb).is_some_and(|b| b.pressed))),
    )))
}
