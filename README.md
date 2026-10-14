# rusty-claw

An open-source engine for playing Captain Claw (Monolith, 1997) natively on
modern Linux, with high-resolution and high-refresh output.

## You need your own copy of the game

This repository holds only source code. It contains no game files: no images,
sounds, levels, music or movies. To play, you need your own legally owned copy
of Captain Claw, and the engine reads its files from that install at run time.

Point the tools at the folder of your install, the one that holds the
`Assets` folder (an extracted asset tree), `CLAW.REZ` (an asset archive), or
both. Community custom levels are read from the install's `Custom` folder: a
level file `Custom/<name>.WWD` plus an optional folder `Custom/<name>/` with
the level's own tiles, images, sounds, animations, palette and music. Scripts
or plugins that ship with a custom level are never run.

## Status

Early work. What exists today:

- `engine/assets`: the asset layer. A layered virtual filesystem over REZ
  archives, extracted folders and custom levels (later sources win, lookups
  are case-insensitive and confined to the mounted sources), readers for the
  image, palette, colour-table, level and animation formats, and the level
  model.
- `claw-info`: prints a summary of a level.

The renderer, the simulation, the level viewer and the game are next.

## Building

Requirements: a Rust toolchain from [rustup](https://rustup.rs). The pinned
version in `rust-toolchain.toml` is installed automatically on first build.

```sh
make check                        # format check, lints and unit tests
make check-assets                 # unit tests of one module
CLAW_INSTALL=/path/to/your/install make test-install MODULE=assets
```

`make test-install` runs tests against your own install and refuses to run
without `CLAW_INSTALL`. Unit tests build their test data in code and never
need an install.

## Tools

```sh
cargo run --release -p claw-info -- --install /path/to/your/install --level 1
cargo run --release -p claw-info -- --install /path/to/your/install --custom "My Level.WWD"
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
