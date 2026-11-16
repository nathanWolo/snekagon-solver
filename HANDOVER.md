# E33 hand-over (branch `climb/tightq`, worktree `<scratch>/wt/tq`)

## Round 2 state (2026-11-16, stopped at about 10:50 at the coordinator's word: the owner wants the machine)

Brief of round 2: correctness first (identity, `cargo test`, check build, `verify 100 1 8 8`, the margin-3
node count, the exactness argument), then the wider screen. Done so far:
- **`cargo test --release` on the current source (895536e): 3 passed** (`tight_question_verdicts_are_exact`,
  `sure_next_length_matches_brute_force`, `start3_counts_one_other_snake_per_piece`). Run by
  `sh tools/locked.sh sh e33/tq_build.sh test v4`; the plain and check builds it made (`tq-v4.exe`,
  `tqchk-v4.exe`) have the same sha256 as v3 (4b67d45c0887 / b7bac1879235): cargo found nothing to rebuild, so
  **tq-v3/tqchk-v3 are the current source**.
- **The exactness argument is written** (section below), with line numbers and the weakest step named.
- **Not done:** option-off identity on v3/v4 (only v1's, round 1), the check-build runs, `verify`, the
  margin-3 explanation, the whole screen. No timing was taken in round 2 (none is void).
- Note on `verify 100 1 8 8`: on this branch `verify` reads only GAMES and SEED (`src/main.rs` 561-565), so
  `8 8` is ignored here and it is `verify 100 1` (the HAND3/THREAT3 arguments exist on `climb/threats`).
  It does not take the rule's options; it checks G against the brute force at every position with <= 8 left.

What was running when stopped: `python e33/tq_run.py e33/jobs6.txt` (identity + margin-3 per position), still
waiting in the lock's queue; stopped by me, no job had started, no output file written (nothing partial).
`e33/jobs7.txt` (check-build probe: proof4 position 0, bench_ply5_blue_wins 0-3) is written, not run.

**Next commands** (from `<main>`; each job is one lock hold of about 35 s; `tq_run.py` skips done jobs):
1. `python <scratch>/wt/tq/e33/tq_run.py <scratch>/wt/tq/e33/jobs6.txt` — expect e6_id_p0 26,095,441 and
   e6_id_ply07 4,174,807; e6_m3 vs e6_m99 vs e6_id_p0 per position (non-quiet: column 5 = nodes) to find
   where margin 3 searches more than the base; e6_off_nco vs e6_m3_nco: the same with E30's cost order off
   (the verifier of E29 explained a node rise by the cost history this way).
2. `python <scratch>/wt/tq/e33/tq_run.py <scratch>/wt/tq/e33/jobs7.txt` — time the check build; stderr
   (`#ERR` in the output file) has the counters. Then chunk proof4 0-23, set1 ply05-ply08 tune-range
   positions and the five `bench_ply5_*` files so each hold stays under ~2 min; settings: the chosen one
   (`--tight-q 6 --tight-q-min 6 --tight-q-hand 3`), the margin-3 one, and the most-firing one
   (`--tight-q 8 --tight-q-hand 4`, or `--tight-q 5` hand 2, 8.4M asked on proof4 0-23).
3. `sh tools/locked.sh <scratch>/wt/tq-v3.exe verify 100 1 8 8`.
4. Screen: `python tools/run.py E33-S6 tune --set proof4 --start 12 --units 32 --cand <scratch>/wt/tq-v3.exe
   --base frozen/climb-base-plain.exe --cand-args "--window 0,1 --tight-q 6 --tight-q-min 6 --tight-q-hand 3"
   --base-args "--window 0,1"` (proof4 tune = units 0-43, positions 0-87); likewise `--set proof4b`
   (tune units 0-87), `--set ply05` (full window); fail4 0-19 by hand (all red to move, `--window 0,1`).
   Then `python tools/hc.py report runs/KEY.jsonl --ratio --lower-is-better` and `hc.py plan`.

## Round 1 state (stopped 2026-11-16 for a PC restart)

`<scratch>` = `<scratchpad>`.
`<main>` = the main checkout (branch `parallel-v9`). Outputs: `<main>/work/tightq/` (one file per run, bench stdout).
Brief: the coordinator's E33 task (the tight question made cheap, judged in place). Read it again; this file is the state.

## The rule as it stands (code written from the rule's statement, no port or reference code)

- `src/bounds.rs` `sure_next_length(t, pos, floor, cand, why)`: G = the opponent's sure next length (after any legal
  move of ours it still has a placement making its longest snake >= G). Returns G if G >= floor, else a number below
  floor. Collects opponent extensions of length >= floor (grow-only generation when floor > largest piece; cap
  `MAX_EXT` = 48, longest kept), then finds our legal moves that block every extension of the top length (generated
  only among moves hitting the smallest block set), filters level by level. Soundness argument in its doc comment
  (passes, no pieces, new snakes, subset of extensions only lowers G). `sure_next_length_brute` = brute force.
- `src/solver.rs` step 2c (just before `let (alpha0, beta0)` in `search`) calls `tight_question` (argument in its doc
  comment): delta = [alpha >= 0]; floor = (what the mover is known to reach: longest now or a witness at this ply /
  the ply above) - delta + 1, raised to `U - delta - margin` when a margin limit is set; target = G + delta; decided
  if the mover's cluster bound U (when known, i.e. hand > reach_hand) < target, else (hand <= `tight_q_hand`, margin
  U - target <= `tight_q_margin`) by `can_reach_touching_w` with `tight_q_work` answering "cannot". Decided: store
  [lo, alpha], return alpha (fail low).
- Options (all off by default): `--tight-q N` (asked at nodes with <= N pieces left, 0 = off), `--tight-q-min K`
  (and >= K), `--tight-q-hand H` (reach question only for mover hand <= H; default 2), `--tight-q-work W` (default
  200), `--tight-q-margin M` (default 99 = no limit), `--tq-study` (rdtsc cycle counters; prints `# TQ` lines:
  asked, G>=floor, decided by cluster, reach questions, decided by reach | cycles/asked in G, cycles/reach
  question, cycles/undecided asked | exits of G [0 cap, 1 no extension, 2 top length, 3 shorter length, 4 blocked
  below floor, 5 pass ends]; and `# TQM` reach questions by margin [questions, decided, cycles]).
- `src/tqcheck.rs` (`--features tqcheck`): at every decided node, G vs brute force, the "cannot" answer with
  unlimited work and (hand <= 2, hand 3 sampled 1/16) vs `one_player_optimum`, and a re-search by a second solver
  (rule off, own table) in the window (claim, claim+1); aborts on any mismatch; counters on stderr at the end of
  `bench`. At undecided asked nodes G vs brute force sampled 1/8 (`TQCHECK_GSHIFT`). The null-window re-search is
  in tqchk-v3 (v1/v2 check exes re-search with the full window).
- Unit tests in `src/solver.rs` (`sure_next_length_matches_brute_force`, `tight_question_verdicts_are_exact`:
  random positions, G exact vs brute force at several floors; verdicts with unlimited work vs exact solves; whole
  solves with the rule at every node vs without, windows (-1,1), (-1,0), (0,1)). `verify` also checks G vs brute
  force at every position with <= 8 pieces left (no random numbers drawn, so the game sequence is unchanged).

## The claim and why it is exact (for a verifier who has not seen the code; round 2, 2026-11-16)

Line numbers are those of commit `895536e` (the source of tq-v3/v4).

**Where.** `src/solver.rs` 1534-1549, step 2c of `search`, after the table probe (1367-1384), the
cluster bounds (1386-1504) and the step 2b reach tests (1506-1533), before the move loop. It is
asked when `--tight-q N` is set, `tight_q_min <= level <= tight_q` (level = pieces left, both sides),
`reach_on`, and the opponent holds a piece. Note alpha >= -1 there: lo starts at -1 (1368) and
`alpha = alpha.max(lo)` (1503); beta <= hi <= 1. Values are -1, 0, 1 (`game_result`, 480-484:
sign of mover's longest snake minus the opponent's at the end of the game).

**The claim** (`tight_question`, 1113-1204). If it returns true, the node's value is <= alpha. The
caller returns alpha and stores the interval [lo, alpha] (1546): an upper bound of alpha.

**The argument.** Let ours/theirs = the two final longest snakes under any continuation.
1. *A lower bound G on theirs* (`sure_next_length`, `src/bounds.rs` 578-743). Exact definition: G =
   min over our legal moves m of max over the opponent's placements e legal after m of
   longest_after(e). Facts it uses:
   a. *Rules* (`pos.rs` 217 `legal_placement`; `naive.rs` 40-70, the rulebook version that `verify`
   compares against on every position): a placement is legal iff its cells are empty, and either it
   touches no tile at all (a new snake: border free of every tile), or it touches an own cell and
   the own cells plus it still form snakes (only own cells matter for that). So our move m can make
   an opponent placement e illegal only by covering a cell of e (grow), or a cell of e or its border
   (new snake). That set is e's `block` (bounds.rs 659 for the grow-only generator: cells; 669:
   cells, or cells|border for a new snake). A move that does not intersect e's block leaves e legal.
   The code treats every intersection as a block (bounds.rs 710, 734), which can only lower G.
   b. *The game goes on after m*: m is a placement, so passes = 0, and the opponent holds a piece
   (checked at 1535 and bounds.rs 584), so `is_over` (`pos.rs` 102: two passes or both hands empty)
   is false and the opponent may play e. If we have no legal move we pass: with passes = 0 now the
   game goes on and every e stays legal (G = top, 717-724); with a pass already made our pass ends
   the game and G = their longest now (719-721).
   c. *m changes nothing of the opponent's* (`pos.rs` 124-157: play adds our cells only), so
   longest_after(e) computed now is the opponent's longest after e at the child; snakes never shrink
   (152), so theirs >= that.
   d. *Lower bound, not equality, is all that is needed*, and every shortcut lowers it: extensions
   below `floor` are not collected (607-610, 641, 658, 668); at most `MAX_EXT` = 48 are kept, longest
   first (614-628); a length class is "blocked" only when some candidate move meets every kept
   extension of every length down to it (726-740); otherwise the return is `now` = their longest now
   (always a lower bound). Our candidate moves are generated exactly (694-715: every legal anchor of
   every orientation of every piece we may place now, `pos.pieces_now()`, filtered to those with a
   cell in the smallest block b, `hit` a superset since cell k sits at anchor index + shift[k] with
   shift >= 0, pieces.rs 36/122-124; then checked exactly against all top-length blocks). A missed
   legal move of ours would *raise* G: this is the one place where an error makes the rule unsound.
   So: G returned >= floor implies G <= true G <= theirs.
2. *An upper bound on ours*, two ways, both facts proved earlier:
   a. the cluster bound U (1536-1544: max of the longest snake and every cluster's upper; equal to
   `upper_bound_ex`'s return, bounds.rs 61-162, which the base already uses as the mover's upper
   bound at 1426 and 1475-1491; `verify` checks it against `upper_bound_reference` and that it never
   rises along a game). Only used when the list did not overflow.
   b. the reach test `can_reach_touching_w(..., target)` answering false (1185; bounds.rs 319-): the
   mover cannot build a snake of `target` even if the opponent never moved again; the opponent's
   tiles only make our placements illegal (rules in 1a), so ours < target in the real game too. Out
   of work answers true (no decision). This is the same function step 2' (1404) and the E29 reach
   threats use; its "cannot" was cleared by the E29 verifier (LOG, "E29, the verifier's verdict").
3. *The verdict.* delta = [alpha >= 0] (1119), target = G + delta (1160). Decided iff U < target
   (1161-1167) or the reach test says "cannot" for target (1184-1203). Then ours <= target - 1 =
   G + delta - 1 <= theirs + delta - 1. alpha >= 0: ours <= theirs, value <= 0 <= alpha. alpha = -1:
   ours < theirs, value = -1 = alpha. Either way value <= alpha.
4. *What only decides how often it fires* (no effect on soundness): `known` and the witness
   (1121-1133), the `floor` and the margin raise of the floor (1134-1141), the hand limit (1169),
   the margin limit (1176-1182), the work limit (1184). All of them only make the rule return false.

**The weakest step (named by me).** 1d, the completeness of our move generation inside
`sure_next_length` (the `hit` prefilter at 702-706 plus `legal_anchors`): if one legal move of ours
that blocks every top extension were not generated, G would be one or more too high and the rule
would cut wrongly, and node counts and root labels would not show it. It is checked against the brute
force (every legal move of ours from `pos.moves`, then every placement of the opponent) at every
decided node of the check build, on a 1/8 sample of undecided asked nodes, by the unit test
`sure_next_length_matches_brute_force`, and at every position with <= 8 pieces left in `verify`. The
second weakest: 1b at the end of the game (a pass ends it only with a pass already made; G = `now`
there), which the brute force (`sure_next_length_brute`, 748-773, plays the pass) also covers.

## Exes (`<scratch>/wt/`, all plain MSVC builds via `e33/tq_build.sh`, none profiled)

| exe | source | sha256 |
| --- | --- | --- |
| tq-v1.exe / tqchk-v1.exe | commit ea239d3 (rule, tests, check build) | 585ee9910637 / 3543e104b3bb |
| tq-v2.exe / tqchk-v2.exe | ea239d3 + `--tight-q-min` + G exit counters | f91e9ea48e60 / 84da5746b471 |
| tq-v3.exe / tqchk-v3.exe | v2 + `--tight-q-margin`, margin counters, floor raised by margin, null-window re-search | 4b67d45c0887 / b7bac1879235 |

The WIP commit holding all of this is the one this file is committed in (v3 source = that commit's `src/`).

## Correctness state

- Flag off (tq-v1): proof4 0-23 `--window 0,1` 26,095,441; ply07 0-589 4,174,807; blue_wins 6,839,041 = base.
  Every flag-off run of v2/v3 is not re-checked (only option parsing added; v3 with `--tight-q 6 --tight-q-min 6
  --tight-q-hand 3` gives 22,770,019 = v2's, consistent).
- `cargo test --release` (v1 source): 3 passed. Not re-run on v3.
- `verify 100 1 8 8`: NOT run yet. Check build runs: NOT run yet. Every screen run: 0 wrong labels (no evidence).

## Screens (proof4 positions 0-23, i.e. `--from 0 --to 24 --window 0,1`, plain, one thread; times one read each)

Base (climb-base-plain, 26,095,441 nodes) read 34.8/35.2 s (s1), 34.3/34.1 (s2), 33.5/35.1 (s3), 37.1/37.3 (s4),
35.5 (s5): the machine drifts 33-37 s; compare with the base runs of the same batch.

| file | setting | nodes | s | note |
| --- | --- | --- | --- | --- |
| s1_q5_p0, s1_q5n_p0 | q5 (levels 0-5), hand 2 | 23,761,872 | 35.9, 35.3 | 8.4M asked, no gain |
| s1_q5h3_p0 | q5 hand 3 | 22,341,296 | 33.6 | |
| s2_q5m4h3_p0 | q5 min4 h3 | 23,723,331 | 33.4 | |
| s2_q5m5h3_p0 | q5 min5 h3 | 25,526,434 | 36.1 | |
| s2_q6m4h3_p0 | q6 min4 h3 | 20,786,614 | 31.2 | |
| s2_q6m5h4_p0 | q6 min5 h4 | 22,332,760 | 30.35 | base 34.3/34.1: 0.887x |
| s3_q6m6h3_p0 | q6 min6 h3 | 22,770,019 | 30.6 | base 33.5/35.1: 0.89x |
| s3_q7m6h4_p0 | q7 min6 h4 | 22,487,770 | 34.4 | level 7 decides 255 of 175k |
| s3_q8m6h4_p0 | q8 min6 h4 | 22,821,119 | 33.2 | |
| s3_q7m7h4_p0 | q7 min7 h4 | 25,921,491 | 38.0 | level 7 alone loses |
| s4_m6w50/100/400/1000 | q6 min6 h3, work 50/100/400/1000 | 25.07M/24.58M/23.16M/23.11M | 37.1/35.4/34.2/33.1 | base 37.1/37.3; work 200 best |
| s4_m6h4 | q6 min6 h4 | 22,770,019 | 32.85 | base 37.1/37.3: 0.88x |
| s5_m99 (v3) | q6 min6 h3 | 22,770,019 | 32.07 | base 35.5: 0.90x |
| s5_m1/m2/m3 | + margin 1/2/3 | 25.49M/25.71M/26.44M | 35.3/35.2/36.7 | margins this small lose |

Cost per asked node (`--tq-study`): level 6: G 4,000-4,300 cycles, a reach question (hand 3) 19,700-21,000,
undecided asked 8,000-9,600; of 1.31M asked, 1.17M reach the floor, 193k decided by the cluster bound, 977k reach
questions, 489k decided (total decided 682k, 52%). Levels 3-4 (hand 2): G 1,300-1,800, reach 1,250-2,050, decided
about 87% of those with G >= floor, but each decision saves about one node: no gain. Level 5: G about 2,000, reach
(hand 3) 2,700-3,000, decided 29%. Level 6 reach questions by margin U - target (s5_m99):
[questions, decided, cycles] 0: 111k/87k/21k, 1: 141k/105k/33k, 2: 63k/47k/27k, 3: 33k/21k/20k, 4: 188k/128k/27k,
5: 135k/76k/26k, 6: 38k/16k/18k, 7+: 267k/9.5k/3k. So a margin limit should be about 6, not 1-3 (s5_m3's node rise
above the base is an order effect of fewer prunes; not explained further).

Current reading: it pays only at level 6 (5 is neutral, 3-4 and 7 lose): `--tight-q 6 --tight-q-min 6
--tight-q-hand 3` is 0.873x nodes and about 0.88-0.90x time on proof4 0-23 (single reads; 24-47, proof4b,
ply-5, fail4 not run yet). The asked node at level 6 costs far more than 1.2x a node; it pays because decided
level-6 nodes save whole subtrees.

## What was running when stopped

`python e33/tq_run.py e33/jobs5.txt` (margin screen): s5_base_p0, s5_m99, s5_m1, s5_m2, s5_m3 finished (files
complete and usable); s5_m2n and s5_base2_p0 not run (killed while waiting for the lock; `tq_run.py` skips jobs
whose output file exists, so rerunning the job file resumes).

## Next steps (commands from `<main>`, every run through the lock; builds with `sh e33/tq_build.sh [test] TAG`
from the worktree via `sh tools/locked.sh sh e33/tq_build.sh notest v4` — it writes `<scratch>/wt/tq-TAG.exe` and
`tqchk-TAG.exe`)

1. Screen margin 5/6/7 at q6 min6 h3 (and h4), and `--tight-q 6 --tight-q-min 5 --tight-q-hand 3`; write a job file
   like `e33/jobs5.txt` (format `OUT|EXE|bench args`) and `python <scratch>/wt/tq/e33/tq_run.py FILE`.
2. Pick the setting; confirm flag-off identity on v3+ (proof4 0-23 26,095,441; ply07 0-589 4,174,807);
   `cargo test --release` (`tq_build.sh test`); `verify 100 1 8 8` with the plain exe (under the lock).
3. Check build (`tqchk-*.exe bench ... <setting>`, stderr has the counters, any mismatch aborts): proof4 0-47
   `--window 0,1` in chunks of 1-3 positions (keep each lock hold under about 2 minutes), the five
   `tests/set1/bench_ply5_*.txt` files, `tests/set1/ply06.txt` 0-199 (full window).
4. Measure vs `frozen/climb-base-plain.exe` (base, cand, cand, base): proof4 0-23 and 24-47; `tests/proof4b`
   positions 0-175 (tune units 0-87, a unit = 2 positions; per-unit `{"cand": s, "base": s}` lines into
   `python tools/hc.py report FILE --ratio --lower-is-better`, for time and nodes; per-position times from the bench
   lines, column 6 = microseconds, column 5 = nodes); `tests/set1/ply05.txt` 0-29 (three tune units, full window);
   `<main>/work/evalorder/fail4.txt` (red-to-move jobs with `--window 0,1`, blue-to-move with `--window -1,0`; jobs
   cost 6-40 s each: take a subset); N in {3,4,5,6} (as `--tight-q-min N --tight-q N` and `--tight-q N`).
5. Never touch gate data (proof4b units 88-442, proof4 units 164-220, work/e24/g-r, set1 gate/confirm/held-out
   ranges); no tools/run.py or provegate.py.
