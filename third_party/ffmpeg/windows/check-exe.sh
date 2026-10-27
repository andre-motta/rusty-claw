#!/bin/sh
# Checks that a Windows build of the game carries FFmpeg inside it: the
# executable names no FFmpeg DLL (avcodec, avformat, avutil, swscale,
# swresample) among the DLLs it loads, and holds FFmpeg's version string.
#
#   sh third_party/ffmpeg/windows/check-exe.sh [target/release/rusty-claw.exe]
#
# Run it from the same MSYS2 shell as build.sh (dumpbin.exe comes with the
# Visual Studio Build Tools).
set -eu

exe=${1:-target/release/rusty-claw.exe}
[ -f "$exe" ] || { echo "no $exe; build it first (see third_party/ffmpeg/README.md)" >&2; exit 1; }
command -v dumpbin.exe >/dev/null 2>&1 ||
    { echo "dumpbin.exe is not on PATH: run this inside the x64 Native Tools Command Prompt's MSYS2 shell" >&2; exit 1; }

dlls=$(dumpbin.exe -nologo -dependents "$exe" | tr -d '\r' | grep -i '\.dll' | sed 's/^ *//')
printf 'DLLs %s loads:\n%s\n' "$exe" "$dlls"
if printf '%s\n' "$dlls" | grep -i -q -E '^(avcodec|avformat|avutil|swscale|swresample)(-[0-9]+)?\.dll$'; then
    echo "FAIL: $exe loads an FFmpeg DLL; it must link FFmpeg statically" >&2
    exit 1
fi
if ! grep -a -q 'Lavc%d' "$exe"; then
    echo "FAIL: $exe does not contain FFmpeg; was it built with --features claw-game/movies?" >&2
    exit 1
fi
echo "ok: FFmpeg is linked into $exe and no FFmpeg DLL is needed"
