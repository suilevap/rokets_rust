# Architecture

One crate, a library (`src/lib.rs`, `TileRiderPlugin`) and a binary
(`src/main.rs`). Each system is a plugin in its own module; plugins meet in
components, resources and messages, not in each other's code.

```text
content  units            (no Bevy app; plain data and parsing)
   ↑       ↑
map::grid  light::bake  car::drive      (pure algorithms, unit-tested)
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
| `units` | World units = the C# "sim units" (32 px), so forces and speeds keep their values. Map y points down, world y up: convert only here | `ConvertUnits` |
| `assets` | Resolves C# content names case-insensitively through `assets/manifest.txt`; loads XML as text assets (works on the web) | content pipeline |
| `map/grid` | The map as cells from the image (alpha: floor below `ImageWallTreshold`, wall above, height in 32 px steps) and `CharMap`; `IsFloor`; distance to walls; corner wedges; walls merged into rectangles | `MapManager`, `TilesMap.GetH`, `MapInfo.BuildTextureFromCharMap`, `MapManager.GetEdgeIndex` |
| `map/colliders` | Static wall colliders | `PhysicsManager.CreateWalls` |
| `map/templates` | **The only place that reads a map entity's type**: turns each map entity into a template of properties (sprite and layer, body kind and density, axis locks, boost, pickup, spin). Plain data, unit-tested | `EntitiesFactory`, entity classes |
| `map/spawn` | Builds entities from templates: sprite quads, Avian bodies (box, friction 0.8, bounce 0.5, C# damping) and behaviour components | `Entity`, `PhysicEntity` |
| `light/bake` | Static light map baked on the CPU: lamps with the `flash2` falloff, wall shadows, two-copy penumbra, ambient, saturation, blur. Walls sample it as is; the light layer (and sprites' tints) add full light over the void around the map. Kept on the CPU for gameplay and shadows | `LightManager` |
| `game` | States `Loading → Playing` (`Failed`), `LoadedMap`, `MapEntity` (despawned on leaving a map), `ChangeMap` (restart / next) | `MapManager.SetMap` |
| `render/map_mesh` | Floor of loose, tilted tiles, always below the ground plane; walls as blocks with fading faces and their light in the vertex colours; tiles are lit by the light layer | `TilesMap.AddTiles2`, `Primitive3d`, `RenderSettings` "Mul_Add05" |
| `render/outland` | Four vertex-coloured quads, a funnel under and around the map that shows between the tiles and past the walls: background colour at the inner edge 32 units down, the map's four `OutlandColor*` at the outer edge 32 units up | `OutlandPrimitive` |
| `render/light_layer` | The light map as one quad at ground level with a multiply blend, over the map and 4 cells around it: darkens the floor and the outland under it on screen; walls stand above it, sprites are drawn after it | `LightManager` light target, `LightBlendState`, `MapManager.AddOutSideLight` |
| `render/layers` | **The draw order**: floor geometry under `FLOOR_TOP`, then sprite layers (ground decor, car chassis, wheels, cabin, gears), ordered by a `depth_bias` rank, as the C# `DepthConst` table and its `BackToFront` sprite sort; all lie within a hundredth of a unit of the ground, so order never changes a sprite's size | `DepthConst`, `Renderer.Draw` |
| `render/sprite` | Sprites as unlit quads tinted by the light map, placed by their layer | `SpriteView` + light layer |
| `render/camera` | Perspective camera from above, trailing its target; in portrait the view turns a quarter turn and the field of view keeps the landscape amount of world on the short side (`ViewRotation`) | `Camera2D` |
| `controls` | `PlayerIntent` (where to go, aim, fire) written by keyboard, gamepad and touch schemes; the last one used is active; screen directions are turned into the world by `ViewRotation` | `IControl`, `CompositePlayerControl` |
| `car/drive` | The driving model as a pure function, run at 60 Hz | `Car.MovementControl`, `Brake`, `AdditionalFriction` |
| `car` | Spawns the player's car on Avian; sprites; front wheels | `Car`, `ControlableEntity` |
| `triggers` | Trigger areas; only level-end actions run so far | `TriggerManager`, `ActionTemplates` |
| `pickups` | `Pickup`: collected by driving into it; the count | `Coin` |
| `boost` | `Booster`: throws touching bodies along its facing and plays its flash | `Accelerator` |
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
- **Map entity types stop at loading.** `map/templates.rs` turns each into
  a set of components; systems act on components (`Pickup`, `Booster`,
  bodies), never on the type. A new kind is a new template arm composed of
  existing pieces, or a new piece with the system that gives it behaviour.
- **Small animations are data**: a `Wave` and a `ColorTrack` (or a `Pulse`)
  in a template or component, played by `render/animate`; no per-entity
  blink or flash systems.
- **Draw order lives in `render/layers.rs`.** A new kind of sprite gets a
  `Layer` there; nothing picks a z height or sort order of its own.
- The car only reads `PlayerIntent`; new control schemes and AI write it.
- Physics and driving run in `FixedUpdate` at 60 Hz (the C# frame).
- Tests are never in the same file as the code: public behaviour is
  tested in `tests/`, private details in a separate `tests.rs` next to the
  module (see `CLAUDE.md`).

## Not ported yet

Everything outside MVP 0.1: menus and HUD, sound, localisation, saves;
weapons and gadgets; other cars and AI; doors, bombs, teleports, platforms,
fake walls; towing, box damage and explosions; the other ~74 action types; particles;
slow motion; the agent/command server; editor hot reload. Rendering gaps:
car shadows (`DynamicShadow`: sprite streaks from the nearest lamp) and
the black outline along wall tops, both deferred to their own PR; the cabin's springy wobble, tyre marks,
dynamic lights, HDR glow and the dirty-lens effect, per-map camera zoom.
