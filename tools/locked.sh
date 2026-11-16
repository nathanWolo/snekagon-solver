#!/bin/sh
# Runs a command while holding this machine's run lock (one lock for every checkout and worktree,
# first come first served):
#
#     sh tools/locked.sh target/release/snek.exe bench tests/proof4/positions.txt --to 24 --window 0,1
#
# The lock itself is tools/lockfair.sh in the main checkout, so that every worktree runs the same
# text whatever its branch holds. Read the rules there.
exec sh "$(cd "$(git rev-parse --git-common-dir)" && pwd)/../tools/lockfair.sh" "$@"
