#!/bin/sh
# Runs a command while holding this machine's run lock, so that timed and heavy runs never overlap.
#
#     sh tools/locked.sh target/release/snek.exe bench tests/proof4/positions.txt --to 24 --window 0,1
#
# One lock for every checkout and worktree of this repository (it lives in the shared .git
# directory). Take it for every timed run and for anything that uses a processor for more than
# about 15 seconds; compiling and quick checks do not need it. A command under the lock must
# finish within 15 minutes: a lock older than 20 minutes is taken to be left behind and is
# taken over.
COMMON="$(cd "$(git rev-parse --git-common-dir)" && pwd)"
LOCK="$COMMON/run.lock"
while ! mkdir "$LOCK" 2>/dev/null; do
  if [ -n "$(find "$LOCK" -maxdepth 0 -mmin +20 2>/dev/null)" ]; then
    rmdir "$LOCK" 2>/dev/null
    continue
  fi
  sleep 3
done
trap 'rmdir "$LOCK" 2>/dev/null' EXIT INT TERM
"$@"
