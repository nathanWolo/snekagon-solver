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

- **Base for new work:** `frozen/base.exe`; its id, commit and date are in `frozen/BASE.txt` and `docs/SHIPLOG.md`.
- **State:** see the top of `docs/LOG.md` and the table in `docs/SHIPLOG.md`.
- **What a gate resolves:** timing noise is about 1-2% per unit (A/A, 2026-11-14). A change of 5% is
  decided in 30 units when it acts on most positions. See `docs/TESTING.md`.

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
4. Screen: `python tools/run.py KEY-S tune --set plyNN --units 20` (node counts are exact; times are not).
5. Gate once: `python tools/run.py KEY gate --set plyNN --hypothesis "..."`.
6. Pass: commit, `python tools/run.py KEY freeze`, update docs. Fail or undecided: `git checkout src/`, log it.

## Working in this repo

- Tools run from the repo root with `python tools/<name>.py`. `tools/hc.py` is the statistics tool.
- The test set is not in git: rebuild it with `python tools/mktests.py <checkout of BoltKey/snekagon-lab at 61eb3a4> tests/set1`
  and compare with `tests/set1/MANIFEST.txt`.
- Shell quirks: Git Bash heredocs mangle backslashes; write scripts as files.
- Leave no processes behind. Kill only your own.

## Where things are

| You need | Read |
| --- | --- |
| The rules, the reference solver and its numbers | `docs/research/rules-and-prior-work.md` |
| The testing contract | `docs/TESTING.md` |
| Every run, in order | `docs/LOG.md`, `runs.jsonl`, `runs/` |
| What we have learned | `docs/LESSONS.md` |
| The backlog | `docs/IDEAS.md` |
| Every frozen version | `docs/SHIPLOG.md`, `frozen/` |
| Code | `src/` (`geom`, `pieces`, `pos`, `bounds`, `solver`, `naive`, `main`) |
