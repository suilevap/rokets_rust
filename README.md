# TileRider (Rust / Bevy)

A port of [TileRider](https://github.com/suilevap/rokets), a 2.5D vehicle
action game written in C# / MonoGame, to Rust and [Bevy](https://bevy.org)
0.20, with [Avian](https://github.com/Jondolf/avian) 2D physics.

The port follows the plan in the C# repository,
`docs/plans/RUST_BEVY_MIGRATION.md`: it reproduces **what the game meant
to achieve**, using the C# game as the reference to compare against, not
its implementation. This is **MVP 0.1** (section 9 of the plan): load a
map, drive the player's car, see the walls and the baked light, pick up
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
| `--record DIR` | Save every frame (time stepped 1/30 s per frame) for `--frames N` frames, then exit; make a video with `ffmpeg -framerate 30 -i DIR/frame_%05d.png out.mp4` |
| `--size WxH` | Window size (default 1280x720; `720x1280` for portrait) |
| `--start x,y` | Put the car at that map cell instead of the map's start (first map only) |
| `--drive SCRIPT` | Drive by script, `x,y:seconds;...` (stick in world directions, y up), logging the car's path every 0.1 s |

Portrait screens (a phone held upright) show the landscape view turned a
quarter turn: up on the screen is right on the map, and the controls follow
the screen. The short side of the screen always shows as much of the map as
the C# camera did; the long side shows more.

Keys: arrows / WASD drive (point where you want to go), `R` restarts,
`N` skips to the next map. A gamepad's left stick or d-pad and touch (a
floating stick where your finger lands) work too; the last one used wins.

On Linux, Bevy needs `pkg-config libwayland-dev libxkbcommon-dev libudev-dev
libasound2-dev libx11-dev`.

## Web build

The game runs in the browser (WebGL2). `.github/workflows/pages.yml`, adapted
from gridvail, builds it to WebAssembly and publishes it to GitHub Pages from
the `gh-pages` branch:

- `main` at `https://suilevap.github.io/rokets_rust/`;
- each pull request from this repository at
  `https://suilevap.github.io/rokets_rust/pr/<number>/`, linked in a comment,
  rebuilt on every push and removed when the pull request closes. Previews
  build the branch's own commit. Changes to `**.md`, `docs/`, `tests/`,
  `tools/` or `.claude/` build nothing; the `no-preview` label skips a build.

The page's bottom-right corner names the build it serves (`PR #<n> · <sha>`
for a preview, "dev" locally). URL parameters are the options without `--`:
`map`, `start` and `drive`, for example
`?map=LetItSnowDay` or `?map=Middleware&start=84,9.5` (next to the finish).

Pages has to be switched on once: Settings → Pages → Deploy from a branch →
`gh-pages`, `/ (root)`. For a private repository that needs a paid GitHub
plan, and the published site (with the game's assets) is public.

To build locally:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked  # match Cargo.lock
cargo build --profile web --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --target web --out-dir dist --out-name tilerider \
  target/wasm32-unknown-unknown/web/tilerider.wasm
cp web/index.html dist/ && cp -r assets dist/assets
python3 -m http.server -d dist 8000
```

## MVP maps

`EditorTest`, `CharMapTest` (walls drawn as text), `LightTest`,
`LetItSnowDay` (gears and a finish), `Middleware` (the first campaign map,
with accelerator pads), `Unboxing` (boxes). Enemies, doors and bombs do not
appear yet.

## Compared with the C# game

`docs/media/` holds recorded clips (driving through Middleware, picking up
a gear, crossing the finish to the next map). `docs/comparison/` holds side-by-side screenshots: the C# game on the left
(taken through its debug command server, `GET /screenshot`), this port on
the right. Not yet in the port: car shadows and wall outlines (next PR),
the glow and dirty-lens effects, accelerator arrows and the other entities.

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
The baked light is checked against light targets saved from the C# game
(`tests/data/light_cs`); `cargo run --example light_dump -- <map> <dir>`
writes ours as PNGs to look at.

See `ARCHITECTURE.md` for how the code is laid out and what is not ported
yet.
