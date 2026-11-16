<scratchpad>/ex/cand1.exe bench tests/proof4/positions.txt --from 0 --to 24 --window 0,1 --quiet > work/evalorder/ident_proof4_cand1.txt &
<scratchpad>/ex/cand1.exe bench tests/set1/ply07.txt --from 0 --to 590 --quiet > work/evalorder/ident_ply07_cand1.txt &
frozen/climb-base-plain.exe bench tests/proof4/positions.txt --from 0 --to 24 --window 0,1 --quiet > work/evalorder/ident_proof4_base.txt &
frozen/climb-base-plain.exe bench tests/set1/ply07.txt --from 0 --to 590 --quiet > work/evalorder/ident_ply07_base.txt &
wait
