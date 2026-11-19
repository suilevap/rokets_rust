# Backlog

Agreed work that is not scheduled yet. The full migration order is in the C#
repository's plan (`docs/plans/RUST_BEVY_MIGRATION.md`, section 9 onwards);
this file keeps what came up along the way. Take an item into its own pull
request; remove it here when it lands.

## Enemies and AI

The map's `SimpleCars` are spawned (`car`, `car/simple`, `car/death`) with
their health, shield, team, weapon and wreck, and stand idle: their
`PlayerIntent` stays zero until an AI writes it. In order:

1. **Turret AI and enemy firing.** `AiSimpleTurret*`, `AlwaysShoot` (fire
   along the car's heading: `c.Fire = Shoot; WeaponAxisControl =
   DirectionVector`), `ShootPattern` (`PeriodicFireControl`), the enemies'
   use of weapons (`TryShoot` with `EnergyComponent` for enemies, the
   firing range and dispersion, `AdjustWeaponMaxAngle` for static cars with
   a weapon the map names, the aim line of static / enemy guns,
   `UpdateAimMagnet`), the other weapons (`PlasmaGun`, `Laser`, `Tesla`,
   `Shotgun`, `Rocketlauncher*`, `ZombieWeapon`), `SetImmortal` and the
   `Immortal` shield, `HighlightEnemy` / `HighlightDeadEnemy`.
2. **Flat behaviour trees and navigation.** `AIBTBaseControl`
   (`BTActionFactory`: chasers, flocking, path followers, `SetPath`,
   `ScanDistance`, `ActionOnBTTrigger`, `FireAccuracy`,
   `IsVisibleForAutoAim`), `AiBlackboard`, navigation over the map.
3. Other `Players` of a map (AI-driven `Car`s: only the first is spawned),
   their clones, `Local` simple cars (`DoorsDream1`, `MyGalaxy`: the player
   drives a `SimpleCar`), `NoneLocal` cars' immunity out of the view.
4. Check C#'s friction on cars: `ControlableEntity` sets
   `PhysicBody.Friction = 0` (`LinearDragCoefficientDefault`) after the
   fixture exists, so cars may have no friction in C#, not 0.8 as boxes
   and cars have here. Compare a car sliding along a wall.

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
