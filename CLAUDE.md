# Snekagon solver: how this project works

This file is the orientation. Details live in `docs/`. Keep it short and true.

## The goal

- **Target:** an exact solver for the board game Snekagon (rules: `docs/research/rules-and-prior-work.md`),
  fast enough that the start position can one day be solved. The score is thread-seconds to solve a
  position exactly, with the value correct. Lower is better.
- **Production:** the real work a full solution needs: positions after about five moves that come up in
  the proof of an opening. The reference repository's `bench/ply5_*` units stand in for it.
- **Limits:** this PC (Ryzen 7 5800XT, 16 threads, 16 GB). Rust only (`stable-x86_64-pc-windows-gnu`).
- **The bar:** owner's words, 2026-11-14: "an ultra fast solver ... with the long term goal of solving it
  from the start position. Good incremental progress would be having a very fast proof number search
  implementation that can solve non-start pos positions fairly quickly."
- **Today:** see "Right now".

## Right now

Update this section in the same change that freezes a version.

- **Base for new work:** `frozen/base.exe` = v9 (alpha-beta with a table, the last pieces in closed
  form, threat zones, exact reach tests asked before the cluster bounds, model ordering, the first
  move searched before the list is generated, and after a failed move the moves that cover the
  refuting reply first). Id, commit and date: `frozen/BASE.txt` and `docs/SHIPLOG.md`. Branch `main`.
- **Parallel solve:** branch `parallel-v9` (`bench --threads N`, `src/par.rs`), the port of branch
  `parallel` (built from v4) onto v9. Exact; 3.4x wall-clock on 8 threads; on v4 the plumbing cost
  about 2% with one thread, which is why it is not merged. A built copy is `frozen/par-v9.exe`.
  Merging is the owner's call (asked 2026-11-15).
- **Found about the game:** after `1:60 4B:16,27,28,39` red wins with `4B:70,81,82,93` (LOG,
  "the position after two moves is a win for red"; three runs and two engine generations agree;
  the reference solver has not been asked).
- **The centre opening `1:60` is being solved:** 172 blue replies after symmetry, each needs a
  red win. The run and how to go on with it:
  `frozen/prove.exe prove work/long/ply1.txt --threads 1 --job-threads 16 --depth 2 --tt 27 --out work/long/ply1_results.txt`.
  It can be stopped at any time; the same command continues from the results file. Progress:
  `grep "root move" work/long/ply1.err`.
- **Proof driver:** also on `parallel-v9`: `snek prove FILE --threads N --job-threads T --depth D
  --out RESULTS` (`src/prove.rs`, a built copy is `frozen/prove.exe`). A coordinator searches the
  first D plies and hands positions to workers; every result goes to RESULTS and a later run
  starts from it. Exact; faster than the shared-table solve on the ply-4 position, slower on the
  drawn ply-3 position (LOG, E18). `--wide` searches with one window; moves that a symmetry of the
  position maps onto each other are searched once. Long solves and their results files are in
  `work/long/`.
- **Proof-number search:** `--pn` (df-pn, `src/pns.rs`). Exact and 28-60x slower than alpha-beta here;
  `docs/IDEAS.md` (graveyard) says why and what would reopen it.
- **State:** the end of `docs/LOG.md`, and the table in `docs/SHIPLOG.md`.
- **What a gate resolves:** timing noise is about 1-2% per unit (A/A). A change that keeps the node
  counts has sd 0.02-0.08 per unit, a pruning change 0.2, an ordering change 0.25-0.45. Plan with
  `python tools/hc.py plan` on the screen's records.
- **Gate units:** the ply-6 gate range is used up. Gates run at ply 7 (about 90 unused gate units
  on 2026-11-15); `tools/run.py` refuses a gate with fewer than 30. More positions are needed soon.

## Standing rules

- **Ask the owner first** before pushing, publishing, or running the reference C++ solver (downloaded
  code). Standing permissions, in the owner's words: "dont block on anything that needs interaction from
  me" (2026-11-14, for the overnight run): local commits and heavy CPU use are fine; nothing outward.
- **A wrong value is the only unforgivable result.** Every run checks every position against its label.
  A candidate that gives one wrong value is blocked, whatever its speed.
- **The base is frozen.** Experiments change `src/`; `frozen/base.exe` changes only by `run.py ... freeze`
  after a passed gate.
- **Test data.** Screen on the tune units. Gate once, on unused gate units. Confirm on unused confirm
  units. Nothing is tuned on held-out units. `tests/ledger.jsonl` records every deciding read.
- **Environment pin:** `cargo build --release` with `-C target-cpu=native`, `lto = "fat"`,
  `codegen-units = 1`, `panic = "abort"`. Timed runs: one process at a time, nothing else heavy running.

## How we test

The contract is `docs/TESTING.md`. In short:

1. Write the hypothesis, expected size and bar in `docs/LOG.md`.
2. Make the one change in `src/` (behind a flag when cheap), `cargo build --release`.
3. Correctness: `target/release/snek verify 100` and the labels (the runner checks them on every unit).
   With a flag: the flag off must give the base's node counts exactly.
4. Screen: `python tools/run.py KEY-S tune --set plyNN --units 20` (node counts are exact; times are not).
5. Gate once: `python tools/run.py KEY gate --set plyNN --hypothesis "..."`.
6. Pass: commit, `python tools/run.py KEY freeze`, update docs. Fail or undecided: `git checkout src/`, log it.

## Working in this repo

- Tools run from the repo root with `python tools/<name>.py`. `tools/hc.py` is the statistics tool.
- The test set is not in git: rebuild it with `python tools/mktests.py <checkout of BoltKey/snekagon-lab at 61eb3a4> tests/set1`
  and compare with `tests/set1/MANIFEST.txt`.
- Shell quirks: Git Bash heredocs mangle backslashes; write scripts as files.
- Leave no processes behind. Kill only your own.
- A timed run needs a quiet machine: no builds, studies or other solves beside it.
- Build experiments that must not disturb a running `target/release/snek.exe` with
  `cargo build --release --target-dir work/target`.
- Several versions on the same fresh units: `python tools/ladder.py KEY --set plyNN --units N v3=frozen/v3-....exe v4=frozen/base.exe`.
- A study build that times the sections of a node: the patch scripts named in `docs/LOG.md` (`timing2_patch.py`), applied to a copy, built into `work/target`, never committed.
- Order ideas are screened on node counts first (exact, free of load): `snek bench FILE --from A --to B --quiet` with the option on and off.

## Where things are

| You need | Read |
| --- | --- |
| The rules, the reference solver and its numbers | `docs/research/rules-and-prior-work.md` |
| The testing contract | `docs/TESTING.md` |
| Every run, in order | `docs/LOG.md`, `runs.jsonl`, `runs/` |
| What we have learned | `docs/LESSONS.md` |
| The backlog | `docs/IDEAS.md` |
| Every frozen version | `docs/SHIPLOG.md`, `frozen/` |
| Code | `src/` (`geom`, `pieces`, `pos`, `bounds`, `solver`, `pns`, `study`, `naive`, `main`; `par` on branch `parallel`) |
| What predicts a good move | `tools/rootstudy.py`, `tools/rootfit.py`, `tools/rootcost.py` on the output of `snek rootmoves` |
