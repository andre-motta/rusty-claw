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

First playable version. Claw stands, walks, runs, jumps (higher the longer
Jump is held), falls, lands, crouches, looks up and climbs, with tile
collision and a camera that follows him, in every retail level and in
community custom levels. Movement is tuned to cover the same distances in
the same times as the original played at 60 frames per second.

Not there yet: enemies, pickups, attacks, damage, sound, music, menus, the
status bar, movies and saved games. Other objects are drawn where the level
places them but do not move yet.

## Building on Fedora

You need a C linker, `make`, and Vulkan drivers (Mesa's for AMD and Intel
GPUs; the NVIDIA driver brings its own), plus the Rust toolchain from
[rustup](https://rustup.rs). No other development packages are needed.

```sh
sudo dnf install gcc make git vulkan-loader mesa-vulkan-drivers
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
| P or Pause | pause |
| F7 | interpolation between game ticks on or off |
| F8 | fit or integer scaling |
| F9 | aspect: fill, centered, 4:3 |
| F10 | next window size |
| F11 | fullscreen |
| Escape | quit |

The game pauses when its window loses focus.

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
| `--no-interp` | draw the latest game tick only |
| `--pack DIR` | an image pack with higher-resolution replacement images |

Window size, aspect, scaling and fullscreen also change while playing (F8
to F11); the game view and the game itself stay as they are.

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
interpolation = true
```

## Replays and benchmarks

An input script is a CSV file with the columns `tick,held_mask,pressed_mask`,
one row per 18 ms game tick (masks: Jump 0x1, Left 0x1000000, Right
0x2000000, Up 0x4000000, Down 0x8000000).

```sh
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
