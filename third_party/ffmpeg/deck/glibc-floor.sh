#!/bin/sh
# Prints the newest glibc version a Linux binary needs (its "glibc floor"):
# the highest GLIBC_x.y symbol version it imports. With a second argument,
# fails when the floor is newer than that version.
#
#   sh third_party/ffmpeg/deck/glibc-floor.sh target/release/rusty-claw
#   sh third_party/ffmpeg/deck/glibc-floor.sh target/deck/rusty-claw 2.31
#
# A system runs the binary only if its glibc (`ldd --version`) is at least
# the floor.
set -eu

bin=${1:?usage: glibc-floor.sh BINARY [MAX]}
max=${2:-}
[ -f "$bin" ] || { echo "no file $bin" >&2; exit 1; }

if command -v objdump >/dev/null 2>&1; then
    versions=$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*')
elif command -v readelf >/dev/null 2>&1; then
    versions=$(readelf -W --dyn-syms "$bin" | grep -o 'GLIBC_[0-9.]*')
else
    echo "needs objdump or readelf (binutils)" >&2
    exit 1
fi
floor=$(printf '%s\n' "$versions" | sed 's/^GLIBC_//' | sort -u -V | tail -n 1)
[ -n "$floor" ] || { echo "$bin imports no versioned glibc symbol" >&2; exit 1; }
echo "glibc floor of $bin: $floor"

if [ -n "$max" ]; then
    newest=$(printf '%s\n%s\n' "$floor" "$max" | sort -V | tail -n 1)
    if [ "$newest" != "$max" ]; then
        echo "FAIL: it needs glibc $floor, newer than $max" >&2
        exit 1
    fi
    echo "ok: at most glibc $max"
fi
