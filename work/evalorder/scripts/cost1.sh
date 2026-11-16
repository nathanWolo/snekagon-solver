frozen/climb-base-plain.exe bench tests/proof4b/positions.txt --from 0 --to 32 --window 0,1 --cost-stats --quiet > work/evalorder/cost_base_0-32.txt &
<scratchpad>/ex/port.exe bench tests/proof4b/positions.txt --from 0 --to 32 --window 0,1 --cost-stats --ref-eval 11 --quiet > work/evalorder/cost_re11_0-32.txt &
<scratchpad>/ex/port.exe bench tests/proof4b/positions.txt --from 0 --to 32 --window 0,1 --cost-stats --ref-eval 9 --quiet > work/evalorder/cost_re9_0-32.txt &
wait
