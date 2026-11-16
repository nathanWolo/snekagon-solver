# E32 hand-over (evaluation as the move order), stopped 2026-11-16 ~10:02 for a restart

Branches: `climb/evalorder` (candidate, this worktree `wt/eo`), `climb/evalorder-fit` (candidate + `snek evalmoves`
study command, worktree `wt/eofit`), `eo-emulate` (candidate + cherry-pick of `ab3bf7e` `--emulate`, conflict
resolved by hand, scratch only). Data copied into `work/evalorder/` of this worktree (`scripts/`, `fit/`, `main/` =
copy of the main checkout's `work/evalorder/`).

## The option
- `--eval-order L`: at nodes with >= L pieces left the listed moves (after the staged first move) are ordered by
  `evalorder::score(child) + w[10]*model_score + w[11]*static_score/64` (src/evalorder.rs, src/solver.rs step 4).
  Table move and killers keep their scores; game-over children get +-2^19. Answer bonus at those levels:
  `--eval-bonus` (default 1<<21, dominant). Off by default (L = 99).
- `--eval-w` 12 integers: est, ub, long, terr, hand, ends, short, fresh, snakes, terrb, model, st. Each term is
  mover minus opponent in the child: est = estimated final longest snake (independent-distance race, ties to
  side to move, capped by cluster bound); ub = cluster upper bound; long = longest now; terr = cells reached
  first (<=40); hand = cells in hand; ends = snake ends with usable free neighbour; short = hand cells the best
  snake has no room for; fresh = room for a new snake; snakes = number of snakes; terrb = race by turns with
  blocking (<=40); model = base's MODEL score of the move; st = static score (size*64+...)/64.
  Default WEIGHTS = model 64, rest 0 (= today's order, apart from the bonus).
- Identity (cand1 = commit be834df-era build, option off): proof4 0-23 (`--to 24`) 26,095,441; ply07 `--to 590`
  4,174,807; same as `frozen/climb-base-plain.exe`. NOT yet re-run on cand3 / final commit. `cargo test` NOT run.

## Exes (scratchpad `ex/`, will be lost; all plain MSVC via tools/build.sh, none profiled)
- port.exe 055e274207ce: climb/field 5e73da7 (the yardstick port; hash differs from 9a178bd705d8, same source)
- cand1 2abea5ae9023 (be834df), cand2 e0ad11bf8dfc (4cd5c48), cand3 ceafd53c992d (db4959e, current terms)
- candem3 1b55ea36741a: eo-emulate 813f7dd (candidate db4959e + --emulate)
- fit3 a9c181f116ea: climb/evalorder-fit (evalmoves)

## Results
1. Yardstick, port vs base, proof4b tune 0-175 (88 units), per-position processes, window 0,1, alternated
   (`main/yard_p4b.jsonl`). Time, cand/base: re9 total 0.708, per-unit geo exp(-0.378)=0.685 [0.honest
   interval exp(-0.535)..exp(-0.222) = 0.586..0.801], sd/unit 0.75; re11 total 0.737, exp(-0.356)=0.701
   [0.600..0.818]; re7 total 0.766, 0.763 [0.651..0.894]. Nodes: re9 0.683, re11 0.724, re7 0.665. Wrong 0.
2. Fail jobs (red does not win): `main/fail4.txt` (103 jobs, sorted by replay nodes, label -1). Cheapest 20
   (0-19, 181.8M nodes): port re9 nodes 0.956, time 1.020 total (per-job sd 0.40); re11 nodes 0.993.
   Left out: jobs 20-102 (>12.7M nodes each).
3. Where the port gains (cost-stats, proof4b 0-31 one process, `main/cost_*_0-32.txt`): base 37.26M, re11
   22.10M. At the root (side 0, 11 left) first-try wins 68.8% -> 84.4%, failed-move nodes 11.54M -> 5.96M,
   winning-move nodes 25.71M -> 16.14M. So two thirds of the gain is a cheaper win, one third fewer failed
   root moves. The root (L=11) alone carries most of it.
4. Offline fits on the order study's level-9 labels (fit/n9*, from wt/o) give nothing out of sample
   (fit9a.txt: test 1.591M vs model 1.622M; logistic test log loss = base rate). Root labels by solving every
   child cost ~1G nodes/root: infeasible.
5. In-tree coordinate search at L=11 on 48 fit roots (`fit/roots.txt` = kids of the 38 train parents of
   climb/order, none in proof4/proof4b; `scripts/tune.py`, `fit/tune3.jsonl`): best
   W1 = -40,0,64,96,0,64,40,128,0,-24,64,64 reads mean log -0.44, total 0.77 in sample; on 48 val roots
   (`fit/roots_val.txt`, test parents) -0.026 / 0.935; on proof4b 0-175 nodes: mean log -0.127, total 1.038
   (W0 = 0,0,0,96,0,0,0,128,0,0,64,0: -0.103 / 1.022; W1 at L9: -0.126 / 1.007). OVERFIT; no gain.
   Port on the same roots: fit 0.876 (re11) / 0.747 (re9) total; val 1.320 / 0.990. 48 roots are too few.
6. Logistic weights (fit/logit*.txt, level-9 labels, x1000): lgA (pure position) = -29,327,337,69,-373,41,21,
   -11,-373,-3,0,0; lgB (with model, st) = -23,353,204,62,-474,-52,25,-4,-812,2,2,-130. On fit roots total:
   lgA 0.754, lgB 0.656, lgA at L9 0.745 (port11 0.876). Val roots run was INTERRUPTED (runAB.sh second line,
   no output). Signs agree with the port's (ub+, long+, hand-, snakes-).
7. Driver read: base only, candem3 option off, `prove work/e24/{a,b}.pos --threads 16 --job-threads 1 --depth 1
   --tt 27 --emulate 16`: a 152,253,809 nodes (209.8 s), b 326,125,056 (425 s). `main/drv/`. No candidate yet.

## Reading
No setting of ours is worth a gate yet. lgB is the only promising one (0.656 on fit roots) and is untested out of
sample. The port's 0.70x on proof4b is real on cold positions; the driver hands down killers that override the
order at job roots (LOG: 96% first-try wins in the driver), so expect much less there.

## Rule notes
Builds all through tools/build.sh. `cargo +stable-x86_64-pc-windows-msvc check` was run several times outside
the lock (type check only, no link, a few seconds each) - that is probably the cargo/rustc seen at 04:46. Driver
holds were 7 min (b). Two of my waiting runs were killed from Windows (their lock tickets expired on their own).

## Next steps
1. Rebuild: `sh tools/build.sh ../../ex/cand3.exe` in wt/eo (and port in wt/f, candem in eo-emulate).
2. Val roots for lgA/lgB: `python work/evalorder/scripts/tree.py EXE work/evalorder/fit/roots_val.txt 0 48 16
   OUT.jsonl off= "lgA=--eval-order 11 --eval-w <lgA>" "lgB=--eval-order 11 --eval-w <lgB>"` (MAIN path is
   hard-coded in tree.py; fix the scratch paths).
3. If lgB holds: proof4b 0-175 nodes (tree.py), then timed paired screen (yard.py, base vs cand, chunk ~10),
   fail4 0-19, ply05 tune (L=11 never fires at ply 5: use L<=10 to read scaling), driver read a-d with
   candem + `--eval-order 11 --eval-w ...`, identity on the final commit, cargo test.
