#!/bin/sh
# Group 2: (1) node counts of the candidate with start3 on/off at the search's work limit and with
# unlimited reach-threat work; (2) planted faults in start3, each check layer alone.
M=<session-folder>/snekagon
W=<scratchpad>/wt/y
C=<scratchpad>/wt/y3c/work/cand.exe
S=$W/work/sh2.exe
O=$W/work/g2
P=$M/tests/proof4/positions.txt
SET=$M/tests/set1
cd $W
R="sh tools/shadow_rt3.sh"
mkdir -p $O
U="--reach-threat-work 4000000000"
$R $C $O lim-on bench $P --to 24 --window 0,1 --quiet
$R $C $O lim-off bench $P --to 24 --window 0,1 --quiet --no-reach-start3
$R $C $O unl-on bench $P --to 24 --window 0,1 --quiet $U
$R $C $O unl-off bench $P --to 24 --window 0,1 --quiet $U --no-reach-start3
$R $C $O unl-on-ply7 bench $SET/ply07.txt --to 590 --quiet $U
$R $C $O unl-off-ply7 bench $SET/ply07.txt --to 590 --quiet $U --no-reach-start3
for f in 1 2 3; do
  for l in 1 2 4 0; do
    SHADOW_FAULT=$f SHADOW_LAYERS=$l $R $S $O f$f-l$l-ply7 bench $SET/ply07.txt --to 590 --quiet
    SHADOW_FAULT=$f SHADOW_LAYERS=$l $R $S $O f$f-l$l-proof bench $P --to 8 --window 0,1 --quiet
  done
done
echo "== group done $(date +%T)" >> $O/progress.txt
