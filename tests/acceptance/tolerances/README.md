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

- Claw's position, state, physics type and the camera point are equal on
  every tick for the first 16.4 s (911 ticks): standing, both jumps,
  walking off the step, walking, the climb, the treasure (score equal) and
  the running jumps.
- At 16416 ms the officer's sword hits Claw in the game while he jumps
  down past it; in the original it misses. In the game that officer starts
  pacing about 4.5 s into the route, while Claw is still far below, and
  waits where Claw lands. Everything after follows from that hit.
- With the level's objects held inert, Claw's horizontal position is equal
  for the whole 32.5 s run and his height until the original's officer
  first hits him at 25938 ms. Claw's own movement and the camera therefore
  follow the original exactly at this clock; no movement fix is needed.
- The random stream differs from the first tick. The cause is the order of
  enemy and object draws, not sound:
  - the game makes the level's objects on their first tick, so their
    set-up draws (the officer's choice of line, its first idle draws and
    the first random wait of each timed ambient sound) come in tick 0 and
    their first update in tick 1, while the original makes them while the
    level loads, before time zero, and updates them first in tick 0. Seeded
    with the random state the level load starts from instead, the game
    draws the same values as the original, one tick late, for the first
    six ticks;
  - an enemy's per-tick roll comes every third logic step in the original
    and on every tick in the game, which ends that agreement.
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
| 10.6 to 12.9 s | climbs the ladder | grows to 39 px: the game climbs at about 240 px/s, the original at 220 px/s, and reaches the top 250 ms early |
| 12.9 to 16.3 s | stands, jumps for treasure, running jump | equal standing, within 7 px in the air; score equal |
| from 16.4 s | the officer | the original's officer hits Claw, the game's does not; up to 410 px apart after |

Within the feel tolerance, the opening holds until the climb (11150 ms) and,
leaving the climb out, until the officer (16384 ms).

## Tolerance values

- `exact.toml` is exact by definition.
- `feel.toml`: the smallest values that hold Claw's own movement on the
  first level's routes at 60 frames per second (leaving out the climb, the
  enemies and a partly held running jump). A fall from standing still runs
  up to about one tick ahead of the original in the middle of the fall at
  this frame rate (both land within 4 ms of each other), so the position
  needs two ticks of timing slack to stay within 8 px; with it, the opening
  holds until the climb (11400 ms) and the route written down from a fast
  run holds until its partly held running jump (6050 ms). State changes
  within two ticks (one tick is not enough: a landing comes two ticks apart
  once); score changes within two ticks. The event slack is a starting
  value.
- The climb difference is not absorbed into the tolerance: the climbing
  speed of the default tuning is to be set to the original's 220 px/s.

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
  matching. The random stream is compared too, once the enemy draw order
  is fixed; until then `references.toml` records it as known.
- **c60 feel gates Claw's movement feel**, on Claw's own columns over
  stretches without enemies (or segments), with `feel.toml`. Enemy
  encounters at c60 are reported, not gated, and a full-level route at c60
  need not finish the level: 16 and 17 ms frames against 18 ms ticks move a
  close landing by a tick (a jump pressed on the frame Claw lands is spent
  in the original and jumps in the game), and a route played without
  looking at the game goes another way from there.

## What the first level's bot routes show

The bot's routes press Right and Jump on their first row. The original
ignores keys already down when play starts until they are released and
pressed again, so in the original Claw stands still at the start, and the
rest of each route plays from the wrong place; neither route finishes the
level there. With the routes delayed by one row, or cut into a segment
with Claw placed at its start, Claw's own movement is equal tick for tick
at `c18` (8.6 s and 13.6 s), until a random draw differs: an officer's
sword that hits in the game and misses in the original, or a standing
attack whose variant comes from the random stream. `references.toml`
lists these first divergences per route until the original plays the new
routes.
