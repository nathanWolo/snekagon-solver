#!/bin/sh
# The machine's run lock, first come first served. Called through tools/locked.sh:
#
#     sh tools/locked.sh target/release/snek.exe bench tests/proof4/positions.txt --to 24 --window 0,1
#
# One lock for every checkout and worktree of this repository (it lives in the shared .git
# directory). Take it for every timed run and for anything that uses a processor for more than
# about 15 seconds. Keep a command under the lock to about 15 minutes, for the others' sake.
#
# Fairness: every caller takes a ticket named by its arrival time; only the oldest ticket may take
# the lock. A script that runs one locked command after another therefore goes to the back of the
# queue each time (before 2026-11-15 it won the race against every waiter, and a waiter behind
# two busy scripts got one turn in 75 minutes).
#
# A lock that nobody has kept fresh for 20 minutes is taken to be left behind (its holder was
# killed) and is taken over. A ticket nobody has refreshed for about 3 minutes belonged to a
# waiter that died and is removed.
COMMON="${RUN_LOCK_DIR:-$(cd "$(git rev-parse --git-common-dir)" && pwd)}"
LOCK="$COMMON/run.lock"
QUEUE="$COMMON/run.queue"
mkdir -p "$QUEUE"
ME=$$
# RUN_LOCK_FIRST=1 puts this caller ahead of the queue (it still waits for the command that holds
# the lock): for something the owner is waiting for at the keyboard, as the analysis board's own
# runs do. Not for routine work.
NAME="${RUN_LOCK_FIRST:+0-}$(date +%s%N)-$ME"
TICKET="$QUEUE/$NAME"
: > "$TICKET"
trap 'rm -f "$TICKET"' EXIT
trap 'rm -f "$TICKET"; exit 130' INT TERM
while :; do
  touch "$TICKET" 2>/dev/null || : > "$TICKET"
  find "$QUEUE" -type f -mmin +2 -delete 2>/dev/null
  if [ "$(ls "$QUEUE" | sort | head -1)" = "$NAME" ]; then
    mkdir "$LOCK" 2>/dev/null && break
    if [ -n "$(find "$LOCK" -maxdepth 0 -mmin +20 2>/dev/null)" ]; then
      rmdir "$LOCK" 2>/dev/null
      continue
    fi
  fi
  sleep 2
done
# While the command runs the lock is kept fresh, so a long run is never taken to be left behind.
# The keeper stops by itself within a minute if this script dies, and the lock then expires.
# (Its output goes nowhere: a caller that reads this script's output must not wait for the keeper.)
( while sleep 60; do kill -0 $ME 2>/dev/null || exit 0; touch "$LOCK" 2>/dev/null || exit 0; done ) </dev/null >/dev/null 2>&1 &
KEEP=$!
trap 'kill $KEEP 2>/dev/null; rmdir "$LOCK" 2>/dev/null' EXIT
trap 'kill $KEEP 2>/dev/null; rmdir "$LOCK" 2>/dev/null; exit 130' INT TERM
rm -f "$TICKET"
"$@"
