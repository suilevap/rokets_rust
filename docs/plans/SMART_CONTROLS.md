# Smart one-finger controls: steps

The Smart scheme (`ControlScheme::Smart`, option C of the touch-control
discussion) is built in layers so that an AI player can later drive the same
commands a finger does (`src/assist/mod.rs` has the design):

1. input produces `PlayerCommand`s (`Commanded` messages);
2. the `Assist` component on the player's car turns its state and the manual
   stick into the car's `PlayerIntent`;
3. an AI player emits the same commands instead of a finger.

## Steps

1. **Settings and menu** (PR #116, done): `ControlScheme`, `auto_fire`,
   `auto_aim`, `tap_to_go`, the Options tab.
2. **Lock, aim, fire** (done): one drag is the relative stick;
   a tap on an enemy is `LockTarget`, a tap on empty ground `ClearTarget`;
   `Assist` aims and fires at the lock or, with none, at the nearest enemy in
   sight; a lock marker; the mouse's short left click is a tap on desktop
   (`--controls smart`, `?controls=smart`).
3. **Tap to grab and drop** (done): a tap on a box is `Grab(Entity)`, a
   tap on the ground while something is towed is `DropAt(Vec2)`
   (`CancelCargo` gives a job up). `assist/magnet.rs` carries them out over
   several frames through the magnet's intent (`gadget`, `gadget_at` /
   `gadget_dir`, `gadget_cancel`), with the button rules shared with the AI
   (`gadget/handling.rs`). Magnet maps are playable in Smart.
4. **Tap your own car = accelerator** (`Boost`): the pad's gadget by tap.
5. **Double-tap the ground = go there** (`GoTo(Vec2)`, `Settings::tap_to_go`):
   a path from `ai::navigation` followed as `ai/nodes/path.rs` does, writing
   `move_dir`; any drag cancels it.
6. **An AI player** that passes levels: emits the same `PlayerCommand`s from
   what it sees (goals from `markers`, orders, enemies), with tests that
   drive `Assist` by messages as `tests/assist.rs` already does.

## Choices made in step 2

- A tap on empty ground clears the lock (rather than being ignored): it is
  the only way to let go of a target by touch. Steps 3 and 5 give the ground
  more meanings (drop what is towed; double-tap goes); a clear stays the
  meaning when nothing else applies.
- A lock outlives the aim help's view and sight rules (it is dropped only
  beyond twice the gun's range, or when the target dies), so it can be driven
  towards; firing still needs range and sight.

## Choices made in step 3

- **What a tap means** (`assist::resolve_taps`, `tap::tap_pick`): the
  nearest within 1.5 units of an enemy (alive, other team, visible) and a
  `Linkable` thing that is no live enemy car (box, ally, wreck; the player's
  own car excluded). An enemy wins a tie. So a tap on a live enemy always
  locks, even where the magnet could take that car (the thumb on a car in a
  fight means the gun); a dead enemy's wreck is a thing. With something on
  the rope every non-enemy tap, on a box or on the ground, is `DropAt` the tap
  (a grab would only drag the cargo: the magnet drags what it tows instead of
  looking for a new one). With nothing on the rope, empty ground clears the
  lock and gives up a pending grab. A car without a magnet sees boxes as
  ground.
- **The job** is the AI's, as one state machine run each frame instead of a
  behaviour tree: hold the button aimed at the cargo (a unit way) or at the
  point, at least 1 s and at most 5 s; let go (which links or drops), wait
  up to 0.5 s for the magnet to show the result. Shared with the AI: the
  constants and the two let-go conditions (`gadget/handling.rs`). Differences
  from the AI: a grab lets go only when the magnet's candidate *is* the
  cargo (the AI at any candidate); a point that drifts out of reach during a
  drop does not abort it (the car stands while it drags).
- **Out of reach**: the request stays pending (a faint green ring on the
  target, or at the drop point) for 10 s while the player drives in range
  (7 units, no wall: the AI's `NEAR`); then it is dropped. The car never
  drives by itself (that is `GoTo`, step 5). A new tap replaces the job, an
  empty-ground tap with nothing towed cancels it.
- **The stick keeps driving**; the magnet's own rule that a dragged cargo
  stands the car is left as it is. A device holding the gadget button (right
  mouse, Shift, touch zone in Classic) wins: the assist writes nothing until
  it lets go, and the job ends `Done` if the player grabbed the cargo
  himself. A job on a car nobody drives (`NoInput`), a dead one or one
  without a magnet is dropped.
- **Testing aid**: `--grab-nearest` (`?grab-nearest`) grabs the nearest box a
  second into the play, as `--lock-nearest` does for the lock.
