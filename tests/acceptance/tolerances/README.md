# Comparing the game with the original

The files here say how closely the game must follow reference traces of
the same route, in the same trace format, recorded from the original game.
The reference traces are never stored in this repository: they live in the
folder the `CLAW_REFERENCE` environment variable names, one folder per
route (`level<N>/<name>/` with `route.csv`, `trace-<clock>-r<k>.csv` and
`meta.toml`).

```text
make compare REF=level1/opening              # both clocks
make compare REF=level1/opening CLOCK=c18    # one clock
make compare REF=level1/opening STRICT=1     # fail on any divergence
CLAW_INSTALL=... CLAW_REFERENCE=... make test-install MODULE=acceptance
```

`make compare` replays the route in the game with the tuning and the random
state at the route's time zero that the store names, keeps going with no
button held while the reference runs on past the route's end, and prints,
for every reference trace, the first divergence with the rows leading up to
it and a per-column summary. It fails on a divergence that
`references.toml` does not list for that route and clock, or that comes
earlier than listed, and when the game's trace ends before the reference
trace; a route and clock with no entry there fails on any divergence.
`STRICT=1` fails on any divergence, listed or not, and when the game's
trace ends before the reference trace. The install test `install_reference`
holds every route in `references.toml` to the same rule, playing it tick by
tick; `install_differential`, the gate, plays every route through the
game's own frame loop at 60 Hz, compares `c18` runs with `exact.toml` and
`c60` runs with `feel.toml`, and fails with the first divergence the file
does not list. Both drivers give the same trace, which that test checks
too.

## The two clocks

- **c18, lockstep.** The original runs one 18 ms logic step per frame, the
  same step the game uses, and the game plays with the original speeds
  (`--tuning original`) and the shared random stream started from the
  original's state at time zero. Every row is compared tick for tick with
  `exact.toml`: equal positions, states, stats, events and random state at
  the same time. This finds logic and random-order differences.
- **c60, feel.** The original runs at 60 frames per second (frames of 17,
  17 and 16 ms) and the game plays with its default tuning, which is tuned
  for that feel. `feel.toml` allows small differences in position and
  timing, and leaves out the random stream, which the two runs step a
  different number of times.

## Files

- `exact.toml`: the lockstep preset. Nothing may differ.
- `feel.toml`: the feel preset. Claw's position and the camera point within
  8 px, after allowing two ticks (36 ms) of timing either way; state,
  physics type and held buttons, and health, score, lives and ammunition,
  within two ticks (36 ms); game events within three ticks (54 ms).
- `references.toml`: the routes the install test compares, each with the
  clock, the tolerance file and the time at which each column is already
  known to leave its tolerance.

## What the first level's opening shows

The opening route stands, makes a tapped and a full jump, walks off the
start step, walks right, climbs the ladder, collects treasure, makes
running jumps and meets the officer who blocks the way. The original ran it
twice at each clock with identical traces.

### c18: tick for tick

- Every column is equal on every tick of the run, the score and the
  random stream included: standing, both jumps, walking off the step,
  walking, the climb, the treasure, the running jumps and the officer.
  With the random draws in the original's schedule the officer's strike
  that hit Claw at 16776 ms only in the game is gone, and with the
  goodies' spin and the kegs' depth (see "What is fixed and what remains")
  the officer falls and its coins are taken in the original's ticks.
- With the level's objects held inert, Claw's horizontal position is equal
  for the whole 32.5 s run and his height until the original's officer
  first hits him at 25938 ms. Claw's own movement and the camera therefore
  follow the original exactly at this clock; no movement fix is needed.
- The random stream is equal from time zero to the end of the run.
- Sound choices that the original draws from the shared stream (Claw's hit
  sound and pain line, the officer's line) already come from the shared
  stream in the game. The sound variant of an animation frame comes from a
  separate stream in both. The chance of Claw's line when he fires a shot
  still comes from the separate stream in the game; the opening fires no
  shot, so it is listed as a known difference to check.

### c60: the drift profile

Positions are the game minus the original at the same time.

| Time | What Claw does | Difference |
| --- | --- | --- |
| 0 to 3.4 s | stands, tapped and full jumps | equal on the ground; up to 15 px in the air (the full jump peaks about one tick apart), same heights (45 and 134 px) |
| 3.4 to 9.2 s | walks off the step, walks right to the wall | -2 to +10 px; both stop at the same wall |
| 9.2 to 10.6 s | walks left to the ladder | within 2 px |
| 10.6 to 12.9 s | climbs the ladder | within the feel tolerance: both climb at 220 px/s |
| 12.9 to 16.3 s | stands, jumps for treasure, running jump | equal standing, within 7 px in the air; score equal |
| from 16.4 s | the officer | the original's officer hits Claw at 16384 ms; the game's strikes a few ticks later and then falls to Claw, while the original's goes on striking |

Within the feel tolerance the opening holds, climb included, until the
officer (16384 ms).

## Tolerance values

- `exact.toml` is exact by definition.
- `feel.toml`: the smallest values that hold Claw's own movement on the
  first level's routes at 60 frames per second (leaving out the enemies
  and a partly held running jump). A fall from standing still runs up to
  about one tick ahead of the original in the middle of the fall at this
  frame rate (both land within 4 ms of each other), so the position needs
  two ticks of timing slack to stay within 8 px; with it, the opening
  holds until the officer (16384 ms) and the route written down from a
  fast run holds until its partly held running jump (6050 ms). State
  changes within two ticks (one tick is not enough: a landing comes two
  ticks apart once); score changes within two ticks. The event slack is a
  starting value.
- No difference is absorbed by widening a tolerance. The climb was a
  tuning difference, not a tolerance one: the default tuning now climbs
  at the original's 220 px/s. `exact.toml` and `feel.toml` are never
  widened to make a route pass; a known difference goes into
  `references.toml` with its reason instead.

## Full-level routes and segments

At c18 Claw's own movement follows the original exactly, so a route played
open loop over a whole level stays on course as long as the enemies and the
random stream do. One difference in an enemy (here the officer's strike)
changes everything after it, and at c60 enemies decide on 16 and 17 ms
frames that the game's 18 ms ticks cannot match. Until the enemies follow
the original tick for tick, a full-level route shows only its first
difference, so later sections of a level go unchecked.

Proposed segments: cut a level's route at anchor ticks where Claw stands
still on the ground with no enemy awake nearby (the search bot knows these
points). Each segment is played in both games from its anchor: Claw is
placed at the anchor point, waits until he stands, the shared random stream
is set to a fixed value at the segment's time zero, and the segment's rows
are played and compared from there. A difference then spoils only its own
segment. A store folder per segment names its start point in `meta.toml`
(`teleport = "X,Y"`); `make compare` and the install tests then place Claw
there, let him settle and set the random stream to the store's
`rng_at_t0` before the segment's first row, as `claw-replay --start X,Y
--seed N` does. A segment cut from a full route starts from the level's
starting world, though (enemies the route defeated earlier are alive,
elevators are at their start), so segments are best planned as short
routes of their own from their start point. The full-level run stays as
the end-to-end check.

## The gate

Both clocks, with different jobs (`install_differential` compares each
route in `references.toml` at its clock's preset):

- **c18 lockstep gates the logic.** It is exact and repeatable, it finds a
  difference at the tick it happens, and it already shows Claw's movement
  matching. The random stream is compared too, once the game draws as
  many numbers per step as the original; until then `references.toml`
  records it as known.
- **c60 feel gates Claw's movement feel**, on Claw's own columns over
  stretches without enemies (or segments), with `feel.toml`. Enemy
  encounters at c60 are reported, not gated, and a full-level route at c60
  need not finish the level: 16 and 17 ms frames against 18 ms ticks move a
  close landing by a tick (a jump pressed on the frame Claw lands is spent
  in the original and jumps in the game), and a route played without
  looking at the game goes another way from there.

## What the first level's bot routes show

The stores `level1/bot-original-v3` (`c18`) and `level1/bot-v3` (`c60`)
hold the original's runs, twice at their clock with identical traces both
times and from the original's random state at time zero, of the routes
that were in `tests/acceptance/routes/` before the ten rules listed in
"What is fixed and what remains" (the goodies, the save points, the
enemy built beside its limits, the kegs' depth, the walk start and the
respawn).

- **`c18`.** The game equals the original on all 6691 compared steps
  (120.4 s): Claw's position, state and physics type, the camera, health,
  score, lives, ammunition and the random stream. The route loses a life
  on the death tiles at 97002 ms in both games: it was planned with the
  walk start the game had before (Claw walked from standing while a
  direction was held; the original walks on its press), and it does not
  finish the level. The only difference is the events column's format: in
  the tick of a save the game also reports the save point and its
  message.
- **`c60`.** As for `level1/bot-v2`, whose route starts the same way:
  within the feel tolerance until Claw is 20 px lower in the jump off the
  ladder at 3217 ms, then the two runs go different ways.

Since that route loses in the original, the bot found the routes in
`tests/acceptance/routes/` again with these rules (same seed, same
margin rules); the original has not played the new routes yet.

The earlier stores stay listed as further checks: `level1/bot-original-v2`
and `level1/bot-v2` hold the routes from before the draws took the
original's schedule (the `c18` one is now equal on every step too), and
`level1/bot-original`, `level1/bot`, the `-delay1` stores and
`level1/bot-original-seg3410` hold earlier routes, whose keys were
already down on their first row.

## What is fixed and what remains

Fixed in the game, and so no longer listed as differences:

- **The level-start latch.** Keys already down when play starts count for
  Claw only once released and pressed again, as in the original. The
  routes start with two empty rows, so their first keys are pressed after
  play starts in both games.
- **The draw order at time zero.** The level's objects are set up while
  the level loads, before time zero, and update first in tick 0, as in the
  original. Their set-up draws (an officer's choice of line, its first
  idle draws, the first random wait of each timed ambient sound) no longer
  land in tick 0, and the random stream is equal from time zero.
- **The climb.** The default tuning climbs up at the original's 220 px/s,
  so the opening's climb at `c60` holds within the feel tolerance.
- **The random draw schedule.** The game draws from the shared random
  stream in the original's schedule (the seven rules listed at the top of
  `references.toml`): the small objects wake in an 831x623 window, objects
  made in the update pass run in it when they land after the object the
  pass visits next, a glitter draws its delay when the pass reaches it,
  the ambient timers run after the rest of the tick, a pacing enemy is at
  a limit only when its exact position is, a woken enemy first moves for
  the time it slept (at most 200 ms), and Claw's hit burst draws its y
  jitter first.
- **Goodies.** An item a crate or an enemy drops spins through its set
  while it flies and becomes collectable with the box of the picture it
  shows then (an edge-on coin is 8 px wide); it moves on an exact
  position, one tick of constant acceleration at a time, with its first
  picture's box grown 10 px left, up and right, and a wall on either side
  turns it around at half speed; and each call it first acts on what its
  previous move left (a landing makes it bounce in the next call, the
  call after the fourth bounce makes it a pickup).
- **Save points.** Saving the game leaves the random stream in the save's
  own state (1695759300), and the ambient timers count no time in the
  tick of a save.
- **Enemies placed beside their limits.** A human enemy placed outside
  its patrol limits starts on the nearer limit, on the floor below it.
  Which step of the original's set-up does this is not known; the game
  moves it there before it is dropped to the floor, which gives the
  original's result.
- **Kegs.** A lying keg is drawn in front of Claw or behind him by where
  his feet are, as the enemies are, so the kegs behind him run before him.
- **The walk start.** Standing, Claw starts a walk when Right or Left is
  pressed, and an attack's end or any change of his state makes a
  direction still held count as pressed in the next tick: a direction
  pressed during a standing attack walks after the attack's end.
- **The respawn.** A lost life with lives left ends its tick with Claw
  dead, and he is back at the checkpoint before the next tick, with the
  camera and the objects around him set up and one update pass run. The
  closing and opening circles are drawn by the game's window between the
  two ticks, outside game time, so recordings and traces have no ticks
  for them.

Known differences that remain, each listed in `references.toml` for the
routes it touches:

- **Draws still out of step.** Two `c18` stores still have a step where
  the two games draw a different number of times (the time is
  `known.rng` in `references.toml`):
  - `level1/bot-original-delay1`: first differs at 47052 ms, where Claw
    has stood since his respawn and the game draws, a step early, one of
    the two numbers the original draws in the next step. Claw's columns
    stay equal until 52452 ms.
  - `level1/bot-original-seg3410`: first differs at 18 ms, the segment's
    first step after Claw is placed and the stream is set: the original
    draws nothing there, the game draws one number. Claw's columns stay
    equal until 19620 ms.

  Neither step is traced to its cause yet. Every other `c18` store is
  equal on every step.
- **The events' format.** In the tick of a save the game also reports
  the save point and its message, which the original's traces do not
  list (`level1/bot-original-v3`, 36450 ms).
- **Open-loop routes at `c60`.** Frames of 16 and 17 ms against the
  game's 18 ms ticks move a landing or a ladder grab by a frame or two,
  and a route played without looking at the game goes another way from
  there.
- **Default-tuning feel at `c60`.** A fall from standing still runs up to
  about one tick ahead of the original in the middle of the fall, and a
  running jump with the jump key held part of the way peaks 11 px lower
  (the route written down from a fast run, 6050 ms).
- **Directions held across a change of Claw's state.** In the original,
  each change of Claw's state takes Left and Right off the latch, so a
  direction held since before play started counts again after Claw's
  next state change (it shows up as a fresh press). The game keeps such a
  direction ignored until it is released and pressed again. The listed
  routes hold no direction when play starts, and no listed first
  divergence is traced to it so far.

## Adding a reference store

1. Play the route in the original at one clock, at least twice, and write
   each run as a reference trace in the trace format. Repeats of a route
   are expected to be identical.
2. Put the runs in a new folder `$CLAW_REFERENCE/level<N>/<name>/`: the
   route as `route.csv`, the traces as `trace-<clock>-r<k>.csv`, and a
   `meta.toml` with `level`, the random state at time zero
   (`rng_at_t0`), and a `[c18]` or `[c60]` table naming the `tuning`
   (`original` or `default`) and the `traces`. A route that starts away
   from the level's start names its start point (`teleport = "X,Y"`). A
   store is never overwritten: when a route changes, the new runs go into
   a new folder (for example with a `-v2` suffix) and the old entry stays.
3. Run `make compare REF=level<N>/<name> CLOCK=<clock>`. It prints the
   first divergence of each column, and fails while the store has no
   entry.
4. Add a `[[compare]]` entry to `references.toml` with the reference, the
   clock, the tolerance (`exact` for `c18`, `feel` for `c60`) and
   `known.<column> = <ms>` for each column the comparison printed, with a
   comment saying in plain words why the route leaves its tolerance
   there. `make compare` then passes, and `make test-install
   MODULE=acceptance` checks the store with both drivers.
5. When a fix in the game moves a listed divergence later or removes it,
   the tests say so; tighten the entry to the new value.
