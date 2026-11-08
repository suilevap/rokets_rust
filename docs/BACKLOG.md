# Backlog

Agreed work that is not scheduled yet. The full migration order is in the C#
repository's plan (`docs/plans/RUST_BEVY_MIGRATION.md`, section 9 onwards);
this file keeps what came up along the way. Take an item into its own pull
request; remove it here when it lands.

## Deferred from MVP 0.1

- **Car shadows** (`DynamicShadow`): sprite streaks from the nearest lamp
  that casts dynamic shadows, split penumbra when the lamp is close, lamp
  hand-over, ambient blob. Taken out of PR #1 as not close enough to the
  original; the removed code is in commit 67be4b9. Read the author's
  article on light and shadow first (moddb "Tile Rider: the history of
  light and shadow…").
- **Wall outlines**: the black line along wall tops (`Primitive3d.AddBorders`).
  Also in 67be4b9.

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
  vibrated the phone or gamepad and shook the camera. Port both: camera
  shake works everywhere; vibration via `navigator.vibrate` on Android
  browsers (none on iOS), the gamepad's `vibrationActuator` in Chrome, Bevy
  gamepad rumble natively. About half a day.

## Rendering and camera

- Camera zoom as in C#: `Camera2DOperatoAllInCadr` zooms out to keep all
  focus entities (players, and in some maps more) in frame, never closer
  than the map's `MinScale`. That is why C# shows more of EditorTest (two
  players) and LightTest than the fixed height here.
- Light differences left after the outland: Middleware's corridor is
  bright in C# and dark here (light bake), and C#'s additive HDR light
  layer tints the gaps between tiles (green on LetItSnowDay).
- Glow and dirty-lens effects (and `DarkCorners`, the vignette); dynamic lights;
  the cabin's springy wobble; tyre marks.

## Web

- Download size: the wasm is 20 MB after `wasm-opt`. Trim Bevy features for
  the web build as gridvail does.

## Code

- Move the remaining inline `#[cfg(test)] mod tests { … }` blocks out of
  source files (see `CLAUDE.md`).
- Compare driving with the C# game by recorded paths (`--drive` here; the
  C# command server can apply the same input).
