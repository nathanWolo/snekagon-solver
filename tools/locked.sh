#!/bin/sh
# Runs a command while holding this machine's run lock, so that timed and heavy runs never overlap.
#
#     sh tools/locked.sh target/release/snek.exe bench tests/proof4/positions.txt --to 24 --window 0,1
#
# One lock for every checkout and worktree of this repository (it lives in the shared .git
# directory). Take it for every timed run and for anything that uses a processor for more than
# about 15 seconds; compiling and quick checks do not need it. Keep a command under the lock to
# about 15 minutes, for the others' sake. A lock that nobody has kept fresh for 20 minutes is
# taken to be left behind (its holder was killed) and is taken over.
COMMON="$(cd "$(git rev-parse --git-common-dir)" && pwd)"
LOCK="$COMMON/run.lock"
while ! mkdir "$LOCK" 2>/dev/null; do
  if [ -n "$(find "$LOCK" -maxdepth 0 -mmin +20 2>/dev/null)" ]; then
    rmdir "$LOCK" 2>/dev/null
    continue
  fi
  sleep 3
done
# While the command runs the lock is kept fresh, so a long run is never taken to be left behind.
# The keeper stops by itself within a minute if this script dies, and the lock then expires.
ME=$$
# (Its output goes nowhere: a caller that reads this script's output must not wait for the keeper.)
( while sleep 60; do kill -0 $ME 2>/dev/null || exit 0; touch "$LOCK" 2>/dev/null || exit 0; done ) </dev/null >/dev/null 2>&1 &
KEEP=$!
trap 'kill $KEEP 2>/dev/null; rmdir "$LOCK" 2>/dev/null' EXIT INT TERM
"$@"
