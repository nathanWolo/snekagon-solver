#!/bin/sh
# Sweep of reach threats on proof 0-11 (nodes and time), one config at a time.
E=work/study.exe
P=tests/proof4/positions.txt
for args in "" "--reach-threat 0" "--reach-threat 0 --reach-threat-work 200" "--reach-threat 0 --reach-threat-work 600" "--reach-threat 5" "--reach-threat 6" "--reach-threat 7" "--reach-threat 5 --reach-threat-work 600"; do
  echo "== $args"
  $E bench $P --from 0 --to 12 --window 0,1 --quiet $args
done
