#!/bin/sh
# The Steam Deck build, inside the Steam Runtime SDK container that
# deck/build.sh starts: the repository is /src and /cache keeps the Rust
# toolchain, the FFmpeg build and the cargo target between runs. Running it
# anywhere else is not useful.
set -eu

say() { printf 'deck: %s\n' "$*"; }
fail() { printf 'deck: %s\n' "$*" >&2; exit 1; }

[ -d /src ] && [ -d /cache ] || fail "run third_party/ffmpeg/deck/build.sh instead; this script runs in its container"

# Everything the build writes stays in /cache, apart from the result.
export HOME=/cache/home
export XDG_CACHE_HOME=/cache/xdg
export CARGO_HOME=/cache/cargo
export RUSTUP_HOME=/cache/rustup
export CARGO_TARGET_DIR=/cache/target
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$HOME" "$XDG_CACHE_HOME" "$CARGO_HOME" "$RUSTUP_HOME" "$CARGO_TARGET_DIR"

say "glibc in the SDK: $(ldd --version | head -n 1)"

# Rust: rustup, checked against its published SHA-256, then the toolchain
# rust-toolchain.toml names.
if ! command -v rustup >/dev/null 2>&1; then
    url=https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
    say "fetching rustup"
    curl -fsSL --retry 2 -o "$HOME/rustup-init" "$url"
    want=$(curl -fsSL --retry 2 "$url.sha256" | cut -d' ' -f1)
    got=$(sha256sum "$HOME/rustup-init" | cut -d' ' -f1)
    [ -n "$want" ] && [ "$got" = "$want" ] || fail "rustup-init has SHA-256 $got, expected $want"
    chmod +x "$HOME/rustup-init"
    "$HOME/rustup-init" -y --no-modify-path --profile minimal --default-toolchain none >/dev/null
    rm -f "$HOME/rustup-init"
fi
channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' /src/rust-toolchain.toml)
[ -n "$channel" ] || fail "no channel in rust-toolchain.toml"
rustup toolchain install "$channel" --profile minimal >/dev/null

# FFmpeg and dav1d: the same pinned build as on any Linux, made with the
# SDK's compiler and glibc, in this cache rather than the host's (meson and
# ninja from the SDK, or from pip into the cache when it lacks them). The
# Deck build must decode AV1, the upscaled movies' format.
nice -n 10 sh /src/third_party/ffmpeg/build.sh
record="$(sh /src/third_party/ffmpeg/build.sh --prefix)/share/rusty-claw/ffmpeg-build.txt"
if grep -q '^dav1d none$' "$record"; then
    fail "FFmpeg was built without AV1 (dav1d); see the build's message above"
fi

# The game, with movies: a build without FFmpeg is an error here.
RUSTY_CLAW_FFMPEG=require nice -n 10 cargo build --release --locked -p claw-game

bin="$CARGO_TARGET_DIR/release/rusty-claw"
if ldd "$bin" | grep -E 'libav|libsw|libdav1d'; then
    fail "the binary loads FFmpeg or dav1d libraries; it must carry them inside"
fi
sh /src/third_party/ffmpeg/deck/glibc-floor.sh "$bin" "${DECK_GLIBC_MAX:-2.31}"
cp "$bin" /src/target/deck/rusty-claw
say "copied to target/deck/rusty-claw"
