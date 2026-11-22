# Snekagon solver

An exact solver for the board game Snekagon, written in Rust with the standard library only, and
a computer proof made with it: **with perfect play the first player wins.**

Snekagon was designed by Adam Španěl and is published by Maty Moves. This repository is not
affiliated with either of them.

## The result

- The first player (red) opens on the centre cell, `1:60`. Blue has 1,932 replies, 172 after the
  board's twelve symmetries. For each of the 172 the solver found a red answer after which blue,
  to move, loses. The search finished on 2026-11-16 and took about 19 hours of one desktop PC
  (Ryzen 7 5800XT, 16 threads).
- **This is the second proof of that result, not the first.** The GitHub user K0tovsk1y published
  one on 2026-10-19 ([announcement](https://github.com/BoltKey/snekagon-lab/issues/3),
  [repository](https://github.com/K0tovsk1y/snekagon-solution)), built on the solver in the
  designer's repository. This project learned of it on 2026-11-21, after its own proof was done.
  The solver here was written from scratch and shares no code with that proof. Red's answer is a
  different move in 158 of the 172 lines.
- The proof's files are in [`proofs/centre-opening/`](proofs/centre-opening): `results.txt` (the
  search results, 17 MB), `certificate.txt` (blue's reply and red's answer for every class) and
  `meta.json`.

Red's other first moves were not looked at, and the full game tree is not stored: below red's
second move the strategy is whatever the solver computes.

## How far it has been checked

- `python tools/certificate.py proofs/centre-opening/results.txt` rebuilds `certificate.txt` from
  the results. It counts the replies and their symmetry classes from the piece shapes alone and
  checks that every class has a reply with a recorded loss. It checks the bookkeeping at the top
  of the proof, not the searches under it.
- One of the 172 lines has been solved again by a second program, the fourth version of the solver
  in the designer's repository (its output is in `proofs/centre-opening/refcheck/`). The other 171
  rest on this solver alone.
- On 2026-11-21 this solver began checking the 172 answers of the earlier proof
  (`tools/crosscheck.py`). The result goes into `docs/LOG.md` when the run ends.
- The earlier proof's own checker has not been run on this proof.
- One known soft spot: at the table size used, the transposition table tells positions apart by
  a 38-bit key check (`docs/LOG.md`, "the table's key is 38 bits strong").

## Building and running

Rust stable and Python 3, no other dependencies. Developed and measured on Windows only.

```
cargo build --release
target/release/snek info
target/release/snek perft 3                      # 91 / 194,388 / 315,706,848
target/release/snek verify 100                   # self-checks on random games
target/release/snek text 1:60 4A:17,18,29,40     # moves -> position text
target/release/snek solve <position text>        # exact value of a position
target/release/snek prove FILE --threads 16 --job-threads 1 --depth 2 --tt 27 --out RESULTS
```

- A move is `<piece>:<cells>`, a cell is `(r + 5) * 11 + (q + 5)`, and the centre is 60.
- `.cargo/config.toml` builds for the CPU of the machine it runs on.
- The timed builds in the log were made with `sh tools/build.sh` (MSVC toolchain, Git Bash,
  optionally profile-guided).
- The second part of the proof was run by the driver on branch `climb/replay`
  (`prove --replay --win-turns`), which is not merged into the default branch.
- `python tools/gui.py` serves a local analysis board for setting up and solving positions.

## What is here

| Path | What |
| --- | --- |
| `src/` | The solver: about 5,800 lines of Rust. Alpha-beta with a transposition table, bounds on how long each snake can still get, a closed form for the last pieces, threat zones, a learned move order, a parallel solve and a proof driver. |
| `proofs/centre-opening/` | The proof's files. |
| `docs/LOG.md` | Every experiment in order, with its hypothesis, numbers and verdict, failures included. |
| `docs/LESSONS.md`, `docs/TESTING.md`, `docs/IDEAS.md`, `docs/SHIPLOG.md` | What was learned, how changes are tested, the backlog, the frozen versions. |
| `docs/research/` | The rules as implemented, and the prior work. |
| `docs/prover-explained.html`, `docs/blog/` | An explainer of the prover written while the proof ran, and a draft write-up of the result. |
| `tools/`, `gui/` | Test runner, statistics, proof tools, the analysis board. |
| `CLAUDE.md` | The working notes for the coding agents and the owner. They describe the owner's machine and the state of the work, not how to use the repository. |

The default branch is `parallel-v9`. `main` is the single-threaded base it grew from. The
`climb/*` branches are experiments as they stood when the speed work was paused on 2026-11-16;
three of them have a `HANDOVER.md`.

## How it was made

The code, the experiments and the log were written by Claude, Anthropic's AI model, working as
coding agents on the owner's PC between 14 and 21 November 2026. The owner set the goal and the
rules of evidence and made the decisions. Changes were accepted by measurement: one hypothesis
per change, a test fixed in advance, on positions the change had not seen, and every run written
down.

## Credit

- **Adam Španěl** designed the game. His repository
  [BoltKey/snekagon-lab](https://github.com/BoltKey/snekagon-lab) holds an engine, a solver, more
  than 22,000 positions with exact values and the account of his own attempt. The rules solved
  here are the rules as that repository implements them, and its labelled positions were this
  project's answer key. They are not copied here: `tools/mktests.py` rebuilds the test set from a
  checkout of that repository.
- **Darragh Griffin** wrote the fourth version of that solver. The closed form for the last
  pieces, the threat zones and the exact reach test here are this project's own implementations
  of ideas his version described and measured, and his version was the mark this solver was
  measured against.
- **K0tovsk1y** proved the result first.

The designer's repository carries no licence, so none of its code is in this one.

## About this copy

This repository is a copy of a working repository, made by `tools/publish.py`.

- Commit hashes here differ from the hashes cited in the docs. The branch `commit-map` holds one
  file that maps each to the other.
- One study branch is left out because it held a port of the reference solver's learned
  evaluation, which has no licence. Two data files with that port's scores are left out of
  `climb/evalorder` for the same reason.
- The branch with a page for playing against the proof is not included.
- The docs name files under `frozen/` and `work/` (built solvers, run outputs). Those were never
  in git.

No licence has been chosen for this repository yet.
