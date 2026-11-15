#!/bin/sh
# Where reach threats spend: profile with counters, proof 0-11.
E=work/study3.exe
P=tests/proof4/positions.txt
echo "== --reach-threat 0 --profile"
$E bench $P --from 0 --to 12 --window 0,1 --quiet --reach-threat 0 --profile
echo "== --profile (off)"
$E bench $P --from 0 --to 12 --window 0,1 --quiet --profile
