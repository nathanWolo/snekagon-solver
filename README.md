# Snekagon solver

An exact solver for the board game Snekagon, written in Rust with the standard library only, and
a second, independent computer proof that with perfect play the first player wins. The first proof
was published by K0tovsk1y on 2026-10-19 (see below).

Snekagon was designed by Adam Španěl and is published by
[Maty Moves](https://matymoves.com/produkt/snekagon/). This repository is not affiliated with
either of them.

## The result

- The first player (red) opens on the centre cell, `1:60`. Blue has 1,932 replies, 172 after the
  board's twelve symmetries. For each of the 172 the solver found a red answer after which blue,
  to move, loses. The search ran on 15 and 16 November 2026 and took about 19.5 hours of one
  desktop PC (Ryzen 7 5800XT, 16 threads).
- **This is the second proof of that result, not the first.** The GitHub user K0tovsk1y published
  one on 2026-10-19 ([announcement](https://github.com/BoltKey/snekagon-lab/issues/3),
  [repository](https://github.com/K0tovsk1y/snekagon-solution)), built on the engine and solver
  in the designer's repository. This project learned of it on 2026-11-21, after its own proof was
  done. The solver here was written from scratch and shares no code with that proof. After the
  replies are matched up by symmetry, red's answer is a different move in 158 of the 172 lines.
- The proof's files are in [`proofs/centre-opening/`](proofs/centre-opening): `results.txt` (the
  search results, 17 MB), `certificate.txt` (blue's reply and red's answer for every class) and
  `meta.json`.

Red's other first moves were not looked at, and the full game tree is not stored: below red's
second move the strategy is whatever the solver computes. The earlier proof stores more of the
tree.

## How far it has been checked

- `python tools/certificate.py proofs/centre-opening/results.txt` prints the text of
  `certificate.txt` from the results. It counts the replies and their symmetry classes from the
  piece shapes alone and checks that every class has a reply with a recorded loss. It checks the
  bookkeeping at the top of the proof, not the searches under it.
- One of the 172 lines, the cheapest by this solver's count, has been solved again by a second
  program: Darragh Griffin's fourth version of the solver in the designer's repository. All 777
  of blue's moves lose there (211 s on 16 threads; output in `proofs/centre-opening/refcheck/`).
  Of the other 171, 14 have the same answer in the earlier proof, so that proof's program
  claims them too (it was not run here). The remaining 157 rest on this solver alone. Checking
  all 172 with the second program would take about 50 to 65 hours.
- This solver has checked the 172 answers of the earlier proof (`tools/crosscheck.py`, run on
  2026-11-21 and 22): by this solver every one of them wins. After red's centre cell, each of
  their replies and their answer to it, blue loses. It took 5.9 hours on 16 threads. That tests
  the top of the earlier proof with a program that shares no code with theirs, but follows the
  rules as the same repository implements them and has the soft spot named below. It does not
  test the positions that proof stores under its answers, or its checker. And it adds nothing
  to the checks of this proof: it was run with the program that made this proof, and agreement
  that 172 more positions are wins could not reveal a win reported in error. The run's output
  for each line is not in this repository. `docs/LOG.md`, "The first proof's answers, checked".
- The earlier proof's own checker has not been run on this proof.
- The rules are the rules as the designer's repository implements them. They have not been
  checked here against the printed game. Both proofs and the one-line check share that reading.
- One known soft spot: at the table size used, the transposition table tells positions apart by
  a 38-bit key check. Over a search this large that means an estimated one to seven false
  matches. Most would change nothing, but one could (`docs/LOG.md`, "the table's key is 38 bits
  strong").

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
| `docs/LOG.md` | The experiments in order, each with its hypothesis, numbers and verdict, failures included. |
| `docs/LESSONS.md`, `docs/TESTING.md`, `docs/IDEAS.md`, `docs/SHIPLOG.md` | What was learned, how changes are tested, the backlog, the frozen versions. |
| `docs/research/` | The rules as implemented, and the prior work. |
| `docs/prover-explained.html` | An explainer of the prover, written while the proof ran. |
| `tools/`, `gui/` | Test runner, statistics, proof tools, the analysis board. |
| `CLAUDE.md` | The working notes for the coding agents and the owner. They describe the owner's machine and the state of the work, not how to use the repository. |

The default branch is `parallel-v9`. `main` is the single-threaded base it grew from. The
`climb/*` branches are experiments as they stood when the speed work was paused on 2026-11-16;
three of them have a `HANDOVER.md`. `parallel`, `eo-emulate`, `gui` and `gui-prove` are older
lines of work, kept for the history.

## How it was made

The code, the experiments and the log were written by Claude, Anthropic's AI model, working as
coding agents on the owner's PC between 14 and 22 November 2026. The owner set the goal and the
rules of evidence and made the decisions. Speed changes were accepted by measurement: one
hypothesis per change, a test fixed in advance on positions the change had not been tuned on, and
every value checked against a known one.

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
- **K0tovsk1y** published the first proof of the result on 2026-10-19: a red answer to each of
  the 172 replies, red's next moves stored deeper in the tree than here, a checker and a page to
  play against it.
- **Nelson Elhage**'s notes on [solving Ultimate Tic Tac Toe](https://minimax.dev/docs/ultimate/)
  were one of the inputs to the design of the proof driver.

The designer's repository carries no licence, so none of its code is in this one.

## About this copy

This repository is a copy of a working repository, made by `tools/publish.py`.

- Commit hashes here differ from the hashes cited in the docs. The branch `commit-map` holds one
  file that maps each to the other.
- One study branch is left out because it held a port of the reference solver's learned
  evaluation, which has no licence. Two data files with that port's scores are left out of
  `climb/evalorder` for the same reason.
- A draft write-up of the result and the branch with a page for playing against the proof are
  not included yet.
- Folders of the owner's machine that scripts and logs name are replaced by `<user>`,
  `<session-folder>` and `<scratchpad>`.
- The docs name files under `frozen/` and `work/` (built solvers, run outputs). Most of them
  were never in git; a few small run outputs under `work/` are.

No licence has been chosen for this repository yet.
