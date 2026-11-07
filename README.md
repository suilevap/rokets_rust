# TileRider (Rust / Bevy)

A port of [TileRider](https://github.com/suilevap/rokets), a 2.5D vehicle
action game written in C# / MonoGame, to Rust and [Bevy](https://bevy.org)
0.20, with [Avian](https://github.com/Jondolf/avian) 2D physics.

The port follows the plan in the C# repository,
`docs/plans/RUST_BEVY_MIGRATION.md`: it reproduces **what the game meant
to achieve**, using the C# game as the reference to compare against, not
its implementation. This is **MVP 0.1** (section 9 of the plan): load a
map, drive the player's car, see the walls, light and shadows, pick up
gears, reach the finish.

## Run

```bash
cargo run --release -- -m EditorTest
cargo run --release -- -m LetItSnowDay
```

| Option | Meaning |
|---|---|
| `-m MAP` | Map name from `assets/Maps/Singleplayer` (default `EditorTest`) |
| `--screenshot PATH` | Save a PNG after `--frames N` frames of play (default 90), then exit |
| `--drive SCRIPT` | Drive by script, `x,y:seconds;...` (stick in world directions, y up), logging the car's path every 0.1 s |

Keys: arrows / WASD drive (point where you want to go), `R` restarts,
`N` skips to the next map. A gamepad's left stick or d-pad and touch (a
floating stick where your finger lands) work too; the last one used wins.

On Linux, Bevy needs `pkg-config libwayland-dev libxkbcommon-dev libudev-dev
libasound2-dev libx11-dev`.

## MVP maps

`EditorTest`, `CharMapTest` (walls drawn as text), `LightTest`,
`LetItSnowDay` (gears and a finish), `Middleware` (the first campaign map;
its enemies and boxes do not appear yet).

## Compared with the C# game

`docs/comparison/` holds side-by-side screenshots: the C# game on the left
(taken through its debug command server, `GET /screenshot`), this port on
the right. Not yet in the port: the glow and dirty-lens effects, accelerator
arrows and the other entities.

Headless screenshots on Linux without a GPU:

```bash
BEVY_ASSET_ROOT=$PWD VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  xvfb-run -a -s "-screen 0 800x480x24" \
  cargo run -- -m Middleware --screenshot shot.png --frames 40
```

(needs `mesa-vulkan-drivers`, `xvfb` and `libxkbcommon-x11-0`).

## Content

`assets/` holds a copy of the C# game's maps, car catalogue and sprites, in
the same layout, so map XML references (`Entities\CargoQuest`) resolve the
same way. Refresh it after editing maps in the C# level editor:

```bash
tools/import_content.py ../rokets
```

## Tests

```bash
cargo test
```

Every map in `assets/` must parse; the driving model, light baking, map
grid and trigger resolution have unit tests that run without a window.

See `ARCHITECTURE.md` for how the code is laid out and what is not ported
yet.
