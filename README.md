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

Claw stands, walks, runs, jumps (higher the longer Jump is held), falls,
lands, crouches, looks up and climbs, with tile collision and a camera that
follows him, in every retail level and in community custom levels.
Movement is tuned to cover the same distances in the same times as the
original played at 60 frames per second.

Claw fights with his sword, fists and feet: standing he swipes, punches,
uppercuts or kicks, and he swipes crouching and in the air. Officers, the
sword-fighting guards of the first level, stand guard, pace, strike back,
flinch and die. Claw loses health when he is struck and cannot be hurt again for a moment. The
game plays its sounds; the window title shows health, score and lives.

Not there yet: the other enemies, the pistol, magic and dynamite, pickups,
dying, music, menus, the status bar, movies and saved games. Other objects
are drawn where the level places them but do not move yet.

## Building on Fedora

You need a C linker, `make`, Vulkan drivers (Mesa's for AMD and Intel
GPUs; the NVIDIA driver brings its own), the ALSA and udev development
packages (for sound and, later, game controllers), plus the Rust toolchain
from [rustup](https://rustup.rs).

```sh
sudo dnf install gcc make git vulkan-loader mesa-vulkan-drivers alsa-lib-devel systemd-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # then open a new shell
git clone https://github.com/andre-motta/rusty-claw.git
cd rusty-claw
make run-game ARGS='--install /path/to/your/install'
```

The first build downloads the pinned Rust version from `rust-toolchain.toml`
and the dependencies, and takes a few minutes. The game binary is
`target/release/rusty-claw`; `make run-game ARGS=...` builds and runs it.

The game needs a desktop session (Wayland or X11) and a GPU with Vulkan
support, which in practice means GPUs from about 2016 on.

## Playing

```sh
rusty-claw --install /path/to/your/install                 # level 1
rusty-claw --install /path/to/your/install --level 4
rusty-claw --install /path/to/your/install --custom "My Level.WWD"
```

| Key | Action |
| --- | --- |
| Left, Right | walk; keep walking to run |
| Up | climb up; look up when standing |
| Down | climb down; crouch when standing |
| Space | jump; hold for a higher jump |
| Ctrl | attack: sword, punch, uppercut or kick; crouching and in the air, a sword swipe |
| Alt | fire the selected weapon (not there yet) |
| Shift | switch weapons: pistol, magic, dynamite |
| 1, 2, 3 | select the pistol, magic or dynamite |
| Z | lift and throw (not there yet) |
| P or Pause | pause |
| F7 | interpolation between game ticks on or off |
| F8 | fit or integer scaling |
| F9 | aspect: fill, centered, 4:3 |
| F10 | next window size |
| F11 | fullscreen |
| Escape | quit |

The game pauses when its window loses focus.

## Sound

The game plays the sounds of your install: every sound of a level is read
and decoded when the level loads. It uses the default output device of
your desktop (PipeWire, PulseAudio or ALSA); without one it runs silently
and says so once. Game sounds pause with the game. Music comes later.

| Option | What it does |
| --- | --- |
| `--no-audio` | no sound; no audio device is opened |

In the config file, `audio = off` does the same. Replays, benchmarks and
headless runs are always silent.

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

Window size, aspect, scaling and fullscreen also change while playing (F8
to F11); the game view and the game itself stay as they are.

The game never draws as fast as the GPU allows unless asked to: with vsync
on, the display's refresh paces drawing, and with vsync off, the frame-rate
cap does. Drawing thousands of frames a second gains nothing, wastes power
and makes some graphics cards whine audibly.

Settings can live in a config file, `~/.config/rusty-claw/config.toml` (or
under `$XDG_CONFIG_HOME`), or any file passed with `--config FILE`; command
line options win over it:

```toml
install = "/path/to/your/install"
level = 1                  # or: custom = "My Level.WWD"
res = "2560x1440"
fullscreen = false
aspect = "fill"            # fill, centered or 4:3
scale = "fit"              # fit or integer
widen_activation = false
vsync = true
fps_cap = 300              # with vsync off; 0 for no cap
interpolation = true
audio = on                 # on or off
```

## Replays and benchmarks

An input script is a CSV file with the columns `tick,held_mask,pressed_mask`,
one row per 18 ms game tick (masks: Jump 0x1, Attack 0x2, Fire 0x4, Switch
weapon 0x8, Lift 0x20, Pistol 0x40, Magic 0x80, Dynamite 0x100, Left
0x1000000, Right 0x2000000, Up 0x4000000, Down 0x8000000).

`--record FILE.csv` writes the input of every tick you play in the window
to such a script when the game ends; replaying it with the same level and
start gives the same game.

```sh
# play and record, then watch the recording
rusty-claw --install PATH --record game.csv
rusty-claw --install PATH --replay game.csv
# watch a script in the window, then take over with the keyboard
rusty-claw --install PATH --replay walk.csv
# no window: write one trace row per tick, with a simulated 144 Hz display
rusty-claw --install PATH --replay walk.csv --trace out.csv --render-hz 144
# benchmark: Claw on a fixed route for 10 s, statistics as JSON
make bench ARGS='--install PATH --level 1 --bench 10 --res 3840x2160'
# the same, drawn offscreen at the full size with a simulated display
make bench ARGS='--install PATH --level 1 --bench 10 --res 5120x1440 --headless'
```

Traces are identical at every display rate. The benchmark reports the CPU
work per frame, the cost of each game tick and the number of live objects,
frames that missed the display's refresh (in a window) or GPU time
(offscreen), and an estimate for a handheld-class CPU. When the desktop is
smaller than `--res`, the window gets what fits and every frame is also
drawn offscreen at the requested size.

The report is one line of JSON on standard output. A second line on
standard error gives the verdict, PASS or FAIL, with each budget: frame
work under 6.9 ms at the 99th percentile, the handheld estimate under
8.3 ms, and no missed refresh. Missed refreshes can be counted only in a
window with vsync on; offscreen or with vsync off the line says they are
not measurable. The benchmark exits with status 3 when a measured budget is
missed. For each missed refresh, the JSON describes that frame (its work,
the waits around it, sprite loads and window changes) and names the likely
cause, to tell a slow frame of the game from a stall in the desktop's
compositor.

The offscreen benchmark draws at its simulated display rate (`--render-hz`,
144 by default), as a real display would; per-frame times never include
that wait. Add `--uncapped` to draw as fast as the machine allows, for
measuring throughput.

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
| `CLAW_GPU_TESTS=1` | optional: run the renderer's GPU tests on synthetic data without an install |

The acceptance suite (`tests/acceptance`) checks that every level loads in
under 3 s, the visible tile grids, Claw's movement against the movement
targets, identical traces at 60, 144 and 240 Hz, frame cost at 3840x2160,
5120x1440 and 640x480, a 640x480 window at 60 frames per second or more,
and that community custom levels load. Some of these tests use the GPU or
open a window for a few seconds.

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

## Layout

| Directory | Package | What it is |
| --- | --- | --- |
| `engine/assets` | `claw-assets` | asset store, format readers, level model |
| `engine/frame` | `claw-frame` | fixed-step frame driver and the types shared by renderer and simulation |
| `engine/render` | `claw-render` | palette renderer |
| `engine/sim` | `claw-sim` | headless simulation |
| `app/info` | `claw-info` | level summary tool |
| `app/viewer` | `claw-viewer` | level viewer |
| `app/game` | `claw-game` | the game (`rusty-claw` binary) |
| `tools/replay` | `claw-replay` | input replay and traces |
| `tests/acceptance` | `claw-acceptance` | end-to-end tests |

## Licence

GPL-3.0-or-later; see [LICENSE](LICENSE). The licence covers this source code
only. Captain Claw and its assets belong to their respective owners and are
not part of this project.
