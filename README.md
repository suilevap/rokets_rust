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
| `--car NAME` | Give the player that car of `Cars.xml` (on every map), e.g. `Truck` to try the rocket launcher no map gives the player |
| `--glow X` | How strongly the additive (HDR) light of lamps with an `AdditiveRate` is added (default 3; `[` and `]` turn it by 0.25 while playing, shown as `glow` in the top line) |
| `--flash X` | How much brighter additive effects (flashes, sparks, bullet trails, the aim line) are drawn (default 1.5; `;` and `'` turn it by 0.25 while playing, shown as `flash` in the top line) |
| `--lang LANG` | The maps' texts in another language: `ru`, `de`, `es`, `it`, `be` (default English) |
| `--drive SCRIPT` | Drive by script, `x,y:seconds;...` (stick in world directions, y up; `x,y,g:seconds` also holds the gadget, aimed ahead, `x,y,f:seconds` the trigger, the gun firing where it points), logging the car's path every 0.1 s |
| `--trace a,b` | With `--drive`: also log these entities (map ids) every 0.1 s, as `trace t id x y angle speed` in the C# game's axes, for `tools/compare.py`, and their health as `health t id hp` |
| `--nav` | Draw the navigation graph over the map (links in green, crossings in orange; red links are dear ones), as `NavGraph.Draw` did in the C# game's AI debug build |

Portrait screens (a phone held upright) show the landscape view turned a
quarter turn: up on the screen is right on the map, and the controls follow
the screen. The short side of the screen always shows as much of the map as
the C# camera did; the long side shows more.

The camera looks ahead of the car and smoothly zooms out as it gains speed,
then returns closer when it slows down. Multiple camera targets share the frame.

Keys: WASD (and the arrows, on a car without a gun) drive (point where
you want to go), `R` restarts,
`N` skips to the next map. A gamepad's left stick or d-pad and touch (a
floating stick where your finger lands) work too; the last one used wins.
Cars with a magnet: hold the right mouse button (or Shift + left) to shoot
it at the cursor and let go over a box to tow it; hold it again to drag
what you tow, let go to drop it. Right Shift / `E` / the gamepad's right
shoulder aim it ahead of the car (or where the arrows point, on a car
with a gun). On touch, hold a finger on the circle at
the top right and drag from there to aim (the scheme is the landscape one
for now).
Cars with a gun (the Minigun and the Railgun so far): hold the left mouse button to fire
at the cursor, or the arrows to aim and fire that way, or right Ctrl to
fire where it points; the gamepad's right trigger fires and its right
stick aims; on touch, hold a finger on the lower right (the whole right
half without a gadget) and drag to aim. A green line shows the aim; the
bar at the top is the energy shots draw on: empty, the gun stops until
it is half full again. The Railgun fires one beam a 1.5 s reload, a
shot that costs 0.6 of the energy and bounces once off walls: it fires
when the left mouse button or the gamepad trigger is held (once reloaded),
on a press of right Ctrl, on letting go of the arrows and on lifting the
touch finger; its aim line is white and wide while it reloads and shows
the bounce. In a drive script (`--drive`) `f` holds the trigger and fires
the Railgun on its first step.
The Hammer (a saw on the nose, drawn under the car) works by itself: drive
into things with it. It hurts bombs, red boxes and cars by the car's speed
(4 to 8) and shoves what it hits; it has no aim line, no fire zone and the
mouse and the gamepad do not fire it (the fire keys only spend its energy).

The map's `SimpleCars` (targets, turrets, enemy buggies) are there: you can
shoot them, ram them and saw them. A hurt one shows a health bar over it for
two seconds, and when it is destroyed it bursts, lies smoking as a wreck for
two seconds and then goes (running its `ActionOnDestroy`). Your own team's
(`NoneFriend`, the `*Friend` controls) are spared by your guns and saw. Their
Hammers spin (the saw works for every control but `NoneLocal`, `NoneEnemy` and
`NoneFriend`) and hurt what touches them.

Turrets (`AiSimpleTurret`) and the enemies that shoot without looking
(`AlwaysShoot`, `ShootPattern`) act now. A turret looks for you within 16
units, in sight, turns its gun on you (it takes a moment to turn; a gun
that cannot turn much, because walls are near it, only fires at what is in
front) and fires only when the gun points your way: a Minigun in bursts of
ten, 1.4 s apart, a Railgun one beam a 1.5 s reload. Their guns need no
energy. It keeps on you until it loses sight; whoever shoots it is
shot back at, from any distance, for five seconds; when it sees nobody it
stands a second and then sweeps the gun about. One that can move (a buggy
with a saw) chases you. A car with a gun can now kill you: you are wrecked
and the level starts again (or you come back at the respawn point), as
before. `AlwaysShoot` fires all the time in the direction the car faces;
`ShootPattern` fires in a random pattern of short bursts. The other enemies
(chasers, path followers, flocks, `Ai`: the behaviour-tree ones) still stand
idle. Enemies do not show a red aim line yet.

Aim help: with a gun, the aim leans towards the enemy you point near (on
screen, in sight, within the gun's range), three quarters of the way, so
shots land more easily; it picks the enemy again every half second.

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
`map`, `start`, `car`, `drive`, `glow`, `flash` and `lang`, for example
`?map=LetItSnowDay`, `?map=Middleware&start=84,9.5` (next to the finish) or
`?map=UnderPressure&start=74,56.5&glow=4` (an additive lamp, brighter).

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
the right. Not yet in the port: the dirty-lens effect and the other
entities.

Headless screenshots on Linux without a GPU:

```bash
BEVY_ASSET_ROOT=$PWD VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  xvfb-run -a -s "-screen 0 800x480x24" \
  cargo run -- -m Middleware --screenshot shot.png --frames 40
```

(needs `mesa-vulkan-drivers`, `xvfb` and `libxkbcommon-x11-0`).

Driving compared with the C# game, step by step: `tools/compare.py` drives
both games by the same `--drive` script, records the car and the listed
entities every 0.1 s of game time (the C# game through its debug server's
`/drive` and `/trace`) and prints them side by side with the gap:

```bash
tools/compare.py AiTest --start 3.5,14.5 --ids 4001,1007 \
    --drive "0,0,g:1;0,0:0.3;0,-1:2.5;0,0:2"   # tow a box with the magnet
```

It needs the C# game built in Debug next to this checkout (`../rokets`,
or `--rokets PATH`); headless it starts Xvfb and PulseAudio itself, and
the C# game runs about twenty times slower than real time, so a few
seconds of driving take a few minutes. Use a map without a scripted
intro (AiTest, not Middleware): the C# game starts the script some
seconds after the map does.

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
To compare the baked light with the C# game by eye,
`cargo run --example light_dump -- <map> <dir>` writes ours as PNGs.

See `ARCHITECTURE.md` for how the code is laid out and what is not ported
yet.
