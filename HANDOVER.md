# E33 hand-over (branch `climb/tightq`, worktree `<scratch>/wt/tq`), stopped 2026-11-16 for a PC restart

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
