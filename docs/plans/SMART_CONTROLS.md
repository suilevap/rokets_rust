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
2. **Lock, aim, fire** (this change, done): one drag is the relative stick;
   a tap on an enemy is `LockTarget`, a tap on empty ground `ClearTarget`;
   `Assist` aims and fires at the lock or, with none, at the nearest enemy in
   sight; a lock marker; the mouse's short left click is a tap on desktop
   (`--controls smart`, `?controls=smart`).
3. **Tap to grab and drop**: a tap on a box is `Grab(Entity)`, a tap on the
   ground while something is towed is `DropAt(Vec2)`. Reuse the logic of
   `ai/nodes/cargo.rs` (`Grab`, `DropByChainGadget`) through the magnet's
   intent (`gadget`, `gadget_at`, `gadget_cancel`), so magnet maps become
   playable in Smart (until then they are not: Smart has no gadget zone).
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
