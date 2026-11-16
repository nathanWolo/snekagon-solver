#!/bin/sh
# Group 1: shadow runs at the new defaults (and start3 off on proof 0-23), with a dump for rtcheck.
M=<session-folder>/snekagon
W=<scratchpad>/wt/y
S=$W/work/sh.exe
O=$W/work/g1
P=$M/tests/proof4/positions.txt
SET=$M/tests/set1
cd $W
R="sh tools/shadow_rt3.sh $S $O"
export SHADOW_DUMP=$O/dump.txt
mkdir -p $O
$R proofA bench $P --to 8 --window 0,1 --quiet
$R proofB bench $P --from 8 --to 16 --window 0,1 --quiet
$R proofC bench $P --from 16 --to 24 --window 0,1 --quiet
$R ply06 bench $SET/ply06.txt --to 200 --quiet
$R ply07 bench $SET/ply07.txt --to 590 --quiet
$R ply08 bench $SET/ply08.txt --quiet
$R other bench $SET/bench_ply5_other_openings.txt --quiet
$R prooffull bench $P --to 3 --quiet
unset SHADOW_DUMP
$R par-ply06 bench $SET/ply06.txt --to 200 --threads 8 --quiet
$R par-proof bench $P --to 8 --window 0,1 --threads 8 --quiet
$R par-prooffull bench $P --from 3 --to 5 --threads 8 --quiet
$R off-proofA bench $P --to 12 --window 0,1 --quiet --no-reach-start3
$R off-proofB bench $P --from 12 --to 24 --window 0,1 --quiet --no-reach-start3
echo "== group done $(date +%T)" >> $O/progress.txt
