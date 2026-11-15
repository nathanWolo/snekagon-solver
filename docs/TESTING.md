# The testing contract

How a change becomes part of the solver. Changes to this file are logged at the bottom.

## The evaluator

- **A unit** is 10 consecutive positions of one test file, each solved exactly from an empty table by the
  base and by the candidate, one process at a time, in interleaved chunks of 10 units (base first, then
  candidate first, and so on).
- **The score of a unit** is log(base seconds / candidate seconds), summed over the unit's positions
  before the ratio is taken. Positive means the candidate is faster. Node counts are recorded beside
  the times; they are exact and repeat on any machine.
- **Correctness comes first:** every position's value must equal its label, in both arms. One wrong or
  missing value blocks the run.
- **Environment pin:** see `CLAUDE.md`. Runs record the SHA-256 of both executables. On the
  climb (branch `parallel-v9`) a deciding read compares two profiled MSVC builds
  (`sh tools/build.sh --pgo`), because that is the build a long proof runs; screens may use
  plain MSVC builds on both arms.

| Measured | Value | When | How |
| --- | --- | --- | --- |
| A/A (base against itself) | nodes identical; time -0.01% [-0.80%, +0.79%] (ply 7), +0.43% [0.00%, +0.85%] (ply 8) | 2026-11-14 | `run.py AA-1 aa --set ply07`, `AA-2 aa --set ply08` |
| Timing noise per unit | sd 0.012 to 0.022 (log ratio); a floor only | 2026-11-14 | the two A/A runs |
| Known-answer check | bounds switched off read -53.5% [-61.3%, -44.2%] and failed `SPRT[0, 0.05]` at unit 30; the reverse read +111% and passed at unit 30 | 2026-11-14 | `KNOWN-WORSE-1`, `KNOWN-BETTER-1` |
| Throughput (first solver) | 30 units of ply 7 take 15 s an arm | 2026-11-14 | `AA-1` |

Noise for planning comes from each candidate's own screen, not from the A/A: a real change speeds some
positions up and slows others down.

## The test set

`tests/set1/`, built by `tools/mktests.py` from the reference repository's published data (commit
`61eb3a4`); checksums in `tests/set1/MANIFEST.txt`. 22,780 labelled positions after 5 to 11 moves, in a
pinned shuffled order, one file per ply, plus the reference's 126 benchmark units after 5 moves.

Each ply file is cut by unit index: the first 15% tune, the next 60% gate, the next 15% confirm, the
last 10% held-out (`tools/run.py` computes the cuts).

| Range | Used for |
| --- | --- |
| tune | developing, screening, profiling. Use freely. |
| gate | the one deciding test of each idea; each unit is used once |
| confirm | sizing accepted changes at milestones; each unit is used once |
| held-out | a rare check that the climb is real. Nothing is tuned on it. |

`tests/ledger.jsonl` lists the units every deciding read used; the runner takes the next unused ones and
refuses a key that was used before.

**Which ply file.** Gate on the file where the base averages between about 5 ms and 500 ms a position.
As the base gets faster, move to the next harder file and say so in the log. A change is also read on
the tune units of the next harder file before it is frozen (see the scaling check).

**The proof benchmark** (`tests/proof4/`, 2026-11-15; `tools/mkproof.py` from the results file
of the paused proof of the centre opening). 592 positions four moves in, red to move, each a win
for red: eight children of each of the 74 positions that were searched to a loss for blue. The
label follows from the parent's value and every solve checks it. This is the real work of a
proof, so it is the ruler when the question is "faster proofs": about 5 M nodes and 5 s a
position for v9 on one thread. Positions 0-47 are tune (used by E23); 48-591 are unused: a
deciding read takes the next unused ones and says which in the log. `tools/run.py` does not
know this set yet; until it does, runs are `snek bench tests/proof4/positions.txt --from A --to B`
for each arm, one at a time for seconds, side by side for node counts.

## Before any speed test

- `target/release/snek verify 100`: the fast move generator, the per-placement check, snakes, hashes
  and bounds against the naive rules, on random games.
- `target/release/snek perft 3` gives 91 / 194,388 / 315,706,848.
- **A change meant to alter nothing** is proved by identical node counts and values on the tune units
  of two ply files, not by a gate.

## The gates

| Kind of change | Hypotheses (log ratio of seconds per unit) | Cap | A pass means |
| --- | --- | --- | --- |
| An improvement | `SPRT[0, 0.05]` | 100 units | it is faster |
| A fix or a simplification | `SPRT[-0.03, 0]` | 100 units | it is not slower |

Error rates 0.05 and 0.05, no decision before 30 units. Command:
`python tools/run.py KEY gate --set plyNN --hypothesis "..."` (add `--h0 -0.03 --h1 0` for the second row).

- **The guard:** a pass also needs the candidate's total time over the gate's units to be no higher than
  the base's. Total time is what a full solution costs; the per-unit ratio counts easy and hard units alike.
- **The scaling check:** before a freeze, `run.py KEY-UP tune` on the next harder file (at least 10
  units) must not show a higher total time. The goal is harder positions, so a change that pays only on
  easy ones is not wanted.
- Each idea is gated once, under a key written in the log first. A fail is final for that change; so
  is reaching the cap undecided. A run that was interrupted is run again whole under a new key.
- Fix the cap after the screen: if the screen's noise says 100 units cannot decide, do not start the gate.
- The number a gate prints where it stops is inflated. Sizes come from confirmations at milestones:
  the current base against an earlier one, fixed length, on unused confirm units.
- A pass belongs to its base.

## What blocks, and what only informs

Blocks: a wrong value; a failed `verify`; a failed identity proof; a crash; the guard; the scaling check.
Informs only: node counts; the reference's published timings (another machine); reads on the
reference's benchmark units; profiles.

## Changes to this contract

| Date | Change | Why | Decided by |
| --- | --- | --- | --- |
| 2026-11-14 | First version. The 5% bar, the 10-position unit, the guard and the scaling check are assumptions awaiting the owner. | set-up | Claude, owner asleep |
| 2026-11-15 | Added the proof benchmark `tests/proof4/`. | The owner put speed of proofs first ("hill climbing our efficiency to get faster proofs") and asked for a benchmark of positions to time each approach on. | Claude, on the owner's word |
| 2026-11-15 | On the climb, gates compare profiled MSVC builds; the driver is read on whole proof jobs with `tools/provegate.py` (fixed length). | E27 passed: the build that runs long proofs is a profiled one, and a change has to pay on that build. Twelve reserved jobs cannot feed a sequential test. | Claude; the owner approved installing the build tools ("Yes, install it") |
| 2026-11-15 | Added a second proof set `tests/proof4b/` (1,184 positions, 16 a parent, none shared with `tests/proof4/`; `--set proof4b`, unit 2 positions: tune 0-87, gate 88-442). A learned move order is gated only on positions whose parents were kept out of its training (24 of the 74 parents are set aside for that). | The first set's gate range was down to 57 unused units after four gates, and a pruning or ordering change needs more units than a same-node change. | Claude |
