#!/bin/sh
E=work/study3.exe
P=tests/proof4/positions.txt
S=<session-folder>/snekagon/tests/set1
echo "== study proof 24-47"
$E bench $P --from 24 --to 48 --window 0,1 --quiet --study
echo "== study ply06 0-199"
$E bench $S/ply06.txt --from 0 --to 200 --quiet --study
