# Lessons so far

What the log has taught, distilled. `LOG.md` has the record and the numbers' sources.

## How to read the numbers

Confirmed: fixed length, fresh units. Gate: stopped where the test decided, so it reads high. Tune:
screens and profiles, for ranking only. Node counts are exact and repeat; seconds are this machine's
(Ryzen 7 5800XT, one thread unless said). The reference's figures are its author's, on another
machine, and were not reproduced here.

## About the problem

- **The tree is two kinds of node.** The winner needs one move; the side being refuted needs all of
  its moves answered. Everything that worked cut one of the two: fewer moves to answer (threat zones),
  nodes decided without a search (last pieces, reach test), a better first move (the model).
- **Near the end the cluster bound is loose and exact answers are cheap.** With one piece in each hand
  the value is two move lists (E1). With one or two pieces in a hand, "can this player still reach
  length T" is a small one-player search that decides 35-60% of the nodes it is asked about (E3).
- **A quarter of the moves at a winner's node win, and they differ sixfold in cost.** At ply 6 about
  69 of 270 root moves win; the median win costs 6 times the cheapest. An order that finds *a* win
  first saves 2x; one that finds the *cheapest* would save 5-8x. Open.
- **Proof-number search is the wrong shape for this game at these depths.** Its premise is that effort
  should be spread until a move shows itself cheap. Here the losing moves are cheap to refute and the
  winning ones expensive to prove, so spreading pays part of every expensive proof. Three separate
  measurements agree: df-pn 28-60x slower, reply-count ordering -7..18% nodes at a loss in time,
  budgeted rounds 7-18x more nodes.
- **The reply that beat a move says where to play next.** After a move fails, the moves that cover a
  cell of the refuting reply are tried first (E14): 23% fewer nodes at ply 7, 41% fewer on 30 ply-5
  positions. The fitted model took a study and a regression to win less. A reply left over from a
  sibling node, a reply tied to one move, and "moves like the killer" all failed (E15-E17).
- **Bounds that answer the same question are paid twice.** The cluster bound cut one node in eight
  where the exact reach test also applied, and the exact test ran at the other seven anyway (E10).
- **Each earlier ply costs about ten times more** (v0: ply 9 0.5 ms, ply 8 6 ms, ply 7 50 ms).
- **A drawn position is two proofs, and which question is asked matters more than who schedules
  the work.** On the drawn ply-3 position one search with the wide window took 140 s; the same
  engine asked "can the mover draw" and then "can the mover win" took 195 s. The narrow pass asks
  "does the opponent at least draw" after every move, where the wide search proved "the opponent
  wins" after the losing ones, and the win is the proof it finds faster (E18).
- **Drawing moves searched side by side each pay for the exact proof.** A parallel driver with the
  wide window had several drawing moves of the root under way before the first raised alpha:
  7.4 billion nodes where 1.8 billion do (E18).

## About measuring

- Timing noise is 1-2% per unit of 10 positions (A/A). A pruning change has sd 0.18-0.24 per unit; an
  ordering change 0.45, because it wins big on some units and loses on others. Plan gates for the
  kind of change.
- **A bound has to be priced.** The reach test halved the nodes and lost time in its first form (1.2
  microseconds a test, 41% deciding). The same idea won 25-34% once a test cost a fraction of that.
  Node counts alone would have accepted the slow form.
- **Fit the event, not the cost.** Searching model weights on the true cost gave 2.5x in sample and
  1.0-1.9x out; a plain logistic fit of "this move wins" transferred. 200 positions are few.
- **Measure the oracle before building.** Twice a cheap measurement redirected the work: "best move
  first" (1.5-3x) against "98% of nodes have five pieces left or fewer" sent the effort to bounds
  before ordering; the root-move study showed the ordering pot was at the root, not below it.
- **Keep the machine quiet for a gate.** `E4` ran beside five study processes. Interleaving and exact
  node counts saved the verdict, but the size had to be measured again.
- **Check the control when a result is small.** The parallel build read -1.9% with one thread. A fresh
  build of the unchanged source read +0.04% [-0.8%, +0.9%], so the loss was real and not build noise.

- **Time the sections, then count what the slow one does.** A cycle counter around each part of
  a node predicted E9's gain to within a point (12% expected, 11% measured). For the reach test the
  count that mattered was not time but work: 7.5 positions built per "no", most of them hopeless
  because a size check counted the same snake twice (E12, 20% of all time).
- **Screen order ideas on node counts.** They are exact and do not care what else is running, so
  three failed ideas cost ten minutes and no gate units.
- **Look at what is left of a range before spending it.** E11's first gate could not decide: 24
  units were left and the test takes no decision before 30. The runner now refuses such a gate.
- **A freeze must check the verdict itself.** I once chained the freeze to the gate on one line; it
  would have frozen a fail. The runner now refuses a freeze without a passed gate of the same file.

## About engineering

- Counting moves by popcount instead of listing them makes perft 3 take 0.13 s.
- Code that never runs still costs: the parallel plumbing took 1-3% from the one-thread search.
  Compiling the search twice (plain and parallel) recovered about a point; the shared atomic table
  itself costs about one.
- A cap "that cannot be reached" was reached: the list of set-aside moves in the parallel search.
- On Windows a running executable cannot be rebuilt. Copy it before a long background run (the runner
  already does); build experiments into `work/target`.
- Git Bash heredocs mangle backslashes: patch scripts are written as files.
- **In a parallel tree search, hand out the least speculative job first.** Young-brothers-wait let
  the free workers pile onto the alternatives of one node that needs a single good move, ahead of
  root moves that had to be searched anyway. Costing each job by how many older moves it runs
  beside took the ply-4 position from 748 M nodes to 530 M, below the shared-table solve (E18).
- **Before blaming a new driver, run the old engine on the same questions.** The driver was 1.4x
  slower than the shared-table solve on the ply-3 position. Giving the old engine the driver's two
  narrow windows reproduced the whole gap in one run: the loss was in the questions (E18).

## Open questions for the owner

- Merge the parallel solve (branch `parallel-v9`: 3.4x wall-clock on 8 threads; the plumbing cost about 2% with one thread on v4)?
- E11 was accepted on a second gate after the first was void for lack of units (LOG, E11). Overrule if you disagree.
- Is 5% the right bar for keeping a speed change?
- The reference solver was not run here (downloaded code). A same-machine comparison needs the owner
  to build and run it; the commands are in `docs/research/rules-and-prior-work.md`.
- The project lives in a folder the app made for this session and needs a permanent home.
- The proof driver (`prove`, branch `parallel-v9`) is exact and restartable but not faster than the
  shared-table solve on a drawn position (E18). Keep it on the branch, or merge it with the
  parallel solve?
