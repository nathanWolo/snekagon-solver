#!/bin/sh
# smoke: candidate and shadow on ply 7 0-60 and proof 0-1 narrow; node counts must agree
M=<session-folder>/snekagon
W=<scratchpad>/wt/y
C=<scratchpad>/wt/y3c/work/cand.exe
S=$W/work/sh.exe
cd $W
echo "cand ply7"; time $C bench $M/tests/set1/ply07.txt --to 60 --quiet | tail -1
echo "shadow ply7"; time $S bench $M/tests/set1/ply07.txt --to 60 --quiet 2>&1 | tail -32
echo "cand proof"; time $C bench $M/tests/proof4/positions.txt --to 1 --window 0,1 --quiet | tail -1
echo "shadow proof"; time timeout 400 $S bench $M/tests/proof4/positions.txt --to 1 --window 0,1 --quiet 2>&1 | tail -32
