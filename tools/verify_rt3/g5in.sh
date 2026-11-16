#!/bin/sh
# Inner script for one lock turn: the planted faults (layer by layer) and position 23 under
# --no-cost-order. Each run has its own timeout.
M=<session-folder>/snekagon
W=<scratchpad>/wt/y
C=<scratchpad>/wt/y3c/work/cand.exe
S=$W/work/sh2.exe
O=$W/work/g5
P=$M/tests/proof4/positions.txt
SET=$M/tests/set1
mkdir -p $O
r() { # r NAME TIMEOUT EXE ARGS...
  n=$1; t=$2; shift 2
  echo "== $n start $(date +%T)" >> $O/progress.txt
  timeout $t "$@" > $O/$n.txt 2>&1
  echo "== $n exit $? $(date +%T)" >> $O/progress.txt
}
X="--reach-threat-start 4"
if [ "$1" = A ]; then
r p23-on 60 $C bench $P --from 23 --to 24 --window 0,1 $X
r p23-off 60 $C bench $P --from 23 --to 24 --window 0,1 $X --no-reach-start3
r p23-on-nco 60 $C bench $P --from 23 --to 24 --window 0,1 $X --no-cost-order
r p23-off-nco 60 $C bench $P --from 23 --to 24 --window 0,1 $X --no-reach-start3 --no-cost-order
for f in 2 3; do
  SHADOW_FAULT=$f SHADOW_LAYERS=1 r f$f-l1-ply7 60 $S bench $SET/ply07.txt --to 590 --quiet
  SHADOW_FAULT=$f SHADOW_LAYERS=1 r f$f-l1-proof 120 $S bench $P --to 8 --window 0,1 --quiet
done
else
for f in 1 2 3; do
  SHADOW_FAULT=$f SHADOW_LAYERS=4 r f$f-l4-proof 120 $S bench $P --to 8 --window 0,1 --quiet
  SHADOW_FAULT=$f SHADOW_LAYERS=0 r f$f-l0-ply7 60 $S bench $SET/ply07.txt --to 590 --quiet
  SHADOW_FAULT=$f SHADOW_LAYERS=0 r f$f-l0-proof 50 $S bench $P --to 24 --window 0,1 --quiet
done
fi
echo "== g5 $1 done $(date +%T)" >> $O/progress.txt
