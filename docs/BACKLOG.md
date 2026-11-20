# Backlog

Agreed work that is not scheduled yet. The full migration order is in the C#
repository's plan (`docs/plans/RUST_BEVY_MIGRATION.md`, section 9 onwards);
this file keeps what came up along the way. Take an item into its own pull
request; remove it here when it lands.

## Enemies and AI

The map's `SimpleCars` are spawned (`car`, `car/simple`, `car/death`) with
their health, shield, team, weapon and wreck. Their controls (`ai`) are live
for `AiSimpleTurret` / `AiSimpleTurretFriend` (`TurretAi`), `AiChaser`
(`ChaserAi`), `MoveByPath` / `MoveByPathFriend` / `MoveByPathSafelyFriend`
(`FollowPathAi`, `FollowPathAiSafe`), `GoToEnemySimplePredictionPosition`
(all flatbt trees), `AttackAiVer2`, `SimpleAttackNearestEnemy`,
`SimpleFollowByEnemyAi`, `MoveAndFireInInitDirection`, `ZombieFlocking`,
`ZombieFlockingFriend`, `SimpleFlocking`, `AlwaysShoot` and
`ShootPattern`; every other
`ControlType` that is an `AIBTBaseControl` stands idle until the next slice.
The navigation (`map/nav`: `NavGraph`, `NavPath`) and the nodes that drive
along paths (`MoveBase`, `MoveToPoint`, `FollowBy`, `TryFireToAnyone`,
`TriggerMapAction`, `Optional`) are in. In order:

1. **The other behaviour trees.** Each with what it still needs (the rest of
   `BTActions/*` and `BTActionFactory`):
   - `Ai` / `AiFriend` (`GameMode.GetAi`: check which tree the single-player
     mode gives; `SimpleAi` = `Selector(Attack, MoveBase, SimpleScanning)`
     would need only ported nodes);
   - `GuardAi`: `Emotion` (the `?` / `!` bubbles), `SeeTargetForward`,
     `PatrolAction` (`MoveToPoint` to `Entity.StartPosition` — the car's start
     — then `PatrolPath` as the `Path`; `SetPath` already stores it),
     `AlwaysTryShoot`, `MoveToRandomVisiblePosition`
     (`GetRandomFreeVisiblePoint`, `RandomHelper.NextVector2Fake`);
   - `SupportAi*`, `OrderExecutor*`: `ExecuteOrders`, `OrdersManager`,
     `FollowLeader`, `SmartAttack` (below); `FindNearestLeader`, the
     `Leader` and the friends (`Mate`) are in with the zombies;
   - `AttackAi` (`AttackAiFriend`): `AttackerAi` = `Selector(SmartAttack,
     Sequence(MoveToPoint("Rnd"), Timer(2, Idle)))`; `SmartAttack` /
     `SmartAttackTarget` is the first version of `SmartAttackVer2` (it
     stands, fires and manoeuvres with no style choice: `Timer`s and
     `Maneur` are ported, add the tree). `ChickenAi` = `PanicBehaviour`
     (`panic_behaviour()` is ported: register it). `HammerAi*`:
     `RemeberTargetPos` (`GetPredictionPosition`) and the Hammer tree.
     The Plasma gun and rocket launcher (`IsImmidiate` false) come with
     the weapons (`Kind::is_immediate`);
   - `CaptureTheBox*` (`CustomAi`): the chain gadget actions
     (`GrabByChainGadget`, `DropByChainGadget`), `FixBlocker` (the path's
     `Blocker` is there), `MoveUntilPositionNonVisible`, custom data on the
     blackboard;
   - `ChaserTargetFromLocalVariable`: `GetTargetFromVariables` (map
     variables read from a tree);
   - `AiSimpleTurretFriendFireAvailableTarget` (`TurretAiFireToAnyone`:
     `SimpleAttackByAngle`, `FindAngleNearestEnemy`; no map uses it).
   And `BTLib`'s `RepeatUntilSuccess`, `DynamicNode`, `Condition` where a tree
   wants them, `Orders`, `LastDamagerPosition`, `AdditionalTargetPosition`.
   A tree whose world reads are heavier than the turret's (many cars) will
   want the gather to copy less per step than it does (it already casts
   sight only on a thinking step; `FollowPathAi` cars also gather the
   enemies every step for `TryFireToAnyone`), and `flatbt-bevy`'s
   `.parallel()` once the blackboard is the only thing a tree touches (the
   port runs single-threaded on the web).
   `GetPredictionPosition` corrects a prediction that lands in a wall
   (`GetNearestFreePoint`: the first 16 of the map's free points): done for
   the zombies' leader (`AiBlackboard::prediction_position`), not yet for
   the targets (`SimpleFollowByTarget`, `WillSeeTarget`): use it there.
   The `Boid` collision category (`Flocking` on a car under half a unit of
   radius: such cars pass through each other) waits for collision layers
   and a map with such a car.
   **Navigation, left open:** a door shoved open by a car does not open
   its navigation links (C# opened them from `OpenDoor()` only too);
   fake walls are free in C#'s distance field (the port has no fake walls
   yet); the free points (`GetFreePoints`, `FreePoints`,
   `GetRandomFreeVisiblePoint`, …); `AIPathCarControl` (`AISimpleCarControl`,
   `AITouchAndRunCarControl`, `PlayerCarControlPath`: no `ControlType`
   reaches them); a debug view of each car's path (`AiBlackboard.Draw`).
2. **Weapons and enemy extras.** The other weapons (`PlasmaGun`, `Laser`,
   `Tesla`, `Shotgun`, `Rocketlauncher*`, `LaserWithDelay`:
   `PeriodicFireControl`'s pre-fire uses its `ChargeTime`), the red aim
   line of a static or enemy gun (`LaserVisiblityUpdate`: drawn for every
   gun, not only the player's, with the Railgun's bounces), `SetImmortal`
   and the `Immortal` shield, `HighlightEnemy` / `HighlightDeadEnemy`.
3. Other `Players` of a map (AI-driven `Car`s: only the first is spawned),
   their clones, `NoneLocal` cars' immunity out of the view, an AI
   control on the player's own car (`SetControl(player, AiChaser)`).
4. Check C#'s friction on cars: `ControlableEntity` sets
   `PhysicBody.Friction = 0` (`LinearDragCoefficientDefault`) after the
   fixture exists, so cars may have no friction in C#, not 0.8 as boxes
   and cars have here. Compare a car sliding along a wall.
5. Compare the turrets with the C# game over many runs: first shots come
   after a random part of the weapon's reload (`PeriodicUpdate.Sparse`), so
   one run each says little. On `JunkYard` the Railgun turret in front of
   the player's start fired at 4.5 s in C# against 1.1 s here, on `Targets`
   and `Friend` they matched in kind (a Minigun kills the 12-health car in
   about 1.4 s, a Railgun in one shot); find out what holds the C# turret
   there (a map script?).

## Phone

- **Accelerometer.** In the C# game it did not steer the car: it tilted the
  camera (`Camera2DOperator.ReadAccelerometer`, per map
  `CameraReadAccelerometer`), with the neutral pose taken at level start
  (`SetCurrentValueAsBase`); also snow in the Christmas theme and wobbling
  menu text. On the web: `deviceorientation` / `devicemotion` read from
  JS; iOS Safari needs `DeviceMotionEvent.requestPermission()` from a tap;
  axes depend on screen orientation (now also portrait). Steering by tilt
  would be a new control scheme writing `PlayerIntent`. About a day; test
  on a real phone through a pull request preview.
- **Force feedback.** One event in C# (`ForceFeedback`): hard head-on hits
  on walls (`Car.cs`, impact over 0.4), damage, the accelerator gadget. It
  vibrated the phone or gamepad and shook the camera. The shake is in
  (`feedback`); still to port: vibration via `navigator.vibrate` on Android
  browsers (none on iOS), the gamepad's `vibrationActuator` in Chrome, Bevy
  gamepad rumble natively. About half a day.

- **Touch controls for the gadget and weapons, portrait first.** The C#
  scheme (`PlayerCarControl`: a floating stick, a gadget circle at the
  top right, flicks across the rope to cut it) was made for landscape and
  works well only there; in portrait the thumbs sit at the bottom and the
  view is turned. For now the magnet takes C#'s landscape zone on every
  screen (the top right quarter, drag to aim, a faint circle marks it):
  design a scheme that suits both
  orientations (e.g. a second floating control in the other half of the
  screen, aim by where it is dragged) and try it on a phone through a
  pull request preview.

## Physics

- Pushing a box while turning (`tools/compare.py AiTest --start 3.5,14.5
  --ids 4001,1007 --drive "1,0:1;1,1:1;0,1:0.5"`): in C# the box slides
  off the bumper and turns (−111° at t=1.5) and the car, free, turns to
  −32° and speeds up to 12.9; here the box stays in front (−96°) and the
  car stays near 0° at 8.2, 1.7 units apart by t=2. Straight driving,
  turning and the magnet tow match within 0.2–1.3 units. Sizes, masses,
  inertia, friction (0.8, geometric mean), restitution (0.5, max) and
  damping are the same, so it is the contact solver (Farseer 3.3.1, 8
  velocity / 3 position iterations, against Avian's substeps); try
  Avian's contact settings (softness, substeps, speculative margin).

## Rendering and camera

- C#'s dirty lens tints the picture (green on LetItSnowDay).
- The additive light adds in linear colour here and in sRGB numbers in
  C# (XNA has no gamma); a fixed ×2 boost (`GLOW_BOOST`) brings the look
  close. Exact only with the glow added in sRGB (a small post-process
  pass) or the whole picture drawn in sRGB numbers as XNA did.
- Glow and dirty-lens effects (and `DarkCorners`, the vignette); dynamic lights;
  tyre marks.

## Web

- Download size: the wasm is 20 MB after `wasm-opt`. Trim Bevy features for
  the web build as gridvail does.

## Code

- Move the remaining inline `#[cfg(test)] mod tests { … }` blocks out of
  source files (see `CLAUDE.md`).
- `tools/compare.py` on a map with `SimpleCars` (Targets, every Minigun
  map but Teleport): they are in the port now, so compare bullets and
  rams on a target against the C# game too. `DestroyEntity` takes a simple
  car out at once in the port (no wreck, no blast, `ActionOnDestroy` runs:
  `NetworkPhysicEntity.Destroy` then `FinalRemove`, read from the source; an
  older note here said it blasts, which the source does not show: check). Sent
  before its first update it crashed the C# game (its weapon is made then and
  reads its place).
- `tools/compare.py` on a map with a scripted intro: start the C#
  script with the map (e.g. `/drive` reporting the map's game time, or
  a `-drive` command-line option), so intros line up.
