# rusty-claw

An open-source engine for playing Captain Claw (Monolith, 1997) natively on
modern Linux, with high-resolution and high-refresh output.

## You need your own copy of the game

This repository holds only source code. It contains no game files: no images,
sounds, levels, music or movies, and nothing converted from them. To play,
you need your own legally owned copy of Captain Claw, and the engine reads its
files from that install at run time, in place, without changing them.

Point the game at the folder of your install, the one that holds the
`Assets` folder (an extracted asset tree), `CLAW.REZ` (an asset archive), or
both. Community custom levels are read from the install's `Custom` folder: a
level file `Custom/<name>.WWD` plus an optional folder `Custom/<name>/` with
the level's own tiles, images, sounds, animations, palette and music. Only
those data files are read; scripts or plugins that ship with a custom level
are never run.

## Status

The first level, La Roca, plays from start to finish:

- Claw walks, runs, jumps, climbs, crouches and looks up, tuned to cover the
  same distances in the same times as the original at 60 frames per second.
- He fights with his sword, fists and feet (standing, crouching and in the
  air), the pistol, magic and dynamite, and lifts and throws powder kegs.
- Officers, soldiers and rats guard, pace, strike, shoot, throw bombs,
  flinch and die, dropping their loot.
- Health, lives, damage from enemies, shots and spikes, death and the
  respawn at the last checkpoint, save points, crates, powder kegs,
  treasure, food, ammunition, power-ups, the map piece and the end-of-level
  tally.
- Elevators, crumbling and toggling pegs and the animated scenery; pegs and
  elevators go back in place after a death.
- Sound effects with positional panning, ambient sounds, Claw's and the
  enemies' speech, and the music (with a SoundFont, see below).
- The status bar, screen transitions, the original-style title, main menu,
  options, load game, in-game menu, tally and game over, saved games,
  settings written back to the config file, gamepads and rebindable keys.

Every retail level loads and runs. In levels 2 to 14, the objects the first
level does not have (other enemies, bosses, traps and the like) are drawn
where the level places them but stand still; see [Known gaps](#known-gaps).

## Building on Fedora

You need a C linker, `make`, Vulkan drivers (Mesa's for AMD and Intel
GPUs; the NVIDIA driver brings its own), the ALSA development package (for
sound) and the udev development package (for gamepads), plus the Rust
toolchain from [rustup](https://rustup.rs).

```sh
sudo dnf install gcc make git curl vulkan-loader mesa-vulkan-drivers alsa-lib-devel systemd-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # then open a new shell
git clone https://github.com/andre-motta/rusty-claw.git
cd rusty-claw
make soundfont                                  # optional: the music's instruments
make run-game ARGS='--install /path/to/your/install'
```

On Debian and Ubuntu the packages are `build-essential git curl
libvulkan1 mesa-vulkan-drivers libasound2-dev libudev-dev`.

The first build downloads the pinned Rust version from `rust-toolchain.toml`
and the dependencies, and takes a few minutes. The game binary is
`target/release/rusty-claw`; `make run-game ARGS=...` builds and runs it.

The game needs a desktop session (Wayland or X11) and a GPU with Vulkan
support, which in practice means GPUs from about 2016 on.

## Playing

```sh
rusty-claw --install /path/to/your/install                 # the title and main menu
rusty-claw --install /path/to/your/install --level 4       # straight into level 4
rusty-claw --install /path/to/your/install --custom "My Level.WWD"
```

With only `--install` the game starts at the title, then the main menu:
new game (pick a level you have opened), load game (a level's start or one
of its two save points), options (sound, video, controls) and quit.
`--level` and `--custom` start straight into that level. In play, Escape, P
or the gamepad's Start button open the in-game menu: resume, options and
end the game. Finishing a level shows the treasure tally, then the main
menu; the next level is then open in the new game and load lists.

The game pauses when its window loses focus.

### Keyboard

| Key | Action |
| --- | --- |
| Left, Right | walk; keep walking to run |
| Up | climb up; look up when standing |
| Down | climb down; crouch when standing |
| Space | jump; hold for a higher jump |
| Ctrl | attack: sword, punch, uppercut or kick; crouching and in the air, a sword swipe |
| Alt | fire the selected weapon |
| Shift | switch weapons: pistol, magic, dynamite |
| 1, 2, 3 | select and fire the pistol, magic or dynamite |
| Z | lift an object in front of Claw; again to throw it (hold to throw farther) |
| Escape, P or Pause | the in-game menu |
| F7 | smooth motion between game ticks on or off |
| F8 | fit or integer scaling |
| F9 | aspect: fill, centered, 4:3 |
| F10 | next window size |
| F11 | fullscreen |

Keys are matched by their position, so the map is the same on any keyboard
layout. Hold the fire key longer to throw dynamite farther.

### Gamepad

Any standard gamepad works, the Steam Deck's controls included:

| Pad | Action |
| --- | --- |
| d-pad or left stick | move, climb, crouch, look up |
| South (A) | jump |
| East (B) | attack |
| West (X) | fire the weapon |
| North (Y) | switch weapons |
| left shoulder | lift and throw |
| right shoulder, left trigger, right trigger | pistol, magic, dynamite |
| Start | the in-game menu |

The menus also follow the d-pad, South to choose and East to go back.

### Rebinding

The controls page of the options lists every action with its keys and pad
buttons. Choose an action with the keyboard and press a key, or choose it
with the gamepad and press a pad button: that becomes its binding. A key
belongs to one action at a time, so taking a key from another action swaps
the two; the quit, pause and display keys cannot be taken. The same page
sets the stick dead zone and restores the default bindings. Bindings are
saved in the config file as `key_<action>` and `pad_<action>` lines (see
below), which can also be edited by hand.

## Sound and music

The game plays the sounds of your install: every sound of a level is read
and decoded when the level loads. It uses the default output device of
your desktop (PipeWire, PulseAudio or ALSA); without one it runs silently
and says so once. Sounds pause with the game; the music holds and carries
on.

Music is the game's own score, played through a General MIDI SoundFont (an
`.sf2` instrument bank). None comes with the source. The default bank is
GeneralUser GS by S. Christian Collins, under its own permissive licence:

```sh
make soundfont
```

fetches a pinned copy into `~/.local/share/rusty-claw/soundfonts/` (or under
`$XDG_DATA_HOME`) and checks its SHA-256. The game takes the first usable
bank from, in order: the `soundfont` setting, the `CLAW_SOUNDFONT`
environment variable, a packaged game's `share/rusty-claw/soundfonts/`,
that user folder, and the common system paths where distributions install
FluidR3_GM. Without a bank, music is off, one message says how to get one,
and the sound effects are unaffected.

| Option | What it does |
| --- | --- |
| `--no-audio` | no sound; no audio device is opened |

In the config file, `audio = off` does the same. The sound options set the
sound and music volumes and turn music, speech and ambient sounds on or
off. Replays, benchmarks and headless runs are always silent.

## Settings and saved games

Settings live in a config file, `~/.config/rusty-claw/config.toml` (or under
`$XDG_CONFIG_HOME`), or any file passed with `--config FILE`; command line
options win over it. Options changed in the menus are written back to that
file: only the changed lines change, every other line and comment stays as
it is, the previous file is kept as `config.toml.bak`, and values given on
the command line are not written back.

```toml
install = "/path/to/your/install"
level = 1                  # or: custom = "My Level.WWD"
res = "2560x1440"
fullscreen = false
aspect = "fill"            # fill, centered or 4:3
scale = "fit"              # fit or integer
widen_activation = false
mid_air_hit_fix = off      # on: no second jump after a hit in mid-air
invincible = off           # play-test aid: Claw takes no damage
infinite_lives = off       # play-test aid: a death never costs a life
vsync = true
fps_cap = 300              # with vsync off; 0 for no cap
interpolation = true
status_bar = "edges"       # edges, or centred in the 4:3 area
audio = on                 # on or off
sound_volume = 80          # 0 to 100
music_volume = 50          # 0 to 100
voice = on                 # Claw's and the enemies' speech
ambient = on               # ambient sounds
soundfont = "/path/to/a/soundfont.sf2"
gamepad = on               # on or off
pad_dead_zone = 0.34       # stick dead zone, 0.05 to 0.95
key_jump = "Space"         # key_<action>: key names, comma separated
pad_jump = "South"         # pad_<action>: pad button names
```

The bindable actions are `left`, `right`, `up`, `down`, `jump`, `attack`,
`projectile`, `toggle`, `lift`, `pistol`, `magic`, `dynamite`, `quit`,
`fullscreen`, `window_size`, `aspect`, `scale`, `interpolation` and `pause`.

By default a hit that ends while Claw is in the air leaves him able to jump
again before he lands, as in the original; speedruns rely on it, and any
hit works, an enemy's shot or your own dynamite. The mid-air hit jump fix
turns that off: Claw falls after such a hit. Turn it on with
`mid_air_hit_fix = on`, `--mid-air-hit-fix on` or the gameplay options
(Options, then Gameplay); it applies to the games started after the
change.

### Play-test aids

Two aids help with trying out the hard parts of a level. Both are off by
default:

- `--invincible`: Claw takes no damage from enemies, shots, hazards,
  explosions or bosses. He never goes into the hurt reaction and is never
  knocked back, so he moves as he does in normal play. Death tiles and pits
  still end his life.
- `--infinite-lives`: losing a life never lowers the lives count, so play
  goes on from the last checkpoint.

The same two switches are on the Gameplay page (Options, then Gameplay) and
in the config file (`invincible = on`, `infinite_lives = on`). Like the
other gameplay switches they apply from the next level started. A flag on
the command line turns its aid on for that run only and is not saved. A
small marker in the bottom-left corner of the screen names each aid that
is on.

A run with an aid on says nothing about how hard a level is. A recording
made with one says so in its header (`# invincible = on`,
`# infinite_lives = on`), and `--replay` plays it with that aid. Routes, the
search bot and the comparisons with the original (`make compare`,
`claw-replay --compare`) refuse such a recording, with a message that says
why.

Saved games are kept in `~/.local/share/rusty-claw/saves/default/` (or
under `$XDG_DATA_HOME`). Touching one of a level's two save points saves
the game there, and finishing a level saves the start of the next one with
the score, health, lives, ammunition and treasure carried over. Every write
is atomic and keeps the previous file; a damaged save is moved aside, never
deleted. Community custom levels are not saved.

## Resolution, aspect and refresh

The game always plays in the original 640x480 game view and runs its logic
in fixed 18 ms steps; drawing happens at your display's refresh rate, with
positions interpolated between steps, at any resolution.

| Option | What it does |
| --- | --- |
| `--res WxH` | window size, for example `2560x1440` (default `1280x960`) |
| `--fullscreen` | borderless fullscreen at the monitor's resolution |
| `--aspect fill` | fill a wide display (default); objects in the extra width stay still until Claw comes near, as they would off screen |
| `--aspect centered` | fill a wide display around the game view, showing past the level's edges |
| `--aspect 4:3` | the original 4:3 view with bars at the sides |
| `--scale fit` | fill the display height: whole-pixel steps, then a smooth remainder (default) |
| `--scale integer` | whole-pixel steps only, with bars |
| `--widen-activation` | with `fill`, keep everything visible on a wide display active; this changes gameplay |
| `--vsync on\|off` | wait for the display's refresh (default on) |
| `--fps-cap N` | with vsync off, draw at most N frames per second (default 300) |
| `--uncapped` | no frame-rate cap with vsync off (same as `--fps-cap 0`); for measuring throughput only |
| `--no-interp` | draw the latest game tick only |
| `--pack DIR` | an image pack with higher-resolution replacement images |

The status bar sits at the edges of the picture; on a wide display
`status_bar = "centred"` (or the video options) keeps it in the 4:3
area. Window size, aspect, scaling and fullscreen also change while playing
(F8 to F11, or the video options); the game view and the game itself stay
as they are.

The game never draws as fast as the GPU allows unless asked to: with vsync
on, the display's refresh paces drawing, and with vsync off, the frame-rate
cap does. Drawing thousands of frames a second gains nothing, wastes power
and makes some graphics cards whine audibly.

## Replays and benchmarks

An input script is a CSV file with the columns `tick,held_mask,pressed_mask`,
one row per 18 ms game tick (masks: Jump 0x1, Attack 0x2, Fire 0x4, Switch
weapon 0x8, Lift 0x20, Pistol 0x40, Magic 0x80, Dynamite 0x100, Left
0x1000000, Right 0x2000000, Up 0x4000000, Down 0x8000000).

`--record FILE.csv` writes the input of every tick you play in the window
to such a script when the game ends; replaying it with the same level and
start gives the same game. The recording's first line, a `#` comment, notes
whether the mid-air hit jump fix was on, and the replay follows it. Replays
and benchmarks never take that option from the config file: without such a
line, or for a benchmark, it is off unless `--mid-air-hit-fix on` is given.
Recordings made before this option existed were played with the fix on and
have no such line, so replay them with `--mid-air-hit-fix on`. A recording
made with a play-test aid on names it in its header, and the replay plays
with it (see [Play-test aids](#play-test-aids)); `claw-replay` accepts the
same `--invincible` and `--infinite-lives` flags.

```sh
# play and record, then watch the recording
rusty-claw --install PATH --level 1 --record game.csv
rusty-claw --install PATH --level 1 --replay game.csv
# no window: write one trace row per tick, with a simulated 144 Hz display
rusty-claw --install PATH --replay walk.csv --trace out.csv --render-hz 144
# benchmark: the combat route for 10 s, statistics as JSON
make bench ARGS='--install PATH --level 1 --bench 10 --res 3840x2160'
# the same, drawn offscreen at the full size with a simulated display
make bench ARGS='--install PATH --level 1 --bench 10 --res 5120x1440 --headless'
```

Traces are identical at every display rate.

The benchmark plays a combat route: Claw starts before the stretch of the
level where the most enemies stand together and takes them on one after
the other, with his sword and the pistol, so enemies fight back and shots,
bursts and pickups fly while frames are measured. The route is planned on a
copy of the game before the run and played once; it lasts a run paced at
the display rate, and the verdict says when an unpaced run outran it. The
offscreen benchmark also plays the sounds and music through a silent
output on the audio thread, so their CPU load is present during the run,
and the handling of the sound and music cues is part of each frame's
measured work.

The report is one line of JSON on standard output: the CPU work per frame,
the cost of each game tick and the number of live objects, frames that
missed the display's refresh (in a window) or GPU time (offscreen), an
estimate for a handheld-class CPU, the route and what it met (enemies
killed, the most enemies and shots live at once). A second line on standard
error gives the verdict, PASS or FAIL, with each budget: frame work under
6.9 ms at the 99th percentile, the handheld estimate under 8.3 ms, and no
missed refresh. Missed refreshes can be counted only in a window with vsync
on; offscreen or with vsync off the line says they are not measurable. The
benchmark exits with status 3 when a measured budget is missed. For each
missed refresh, the JSON describes that frame (its work, the waits around
it, sprite loads and window changes) and names the likely cause, to tell a
slow frame of the game from a stall in the desktop's compositor.

The offscreen benchmark draws at its simulated display rate (`--render-hz`,
144 by default), as a real display would; per-frame times never include
that wait. Add `--uncapped` to draw as fast as the machine allows, for
measuring throughput. When the desktop is smaller than `--res`, a window
benchmark gets what fits and every frame is also drawn offscreen at the
requested size.

## Routes and comparing with the original

A route is an input script that plays a level from its start. Besides the
rows, it can name how it is played in `#` header lines (`# level N`,
`# tuning default|original`, `# seed N`, and the behaviour switches it was
made with), and it can hold `@` lines for what happens between two ticks:
a pause or a menu screen with its wall time, or the in-game menu's end of a
life. Replays, `claw-replay` and the benchmark all follow them.

The routes in `tests/acceptance/routes/` finish the first level:
`level1.csv` with the default tuning and `level1-original.csv` with the
original game's speeds. They are written by the search bot, `claw-bot`,
which tries short sequences of moves on copies of the headless game and
keeps the ones that bring Claw closer to the level's end. Its budget counts
the positions it tries, not time, so the same arguments always give the
same route.

```sh
# watch the bot's route
make run-game ARGS='--install PATH --level 1 --replay tests/acceptance/routes/level1.csv'
# search a route (the report goes next to it as route.json)
cargo run --release -p claw-bot -- --install PATH --level 1 --out route.csv
# benchmark along a route from the level's start to its end, offscreen
make bench ARGS='--install PATH --level 1 --headless --res 3840x2160 --bench-route tests/acceptance/routes/level1.csv'
```

The route benchmark measures every frame as the combat benchmark does,
fails when a sprite set had to be loaded while drawing, and prints a digest
of the route's trace: the same on every run, at any display rate.

A reference trace is a run of the original game playing the same route,
written in the same trace format, one row per game step. Reference traces
are not part of this repository; point `CLAW_REFERENCE` at the folder that
holds them, one folder per route (`level<N>/<name>/` with the route as
`route.csv`, the traces as `trace-<clock>-r<k>.csv` and `meta.toml` saying
how they were played). A route can be played on two clocks:

- `c18`, one 18 ms game step per frame, compared tick for tick with the
  original speeds and the same random state: every column must be equal;
- `c60`, 60 frames per second, compared with the default tuning within the
  small differences in position and timing that `feel.toml` allows.

```sh
# replay the route with the store's tuning and random state, compare with
# every reference trace, print the first divergence and a per-column table
CLAW_INSTALL=PATH CLAW_REFERENCE=DIR make compare REF=level1/opening
make compare REF=level1/bot CLOCK=c60      # one clock
make compare REF=level1/bot STRICT=1       # fail on any divergence
# one trace with the replay tool
make replay ARGS='--install PATH --script route.csv --tuning original --seed N --compare trace.csv --tolerance exact'
```

`make compare` fails on a divergence that
`tests/acceptance/tolerances/references.toml` does not list for the route,
or that comes earlier than listed; that file records, with a reason, where
each compared route is known to leave its tolerance. A route that starts
somewhere in the middle of a level names its start point in `meta.toml`
(`teleport = "X,Y"`): Claw is placed there and left to settle before the
route starts, and once he stands the comparison also sets the random
stream to the store's `rng_at_t0`, as `claw-replay --start X,Y --seed N`
does (the game's own `--start` with `--replay` places Claw but does not
reset the random stream). `make test-install MODULE=acceptance`
with `CLAW_REFERENCE` set runs every listed route twice: tick by tick
(`install_reference`), and through the game's own frame loop at 60 Hz at
the gate's preset, exact at `c18` and feel at `c60` (`install_differential`).
`tests/acceptance/tolerances/README.md` explains the clocks and the
tolerance values.

## Development

```sh
make check                        # format check, lints and unit tests
make check-game                   # unit tests of one module
CLAW_INSTALL=/path/to/your/install make test-install MODULE=acceptance
```

Unit tests build their test data in code and never need an install. The
install tests read your install at run time and skip themselves without it:

| Variable | Used by |
| --- | --- |
| `CLAW_INSTALL` | every install test: `make test-install MODULE=<module>` refuses to run without it |
| `CLAW_GOLDEN_DIR` | optional: a folder of reference visible-tile grids (`level<N>_<W>x<H>.json`) the render and acceptance tests compare against |
| `CLAW_TOLERANCES` | optional: a TOML table of movement targets to check instead of the built-in one |
| `CLAW_REFERENCE` | optional: the folder of reference traces that `make compare` and the reference tests read (those tests skip without it) |
| `CLAW_GPU_TESTS=1` | optional: run the renderer's GPU tests on synthetic data without an install |
| `CLAW_AUDIO_SMOKE=1` | optional: open the real audio device in the audio tests |

The acceptance suite (`tests/acceptance`) plays the first level from start
to finish on a route built from the level's own records (every enemy
killed, every checkpoint, a death and the respawn, crates, a powder keg,
treasure and the tally, the elevator and the toggling pegs, the map piece)
and checks that the whole game is identical at 60, 144 and 240 Hz with
jitter. It also covers each weapon against each enemy, a saved game round
trip and the tally through the menus, a minute of random input on every
retail level, every level loading in under 3 s, the visible tile grids,
Claw's movement against the movement targets, frame cost at 3840x2160,
5120x1440 and 640x480, a 640x480 window at 60 frames per second or more,
and community custom levels. Some of these tests use the GPU or open a
window for a few seconds; on a locked or headless desktop run them under a
virtual display (`xvfb-run`). `tests/acceptance/PLAYTEST.md` is the list of
things to try by hand before a release.

Other tools:

```sh
make run-viewer ARGS='--install PATH --level 1'          # free-camera level viewer
make replay ARGS='--install PATH --level 1 --script s.csv --trace t.csv'
cargo run --release -p claw-info -- --install PATH --level 1
```

The viewer follows the same rules as the game: vsync on by default
(`--vsync off` to turn it off), a cap of `--fps-cap N` frames per second
(300 by default) with vsync off, and `--uncapped` only for throughput runs.
Its offscreen benchmark (`--headless-bench SECONDS`) draws at `--bench-hz`
unless `--uncapped` is given.

## Known gaps

- Levels 2 to 14: their own enemies, bosses, traps, ropes, conveyor belts
  and other objects do not move or act yet.
- No movies or cut scenes, and no multiplayer (curses only act in
  multiplayer games, so they vanish in single player as in the original).
- A few details still follow a reasonable guess rather than a measurement
  of the original: carrying an object walks at 30 px/s, rats that reach a
  wall turn back at walking speed, enemies treat goo like spikes, and a
  magic hit on an enemy shows no lingering flame or frost. The play-test list
  (`tests/acceptance/PLAYTEST.md`) asks about each one a player can see on
  the first level and names the rest, such as goo, as untested there.
- The detail setting and the play area size are shown but have no effect
  yet; the game always draws in high detail.
- One profile, without profile screens.

## Layout

| Directory | Package | What it is |
| --- | --- | --- |
| `engine/assets` | `claw-assets` | asset store, format readers, level model |
| `engine/frame` | `claw-frame` | fixed-step frame driver and the types shared by renderer and simulation |
| `engine/render` | `claw-render` | palette renderer, screen sprites and transitions |
| `engine/sim` | `claw-sim` | headless simulation and the level logics |
| `engine/audio` | `claw-audio` | mixer, output device and music synthesis |
| `engine/ui` | `claw-ui` | status bar, menus and the title, tally and game-over screens |
| `app/info` | `claw-info` | level summary tool |
| `app/viewer` | `claw-viewer` | level viewer |
| `app/game` | `claw-game` | the game (`rusty-claw` binary) |
| `tools/replay` | `claw-replay` | input replay, traces and their comparison |
| `tools/bot` | `claw-bot` | search bot that writes winning routes |
| `tests/acceptance` | `claw-acceptance` | end-to-end tests |

## Licence

GPL-3.0-or-later; see [LICENSE](LICENSE). The licence covers this source code
only. Captain Claw and its assets belong to their respective owners and are
not part of this project. The SoundFont that `make soundfont` fetches is
covered by its own licence.
