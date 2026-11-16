# E32 hand-over (evaluation as the move order)

## Round after the restart (agent 2, from 10:27)
- Task 1 (the order of red's tries at the 25 tune replies, offline): DONE as a first pass.
  Inputs `work/evalorder/top/` (tune25.txt copied from climb/replay; em.csv = fit3 evalmoves --nodes 1;
  kids.txt = fit3 children; port.txt = scratch `portmoves.exe refmoves`, built from detached worktree
  `wt/eoport` at climb/field 5e73da7 + an uncommitted `refmoves` command, never on a branch).
  Script `work/evalorder/scripts/toprank.py DIR [--perreply ORDERS] [--sim ORDERS] [--top N ORDERS]`;
  output `top/rank1.txt`. Labels: base 145 F, turns 70 F, paused 42 F (= recorded), 59 known winners, 0 conflicts.
- Task 1 numbers (rank1.txt). Pure orders (no killers/images/answer rule): label of the FIRST move, W/F/? over 25:
  model 15/3/7, port 5/1/19, lgA 3/2/20, lgB 2/2/21, W0 3/4/18, W1 4/4/17. Known failures ranked before the first
  known winner (sum): model 27, port 18, lgA 19, lgB 29, W0 16, W1 18 -- but with 826-1457 unlabelled moves before
  it for the eval orders (1172 for model, all in 7 replies), so these are not comparable: labels come from arms that
  ran model-based orders (selection bias toward the model). Lower bound on failures (known F before first non-F):
  model 4, port 1, lgA 2, lgB 2. Recorded: base 145, turns 70, paused 42.
  Driver simulation (`--sim`, chained, killers+images+answer rule) of model: 97 F + 20 unlabelled vs recorded 145:
  the sim is sequential, the driver opens tries in parallel once the root has 2 failures (cost rule in prove.rs),
  so recorded failures include tries run beside the winner.
  Reading so far: the pure model order's first move is a known winner in 15/25 replies (base arm: 7 first-try wins);
  the images bonus/killers, not the model, push winners back. No eval order can be judged without labels for its
  top moves. `top/unl_model.txt` lists the model's 7 unlabelled first moves (blue to move, label -1 = red wins):
  proving them (~50-230 s each on 16 threads) would fix the model's first-try rate.
- How the driver orders red's tries at a replay root (src/prove.rs new_node, climb/replay a762d50): `top.ordered_moves`
  (static+MODEL score at level>=7, threat-ruled moves dropped; hint = known move, killers = last two root winners
  handed down by --replay at TT_MOVE/KILLER scores), then +WINNER_BONUS (2^24) on images under the 12 symmetries of
  every earlier root winner (--win-cap N keeps N images, --win-recent K the K latest winners, --win-turns interleaves
  model and image lists as ranks), known results first, symmetry dedup; after a failed try the answer rule adds
  answer_bonus(1024)<<16 to moves covering the refuting reply (dominant over the model's 22..158 range).
  `--eval-order` does NOT reach this node: it is only in Solver::search step 4; ordered_moves is a separate copy.
  To reach it: add the use_eval branch (evalorder::score of the child + model/static terms) into ordered_moves.
- Task 2 step 2 DONE (node counts, valid despite the board's unlocked load): 48 val roots, cand3, L=11, vs off
  (from val1): lgA total 1.050, mean log +0.040 +-0.263; lgB total 1.093, mean log +0.071 +-0.285; 0 wrong.
  `fit/val2.jsonl`, `fit/val2_partial.txt` (results salvaged from the lane outputs after the stop; all 96 had finished).
  lgB does NOT hold out of sample: line stopped per the brief (0.656 in sample was overfit, like W1).
- Stopped 10:40 on the coordinator's order (owner uses the board). Nothing of mine running; no lock waiter of mine.
- Next (if resumed): label the model's unlabelled first moves, e.g. for each line of top/unl_model.txt
  `sh tools/locked.sh <main>/frozen/climb-base.exe prove FILE --threads 16 --job-threads 1 --depth 1 --tt 27 --upper-first --out OUT --deadline 170`
  (check option names in src/main.rs; resume via results file), then rerun
  `python work/evalorder/scripts/toprank.py work/evalorder/top --perreply model,port,lgA,lgB,W0,W1 --sim model,port,lgA,lgB,W0,W1 --top 5 model,port`
  with the new labels added as a fourth SRC.

# Earlier hand-over, stopped 2026-11-16 ~10:02 for a restart

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
