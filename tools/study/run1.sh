#!/bin/sh
# Identity, verify, and a first study read. Run from WT under work/lockwait.sh.
E=work/study.exe
P=tests/proof4/positions.txt
echo "== identity (flags off), proof 0-4, expect 13150272"
$E bench $P --from 0 --to 4 --window 0,1 --quiet
echo "== reach threats on (all levels), proof 0-4"
$E bench $P --from 0 --to 4 --window 0,1 --quiet --reach-threat 0
echo "== verify 30"
$E verify 30
echo "== study, proof 0-1"
time $E bench $P --from 0 --to 1 --window 0,1 --quiet --study
