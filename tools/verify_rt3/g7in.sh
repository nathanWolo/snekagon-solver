#!/bin/sh
M=<session-folder>/snekagon
C=<scratchpad>/wt/y3c/work/cand.exe
P=$M/tests/proof4/positions.txt
O=<scratchpad>/wt/y/work/g7
mkdir -p $O
timeout 100 $C bench $P --to 12 --window 0,1 --no-reach-start3 > $O/cand-off-0-11.txt 2>&1
timeout 100 $C bench $P --from 12 --to 24 --window 0,1 --no-reach-start3 > $O/cand-off-12-23.txt 2>&1
timeout 100 $C bench $P --to 8 --window 0,1 > $O/cand-on-0-7.txt 2>&1
