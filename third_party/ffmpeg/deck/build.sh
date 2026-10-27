#!/bin/sh
# Builds rusty-claw for the Steam Deck (and any x86-64 Linux with an older
# glibc) inside the Steam Runtime 3 "sniper" SDK container, with movies:
# FFmpeg is built from the pinned source in ../build.sh and linked in
# statically, so the Deck needs no FFmpeg of its own.
#
#   sh third_party/ffmpeg/deck/build.sh
#
# Needs podman or docker and a network connection (the SDK image, the Rust
# toolchain, the FFmpeg and nasm sources and the crates are downloaded on
# the first run). The result is target/deck/rusty-claw; its glibc floor is
# checked against DECK_GLIBC_MAX (default 2.31, the SDK's own glibc).
#
# Settings, all optional:
#   DECK_SDK_IMAGE   the SDK image (default the latest sniper SDK; set a
#                    dated tag from the registry to repeat a build exactly)
#   DECK_CACHE       where the toolchain, FFmpeg and the build tree are kept
#                    between runs (default ~/.cache/rusty-claw/deck)
#   DECK_GLIBC_MAX   the newest glibc version the binary may need
#   CONTAINER        podman or docker (default: whichever is installed)
set -eu

say() { printf 'deck: %s\n' "$*"; }
fail() { printf 'deck: %s\n' "$*" >&2; exit 1; }

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
[ -f "$repo/Cargo.toml" ] && [ -f "$repo/rust-toolchain.toml" ] || fail "cannot find the repository root from $here"

image=${DECK_SDK_IMAGE:-registry.gitlab.steamos.cloud/steamrt/sniper/sdk:latest}
cache=${DECK_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/deck}
glibc_max=${DECK_GLIBC_MAX:-2.31}

engine=${CONTAINER:-}
if [ -z "$engine" ]; then
    if command -v podman >/dev/null 2>&1; then
        engine=podman
    elif command -v docker >/dev/null 2>&1; then
        engine=docker
    else
        fail "needs podman or docker to run the Steam Runtime SDK"
    fi
fi

# The files the build writes belong to you: podman keeps your user id in
# the container, docker runs as your user id. SELinux labels are left alone
# rather than relabelling the checkout.
case "$engine" in
    podman) user="--userns=keep-id" ;;
    *) user="--user=$(id -u):$(id -g)" ;;
esac

mkdir -p "$cache" "$repo/target/deck"
say "building in $image with $engine (cache: $cache)"
"$engine" run --rm \
    "$user" \
    --security-opt label=disable \
    -v "$repo:/src" \
    -v "$cache:/cache" \
    -w /src \
    -e DECK_GLIBC_MAX="$glibc_max" \
    -e FFMPEG_JOBS="${FFMPEG_JOBS:-}" \
    "$image" \
    sh /src/third_party/ffmpeg/deck/inside.sh
say "done: $repo/target/deck/rusty-claw (copy it to the Deck; see third_party/ffmpeg/README.md)"
