# Snekagon: the rules and the prior work

Written 2026-11-14 from the publisher's rules PDF, Board Game Arena's help pages and the public repository
`github.com/BoltKey/snekagon-lab` (commit `61eb3a4`), which holds an engine, an exact solver and their
measurements. When this was written that repository had been read as data only; its solver has been run here since (see "Not verified").

## The rules, as the reference implements them

- **Board:** a hexagon of side 6, 91 cells. Axial coordinates (q, r) with |q|, |r|, |q+r| <= 5.
  Cell index = (r+5)*11 + (q+5), so the centre is 60. Neighbours are at index offsets +1, -1, +11, -11, -10, +10.
- **Pieces** (may be rotated and flipped), as (q, r) offsets:

  | name | cells | shape |
  |---|---|---|
  | `1` | 1 | `(0,0)`. Red only. |
  | `2` | 2 | `(0,0) (1,0)` |
  | `3I` | 3 | `(0,0) (0,1) (0,2)` straight |
  | `3V` | 3 | `(0,0) (1,0) (1,1)` bend |
  | `4I` | 4 | `(0,0) (-1,1) (-2,2) (-3,3)` straight |
  | `4A` | 4 | `(0,0) (1,0) (1,1) (1,2)` (12 orientations) |
  | `4B` | 4 | `(0,0) (0,1) (1,1) (1,2)` zig-zag |
  | `4C` | 4 | `(0,0) (-1,1) (-1,2) (0,2)` (the "U") |

  Red has 25 cells, blue 24. There are 40 orientations and 2,602 placements in all.
- **First move:** red places the 1-hex anywhere. Then the players alternate, blue first, seven pieces each.
- **A turn:** place one remaining piece on empty cells, either
  - as a **new snake**: it touches no tile of either colour, or
  - to **grow**: it touches at least one own tile (it may touch the opponent's too, may join own snakes,
    may close a ring).
- **Snake shape:** after the move every own cell has at most two own neighbours and no three own cells are
  mutually adjacent. So every own component is an induced path or ring.
- **Pass:** a player with no legal placement passes. The game ends after two passes in a row, or when both
  players are out of pieces.
- **Result:** the longer longest snake (cells of one component) wins; equal is a draw.
- **Notation:** a move is `<piece>:<cell>,<cell>,...` with cells ascending, or `pass`.
  A position is `<red hex>_<blue hex>_<red pieces>_<blue pieces>_<side>_<ply>_<passes>`: cells as 32 hex
  digits of the 128-bit mask (high half first), piece sets as decimal bit masks (bit p = piece p in the order
  of the table above), side 0 = red.

Consequences used by every solver:

- A body cell (two own neighbours) can never get another own neighbour, so a player can never again occupy
  a cell next to its own snake body.
- Placing a piece only removes opponent placements (the cells it covers, and new snakes that would touch
  it); it never adds any.

## Numbers to check an implementation against

- perft: 1 -> 91, 2 -> 194,388, 3 -> 315,706,848.
- 2,602 placements.
- `data/labels_good_v4.csv` (13,942 positions, plies 6-11), `labels_noisy_v4.csv` (8,593),
  `labels_good_v3.csv`, `labels_good_ply5.csv` (245 at ply 5): each row has the position text and the exact
  value for the side to move (+1 win, 0 draw, -1 loss), computed by that repository's solver.
- `bench/ply5_*.txt`: 126 positions after five moves, as move lists. The first four sets (24 each) are all
  wins for the side to move (blue). `ply5_ublock_refutations.txt` (30) holds 27 losses and 3 draws for blue.

## What the reference solver does (its v4)

Negamax alpha-beta over {loss, draw, win} with a window, and:

1. a transposition table keyed by the symmetry-canonical hash up to ply 6;
2. **length bounds**: lower = longest snake now; upper = per cluster of (own cells + growable cells), own
   cells plus the best subset-sum fit of the remaining pieces into the growable regions, counting only the
   k+1 longest snakes of a cluster when k pieces are left. A node is decided when one side's lower bound
   beats the other's upper bound;
3. **the last pieces** in closed form: with at most one piece each, the mover places its piece and the
   opponent takes its longest extension that is still legal;
4. **tight bounds** near the end (at most 5 pieces left in total; at 6 only after a node's first move
   failed): guaranteed growth one and two plies ahead, and the exact **one-player optimum** (could this
   player still reach k cells if the opponent never moved again; a depth-first search over paths of whole
   snakes and pieces, memoised per cluster, 20,000 nodes per question);
5. **threat zones**: an opponent placement that would decide the game by the bounds if played now must be
   covered (or touched, for a new snake) by our move, so only moves through every such zone are searched;
6. ordering: table move, two killers per ply, then piece size * 64 + 256 for growing an own snake + 128 for
   covering a cell next to an opponent snake end + 32 for touching the opponent; with seven or more pieces
   left, a learned evaluation of each child once the table move or a killer has failed. No history table.

Its authors report that the winning side finds its move at the first try about 90% of the time, so the
tree is close to a minimal proof and the cost is the loser having to answer every move.

Tried there and rejected (no gain): enhanced transposition cutoffs, one table shared by all threads of a
solve, deeper evaluation ordering, a cheap single-region bound before the exact one, "inert move"
equivalence.

## Reported costs (their machines, not ours)

| positions | v3 | v4 |
|---|---|---|
| `ply5_blue_wins.txt` (24) | 361.2 M nodes, about 207 thread-s | 4.40 M nodes, 13 thread-s |
| `ply5_blue_wins_holdout.txt` (24) | 425.4 M, 253 | 4.31 M, 14 |
| `ply5_random_units.txt` (24) | 285.2 M, 157 | 6.16 M, 18 |
| `ply5_other_openings.txt` (24) | 220.5 M, 117 | 7.45 M, 15 |
| `ply5_ublock_refutations.txt` (30) | 2,560 M, 2,059 | 37.37 M, 137 |
| 12 random positions after 5 random moves | 751.6 M, 487 s | 15.54 M, 56 s |
| 12 random positions after 6 random moves | 51.65 M, 34 s | 2.47 M, 8 s |

Measured on a Ryzen 7 250 (16 threads) with other work running. So v4 spends about 3 microseconds a node.

**On this PC (2026-11-16, at the owner's word):** the same five files, v4 at `61eb3a4`, one thread,
`BENCH_FRESH=1`, built with g++ 13.3 in WSL2: 59,709,026 nodes (its published counts, exactly) in 140.2
and 141.4 thread-seconds (two rounds); 2.36 microseconds a node. This project's frozen base of that day
took 77.6 and 78.7 s on the same files (104.5 M nodes), and all 126 values agree. `docs/LOG.md`,
"Same machine, the reference solver".
Later that day climb base 4 (reach threats at every level) took 0.842x of that base's time on the five
files with 53,198,200 nodes, fewer than the reference's, so about 0.47x of the reference's time. The
reference still needs fewer nodes on `blue_wins`, `blue_wins_holdout` and `random_units`.

## What is known about the game

- After `1:60 4B:16,27,28,39 4A:7,8,9,20`, blue wins with `4I:15,25,35,45` (all 956 red replies lose).
- After `1:60 4B:16,27,28,39`, blue does not win: red's reply `4C:40,41,50,52` refutes all 938 blue moves
  (382 s on 5 threads), and blue's `4A:49,59,70,81` there does not lose.
- One position after 4 moves from engine self-play is a red win (1.2 G nodes with the v2 solver).
- The value of the start position, and of the centre opening `1:60`, was open in that repository when
  this was written. It is settled since: red wins (see "Added 2026-11-21" below).
- Their estimate for the centre opening at v2/v3 speed: 10,000 to 30,000 thread-hours.
- By symmetry there are 12 distinct first moves.

### Found in this project (not in the reference's notes)

- After `1:60 4B:16,27,28,39`, red wins with `4B:70,81,82,93` (2026-11-15; `docs/LOG.md`). So the
  blue reply the reference studied loses against the right answer. Checked by three runs of this
  project's engines; not checked with the reference solver.

## Added 2026-11-21: what this note missed

- **An earlier proof.** Issue #3 on that repository ("Snekagon is solved", 2026-10-19, user
  K0tovsk1y) announces a proof that red wins with `1:60`, in the repository
  `K0tovsk1y/snekagon-solution`: 172 classes of blue's reply, a red answer to each
  (`proof/index.txt`), 166,912 classes of blue's fourth move with red's fifth move stored, a
  checker and a play page; about 160 laptop-hours (8-core laptop) on 16 to 19 October 2026.
  This note was written from the README and did not look at the issues. `docs/LOG.md`,
  "an earlier proof found".
- **Who wrote what.** The repository is the designer's, Adam Španěl (git "Adam Spanel",
  GitHub BoltKey): engine, solver v1 to v3, the pilot, the distributed solve, the video
  (commits 2026-09-30 to 10-12). Solver v4 and the tools `refute` and `proveline` are commit
  `184001d` by Darragh Griffin (pull request #1, 2026-10-16); the network engine is his too.
  "The reference solver (its v4)" above is therefore his version.
- **How far the designer's own attempt got** (the video's script and fact sheet, in
  `video/` of that repository): 16 of red's 1,589 answers to `4B:16,27,28,39` proved to lose
  in a week, about 1,300 thread-hours; `3V:40,41,50` resisted 40 blue moves. Later, in
  Griffin's notes: blue's only win against it is `3V:9,20,21`.

## Not verified here

- The reference code was run on this machine once, on 2026-11-16, for its `benchset` on its five bench
  files (the owner's word, that day: "Yes, run it"). Every other figure above is as reported in that
  repository's README and data files. Its other modes (`serve`, `worker`, which use the network) have not
  been run.
- The piece shapes were read from that repository's `RULES.md` and reference implementation, which read
  them off the print-and-play sheet. I did not see the sheet.

## Running the reference solver yourself (for a same-machine comparison)

Done here on 2026-11-16 (see above; the build is in WSL at `/opt/snekref`, not in `/tmp`, which WSL
empties when the distribution stops). To repeat it, in WSL (or any Linux shell with g++ 11 or later), from a checkout of
`github.com/BoltKey/snekagon-lab` at commit `61eb3a4`:

```bash
g++ -O3 -march=native -std=c++20 -pthread -o build/snek src/main.cpp
```

```bash
BENCH_FRESH=1 ./build/snek benchset bench/ply5_blue_wins.txt 1
```

The second command solves the 24 positions of one bench file on one thread from an empty table (as
its README describes). Our figures for the same five files are in `docs/LOG.md` under "Where v4
stands on the reference's bench files"; the files themselves are rebuilt by `tools/mktests.py` as
`tests/set1/bench_*.txt` and run with `frozen/base.exe bench tests/set1/bench_ply5_blue_wins.txt`.
Read the script and the build line before running them. (When this was written the sources had been
read as data and not executed; the solver has been built and run here since 2026-11-16.)
