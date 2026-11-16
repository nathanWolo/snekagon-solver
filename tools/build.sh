#!/bin/sh
# Builds the solver the way the climb's base is built (LOG, E27): the MSVC toolchain, and with --pgo a
# profile-guided build. Run from the root of a checkout or worktree.
#
#     sh tools/build.sh OUT.exe          plain MSVC build of this tree (for screens: both arms plain)
#     sh tools/build.sh --pgo OUT.exe    profiled build (for gates and for the frozen base)
#
# The profile is always collected the same way: the instrumented build on proof positions 0-23 with
# --window 0,1 and on ply 7 positions 0-99 (tune positions; the ply-7 file is read from the main
# checkout, a worktree does not have it). The profile run and the compile go through the run lock
# (do not wrap this script in locked.sh).
#
# The linker fails when the target directory's path is long (this repo's folder is), so everything is
# built in a short directory: $SNEK_BUILD_DIR, or one under the temporary directory named after this
# checkout.
set -e
TC=stable-x86_64-pc-windows-msvc
PGO=0
if [ "$1" = "--pgo" ]; then PGO=1; shift; fi
OUT=$1
[ -n "$OUT" ] || { echo "usage: sh tools/build.sh [--pgo] OUT.exe" >&2; exit 2; }
HERE="$(pwd)"
MAIN="$(cd "$(git rev-parse --git-common-dir)/.." && pwd)"
B="${SNEK_BUILD_DIR:-$(cygpath -m "${TEMP:-/tmp}")/snekb/$(printf %s "$HERE" | cksum | cut -d' ' -f1)}"
mkdir -p "$B"
build() { # build TARGET-SUBDIR RUSTFLAGS
  # the compile takes the machine's run lock too: a build beside a timed run costs that run a
  # percent or two (2026-11-15, a compile overlapped a gate chunk)
  # cargo's own exit status decides: an exe left in the build directory by an earlier build is
  # not a result (2026-11-15: after a compile error the old exe was copied out and its hash
  # printed, because only the file's existence was checked)
  if RUSTFLAGS="$2" sh "$MAIN/tools/locked.sh" cargo +$TC build --release --target-dir "$B/$1" > "$B/$1.log" 2>&1; then OK=1; else OK=0; fi
  grep -E "^error|LNK|Finished" "$B/$1.log" | cut -c1-300 || true
  [ $OK = 1 ] && [ -f "$B/$1/release/snek.exe" ] || { echo "the build failed (log: $B/$1.log)" >&2; exit 1; }
}
if [ $PGO = 0 ]; then
  build plain "-Ctarget-cpu=native"
  cp "$B/plain/release/snek.exe" "$OUT"
else
  PROFDATA="$(ls "$HOME"/.rustup/toolchains/stable-x86_64-pc-windows-*/lib/rustlib/*/bin/llvm-profdata.exe 2>/dev/null | head -1)"
  [ -n "$PROFDATA" ] || { echo "llvm-profdata not found: rustup component add llvm-tools" >&2; exit 1; }
  rm -rf "$B/prof"; mkdir -p "$B/prof"
  build gen "-Ctarget-cpu=native -Cprofile-generate=$B/prof"
  sh tools/locked.sh sh -c "\"$B/gen/release/snek.exe\" bench tests/proof4/positions.txt --from 0 --to 24 --window 0,1 --quiet | tail -1; \"$B/gen/release/snek.exe\" bench \"$MAIN/tests/set1/ply07.txt\" --from 0 --to 100 --quiet | tail -1"
  "$PROFDATA" merge -o "$B/prof/merged.profdata" "$B"/prof/*.profraw
  build use "-Ctarget-cpu=native -Cprofile-use=$B/prof/merged.profdata"
  cp "$B/use/release/snek.exe" "$OUT"
fi
sha256sum "$OUT" | cut -c1-12
