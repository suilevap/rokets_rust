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
4. **Tap your own car = accelerator** (`Boost`, done): the pad's gadget by
   tap; see "Choices made in step 4".
5. **Double-tap the ground = go there** (`GoTo(Vec2)`, `Settings::tap_to_go`,
   done): a path from `ai::navigation` followed as `ai/nodes/path.rs` does,
   writing `move_dir`; any manual drive input cancels it. See "Choices made
   in step 5".
6. **An AI player** that passes levels: emits the same `PlayerCommand`s from
   what it sees, with tests that drive `Assist` by messages as
   `tests/assist.rs` already does. What it needs:
   - *A level-goal reader*: where the level wants the car (the finish and
     gear markers of `markers`, trigger areas, `Orders`, the objectives of
     the map's actions), as a list of world points and things; nothing of
     this exists as one query yet.
   - *A choice of commands* each think (a few times a second, like the
     enemy trees): `GoTo` the next goal point; `LockTarget` the enemy that
     blocks the way or hurts (else leave the soft target to the assist);
     `Grab` / `DropAt` where a goal is a box on a pad; `Boost` on a long
     straight; `CancelGoTo` when the goal changes. It never writes
     `move_dir` itself.
   - *Jobs it can use as they are*: `GoTo` (`assist/goto.rs`, with the
     refusal marker's `Assist::refused` as feedback: a refused point means
     "pick another"), the magnet's and the boost's jobs, auto-aim and fire.
   - *Missing for it*: a `Grab` that drives within reach by itself (see
     step 5's follow-up), a way to tell that a job has ended (today only
     `Assist` fields going `None`; a `JobEnded` message would be better),
     and a `GoTo` that follows a moving point (C#'s `FollowBy`).

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

## Choices made in step 4

- **Gadgets ported: the Magnet and the Accelerator** (`gadget::WithGadget`;
  the `Bomb` and `EditorPointer` are not ported). Both are reachable in
  Smart: the magnet by `Grab` / `DropAt`, the accelerator by `Boost`. A car
  has one gadget.
- **What a tap on the own car means** (`assist::resolve_taps`): a tap within
  `tap::own_car_reach` of the car's middle (its radius plus 0.5, at least
  1.2 units) that is *strictly* nearer than the nearest enemy or thing
  (`tap_pick`; an enemy or box as near still wins, so a fight beside the car
  keeps the gun) is: `Boost` with an accelerator; with a magnet and a grab
  or drop pending `CancelCargo` (the tap on the own car is the way to give a
  job up while something is towed, where any ground tap is a drop; what is on
  the rope stays, there is no drop "at the car"); otherwise nothing (the lock
  stays, no clear).
- **The job** (`assist/boost.rs`, `Assist::boost`): hold the gadget button
  until `Accelerator::active()` says it has worked (its own `CHARGE`, 0.25 s,
  read off the gadget, not copied), then let go; at most `CHARGE + 0.5` s,
  then let go and cancel. A pure `step` over a `Scene`, as the magnet's.
- **Not ready is refused**: a press without the whole energy bar only
  fizzles (a quarter of the boost, no blast, a ping), which a tap would
  never mean, so `apply_commands` ignores `Boost` unless
  `Accelerator::ready()` (nothing is shown). A device holding the gadget
  button wins (the job ends, nothing is repeated). The car must be alive and
  driven.
- **Feedback**: the green job ring closes on the own car while the button
  is held (`assist/view.rs`). The gadget's own glow and ring show the rest.
- **Testing aid**: `--boost-now` (`?boost-now`) boosts a second into the
  play (Race2, Revenge, WinnersRoom have accelerator cars).

## Choices made in step 5

- **Double tap** (`tap::is_double_tap`, `resolve_taps`): two taps within
  0.4 s (release to release; a tap itself may last 0.25 s, so the 0.35 s of
  the first sketch left no gap) and 1 world unit of each other. World units,
  not pixels: the camera zoom decides what a unit is on the screen. A third
  tap starts afresh. Taps are timed in real time (`Time<Real>`), as the
  devices are.
- **The first tap acts at once** (a single tap is never delayed to wait for
  a second: cheap and responsive). A double tap is a *plain-ground* tap
  twice: nothing under the finger, not the own car, nothing on the rope.
  Then the first tap was `ClearTarget` (and gave up a pending grab), the
  second adds `GoTo`; both harmless. On an enemy, a box or the own car a
  double tap is that tap twice (a lock, a grab, a boost refused for want of
  energy). **With something towed it is a drop twice, never a drive**: a
  drop is pending on the magnet's job and the magnet stands the car while it
  drags. `Settings::tap_to_go` off: no `GoTo` from taps (the command itself
  stays for an AI).
- **The job** (`assist/goto.rs`, `Assist::goto`): a pure `step` over a
  `Scene`, the planner passed in. Plan with `plan_path` and steer with
  `follow_stick` and `arrived`, which were extracted from the AI's
  `go_to_position`, `MoveBase.tick` and `path_complete` (the AI uses them
  too). Arrived is within the car's radius of the point. Re-plan every 3 s;
  not moved 0.5 in 2 s re-plans, twice running gives up; 30 s time-out.
- **Refusals**: no path, or a path a closed door blocks, is refused with a
  red ring (0.8 s, `Assist::refused`) and the car does not move. With no
  navigation graph or no way in it the car drives straight when no wall is in
  the line (`SharedTerrain`); otherwise it is refused.
- **Manual input wins**: any nonzero `move_dir` when the assist runs (the
  devices write it afresh each frame) ends the job in that frame and the
  stick is left as the hand wrote it. A tap on the own car that does nothing
  else (no accelerator, no pending job) is `CancelGoTo`. A single tap
  elsewhere does not cancel the drive (it clears the lock, as ever); a new
  `GoTo` replaces it. A dead car or one nobody drives drops it.
- **Independent of the rest**: auto-aim and fire, the magnet's and the
  accelerator's jobs run as before; the drive only writes `move_dir`.
- **Visual**: a blue ring at the destination (`assist/view.rs`). The path's
  dots are not drawn (follow-up if wanted).
- **Follow-up, not built**: `Grab` on an out-of-reach box could drive within
  reach first (`GoTo` to the box with a 7-unit arrival). It needs a second
  arrival radius on the job and a rule for when the grab waits; left for
  step 6, where the AI needs it too. A double tap on a box is a grab twice
  until then.
- **C# ancestor**: `PlayerCarControlPath` (never enabled): while the mouse
  button or a touch was *held*, `FollowToPosition` made the car follow the
  cursor along navigation paths (new path every second, extended when the
  point moved a little). Here it is one point, one command; the held-finger
  following is the stick's job again.
- **Testing aid**: `--goto X,Y` (`?goto=X,Y`) sends `GoTo` to that map cell a
  second into the play.
