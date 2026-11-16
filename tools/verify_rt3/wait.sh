#!/bin/sh
# waits up to 170 s for a new "exit" line in any group's progress file, then prints the last lines
cd <scratchpad>/wt/y/work
n0=$(cat g*/progress.txt | grep -c "exit\|done")
i=0
while [ $i -lt 30 ]; do
  n=$(cat g*/progress.txt | grep -c "exit\|done")
  [ "$n" -gt "$n0" ] && break
  sleep 5
  i=$((i + 1))
done
date +%T
for f in g*/progress.txt; do tail -n 1 $f | cut -c1-45; done
