# E34 hand-over (end a failed red try sooner), branch `climb/failtry`

Stopped 2026-11-16 for a PC restart. Brief: coordinator's E34 brief (LOG, "E34: end a failed try
sooner"). Worktree `<scratchpad>\wt\ft`.
`$W` below = `<main checkout>/work/failtry` (main checkout: `...\scratch-2026-11-14-446a6d\snekagon`).
Run every command from the worktree root.

## Commits

- `a5a9247` `prove --job-log FILE` (measurement only) and `tools/joblog.py` (reads it).
- `b4d4015` `prove --hold S` (the change, off by default).
- this commit: `work/checks.sh`, `work/checks2.sh`, `work/pairs10.py`, this file.

## Executables (in `$W`)

| exe | source | build | id |
| --- | --- | --- | --- |
| `log-1.exe` | `a5a9247` (+ the `P` marker line, profile from the source just before it) | `--pgo` | `dc54e4b0ef94` |
| `hold-1.exe` | `b4d4015` | `--pgo` | `f397bcee0553` |

## Step 1, the measurement (done; one run, tune data)

`sh tools/replay.sh $W/log-1.exe $W/tune4.txt $W/log4 --job-log $W/log4/jobs.txt` (roots 0-3 of
`tune25`). Summary: 30 tries, 26 failed, 1,115 s wall, 17,414 thread-s, all certificates ok
(recorded base arm roots 0-3: 1,233 s). Table: `$W/log4/joblog_table.txt`
(`python tools/joblog.py $W/log4/jobs.txt --tries`).

- The machine is full the whole time: 15.0 of 16 workers busy while any refuting job runs.
- 26 failed tries: 8,200 thread-s. Refuting jobs themselves 1,214 s. **Red-win jobs under the same
  failed try: 1,827 thread-s before its refuting job started, 4,995 while it ran** (83% of the failed
  tries' thread-s). Thrown away when the try fails.
- Two regimes. (i) While the root has fewer than 2 failed tries, a try's replies all go out once its
  first comes back (need 1): the refuting reply is often deep in blue's order (places 6, 16, 17, 21,
  37, 59, 100, 513; start 9-157 s after the try's first job) and up to 15 workers prove red wins
  under the try meanwhile. These are the expensive failures (e.g. root 3 try 4B:71,80,81,90: 181 s,
  2,899 thread-s, refuting reply place 513, ran 92 s). (ii) After 2 failures the root counts as
  "every move needed": 16-17 tries live at once, each failing try is one refuting job at place 0-3
  on one worker, little same-try waste.
- Winning try alone (its jobs' thread-s): 1,280 / 1,538 / 741 / 1,861 at roots 0-3, i.e. about
  80, 96, 46, 116 s on 16 workers, against 109, 186, 50, 770 s per root.
- Reading: the hypothesis holds where it matters (regime i); the stop rule did not fire.

## Step 2, the change (built)

`--hold S`: a coordinator node one of whose jobs has run >= S seconds opens no new move while any
other work can be handed out; if nothing else is left the held nodes' moves go out (counted, printed
as `# hold S: N jobs went out from held nodes`). Only the order of hand-outs changes; windows,
stored values and pruning are untouched. Code: `src/prove.rs`, `find_work` (the `self.holding`
block) and `pass` (two find passes). Chosen over (b) split and (c) helpers because the waste is
same-try red-win jobs on a full machine (wall follows thread-seconds), not idle time: shortening the
refuting job does not cut that work by itself, and the earlier `--split` showed coordinator load.
Known cost: a winning try with a long red-win job is held too (seen at root 2 below). From the base
arm's results file, seconds a hold would spend past S on red-win vs refuting jobs: S=10 10,883 vs
9,325 (P(refute | past S) 0.11); S=20 5,147 vs 7,917 (0.33); **S=30 3,302 vs 6,659 (0.48)**.

## Step 3, checks (done unless said)

- Option off: proof4 0-23 `--window 0,1` 26,095,441 (both builds' profile runs); ply07 0-589
  inclusive (`--to 590`) 4,174,807 on `hold-1.exe` and on `frozen/climb-base-plain.exe`
  (`--to 589` gives 4,173,893 on both). `cargo test` passes (1 test), run under the lock with
  `--target-dir` in the short build directory.
- Option on (`--hold 0.001`, 16 threads): the five `bench_ply5_*` files at depths 1, 2, 3 all
  right (`$W/checks.txt`); the hold fires (14,332 fallback hand-outs at depth 1, 3,107 at depth 2 on
  `ublock_refutations`, `$W/checks2.txt`). A results file cut at 3 s (15 of 30 done) resumed and
  finished, all right (9,911 positions read).
- Replay certificates: `log4` all ok. `hold10` not summed yet (unfinished).

## Step 4, the screen (cut short)

Running when stopped: `sh tools/replay.sh $W/hold-1.exe $W/tune10.txt $W/hold10 --hold 10 --job-log $W/hold10/jobs.txt`.
Pieces 1-2 done; I stopped the solver during piece 2 (688 s) inside root 6 (232 s in) and killed
`replay.sh`. `$W/hold10/STOP` exists: **delete it before resuming**. The partial output is usable:
the replay resumes from `results.txt` (root 6 starts again cold; its lost seconds are added by
`replay_sum.py`).

Roots done with hold 10 against the recorded base arm (wall s; one run each, tune data):

| root | base4 tries/failed, wall | hold10 tries/failed, wall | ratio |
| --- | --- | --- | --- |
| 0 | 2/1, 96.0 | 2/1, 98.3 | 1.02 |
| 1 | 4/3, 210.6 | 13/12, 204.9 | 0.97 |
| 2 | 1/0, 66.9 | 1/0, 157.4 | 2.35 (the winner held behind its own 60 and 94 s red-win jobs) |
| 3 | 27/26, 809.4 | 39/38, 632.0 (439.1 + 192.9, cut once) | 0.78 |
| 4 | 4/3, 242.8 | 2/1, 143.1 | 0.59 |
| 5 | 1/0, 93.6 | 2/1, 117.2 | 1.25 |
| 0-5 | 1,520 | 1,353 | 0.89 (inside the 10% no-read band) |

## Next steps

1. Finish the hold 10 screen: `rm $W/hold10/STOP; sh tools/replay.sh $W/hold-1.exe $W/tune10.txt $W/hold10 --hold 10 --job-log $W/hold10/jobs.txt`
   (about 30-40 min of locked time; it ends with `summary.txt`, check "all ok").
2. Second setting, S=30 (better targeted, see step 2): `mkdir -p $W/hold30; sh tools/replay.sh $W/hold-1.exe $W/tune10.txt $W/hold30 --hold 30 --job-log $W/hold30/jobs.txt`.
3. Compare root by root with the recorded base arm (whose summary indexes 25 roots):
   `python work/pairs10.py $W/hold10/summary.txt <main>/work/replay/base4/summary.txt 10 wall > $W/pairs_hold10_wall.jsonl`
   (also `thread`), then `python tools/hc.py report $W/pairs_hold10_wall.jsonl --ratio --lower-is-better`.
   Per-root seconds lost on stopped jobs are in the summaries' `stopped_s` column;
   `python tools/joblog.py $W/hold10/jobs.txt --tries` shows where failed tries' seconds went.
4. Report to the coordinator (no gate, no freeze, no merge by this line).
