#!/bin/sh
# Group 3: the proof driver on tune jobs a and b with the candidate; rtcheck on the dump.
M=<session-folder>/snekagon
W=<scratchpad>/wt/y
C=<scratchpad>/wt/y3c/work/cand.exe
S=$W/work/sh2.exe
O=$W/work/g3
cd $W
R="sh tools/shadow_rt3.sh"
mkdir -p $O
P=$M/tests/proof4/positions.txt
$R $C $O pp-on bench $P --to 24 --window 0,1
$R $C $O pp-off bench $P --to 24 --window 0,1 --no-reach-start3
S3=$W/work/sh3.exe
SHADOW_LAYERS=1 SHADOW_RQ_SHIFT=30 $R $S3 $O chg-on bench $P --to 24 --window 0,1
SHADOW_LAYERS=1 SHADOW_RQ_SHIFT=30 $R $S3 $O chg-off bench $P --to 24 --window 0,1 --no-reach-start3
X="--reach-threat-start 4"
$R $C $O pp-on4 bench $P --to 24 --window 0,1 $X
$R $C $O pp-off4 bench $P --to 24 --window 0,1 $X --no-reach-start3
$R $C $O unl-on4 bench $P --to 24 --window 0,1 $X --reach-threat-work 4000000000
$R $C $O unl-off4 bench $P --to 24 --window 0,1 $X --reach-threat-work 4000000000 --no-reach-start3
SHADOW_LAYERS=1 SHADOW_RQ_SHIFT=30 $R $S3 $O chg-on4 bench $P --to 24 --window 0,1 $X
SHADOW_LAYERS=1 SHADOW_RQ_SHIFT=30 $R $S3 $O chg-off4 bench $P --to 24 --window 0,1 $X --no-reach-start3
for j in a b; do
  rm -f $O/$j.res
  $R $C $O prove-$j prove $M/work/e24/$j.pos --threads 16 --job-threads 1 --depth 1 --tt 27 --out $O/$j.res
done
echo "== group done $(date +%T)" >> $O/progress.txt
