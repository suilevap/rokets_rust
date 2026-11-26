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
| `-m MAP` | Map name from `assets/Maps/Singleplayer`; the game starts in it. Without `-m` (and without `--screenshot`, `--record`, `--drive` or `--start`, which run the default map `EditorTest`) the game opens in the map menu |
| `--menu` | Open in the map menu even with a map named; with `--screenshot` the menu is what is saved |
| `--tab TAB` | The menu's first tab: `campaign`, `all` or `options` (`?menu=1&tab=options` in the browser) |
| `--screenshot PATH` | Save a PNG after `--frames N` frames of play (default 90), then exit |
| `--record DIR` | Save every frame (time stepped 1/30 s per frame) for `--frames N` frames, then exit; make a video with `ffmpeg -framerate 30 -i DIR/frame_%05d.png out.mp4` |
| `--size WxH` | Window size (default 1280x720; `720x1280` for portrait) |
| `--start x,y` | Put the car at that map cell instead of the map's start (first map only) |
| `--car NAME` | Give the player that car of `Cars.xml` (on every map), e.g. `Truck` to try the rocket launcher no map gives the player |
| `--glow X` | How strongly the additive (HDR) light of lamps with an `AdditiveRate` is added (default 3; `[` and `]` turn it by 0.25 while playing, shown as `glow` in the top line) |
| `--flash X` | How much brighter additive effects (flashes, sparks, bullet trails, the aim line) are drawn (default 1.5; `;` and `'` turn it by 0.25 while playing, shown as `flash` in the top line) |
| `--lang LANG` | The maps' texts in another language: `ru`, `de`, `es`, `it`, `be` (default English; `en` forces English over a saved language). Wins over the saved setting for this run |
| `--drive SCRIPT` | Drive by script, `x,y:seconds;...` (stick in world directions, y up; `x,y,g:seconds` also holds the gadget, aimed ahead, `x,y,f:seconds` the trigger, the gun firing where it points), logging the car's path every 0.1 s |
| `--run ACTION` | With `--drive`: run that map action on the script's first step (repeat it for several), as the C# game's debug server does with `/run?cmd=ACTION`, e.g. an AI's `MoveCargoOrder` |
| `--trace a,b` | With `--drive`: also log these entities (map ids) every 0.1 s, as `trace t id x y angle speed` in the C# game's axes, for `tools/compare.py`, and their health as `health t id hp` |
| `--nav` | Draw the navigation graph over the map (links in green, crossings in orange; red links are dear ones), as `NavGraph.Draw` did in the C# game's AI debug build |
| `--controls classic\|smart` | The touch scheme for this run (not saved; `?controls=smart` in the browser); with Smart, `--lock-nearest` locks the nearest enemy a second into the play and `--grab-nearest` grabs the nearest box with the magnet, `--boost-now` uses the accelerator, `--goto X,Y` drives to that map cell (for screenshots; `?grab-nearest`, `?boost-now`, `?goto=X,Y` in the browser) |
| `--no-music` | Start with the level music off (`M` switches it while playing). Wins over the saved setting for this run. The maps' `PlayLevelMusic` actions play the songs of `assets/Sounds/Music` |

**The map menu** has three tabs: *Campaign* (the 36 maps of `MapList.xml`, in
order) and *All maps* (every file of `assets/Maps/Singleplayer`, by name,
listed in `assets/Maps/AllMaps.txt`, which `tools/import_content.py` writes
because the browser cannot list a folder). Click or tap a map to play it; on
the keyboard use the arrows (and page up / down, home / end) to choose,
left / right or `Tab` to change the tab, `Enter` to play and `Esc` to go back;
on a gamepad the d-pad, the shoulder buttons (tabs), A and B. The lists
scroll with the wheel or by dragging. The third tab, *Options*, holds the
settings (below); there Enter, a click or a tap, or left / right on the
keyboard and the d-pad, change the chosen row. Opened from a map, `Esc` / "Back"
starts that map again from its beginning. In the browser, `?menu=1` is the
menu and `?map=NAME` skips it.

**Settings** (the *Options* tab) are kept between runs: native in
`$XDG_CONFIG_HOME/tilerider/settings.json` (else `%APPDATA%\tilerider\` on
Windows, `~/Library/Application Support/tilerider/` on macOS,
`~/.config/tilerider/`), in the browser in `localStorage` (`tilerider.settings`,
per browser and site, so the main page and every pull request preview, which share one address, share it too). They are
written when one changes; a missing or corrupt file means the defaults. The
rows: *Controls* (Classic, the zones described below, or Smart, one-finger
controls, see below), the smart options *Auto-fire* and *Auto-aim* (dimmed
while Classic) and *Tap to go* (not built yet), *Music* (the same as `M`) and *Language* (the maps' texts; read
at start, so it shows from the next run). `--no-music`, `--lang` (`no-music`,
`lang` in the browser) win for the run they are given in and are not saved.

**Smart controls** (the *Controls* row, or `--controls smart` /
`?controls=smart` for one run) play with one finger and no zones: drag
anywhere to drive (the stick is where the finger landed), tap an enemy to
lock it (a red ring shows), tap empty ground to let it go. The car turns its
gun on the locked enemy (*Auto-aim*) and fires while the gun points at it, in
range and in sight (*Auto-fire*); with nothing locked it takes the nearest
enemy in sight (a faint ring). With a magnet, tap a box (or an ally, or a
wreck) to grab it: the car holds the gadget button, aims it and lets go as the
enemy cars' magnets do, and a green ring shows the target (fainter while it
waits for you to drive within about 7 units in sight; it gives up after 10 s).
While a box is on the rope, tap the ground to drop it there (a green ring marks
the point). A tap on an enemy always locks, even on a car the magnet could
take; the stick keeps driving all the while. A double tap on bare ground (nothing on the rope) drives the car there along
the navigation path (a blue ring marks the point; any drive input of yours
cancels it; a point with no way is refused with a red ring; the Options tab's
"Tap to go" turns it off; `docs/plans/SMART_CONTROLS.md`). To try it on a desktop: a short left click is a tap
(the mouse neither aims nor fires in Smart); `--lock-nearest` locks the
nearest enemy a second into the play, `--grab-nearest` grabs the nearest box, `--boost-now`
boosts, `--goto X,Y` drives to a map cell (for screenshots). Maps with enemies:
`cargo run --release -- -m TheLastStand --controls smart`.

Portrait screens (a phone held upright) show the landscape view turned a
quarter turn: up on the screen is right on the map, and the controls follow
the screen. The short side of the screen always shows as much of the map as
the C# camera did; the long side shows more.

The camera looks ahead of the car and smoothly zooms out as it gains speed,
then returns closer when it slows down. Multiple camera targets share the frame.

Keys: WASD (and the arrows, on a car without a gun) drive (point where
you want to go), `R` restarts,
`N` skips to the next map, `Esc` (or the gamepad's Start, or the "Menu" button at the top left of a touch screen) opens the map menu. A gamepad's left stick or d-pad and touch (a
floating stick where your finger lands) work too; the last one used wins.
Cars with a magnet: hold the right mouse button (or Shift + left) to shoot
it at the cursor and let go over a box to tow it; hold it again to drag
what you tow, let go to drop it. Right Shift / `E` / the gamepad's right
shoulder aim it ahead of the car (or where the arrows point, on a car
with a gun). On touch, hold a finger on the circle at
the top right and drag from there to aim (the scheme is the landscape one
for now).
Cars with the accelerator (Race2's `ScoutAccelerator`; Derby, Friend and
Trophy hand it over): hold the same gadget button a quarter of a second
(right mouse, right Shift or `E`, the gamepad's right shoulder, a finger on the
top right circle) for a burst of speed along the car and a blast around it,
which spends the whole energy bar; the bar's refilled in about three
seconds, a ring tells so, and a flash glows at the tail while it is ready.
Pressed before that, it only gives a quarter of the push.
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
the Railgun on its first step. The Shotgun (enemies', in `Swarm` and
`UnderPressure`) fires the same way, five pellets in a spread a 2 s reload,
and shoves its car back.
The Hammer (a saw on the nose, drawn under the car) works by itself: drive
into things with it. It hurts bombs, red boxes and cars by the car's speed
(4 to 8) and shoves what it hits; it has no aim line, no fire zone and the
mouse and the gamepad do not fire it (the fire keys only spend its energy).

The HUD at the top shows the car's health (a cell for each 4 points; lost ones fade out red, a shimmer runs along) and its shield (thin cells), with the gadget's and the gun's icons to the left of them (not while the last device is touch), and when a life is spent at a respawn point a battery with the lives left appears in the middle of the screen. The score is not shown yet.

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
the right. Not yet in the port: the other entities.

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
