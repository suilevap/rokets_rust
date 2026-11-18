# Architecture

One crate, a library (`src/lib.rs`, `TileRiderPlugin`) and a binary
(`src/main.rs`). Each system is a plugin in its own module; plugins meet in
components, resources and messages, not in each other's code.

```text
content  units            (no Bevy app; plain data and parsing)
   ↑       ↑
map::grid  light::{bake,raster}  car::drive   (pure algorithms, unit-tested)
   ↑          ↑            ↑
game (loading, LoadedMap, MapEntity, ChangeMap)
   ↑
map::colliders  render  controls  car  pickups  triggers  hud
   ↑
main
```

## Modules

| Module | What it does | C# origin |
|---|---|---|
| `content/` | Parses the C# content: map XML (`MapData`), `Cars.xml` (base cars resolved), `MapList.xml`, action references (`NextMap()`, `CloseDoor(1001)`). Unknown elements are ignored; nothing here knows Bevy | `GameLibrary/` |
| `content/actions/` | Action types as data, for the game and a level editor: `def.rs` (the `actions!` macro: one declaration per type makes the struct, reading and writing its XML, and a `Field` table with what C#'s attributes said: optional, key, where its values come from (`source.rs`: a simplified XPath into the map or another content file, a content folder, or the templates, so an editor resolves any field the same way), description from the doc comment, default; `Base` for `ActionData`'s own fields; templates with their signatures), `defs.rs` (every C# action type, in C#'s field order; `tools/gen_action_defs.py` added the ones not written by hand, and checks those against C#), `node.rs` beside it (the XML tree they read and write) | `ActionData.cs`, `ActionTemplates.cs`, `FieldReferenceAttribute.cs` |
| `units` | World units = the C# "sim units" (32 px), so forces and speeds keep their values. Map y points down, world y up: convert only here | `ConvertUnits` |
| `assets` | Resolves C# content names case-insensitively through `assets/manifest.txt`; loads XML as text assets (works on the web) | content pipeline |
| `map/grid` | The map as cells from the image (alpha: floor below `ImageWallTreshold`, wall above, height in 32 px steps) and `CharMap`; `IsFloor`; the chamfer distance field (`CreateDistanceField`); corner wedges; walls merged into rectangles | `MapManager`, `TilesMap.GetH`, `MapInfo.BuildTextureFromCharMap`, `MapManager.GetEdgeIndex` |
| `map/colliders` | Static wall colliders | `PhysicsManager.CreateWalls` |
| `map/templates` | **The only place that reads a map entity's type**: turns each map entity into a template of properties (sprite and layer, body kind and density, axis locks, boost, pickup, spin, shadow); which fixed objects cast shadows into the light. Plain data, unit-tested | `EntitiesFactory`, entity classes |
| `map/spawn` | Builds entities from templates: sprite quads, Avian bodies (box, friction 0.8, bounce 0.5, C# damping) and behaviour components | `Entity`, `PhysicEntity` |
| `light/bake` | The static light, baked on the CPU as `LightManager` drew it: a target ~512 texels long over the map and 4 cells around it, ambient with a white border, per lamp the `flash2` spot with black shadow quads behind wall edges facing it, and fading ones behind fixed objects' far edges (`TryCastStaticShadow`), added twice turned ±0.02 rad (penumbra); the light layer is that blurred (½ ¼ ⅛ copies) plus full light over the void; the walls' light is it at ¼ size, blurred, read per texel; additive lamps also fill the additive light and a coarse map of where it reaches. Matched light targets saved from the C# game when ported (PRs #7, #11); `examples/light_dump` writes ours to compare. Kept on the CPU for gameplay and shadows | `LightManager`, `MapManager.AddOutSideLight` |
| `light/shadow_lamps` | Which lamp throws a moving object's shadow, per spot (4 light texels): each lamp for dynamic shadows claims the spots in its box it can see (a walk a unit at a time against wall cells) unless a lamp seen from there is nearer. Plain data, unit-tested | `LightManager.FillStaticLightPos`, `GetStaticLightInfo`, `MapManager.LineCollide` |
| `particles` | Particles as plain data, blended or additive (own pools): a `Style` is a begin and an end state (speed, scale, spin, turn, premultiplied colour, height) and a lifetime, values moving linearly; a fixed `Pool` (128) where optional effects give way to newer ones when full and the others live out their life. `particles/styles` holds the C# styles in our units (only those something spawns). Unit-tested | `Particles/`, `ParticleSystemManager` |
| `light/raster` | A software stand-in for the render-target drawing the C# light used: linear-sampled sprites, vertex-coloured triangles, premultiplied alpha and additive blending into a saturating target, `BlurSurface` | `LightManager.DrawOnLightMap`, XNA blend states |
| `game` | States `Loading → Playing` (`Failed`), `LoadedMap`, `MapEntity` (despawned on leaving a map), `ChangeMap` (restart / next) | `MapManager.SetMap` |
| `render/map_mesh` | Floor of loose, tilted tiles, always below the ground plane, those next to a wall leaning on it (edge by the wall up, turned by the distance field's gradient × π/8); walls as blocks with fading faces and their light in the vertex colours; tiles are lit by the light layer | `TilesMap.AddTiles2`, `Primitive3d`, `RenderSettings` "Mul_Add05" |
| Wall colours (`render/map_mesh`) | The standard unlit material uses linear vertex colours from one ramp by absolute height. Light saturates before the face fade; each side starts at the floor, including the hidden part behind a shorter neighbour. Equal-height corner wedges hide internal faces. The gradient approximates XNA in linear RGB, keeping height changes seamless | `Primitive3d.InitPositionWithLight`, `RenderSettings` "Mul_Add05" |
| `render/wall_outline` | Black strips 3/32 units wide along height changes and wall feet, including the map boundary. Squares and corner wedges share mitered joins; feet straddle the edge with their inner half hidden by the wall. Each convex corner uses one upright quad ending at the higher adjacent surface. The shared `render/lines` material rotates its width towards the camera on the GPU; top/foot strips keep their flat miters. One static mesh; tops lifted, feet lowered and uprights offset towards the camera by 0.1/32 units | `Primitive3d.AddBorders`, `LinePrimitive` (`InitVertical`), `StaticLinesWidth` |
| `render/lines` | Reusable unlit 3D ribbon renderer: `LineMeshBuilder` batches arbitrary segments with per-segment world width and optional fixed quads; `LineMaterial` controls colour/opacity and camera offset. Two triangles per segment, expanded on the GPU using the segment axis and current view. Independent of maps and the world up direction; see `cargo run --example lines` | Generalised from `LinePrimitive` |
| `render/additive_light` | The additive (HDR) light of lamps with an `AdditiveRate`, laid over the map with an additive blend and no depth test (over walls and cars, as C#); shown by `rate²` (boosted ×2 for Bevy's linear blending against XNA's sRGB sums) where `rate` eases (0.75/s) towards `1 − min(v / 0.2, 1)`, `v` the additive light where the camera looks: the eye adapting to the dark. A standard unlit material whose extension only turns off the depth test | `LightManager` (`IgnoreDepthWorldAdditive`), `HdrHelper` |
| `render/wall_ao` | The floor darkened at the foot of walls: one texel per cell from the chamfer distance field (`MapGrid::chamfer_distance`), half dark on walls, a quarter within two cells, stretched with linear filtering over the map at ground level; maps with cells of 3 units or more have none | `LightManager.CreateWallAmbientOcclusion`, `MapManager.CreateDistanceField` |
| `render/shadow` | Shadows of the car and loose objects: a `Gradient4Pixel` streak away from the spot's lamp, darkest halfway to its radius, up to 4 radii long and cut by the distance field, as wide as the caster seen from the lamp; a pair at half strength when it looks wider than 2° (penumbra), fading out from 20° to 45°; ½ s hand-over between lamps; a quarter-black contact box under it. Black alphas are converted so they darken in linear light as XNA's did in sRGB | `DynamicShadow` |
| `render/particles` | Draws the pool each frame as one mesh per texture and layer (vertex colours, lit by the light layer), wheel streaks under the cars and smoke over them | `ParticleSystemManager.Draw` |
| `render/outland` | Four vertex-coloured quads, a funnel under and around the map that shows between the tiles and past the walls: background colour at the inner edge 32 units down, the map's four `OutlandColor*` at the outer edge 32 units up | `OutlandPrimitive` |
| `render/light_layer` | The light map as one quad at ground level with a multiply blend, over the map and 4 cells around it: darkens the floor and the outland under it on screen; walls stand above it, sprites are drawn after it | `LightManager` light target, `LightBlendState`, `MapManager.AddOutSideLight` |
| `render/layers` | **The draw order**: floor geometry under `FLOOR_TOP`, then sprite layers (ground decor, car chassis, wheels, cabin, gears), ordered by a `depth_bias` rank, as the C# `DepthConst` table and its `BackToFront` sprite sort; all lie within a hundredth of a unit of the ground, so order never changes a sprite's size | `DepthConst`, `Renderer.Draw` |
| `render/sprite` | Sprites as unlit quads tinted by the light map, placed by their layer | `SpriteView` + light layer |
| `render/camera` | Perspective camera with a 0.7 s velocity lead and separate eased focus/position/zoom. Solo targets zoom out with speed (height 16–40); groups fit six-radius margins within those zoom limits. Map entry snaps after car spawning. Portrait rotates the view and controls a quarter turn while keeping short-side coverage (`ViewRotation`) | `Camera2D` |
| `controls` | `PlayerIntent` (where to go, aim, fire) written by keyboard, gamepad and touch schemes; the last one used is active; screen directions are turned into the world by `ViewRotation` | `IControl`, `CompositePlayerControl` |
| `car/drive` | The driving model as a pure function, run at 60 Hz | `Car.MovementControl`, `Brake`, `AdditionalFriction` |
| `car` | Spawns the player's car on Avian; sprites; front wheels; the cabin swaying on two springs pushed by the front and back points' acceleration, idling at rest (`car/cabin`); smoke (`car/smoke`: streaks behind slipping rear wheels, exhaust puffs when slow, idle puffs); impacts (`car/impact`: on a contact's start, the speed relative to what it hit over twice the top speed; above 0.2, a `CarImpact` message and sparks off the contact point) | `Car`, `ControlableEntity`, `Car.CreateTrail`, `Car.OnCollide`, `EffectManager.CollideEffect` |
| `triggers` | Trigger areas painted into a cell map in the map's order (later over earlier); a trigger with subscribers is set off by them only (a clone by its source), one without by the player's car only, each entity in one trigger at a time; enter and leave actions, `Amount` uses, armed or not; level end | `TriggerManager` |
| `actions` | Map actions by registry, as `ActionRunner`: each type defined in `content/actions/defs.rs` is carried out by a system `In<Run<T>> → bool` registered with `app.add_map_action` (templates such as `ActivateEntity(id)` come with the definition, picked by name and number of arguments); the core (`actions/mod.rs`) knows none of them. `RunAction` messages run every map action of a name in order (or a template), `NextAction` after success, sequences now or `TimeBetweenInSec` apart, `RunOnMapStart`; handlers run one at a time with the world to themselves. Types not ported are logged once per map | `ActionRunner`, `ActionTemplates`, `TriggerManager.RunAction` |
| `actions/variables` | Integer variables per map item (with its `Id`) and one global set, cleared with the map; `ChangeIntVariable` / `SetVar` / `SetGlobalVar` / `SetVarEx` set them, `CheckIntVariable` / `if` / `IfEx` compare with a number or another variable and run the action for less, equal or greater (succeeding only then, so `if` gates `NextAction`); built-in `ElapsedActiveTime` | `ActionRunner` (`GetValues`, `GetInt`, `CheckIntVariableRun`) |
| `button` | Floor buttons (`CreateButton`): a static sensor that its subscribers (or, with none, the player's car) must stand on for a second, looked at every 0.25 s with four lights filling up; then `ActionPress` runs for whoever stood there, and leaving runs `ActionRelease` (without one it stays pressed). Loose bodies on it are drawn to its middle. The press logic (`Press::look`) is plain data, tested apart | `ButtonBase`, `Button` |
| `teleport` | Teleports (`CreateTeleport`): a static sensor that moves a body (its subscribers, or any) to its target keeping its speed and runs its `Action` for it; the arrival is hidden for 0.5 s, and contacts starting meanwhile do not count, so it does not bounce back through a teleport at the target (`GenerateBackTeleport`). The camera jumps along on long jumps. A dark pulsing hole with streaks drawn into it | `Teleport` |
| `actions/focus` | What the camera frames: `AddEntitiesToFocus` (`FocusOnEntity`, `AddFocusOnEntity`) gives entities a `CameraTarget` (or brings theirs back), `RemoveEntitiesFromFocus` marks them `Unfocused`, `InstantCameraMove` puts the camera somewhere at once | `Camera2DOperatoAllInCadr`, `ActionRunner` |
| `actions/clone` | `CloneObject` (`Clone(id)`) and `CloneObjectByPath`: a map entity made again from its map data at random in an area (around an item) or spread along a path, taking free `TargetIds` (or coming back itself if gone); a clone carries `CloneOf`, and triggers, buttons and teleports take it as its source (`source_id`). Cars are not cloned yet (no AI) | `RunCloneObjectBase`, `RegistreClone` |
| `actions/lifecycle` | `ActionOnCreate` (entities when they appear, the player's car at map start) and `ActionOnDestroy` (entities the game destroys: `DestroyEntity`, bombs and red boxes going off) | `TriggerManager.OnNewEntity`, `OnEntityDestroy` |
| `actions/camera_path` | `CameraShowPath`: the camera flies a path (`CameraOverride` instead of its targets), physics paused while it moves, then `ActionOnEnd` (and back, `ActionOnReverseEnd`); it stays there until a focus action hands the camera back | `Camera2DOperatorFollowPath` |
| `actions/splash` | `ShowSplash` without the splash yet: its `ActionOnHide` runs when it would hide (`AutoHideTimeSec`, 2 s for one left to a tap), so map scripts that go on from splashes do | `ShowSplashRun`, `SplashScreen` |
| `platform` | Moving platforms (`AddMovingPlatformComponent`): an entity's speed set every step, still for half of `PeriodSec`, then out to its end (`Position`, or its length × `OpenDistanceScale` ahead) and back each half period | `MovingPlatform` |
| `actions/control` | `SetControl` on the player's car: `Local` gives it back, anything else (`NoneLocal` in intros, AI types for now) marks it `NoInput`; the car also starts so when the map's player is not `Local`, and keeps its state across a respawn | `SetControlRun` |
| `info` | `ShowText`: lines by key from the game's texts (`assets/Strings/Strings.resx`, `content/strings.rs`; English for now) in a box over the game for `ShowTimeSec` (7 s default), physics paused with `PauseGame` | `UIManager.ShowInfo`, `StringsExt` |
| `door` | Doors: `AddDoorComponent` makes a box slide along its facing by its length × `OpenDistanceScale`; `OpenDoor(id)` / `CloseDoor(id)` free and push it; at either end it snaps, stops, runs `ActionOnOpen` / `ActionOnClose`, fixes itself if static or fixed on open; a placeholder shows where it opens to. Its actions are registered here | `Door`, `AddDoorComponent`, `OpenCloseDoor` |
| `pickups` | `Pickup`: collected by driving into it; the count | `Coin` |
| `boost` | `Booster`: throws touching bodies along its facing and plays its flash | `Accelerator` |
| `explosion` | `Health`, `Hit`, `Destroyed`; `Explosion` messages throw dynamic bodies away from the blast (one push each, as strong as Farseer's rays together: angle covered × force × 180/π, fading to the range's edge) and hit what can be hurt in range, walls sheltering what is behind them, with the boom particles; `Explosive` blows up on touch (bombs) or after a fuse once hit (red boxes, so chains); bombs beep | `WeaponHelper.CreateExplosion`, Farseer `Explosion`, `Bomb`, `DamagableBox`, `ControlableEntity.TakeDamage` |
| `game_mode` | When the player's car is `Destroyed`: it turns into its wreck (`Destroyed1/2`), undriven and unbraked so a blast throws it far, bursts into smoke and fire; 2 s later the level restarts, or, once `SetRespawnPoint` / `SaveAction` (`Save()`) has set a respawn point, a life goes (three a level), `ActionOnRespawn` runs and after 2 s more the car is back at the point, the camera on it alone. The top line shows the lives | `GameModeSinglePlayer`, `Car.OnKilled`, `Car.Respawn`, `CreateDestroyedEntity` |
| `feedback` | Force feedback, so far a camera jolt (`CameraShake` in `render/camera`, dying away at 6/s): head-on car hits over 0.4, damage to the player by the share of health lost, blasts in sight | `Control.ForceFeedback`, `Camera2D.Shake` |
| `sound` | Sound cues worked out from game events, playing or not: `PlaySound` messages for hits (one of three bumps, louder the harder), blasts (pitch round), damage and `Accident`, gears, finish; the car's engine loop (stick → volume, speed → pitch) and tyre squeal (eased 3/s towards the slip's loudness) as `LoopCue`s on `CarSounds`. `sound/playback` plays them with Bevy audio behind the `sound` cargo feature (default; `--no-default-features` for a silent build); `web/index.html` resumes the browser's audio on the first touch, click or key | `SoundManager`, `SoundView`, `EffectManager`, `Car.EngineSoundControl` |
| `render/spin` | `Spin`: turns on the spot | `Coin` |
| `animation` | Animation as plain data: `Wave` (sawtooth / triangle phase over game time), `ColorTrack` (colour keyframes, stepped or blended), `Pulse` (one-shot blend). Unit-tested | `PhaseHelper`, `PeriodicUpdate.RemainPart` |
| `render/animate` | `TintLoop` (a colour track on a wave, per sprite) and `TintPulse` (a one-shot blend over an entity's sprites); game time pauses and slows with the game, and loops on the same wave stay in step | `PhaseHelper` users |
| `hud` | Debug text, `R` / `N` keys | — |
| `script` | `--drive`: scripted stick input and a logged path, for comparing driving | — |

## Conventions

- **Port intent, not implementation.** Each module's doc comment says what
  the C# code was for and where it came from; the C# game is the reference
  to compare against (screenshots, recorded driving).
- **Keep the signature tricks** (sprite shadows, tile floor, vertex-lit
  walls, soft baked light) before trying engine-standard replacements.
- Map data is parsed in full even when unused; unsupported entities and
  actions are logged per map, so coverage is visible.
- **Action definitions are apart from running them.** A type's fields,
  metadata and templates are declared once in `content/actions/defs.rs`
  (no Bevy, so a level editor can use them); the game adds one file with
  its system and an `add_map_action` line (`src/actions/body.rs` is a
  small example); the runner and triggers do not change. Every action in
  the maps of a defined type must read, write back and read again the
  same (`tests/action_defs.rs`).
- **Map entity types stop at loading.** `map/templates.rs` turns each into
  a set of components; systems act on components (`Pickup`, `Booster`,
  bodies), never on the type. A new kind is a new template arm composed of
  existing pieces, or a new piece with the system that gives it behaviour.
- **Small animations are data**: a `Wave` and a `ColorTrack` (or a `Pulse`)
  in a template or component, played by `render/animate`; no per-entity
  blink or flash systems.
- **Draw order lives in `render/layers.rs`.** A new kind of sprite gets a
  `Layer` there; nothing picks a z height or sort order of its own.
- **C# axes differ from ours in y and z**: C# has y down and z (depth)
  away from the camera; ours has y up and z towards it. Translate points, angles and 3D
  turns through `units` (`from_cs`, `from_cs_rotation`); its doc lists
  which signs flip. Sign-symmetric formulas hide such slips: compare with
  the C# game.
- The car only reads `PlayerIntent`; new control schemes and AI write it.
- Physics and driving run in `FixedUpdate` at 60 Hz (the C# frame).
- Tests are never in the same file as the code: public behaviour is
  tested in `tests/`, private details in a separate `tests.rs` next to the
  module (see `CLAUDE.md`).

## Not ported yet

Everything outside MVP 0.1: menus and HUD, music, localisation, saves;
weapons and gadgets; other cars and AI; doors, bombs, teleports, platforms,
fake walls; towing; the other action types (AI orders, drawn splashes and text, markers, …); most particle effects (the pool and the car's smoke are in);
slow motion; the agent/command server; editor hot reload. Rendering gaps:
tyre marks,
dynamic lights, the dirty-lens effect, per-map camera zoom.
