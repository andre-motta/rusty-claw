#!/bin/sh
# A drill of build.sh's AV1 fallback on FFmpeg's side: dav1d builds, but
# FFmpeg's configure cannot use it (a pkg-config that knows no dav1d comes
# first on PATH), and the build must still end with FFmpeg for the original
# movies, without AV1, saying why and recording it; a second run must find
# that build without trying AV1 again.
#
#   sh third_party/ffmpeg/av1-fallback-drill.sh DIR
#
# DIR, an empty or new folder, becomes the cache (XDG_CACHE_HOME) of the
# drill's builds, so your own cache is only read: the source tarballs
# already downloaded there are copied in. It builds dav1d once and FFmpeg
# twice (a few minutes) and prints the prefix of the build without AV1 last,
# for example for
#
#   FFMPEG_PREFIX=<that prefix> cargo test -p claw-movie --test smoke
set -eu

fail() { printf 'drill: %s\n' "$*" >&2; exit 1; }

[ $# -eq 1 ] || fail "usage: sh third_party/ffmpeg/av1-fallback-drill.sh DIR"
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$1"
dir=$(cd "$1" && pwd)
[ -z "$(ls -A "$dir")" ] || fail "$dir is not empty"
real_pkg_config=$(command -v pkg-config 2>/dev/null) || fail "the drill needs pkg-config"

cache="$dir/cache"
mkdir -p "$dir/bin" "$cache/rusty-claw/ffmpeg/downloads"
own="${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/ffmpeg/downloads"
for f in "$own"/*.tar.*; do
    if [ -f "$f" ]; then cp "$f" "$cache/rusty-claw/ffmpeg/downloads/"; fi
done

# Answers every question but those about dav1d, as the real one.
{
    printf '#!/bin/sh\n'
    printf 'case " $* " in *dav1d*) exit 1 ;; esac\n'
    printf 'exec "%s" "$@"\n' "$real_pkg_config"
} >"$dir/bin/pkg-config"
chmod +x "$dir/bin/pkg-config"

build() {
    env -u FFMPEG_PREFIX -u RUSTY_CLAW_FFMPEG_AV1 -u RUSTY_CLAW_FFMPEG_H26X \
        XDG_CACHE_HOME="$cache" PATH="$dir/bin:$PATH" sh "$here/build.sh" "$@"
}

build >"$dir/first.log" 2>&1 || { tail -n 30 "$dir/first.log" >&2; fail "the build failed (log in $dir/first.log)"; }
grep -q '^ffmpeg: building dav1d' "$dir/first.log" ||
    fail "dav1d was not built, so FFmpeg's configure was not reached with it (log in $dir/first.log)"
grep -q "^ffmpeg: building without AV1: FFmpeg's configure" "$dir/first.log" ||
    fail "the build did not go on without AV1 after FFmpeg's configure (log in $dir/first.log)"

prefix=$(build --prefix)
av1=$(build --av1)
case "$av1" in
    "no: FFmpeg's configure"*) ;;
    *) fail "build.sh --av1 says '$av1', not the configure failure" ;;
esac
grep -qx 'dav1d none' "$prefix/share/rusty-claw/ffmpeg-build.txt" ||
    fail "the build in $prefix is not the one without AV1"
[ -f "$prefix/lib/libavcodec.a" ] || fail "$prefix holds no libavcodec.a"
[ ! -e "$prefix/lib/libdav1d.a" ] || fail "$prefix holds dav1d"
[ -f "$cache/rusty-claw/ffmpeg/av1-failed.txt" ] || fail "no av1-failed.txt was kept"

build >"$dir/second.log" 2>&1 || fail "the second run failed (log in $dir/second.log)"
grep -q "is already built in $prefix" "$dir/second.log" ||
    fail "the second run did not find the build without AV1 (log in $dir/second.log)"
! grep -q 'building dav1d' "$dir/second.log" || fail "the second run tried AV1 again"

printf 'drill: passed; %s\n' "$av1"
printf '%s\n' "$prefix"
