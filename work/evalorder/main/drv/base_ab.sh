for j in a b; do rm -f work/evalorder/drv/base_$j.res; <scratchpad>/ex/candem3.exe prove work/e24/$j.pos --threads 16 --job-threads 1 --depth 1 --tt 27 --emulate 16 --out work/evalorder/drv/base_$j.res > work/evalorder/drv/base_$j.out 2> work/evalorder/drv/base_$j.err & done
wait
