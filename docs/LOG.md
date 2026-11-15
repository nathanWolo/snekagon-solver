# Run log

Every run in order, newest at the bottom. `runs.jsonl` holds one structured record per run and
`runs/KEY.jsonl` the units. Numbers are labelled: (tune) screens and profiles, (gate) stopped where the
test decided, (confirm) fixed length on fresh units.

## 2026-11-14 Set-up

- **Engine checks.** perft 1-3 = 91 / 194,388 / 315,706,848 and 2,602 placements, as the reference
  reports. `snek verify 300 7`: the fast generator, the per-placement check, snakes and hashes equal the
  naive rules on 4,720 positions; `verify 150 11` adds the upper bound against a flood-fill reference
  (2,364 positions) and checks it never rises along a game.
- **First solver v0** (alpha-beta over {-1, 0, 1}, 2-way table, length bounds, static ordering:
  table move, two killers, piece size, grows own snake, blocks an opponent end). Values equal the
  labels on 2,000 positions at ply 11, 2,000 at ply 10 and 4,000 at ply 9; with the bounds off, on
  2,000 at ply 11 and 300 at ply 10. About 3 M nodes a second.
- **v0 frozen** as the base: `917e1b93969f`, commit `44a10bb`.
- **AA-1** (ply 7, tune units 0-29): nodes identical, time -0.01% [-0.80%, +0.79%], sd 0.022 per unit.
  **AA-2** (ply 8): +0.43% [0.00%, +0.85%], sd 0.012. The ruler reads zero.
- **KNOWN-WORSE-1** (ply 10, bounds off as the candidate): -53.5% [-61.3%, -44.2%], 7.1x the nodes;
  `SPRT[0, 0.05]` fails at unit 30. **KNOWN-BETTER-1** (the reverse): passes at unit 30. The evaluator
  can fail and can pass.
- **Profile P0** (tune, first 100 positions of ply 7, v0): 15.0 M nodes, 6.3 s. Nodes by pieces left in
  both hands: 0: 1.03 M, 1: 4.11 M, 2: 4.53 M, 3: 4.15 M, 4: 0.76 M, 5: 0.39 M, 6+: 0.02 M. So 64% of the
  nodes have at most two pieces left and 92% at most three.
- **Where v0 stands** (tune): about 50 ms a position at ply 7, 6 ms at ply 8, 0.5 ms at ply 9. Each
  earlier ply costs about ten times more, so a ply-5 unit would take v0 several seconds; the reference's
  v4 reports about 0.5 s (on its machine).

## 2026-11-14 E1: the last pieces in closed form

- **Hypothesis** (written before the run): v0 searches the last two plies move by move. With at most one
  piece in each hand the value follows from the two move lists: the mover places its piece (or passes),
  then the opponent takes its longest extension that is still legal. Profile P0 puts 64% of v0's nodes
  at two pieces left or fewer. Expected: 2-3x fewer nodes and 1.5-2x less time at ply 7.
- **Test:** flag `--no-last` restores v0 behaviour (identity check on node counts). Screen on ply 7 tune,
  gate `E1` on ply 7 with `SPRT[0, 0.05]`, scaling check on ply 6 tune.
- **Identity:** with `--no-last` the first 100 positions of ply 7 take 14,995,547 nodes, as v0 does.
- **Correctness:** `verify 60 3` passes; labels equal on all 12,000 positions of plies 9-11.
- **Screen (tune, ply 7, 100 positions):** 9.65 M nodes against 15.0 M, 3.5 s against 6.1 s.
- **Gate `E1`** (ply 7, gate units 59-88, `SPRT[0, 0.05]`): PASS at unit 30, LLR +16.8. Where it stopped:
  +60.6% [+49.5%, +72.6%] (gate); total time 9.99 s against 16.31 s; nodes 0.64x.
- **Scaling check `E1-UP`** (ply 6, tune units 0-5): total time 11.6 s against 18.6 s (0.62x). No harm.
- **Verdict:** accepted. Frozen as v1.
- **What it taught:** the closed form removes the nodes with 0 and 1 pieces left; what remains sits at
  2 and 3 pieces left (4.3 M and 4.2 M of 9.65 M), so the next gains are there and in the number of
  replies searched at the loser nodes above them.

## 2026-11-14 E2: threat zones

- **Hypothesis** (written before the run): at a node where the mover is losing, v1 searches every move.
  If an opponent placement b, played now as a free move, already decides the game by the bounds, then
  every move of ours that leaves b playable (does not cover it; does not touch it when b starts a new
  snake) is worth no more than that verdict, because the opponent answers b and our upper bound only
  falls as cells fill. So only moves through the zone of every such threat need a search. Threats come
  from the opponent killer moves one ply down and, once a first move has failed, from one scan of all
  opponent placements. The reference credits this for most of its v4 gain. Expected: 2x or more
  fewer nodes at ply 7, more at ply 6.
- **Risk:** the scan costs about a move generation plus a bound per candidate; it may not pay near the leaves.
- **Test:** flag `--no-threats` restores v1 (identity on node counts). `verify` gains a check of the
  monotonicity the argument needs. Screen on ply 7 tune, gate `E2` on ply 7, scaling check on ply 6.
- **Identity:** with `--no-threats` the first 100 positions of ply 7 take 9,651,030 nodes, as v1 does.
- **Correctness:** `verify 3000 7` passes, with 2.3 M checks that a move outside a threat's zone leaves
  the threat playable and does not raise our bound with the threat's cells blocked. Labels equal on all
  12,000 positions of plies 9-11 and on the first 600 of ply 8.
- **Screen `E2-S`** (tune, ply 7, 20 units): nodes 0.44x, total time 0.66x.
- **Gate `E2`** (ply 7, gate units 89-118, `SPRT[0, 0.05]`): PASS at unit 30, LLR +11.5. Where it stopped:
  +59.5% [+46.3%, +73.8%] (gate); total time 7.46 s against 11.75 s (0.63x); nodes 0.44x.
- **Scaling check `E2-UP`** (ply 6, tune units 0-5): total time 6.56 s against 12.09 s (0.54x), nodes
  0.39x. The gain grows with depth, as expected of a cut at the loser's nodes.
- **Verdict:** accepted. Frozen as v2.
- **What it taught:** nodes fell 2.3x but time only 1.6x: a node now costs about 0.55 microseconds
  against 0.37 (the scan and a second bound per node). Profile of the first 100 ply-7 positions after E2:
  4.30 M nodes, of which 2 pieces left: 2.06 M, 3: 1.51 M, 4: 0.44 M, 5: 0.26 M. Nodes with two pieces
  left are now half of the tree, so a closed form or a tighter bound there (E3) is the next pot, and
  the cost of a node (E7) is worth more than before.
- **Where v2 stands at ply 5** (tune, first 20 positions of `ply05`): 37.9 M nodes, 28.1 s: 1.9 M nodes
  and 1.4 s a position. The reference reports about 0.18 M nodes and 0.54 s a position on its ply-5
  bench units (its machine). By pieces left: 2: 21.9 M (the closed-form leaves), 3: 7.3 M, 4: 6.9 M,
  5: 1.05 M, 6: 0.73 M. The side that must be refuted searches 6.6 moves a node at five pieces left
  and 3 at three; the winner's nodes search 1.0 to 1.4.

## 2026-11-14 E5: proof-number search (df-pn)

- **Hypothesis** (written before the run): the tree is very uneven. The winner needs one move a node
  and the loser, after threat zones, a handful. Depth-first proof-number search walks to the child that
  is cheapest to refute, and with threat zones the number of replies a move leaves is exactly that
  cost, so it should find the forcing lines that alpha-beta finds only when its static order happens to
  put them first. Same node logic as v2 (game over, last pieces, length bounds, threat zones), two
  questions a position ("does the mover win", then "does the opponent win"), its own table.
  Expected: unknown sign at ply 7 (shallow trees, re-expansion costs); a gain that grows with depth if
  the idea is right. The owner asked for a fast proof-number search by name.
- **Risks:** a node is expanded again each time the search returns to it; wide winner's nodes get one
  expansion per child before the search commits; proof numbers over-count in a graph with many
  transpositions.
- **Test:** flag `--pn` (off = v2, untouched). Labels on plies 9-11 first. Screen on ply 7 and ply 6
  tune against v2; tune the two knobs (threshold slack, start numbers by move rank) on tune units only;
  gate once on ply 6 (the harder file, because the question is what helps deep positions) with
  `SPRT[0, 0.05]`; scaling check on ply 5 tune.
- **First version (tune only):** correct on all 12,000 positions of plies 9-11 and the first 100 of
  ply 7, and far slower than v2: 66.7 M nodes and 151 s against 4.3 M and 2.35 s on those 100. Every
  child of a wide node is expanded once before the search commits, and a node is expanded again on
  every return. Threshold slack is the big knob: with the child's delta threshold at twice the
  second-best (`--pn-slack 1`) the first 30 positions take 9.0 M nodes against 52.5 M; start numbers by
  move rank help a little (6.6 M). Alpha-beta: 1.76 M. Not gated; set aside for the two measurements
  below, to be resumed as a hybrid (proof numbers above, alpha-beta below).

## 2026-11-14 Pot sizing: where can the nodes go?

- **Ordering oracle** (tune; `--oracle` solves each position twice, the second time trying the first
  solve's best move first where the table still has it): ply 7 (100 positions) 4.30 M to 2.81 M nodes;
  ply 6 (30) 5.96 M to 1.96 M; ply 5 (10) 15.1 M to 8.9 M. So better ordering is worth 1.5-3x here.
- **Where the nodes are** (ply 5 profile above): 98% at five pieces left or fewer. A node with five
  pieces left roots about 36 nodes, one with six about 51.
- **Reading:** the reference's 10x lead in nodes at ply 5 cannot come from ordering alone; it has to
  come from deciding nodes with five pieces left or fewer without searching them. E3 before E4.

## 2026-11-14 E3: can the mover still reach the length it needs?

- **Hypothesis** (written before the run): the cluster bound is loose near the end: it assumes every
  snake of a cluster joins and every remaining piece fits. A snake grows only at its two ends, so
  whether a player can still reach a length T is a small one-player search: place pieces that extend
  one snake, with the opponent frozen (its moves only take placements away, so "cannot reach T even
  alone" is a proof). At a node with at most five pieces left, before generating moves: if the mover
  cannot reach the opponent's present length (plus one when a draw is not enough), the node fails low.
  Expected: most loser-to-move nodes near the end are decided without a search; 2x or more fewer
  nodes at ply 7, more at ply 5, at a cost of some microseconds a test.
- **Risk:** the test costs as much as tens of nodes; it pays only if it cuts often.
- **Test:** flag `--no-reach` restores v2 (identity). `verify` gains a check: along random games no
  player ever reaches a length the test called unreachable earlier. Screen on ply 7 and ply 6 tune for
  the piece limit and the work limit, gate `E3` on ply 6, scaling check on ply 5.
- **Check built instead of the one planned:** `verify` compares the test, with no work limit, against
  the best of every sequence of placements (hands of up to three pieces), for every target from the
  present length to the optimum plus two. That is exact, where the planned check was only one-sided.
  `verify 300 11`: 8,666 such checks pass.
- **First version (tune):** the test for both players at five pieces left or fewer: nodes 4.30 M to
  2.36 M on the first 100 ply-7 positions, time 2.36 s to 2.72 s. Slower: 1.14 M tests at about 1.2
  microseconds, 41% deciding. By kind: the opponent's test with one piece in its hand decides 60%; a
  hand of three pieces decides 10%.
- **Second version (tune):** stop at the first placement that reaches the target, try the largest
  pieces first, generate only placements that touch the snake being built, reuse the node's clusters,
  and test a player only when it holds at most two pieces (at any depth). Ply 7, 100 positions: 2.40 M
  nodes, 1.82 s. Ply 6, 60 positions: 5.17 M nodes and 4.18 s against 10.29 M and 5.93 s. Hands of
  three: 4.82 M nodes but 4.46 s. Work limit 50 / 200 / 1000: 4.20 / 4.18 / 4.34 s. Kept: two pieces, 200.
- **Identity:** `--no-reach` gives 4,301,306 nodes on the first 100 ply-7 positions, as v2 does.
- **Correctness:** labels equal on all 12,000 positions of plies 9-11 and the first 1,000 of ply 8.
- **Gate `E3`** (ply 6, gate units 38-67, `SPRT[0, 0.05]`): PASS at unit 30, LLR +11.6. Where it stopped:
  +31.4% [+23.3%, +40.1%] (gate); total time 20.74 s against 27.74 s (0.75x); nodes 0.52x.
- **Scaling check `E3-UP`** (ply 5, tune units 0-2): 31.6 s against 47.9 s (0.66x); nodes 0.38x.
- **Verdict:** accepted. Frozen as v3.
- **What it taught:** a bound has to be priced. The same idea lost time in its first form and won 25-34%
  once a test cost a fraction of a microsecond. After E3 the first 100 ply-7 positions have 16 k nodes
  with six pieces left and 261 k with five: the side being refuted at six pieces left still tries 16
  moves a node. That is the next pot: more threats (so smaller zones) or better refutation order.

## 2026-11-15 Two cheap forms of the proof-number idea, inside alpha-beta (tune only, both dropped)

- **Why:** before putting more work into df-pn, test what it rests on: that the effort a move needs
  can be read from the number of replies it leaves, and that spreading effort over the moves of a
  node beats committing to the first.
- **Profile of v3 at ply 6** (tune, first 60 positions, `--profile`): below the root the winner's first
  move cuts 69-89% of the time, but at the root 1,745 moves are searched for 60 positions.
- **E5b, fewest replies first** (`--order-level L`: with at least L pieces left, order moves by the
  number of replies they leave after threat zones): nodes 5.17 M to 4.81 M at the root only (L = 9),
  4.55 M for L = 8, 4.26 M for L = 6; time 4.3 s to 4.2 / 6.2 / 25.5 s. The count is a weak predictor
  and costs a scan per move. Dropped.
- **E5c, budgeted rounds** (`--budget-level L --budget-nodes B`: every move gets B nodes, the undecided
  ones go to the next round with 4B): 35 M to 96 M nodes against 5.17 M, for every L in 7-9 and B in
  200-5000. Far worse. Dropped.
- **What it taught:** most moves at a winner's node are either wins that are expensive to prove or
  losses that are cheap to refute. Spreading effort pays for every expensive win in part; committing
  to one pays once. That is the reverse of the shape proof-number search is built for, and it
  explains the first df-pn's 60x deficit. The lever at the winner's nodes is a better first guess.
- **Reopen if:** positions appear where the first moves in order are expensive losses (deep ties).

## 2026-11-15 Study: what predicts a cheap winning move? (tune)

- **Tool:** `snek rootmoves FILE` solves every legal move of each position on its own (window "does
  this move win", empty table, 300 k node cap) and prints features of the move, the answer and the
  nodes. `tools/rootstudy.py` and `tools/rootfit.py` read it.
- **Ply 6, tune positions 0-39** (10,717 moves; 37 positions have a win): 69 of 270 moves win on
  average. Nodes an ordering pays at the root (every move up to and including the first win, summed):
  cheapest win first 0.61 M; any win first 1.82 M; the solver's static order 5.01 M (first move wins
  35%, mean rank of the first win 13.7). So the root alone holds a factor of 2.7 to 8.
- **Single features** that beat the static order when put in front of it: opponent's upper bound after
  the move low (first move wins 49%), few free cells at the opponent's snake ends (57%).
- **Logistic model** of "this move wins" on all 15 features, fit on the same positions: 1.90 M, first
  move wins 70%, mean rank 3.1. Largest weights: lead after the move (+), free cells at the opponent's
  ends (-), free cells at our ends (+), our mobility after the move (+).

## 2026-11-15 E4: order the upper moves by a fitted model

- **Hypothesis** (written before the run): with seven or more pieces left the static order (size,
  grows, blocks an end) finds a winning move first only about a third of the time; a logistic model
  on a few cheap features of the move finds one first about twice as often, and each miss costs a
  whole refutation. Fit on tune positions only (plies 6-8), validate on other tune positions, then
  use the integer weights in the solver for nodes with at least L pieces left. Expected from the
  study: up to 2.5x fewer nodes at the root of ply 6; less in total because lower levels keep the
  static order; more at ply 5.
- **Risks:** the model is fit on root positions, the tree holds stranger ones; feature cost at every
  upper node; the fit could be particular to these 40 positions (hence the validation files).
- **Test:** flag `--no-model` restores v3 (identity). Choose features and L on tune by node counts
  and time. Gate `E4` on ply 6 with `SPRT[0, 0.05]`; scaling check on ply 5 tune.
- **Data** (tune units only): `work/rm2_*.csv` from `snek rootmoves`: ply 6 positions 0-79 and 140-199,
  ply 7 positions 0-119, ply 5 positions 0-7; 49,892 moves in 209 positions with a win.
- **What predicts what, within a position** (`tools/rootcost.py`): a win is cheap when the move leaves
  few replies after threat zones (correlation with log nodes +0.44 to +0.50) and when our best next
  extension is large (-0.40); a move wins more often with a larger lead and next extension (+0.2).
  The median win costs 6 times the cheapest win of its position.
- **Fits** (`tools/rootfit.py`; cost = nodes up to and including the first win, summed): searching
  the weights on the cost itself overfits (train 2.5x better than static, test 1.0-1.9x); a plain
  logistic fit of "wins" transfers. Out of sample: 12 features, ply 6: 3.39 M to 1.84 M; ply 7: 1.00 M
  to 0.58 M. Eight cheap features (lead, size, grows, joined length, covers a cell at an opponent end,
  touches the opponent, free cells at our ends and at theirs after the move), all data: 11.1 M to
  6.9 M; twelve: 6.8 M. The ideal order (cheapest win first) would pay 1.8 M.
- **In the solver** (tune, ply 6 positions 200-259, not used for the fit; base 5.24 M nodes, 4.32 s):
  twelve features at 9 / 8 / 7 or more pieces left: 3.63 / 3.14 / 2.70 M nodes, 3.19 / 3.22 / 3.99 s (the
  four look-ahead features need a child position per move). Eight cheap features at 9 / 8 / 7 / 6 / 5:
  3.50 / 3.32 / 2.92 / 2.93 / 2.77 M nodes, 2.94 / 2.87 / 2.59 / 2.65 / 2.74 s. Kept: eight features, seven
  or more pieces left.
- **Identity:** `--no-model` gives 2,400,236 nodes on the first 100 ply-7 positions, as v3 does.
- **Correctness:** labels equal on all 12,000 positions of plies 9-11 and the first 1,000 of ply 8.
- **Gate `E4`** (ply 6, gate units 68-103, `SPRT[0, 0.05]`): PASS at unit 36, LLR +2.97. Where it stopped:
  +45.6% [+25.3%, +69.3%] (gate); over the 40 units run: total time 19.85 s against 27.44 s (0.72x),
  nodes 0.67x. **Flaw in the run:** five study processes were still using other cores, against the
  contract's "nothing else heavy running". Both arms ran interleaved under the same load and the node
  ratio does not depend on load, so the verdict stands; the size is measured again below.
- **Scaling check `E4-UP`** (ply 5, tune units 0-2): 23.7 s against 25.8 s (0.92x), nodes 0.87x: no harm,
  and a smaller gain than at ply 6. The fit has only seven ply-5 positions behind it.
- **Verdict:** accepted. Frozen as v4.
- **What it taught:** (1) the noise per unit is 0.45 for an ordering change against 0.18-0.24 for a
  pruning change: it wins big on some units and loses on others, so its gate needed 36 units where the
  others needed 30 at five times the margin. (2) Fitting to the cost itself overfits 200 positions;
  fitting "wins" does not. (3) The cheap features did as well in the solver as the expensive ones.

## 2026-11-15 E8: a parallel solve

- **Hypothesis** (written before the run): the score so far is thread-seconds; what the owner will feel
  is wall-clock ("solve non-start positions fairly quickly"). The tree has wide nodes where every move
  must be refuted, so threads that share the table and set aside a move another thread is already
  searching (simplified ABDADA) should divide that work with little waste. Expected on 8 cores / 16
  threads: 4-8x less wall-clock on positions that take a second or more, less on small ones; more
  nodes in total (duplicated first moves).
- **Risks:** a table shared without locks must never return a torn entry (a wrong value is the one
  unforgivable result): entries are stored as key ^ data and data and checked on read. The single
  thread path must not change.
- **Test:** (1) identity: one thread, node counts equal to v4 on the tune files; A/B time against v4 on
  ply 7 tune must show no loss (the table is now atomic words). (2) correctness with 16 threads: labels
  on plies 8-11 and the tune units of plies 5-7, twice. (3) wall-clock and total nodes for 1, 2, 4, 8, 16
  threads on ply 5 tune units. This is a second score (wall-clock), reported beside thread-seconds, so
  there is no SPRT gate against the single-thread base; the one-thread identity is the gate.
- **Built:** the table is now atomic words shared through an `Arc` (an entry is key ^ data and data); a
  `Busy` table of (position, move) pairs being searched; a thread sets a busy move aside and returns
  to it after the others (simplified ABDADA); every 256 nodes a thread looks whether the table now
  decides a node on its path and unwinds to it. `bench --threads N` and `src/par.rs` drive it.
- **Identity:** one thread gives v4's node counts exactly (1,897,508 on the first 100 ply-7 positions;
  10,481,020 on the 59 tune units).
- **Correctness, 16 threads:** two passes over all of plies 9-11, the first 1,500 of ply 8, 590 of
  ply 7 and 380 of ply 6: 28,940 solves, none wrong. `verify 100 9` passes.
- **Wall-clock** (tune, ply 5 positions 0-29; v4 one thread 23.9 s, 26.9 M nodes):

  | Threads | Seconds | Speed-up | Nodes |
  | --- | --- | --- | --- |
  | 2 | 14.0 | 1.7x | 30.6 M |
  | 4 | 8.8 | 2.7x | 37.7 M |
  | 8 | 6.1 | 3.9x | 45.5 M |
  | 16 | 6.1 | 3.9x | 61.0 M |

  The machine has 8 cores; the second 8 threads add nodes, not speed.
- **What mattered on the way** (tune): unwinding to a resolved ancestor 8.4 s to... little by itself;
  searching the first two plies jointly instead of spreading over alternatives 8.4 s to 6.3 s (at a
  node the winner must find one move, spreading puts one thread on each candidate); no cap on the
  set-aside list 7.3 s to 6.1 s (a long node sets aside far more than 32 moves); a plain pointer to the
  busy table instead of an `Arc` clone per node.
- **The one-thread cost** (tune, ply 7, 59 units, against v4, nodes identical): -1.9% [-3.1%, -0.7%]
  with the search compiled twice (plain and parallel copy); -2.6% with a cached bucket pointer on top;
  -1.9% with the profile counters compiled out as well; ply 6, 20 units: -1.5% [-2.4%, -0.5%]. Bisect:
  v4 plus the atomic table alone reads -1.2% [-2.3%, -0.1%]. So the shared table costs about 1% and
  the rest of the parallel plumbing about another 1%, and it moves by a point from build to build.
- **Verdict:** by the test written above ("no loss" with one thread) E8 does not become the base. It
  is exact and it is 3.9x faster in wall-clock on this machine, so it is kept whole on the branch
  `parallel`; `main` stays v4. Merging it (and accepting about 2% in thread-seconds) is the owner's
  call.
- **What it taught:** (1) in this tree the winner's nodes need one move, so parallel work must be
  shared inside the proof of the first move, not spread over alternatives; (2) a cap that "cannot be
  reached" was reached; (3) plumbing that is never executed still costs a point or two in a hot
  function, and compiling the function twice recovers only part of it.

## 2026-11-15 Where v4 stands on the reference's bench files

- **What:** the reference repository's five files of positions after five moves (`tests/set1/bench_ply5_*.txt`,
  126 positions), solved by `frozen/base.exe` (v4, one thread, empty table per position) and by
  `frozen/par.exe --threads 8` (branch `parallel`), one job at a time on a quiet machine. One pass each,
  so the seconds are a single reading (timing noise here is 1-2%).
- **Correctness:** the first four files hold wins for the side to move and the fifth 27 losses and 3
  draws, as the reference reports. Both builds gave exactly those values on all 126 positions.

  | File | Positions | v4, one thread | Parallel, 8 threads (wall-clock) | Reference v4 as published (its machine) |
  | --- | --- | --- | --- | --- |
  | `ply5_blue_wins` | 24 | 29.9 M nodes, 28.1 s | 7.1 s (52.7 M) | 4.40 M nodes, 13 thread-s |
  | `ply5_blue_wins_holdout` | 24 | 32.4 M, 29.9 s | 6.7 s (49.8 M) | 4.31 M, 14 |
  | `ply5_random_units` | 24 | 23.1 M, 21.4 s | 5.9 s (41.9 M) | 6.16 M, 18 |
  | `ply5_other_openings` | 24 | 16.4 M, 13.1 s | 3.1 s (25.4 M) | 7.45 M, 15 |
  | `ply5_ublock_refutations` | 30 | 85.0 M, 106.3 s | 22.6 s (130.0 M) | 37.37 M, 137 |
  | **All** | 126 | 186.9 M, 198.8 s | 45.5 s (299.8 M) | 59.7 M, 197 |

- **Reading it:** in thread-seconds v4 is level with the reference's published figures (198.8 against
  197), on a different machine: theirs was a Ryzen 7 250 "with other work running", ours a quiet
  Ryzen 7 5800XT, so this says "same order", not "faster" or "slower". The reference solver was not
  run here. The make-up differs: the reference searches 3.1 times fewer nodes (6.8 times fewer on
  `blue_wins`, 2.3 times fewer on the refutations) and pays about three times more for a node. So its
  pruning and ordering are ahead of ours where the mover wins, and our node is cheaper.
- **What it says to do next:** the gap is largest on positions the mover wins, which is where the
  order of moves decides the cost (IDEAS, Open 1), and the reference also runs its exact one-player
  bound at up to five pieces left where ours stops at hands of two (Open 3 and 4).
- **Wall-clock:** the parallel build solves the 126 positions in 45.5 s on 8 cores, 4.4 times less
  than one thread, for 1.6 times the nodes.

## 2026-11-15 Confirmation: every frozen version on the same unused units

- **What:** `tools/ladder.py` on unused confirm units, each version twice in alternating order, quiet
  machine, one thread. Fixed length, so these are the sizes to quote (a gate reads high). Keys
  `LADDER-7`, `LADDER-6`, `LADDER-5`; all values correct.

  | Version | Ply 7, 300 positions | Ply 6, 200 positions | Ply 5, 30 positions |
  | --- | --- | --- | --- |
  | v0 | 13.17 s, 39.7 M nodes | 50.96 s, 153.4 M | not run |
  | v1 | 9.06 s, 26.1 M | 35.42 s, 105.8 M | not run |
  | v2 | 5.92 s, 10.7 M | 22.64 s, 43.5 M | 43.03 s, 84.2 M |
  | v3 | 4.48 s, 5.7 M | 16.57 s, 20.9 M | 31.03 s, 36.2 M |
  | v4 | 3.99 s, 4.7 M | 10.10 s, 12.6 M | 15.70 s, 17.9 M |

- **v4 against v0:** 3.30 times faster at ply 7 (per-unit ratio +251% [+210%, +298%]) and 5.05 times at
  ply 6 (+420% [+308%, +562%]). At ply 5 v4 is 2.74 times faster than v2 (+168% [+94%, +272%], three
  units only). The gain grows with the depth of the position: each step cuts the tree, not the node.
- **v4 against v3** (the size of E4, whose gate ran on a loaded machine): 1.12 times at ply 7 (+18%
  [+7%, +31%]), 1.64 times at ply 6 (+70% [+34%, +116%]), 1.98 times at ply 5 (+91% [+29%, +184%]).
  The tune read at ply 5 (0.92x on three units) was low; this one has three units too, so the size at
  ply 5 is still loose, but the sign is not.

## 2026-11-15 Study: where a node's time goes

- **What:** a study build with a cycle counter around each section of `search` (script
  `timing_patch.py`, not committed), on the first 100 ply-6 tune positions (5.46 M nodes, 4.4 s).
- **Share of the time:** bounds 27%, reach tests 25%, threat scans 16%, generating and scoring moves
  14%, the closed-form leaves 8%, picking 3%, killer threats 3%, copy and play 2%, table 1.5%.
  The bounds cost about 1,300 cycles a node with three pieces left: more than everything else there.
- **The first move:** at nodes that search, the first move cuts 75% of the time (94-97% of the nodes
  that cut at all, below the root), and that move is the first killer in 91% of those cases
  (1.23 M of 1.33 M), the table move almost never (1,065).
- **What it says:** (1) the move list is generated and scored at every searching node, and thrown
  away at three in four: try the killer first. (2) Bounds and reach tests are half the time: both
  recompute everything at every node. (3) A scan recomputes the whole bound for each candidate
  placement although only the cluster it covers can change.

## 2026-11-15 E9: try the table move and the killers before generating the moves

- **Hypothesis** (written before the run): three in four searching nodes cut on their first move, and
  that move is a killer or the table move in 97% of those cases. Checking those up to three moves
  for legality one at a time and searching the first of them before the list is built removes the
  generation, scoring and picking (17.5% of the time) at those nodes. The order of search is
  unchanged, so the node counts must be identical to v4.
- **Expected:** 0.73 x 17.5% = about 12% less time at ply 6, minus the legality checks.
- **Test:** identity: node counts equal to v4 on the tune files of plies 5-7. Gate `E9`: time against
  v4, ply 6, `SPRT[0, 0.05]` on the log ratio, unused gate units. Same nodes, so the noise is the
  timing noise (1-2% a unit) and the gate should decide in under 20 units.
- **Identity:** node counts equal to v4 on the first 100 tune positions of ply 7 (1,897,508) and ply 6
  (5,459,285), with the flag on and off. `verify 60` passes.
- **Screen `E9-S`** (ply 6, tune units 0-19): +13.4% [+12.4%, +14.4%], time 0.879x.
- **Gate `E9`** (ply 6, unused gate units): PASS at unit 30. +11.4% [+9.8%, +13.0%], time 0.895x
  (16.91 s to 15.14 s), nodes identical (19,986,475).
- **Verdict:** accepted. Frozen as v5 (`c01a8bd36dee`, commit `68d5e9e`).
- **What it taught:** the prediction from the timing study (12%) was within a point of the result. A
  section timer in a study build is cheap and sizes a speed idea before it is written.

## 2026-11-15 Long solve: the position after four moves that the reference reports as a blue win

- **What:** `1:60 4B:16,27,28,39 4A:7,8,9,20 4I:15,25,35,45`, red to move. The reference reports that
  all 956 red replies lose. `frozen/par.exe bench work/long/ply4_blue_wins.txt --threads 8 --tt 24`
  (branch `parallel`, built from v4), nothing else running.
- **Result:** value -1 for red (blue wins), as published. 768.2 M nodes, 134.5 s wall-clock on 8
  threads (about 18 thread-minutes). One run.

## 2026-11-15 E10: ask the exact question first where it applies, and skip that side's cluster bound

- **Hypothesis** (written before the run): at nodes where the reach test applies to a player (a hand
  of one or two pieces), that player's cluster bound is redundant: the exact one-player test answers
  the same question and is run anyway unless the cluster bound cuts, which it does at only 12% of
  the nodes with three or four pieces left. The two cluster bounds are 27% of the time, 82% of that
  at those levels. So: ask the exact question first, compute a cluster bound only for a side with no
  exact test, and compute the mover's clusters only when a threat scan needs them. The reach test
  loses its cluster filter for the first piece; instead, when the target is more than the cells in
  hand, the first piece must touch an own snake (the final snake holds an old snake, and its pieces
  can be ordered outward from it).
- **Expected:** about 18% of v4's time, so about 20% less time than v5 at ply 6, if a reach test without
  the cluster filter costs no more than before. Node counts change a little (the rare cases where a
  cluster bound tightened the window).
- **Risks:** a wrong value is the unforgivable result: `verify` gets the new test against brute
  force, and the labels are checked on every set.
- **Test:** flag off gives v5's node counts exactly. Screen on ply 6 tune. Gate `E10`: time against v5,
  ply 6, `SPRT[0, 0.05]`, unused gate units.
- **Built:** `can_reach_touching` (no clusters; the first piece touches an own snake when the target
  is more than the cells in hand), checked against brute force in `verify`; in `search`, the exact
  tests come before the bounds, a side with a test gets no cluster bound, and the mover's clusters
  are computed when a scan first needs them. Flag `--no-lazy`.
- **Identity:** `--no-lazy` gives v5's node counts exactly (1,897,508 and 5,459,285 on the first 100
  tune positions of plies 7 and 6). `verify 100 3` passes (2,796 reach checks).
- **Correctness:** all of plies 8-11 (16,000 positions), 590 of ply 7, 380 of ply 6: none wrong.
- **Screen `E10-S`** (ply 6, tune units 0-19): +15.5% [+12.3%, +18.9%], time 0.859x, nodes 1.0015x.
- **Gate `E10`** (ply 6, unused gate units 138-167): PASS at unit 30. +16.7% [+13.3%, +20.3%], time
  0.857x (13.33 s to 11.43 s), nodes 1.003x.
- **Verdict:** accepted. Frozen as v6 (`ab0ef6e42b6d`, commit `5c09458`).
- **What it taught:** two tests that answer the same question were both being paid for. The cheap,
  loose one was a filter that cut one time in eight; the exact one was run the other seven times
  anyway. Less than the 20% expected: the test without the cluster filter costs a little more.

## 2026-11-15 Study: where v6's time goes

- **What:** the study build again (`timing2_patch.py`: cycles per routine, split by caller), on the
  first 100 ply-6 tune positions (5.47 M nodes, 3.5 s, about 15,800 M cycles).
- **Shares:** reach tests 37% (3.07 M tests at 1,890 cycles), threat scans 18% (371 K scans at 7,490
  cycles, of which 80% is 4.34 M upper bounds at 509 cycles), last pieces 8%, killer threats 5%,
  generating 4%, upper bounds in the search 3.5%, the rest (table, copy, play, the search itself) 25%.

## 2026-11-15 E11: in a threat scan, recompute only the clusters the placement covers

- **Hypothesis** (written before the run): a scan computes the mover's whole upper bound again for
  each candidate placement (11.7 times a scan), although a placement changes only the clusters it
  covers, and the player's shape and snake neighbourhoods do not change at all between candidates.
  Computing those once per scan and flooding only the covered clusters gives the same number for
  less: the node counts must be identical to v6.
- **Expected:** the bound inside a scan is 14% of the time; if it drops by 60%, about 8% less time.
- **Test:** identity by node counts on the tune files. Gate `E11`: time against v6, ply 6,
  `SPRT[0, 0.05]`, unused gate units.
- **Built:** `BoundCtx` (a player's snake lengths and neighbourhoods) and `upper_bound_cells` (the
  cluster loop over a given set of cells); `upper_bound_ex` now calls them, and a scan builds the
  context once and passes only the covered clusters. Flag `--no-narrow`.
- **Identity:** node counts equal to v6 with the flag on and off (1,901,619 and 5,467,404 on the
  first 100 tune positions of plies 7 and 6). `verify 60 5` passes.
- **Screen `E11-S`** (ply 6, tune units 0-19): +5.2% [+4.7%, +5.8%], time 0.951x. Less than expected:
  a covered cluster is usually the large one, so the flood is not much smaller.
- **Gate `E11`** (ply 6): **void, my mistake.** Only 24 unused gate units were left in the ply-6
  range (E3, E4, E9 and E10 used the rest) and the test takes no decision before 30 units, so this
  gate could only end undecided. I did not check before starting it. Its reading on those 24 fresh
  units: +5.3% [+4.4%, +6.2%], time ratio in line with the screen, nodes identical.
- **What I did about it:** `tools/run.py` now refuses a gate with fewer than 30 unused units. The
  ply-6 gate range is used up; gates from here on run at ply 7 (177 unused gate units). Because the
  void gate's reading was in favour, running a second one cannot select for a lucky result, so E11
  gets one properly sized gate at ply 7, declared here before it runs: key `E11-P7`, time against
  v6, `SPRT[0, 0.05]`, cap 100. The owner may overrule this and revert the commit.
- **Gate `E11-P7`** (ply 7, unused gate units 119-148): PASS at unit 30. +4.8% [+4.2%, +5.4%], time
  0.954x (2.94 s to 2.81 s), nodes identical (4,634,616).
- **Verdict:** accepted. Frozen as v7 (`d9bb32dbac68`, commit `5fd3fee`).
- **What it taught:** look at what is left of a range before spending it. A gate that cannot reach
  the test's minimum only burns fresh units.

## 2026-11-15 Study: what a reach test costs

- **What:** the study build with counters inside the reach test (`reachstudy_patch.py`), v7, first
  100 ply-6 tune positions. Reach tests are 37% of the time (3.07 M tests).

  | Hand | Answer | Tests | Cycles each | Trial positions built each |
  | --- | --- | --- | --- | --- |
  | 1 piece | no | 712 K | 740 | 0 |
  | 1 piece | yes | 340 K | 600 | 0 |
  | 2 pieces | no | 925 K | 4,070 | 7.5 |
  | 2 pieces | yes | 1,090 K | 1,200 | 1.6 |

- **Reading it:** the two-piece "no" answers are 64% of the reach time, a quarter of all time. Of
  them 500 K have a target that the longest snake plus both pieces cannot make (another snake has to
  be joined), 425 K a target within that count. Each builds 7 to 8 positions after the first piece
  and looks at every placement of the second: about 500 cycles a position.
- **Why so many:** the size check before a position is built adds the longest snake even when the
  first piece already sits on it, so it almost never fails; and the first piece is tried on every
  snake, small ones included.

## 2026-11-15 E12: the reach test starts from the longest snake of the target snake

- **Hypothesis** (written before the run): a final snake that holds old snakes can be built outward
  from its longest old snake. So the first piece may be taken to touch that one, every other old
  snake in it is no longer, and with two pieces at most one more snake comes in through the second
  piece, which must then lie within (size of the second piece + 1) steps of where it can attach.
  These three facts prune most first placements before a position is built, and a test whose target
  no snake can start is answered without generating anything. The answers do not change (the test
  stays exact), so node counts stay equal to v7 unless a work limit was being hit.
- **Expected:** half of the two-piece "no" cost and part of the one-piece cost: 10-14% less time.
- **Risks:** the test is the source of proofs: `verify` checks it against brute force over every
  sequence (hands of one to three), and the labels are checked on every set.
- **Test:** node counts against v7 on the tune files; gate `E12` at ply 7, `SPRT[0, 0.05]`, cap 100.
- **Built:** in `can_reach_touching`, for hands of one and two pieces: the snakes that can be the
  longest old snake of a snake of the target length (by size); the first piece touches one of them;
  before a position is built for the second piece, the size check counts only snakes not yet joined
  and no longer than the longest joined, and a snake that must still come in has to have an end
  within (second piece + 1) steps (`geo.within`). Switch `--no-longest`.
- **Identity:** node counts equal to v7 with the switch on and off (1,901,619 and 5,467,404).
  `verify 150 11`: 4,504 reach checks against brute force, all equal.
- **Correctness:** all of plies 8-11, 590 of ply 7, 380 of ply 6: none wrong.
- **Screen `E12-S`** (ply 7, tune units 0-29): +23.7% [+20.0%, +27.6%], time 0.827x.
- **Gate `E12`** (ply 7, unused gate units 149-178): PASS at unit 30. +28.0% [+22.2%, +34.2%], time
  0.800x (3.20 s to 2.56 s), nodes identical (5,251,293).
- **Verdict:** accepted. Frozen as v8 (`a5a28e371f18`, commit `2adb793`).
- **What it taught:** counting what a routine does (7.5 positions built per "no") found more than
  timing it. The size check was sound and almost useless, because it counted the same snake twice.
- **Process note:** I chained the freeze to the gate command in one line. The verdict was a pass, but
  a line like that would also have frozen a fail. The freeze now checks the verdict first.

## 2026-11-15 Side study: a second look at the best few moves (not built)

- **What:** `tools/rerank.py` on the root-move files of the earlier study (ply 5, 6, 7): order the
  moves by the model, then re-order the first K by a look-ahead feature and count the nodes up to the
  first win.
- **Result:** nothing that holds across files. Fewest replies first, K = 10: 0.89x, 1.08x, 0.85x on
  the three ply-6 files, 1.03x at ply 7, 1.22x at ply 5. The other keys are within a few percent
  of 1.00x or worse.
- **Where the root's cost is:** 21-55% is moves tried before the first win, and the win that is found
  costs about twice the cheapest one. The cheapest win alone would cost 3.2-4.3 times less at ply 6.
- **Verdict:** not built. The open idea "cheapest win first" stands, and these features are not it.

## 2026-11-15 E13: a reach test whose target needs a second snake asks first whether one is near

- **Hypothesis** (written before the run): when the target is more than a snake plus the cells in
  hand, another old snake has to be joined, and one of them must have an end within (cells in hand +
  1) steps of an end of the starting snake. 500 K of the 925 K two-piece "no" answers have such a
  target and cost about 2,000 cycles each in v8. A distance check per snake (a table of the cells
  within r steps) answers those where no snake is near without generating a placement. Answers do
  not change; node counts stay equal to v8.
- **Expected:** about 5% less time.
- **Test:** node counts against v8; `verify` (reach against brute force); gate `E13` at ply 7,
  `SPRT[0, 0.05]`, cap 100.
- **Built and reverted.** Identity held (node counts equal to v8, `verify 150 21` passed), and the
  time did not move: 1.013 s against 1.003 s on 100 ply-7 tune positions and 2.786 s against 2.780 s
  at ply 6, switch on against off in the same build (read beside the data-generation jobs, so only
  the comparison counts).
- **Verdict:** dropped at the screen; no gate run. `git checkout src/`.
- **What it taught:** I did not size the pot. Two pieces make the radius up to nine steps on a board
  eleven cells across: nearly every snake is "near". The check that does prune is the one E12 already
  has, for the second piece alone (radius up to five). *Reopen if* a tighter necessary condition
  than distance is found (free cells along the way, not steps).

## 2026-11-15 E14: after a failed move, moves that answer the refutation come first

- **Hypothesis** (written before the run): at a node where the first move failed, the opponent's
  reply that refuted it (the first killer one ply down) is likely to refute most other moves that
  leave it alone, each at the price of a subtree. The side-study above puts 21-55% of a root's cost
  in moves tried before the first win. Raising the score of the remaining moves that cover or touch
  that reply's cells should find the win sooner. Order only: values cannot change.
- **Expected:** unknown; a guess of 5-15% fewer nodes at ply 6 and 7, more at ply 5.
- **Test:** node counts on the tune units of plies 7 and 6 for a few sizes of the bonus and levels
  (screen); if a setting gives 5% fewer nodes on both, gate `E14` at ply 7, `SPRT[0, 0.05]`, cap 100.
- **Built:** in the move loop, after a move that did not cut: the first killer one ply down is the
  reply that refuted it; every remaining move that covers one of that reply's cells gets a bonus
  larger than any score, once per reply. `--answer-bonus 0` turns it off.
- **Screen, node counts** (tune; ply 7 positions 0-299 and ply 6 positions 0-149; v8: 5.64 M and
  8.02 M): cover or touch the reply 4.59 M and 6.48 M; **cover only 4.35 M and 5.62 M**; the threat
  zone of the reply 4.23 M and 5.97 M; a bonus per covered cell 4.54 M and 8.55 M (worse); only with
  seven or more pieces left 4.64 M and 6.47 M; small bonuses the same or a little worse. Chosen:
  cover only, bonus above all scores, every level. The choice among the first three is within the
  noise of an ordering change; I took the simplest.
- **At ply 5** (tune): 30 positions 26.87 M to 15.82 M nodes, 13.25 s to 8.21 s; ten of the
  refutation positions 38.03 M to 33.13 M.
- **Identity and correctness:** bonus 0 gives v8's node counts; all of plies 8-11, 590 of ply 7 and
  380 of ply 6 have the label's value.
- **Screen `E14-S`** (ply 7, all 59 tune units): +27.0% [+18.3%, +36.4%], time 0.771x, nodes 0.769x,
  sd 0.28 a unit.
- **Gate `E14`** (ply 7, unused gate units 179-208): PASS at unit 30. +24.7% [+14.6%, +35.7%], time
  0.836x (2.27 s to 1.90 s), nodes 0.863x.
- **Verdict:** accepted. Frozen as v9 (`2deb69fb855b`, commit `84e8f3b`).
- **What it taught:** the information a failed move leaves behind (which reply beat it) was being
  used only when that reply won by the bounds at once. Used as an order, it is worth more than the
  whole fitted model was at ply 7. The variants were chosen on tune units and the gate ran once.

## 2026-11-15 E15: use the last known refuting reply before the first move as well

- **Hypothesis** (written before the run): E14 uses the reply that refuted a move only after a move
  has failed at this node. When no table move or killer is available, the first move is picked
  without it, although the first killer one ply down (the reply that beat a move at a sibling) is
  often still the opponent's best reply here. A bonus at generation for moves that cover its cells
  should raise the first-move success. Order only.
- **Expected:** a few percent fewer nodes; the first move already cuts 94-97% of the time below the
  root, so the pot is the root and the nodes without a killer.
- **Test:** node counts on tune units of plies 7 and 6 for a few bonus sizes; gate at ply 7 only if
  a setting gives 5% fewer nodes on both.
- **Built and reverted.** Node counts (tune, ply 7 positions 0-589 and ply 6 positions 0-199; v9: 8.08 M
  and 7.14 M): bonus 32: 8.25 M and 7.18 M; 128: 8.10 M and 7.45 M; 1024: 8.26 M and 7.30 M. Nothing
  better than v9 at either ply.
- **Verdict:** dropped at the screen; no gate run.
- **What it taught:** the reply that refuted *this* node's move is information; a reply left over
  from a sibling is not, once the node's own killer has been tried.

## 2026-11-15 Side study: refitting the ordering model on five times the data (no change)

- **What:** every root move of all tune positions of plies 6 and 7 solved with v8 (`work/rm3/`,
  197,000 moves, 706 positions with a win), `tools/rootfit.py --win-only`, train on four files, test
  on two.
- **Result:** weights `21,17,14,-1,2,-2,8,-6` against the solver's `21,17,21,-1,1,-3,9,-7`: the same
  model. Held-apart cost 4.14 M nodes against 6.39 M for the static order and 1.57 M for the ideal;
  the first move wins 59% of the time.
- **Verdict:** no experiment. More data does not move these eight features; the gap to the ideal
  (2.6 times) needs different information. E14 found some of it in the search itself.

## 2026-11-15 E16: remember the reply that beat each move (a counter-move table)

- **Hypothesis** (written before the run): where the first killer does not cut, the node tries
  static-ordered moves. The reply that last beat the very move just played (by either side, anywhere
  in the tree) is a better first guess than a general killer in those cases, and the same move comes
  up in many sibling subtrees. A table of one reply per (side, move), tried with the killers before
  the list is generated. Order only.
- **Expected:** later cuts are 3-6% of the searching nodes below the root; if half become first-move
  cuts, a few percent fewer nodes.
- **Test:** node counts on tune units of plies 7 and 6, with the counter-move before and after the
  killers; gate at ply 7 only if a setting gives 5% fewer nodes on both.
- **Built and reverted.** Node counts (tune, ply 7 positions 0-589 and ply 6 positions 0-199; v9: 8.08 M
  and 7.14 M): counter-move after the killers 8.28 M and 7.83 M; before the killers 16.69 M and
  16.82 M (twice the nodes).
- **Verdict:** dropped at the screen; no gate run.
- **What it taught:** the killer is strong here because the same reply beats nearly every move at a
  node (the side being refuted has no good move); a reply tied to one particular move is a worse
  guess than the reply that just beat its sibling.

## 2026-11-15 E17: when the killer is blocked, try the moves that overlap it

- **Hypothesis** (written before the run): E14 makes the side being refuted cover the killer's
  cells. The killer is then illegal and the refuting side falls back on the static order. The moves
  most like the killer (sharing cells with it) are the natural next tries. A bonus at generation for
  moves that overlap a killer that is not legal here. Order only.
- **Expected:** a few percent fewer nodes.
- **Test:** node counts on tune units of plies 7 and 6 for a few bonus sizes; gate at ply 7 only if
  a setting gives 5% fewer nodes on both.
- **Built and reverted.** Node counts (tune, ply 7 positions 0-589 and ply 6 positions 0-199; v9: 8.08 M
  and 7.14 M): bonus 64: 8.36 M and 7.52 M; 256: 8.93 M and 8.61 M; 2048: 9.31 M and 9.02 M. Worse
  the larger the bonus.
- **Verdict:** dropped at the screen; no gate run.
- **What it taught:** three order ideas built on E14's success failed in ten minutes at no cost in
  gate units. Screening on node counts is what makes guessing affordable.

## 2026-11-15 E8 again: the parallel solve ported onto v9 (branch `parallel-v9`)

- **What:** the branch `parallel` was built from v4. The same changes (shared atomic table, busy
  table, set-aside moves, unwinding, the search compiled twice) were put onto v9 by a script
  (`parv9_patch.py`), fitted around v9's first-move step and answer bonus. Commit `d609ccb` on the
  new branch `parallel-v9`; a built copy is `frozen/par-v9.exe`. `main` is unchanged.
- **Identity:** one thread gives v9's node counts exactly (8,076,471 on ply 7 positions 0-589 and
  7,136,589 on ply 6 positions 0-199). `verify 40` passes.
- **Correctness:** 16 threads on all of plies 8-11, 590 of ply 7 and 380 of ply 6, then 8 threads
  on ply 8 and the same tune positions: 21,940 solves, none wrong.
- **Wall-clock** (tune, ply 5 positions 0-29; v9 one thread 8.16 s, 15.8 M nodes; the same build with
  one thread 8.22 s):

  | Threads | Seconds | Speed-up | Nodes |
  | --- | --- | --- | --- |
  | 2 | 4.69 | 1.74x | 17.3 M |
  | 4 | 3.06 | 2.67x | 22.4 M |
  | 8 | 2.44 | 3.35x | 30.6 M |
  | 16 | 2.18 | 3.74x | 34.5 M |

  The speed-up at 8 threads is a little lower than on v4 (3.9x): v9's tree is leaner, so there is
  less to share. In absolute terms the 30 positions went from 6.1 s (v4, 8 threads) to 2.4 s.
- **The ply-4 position again** (`1:60 4B:16,27,28,39 4A:7,8,9,20 4I:15,25,35,45`, 8 threads,
  `--tt 24`): value -1 for red as before; 570.2 M nodes, 62.2 s (v4-based build: 768.2 M, 134.5 s).
- **Not measured:** the one-thread cost of this build against v9 by SPRT (the 0.8% above is one
  reading). On v4 the same plumbing cost 1.9% [0.7%, 3.1%].

## 2026-11-15 Long solve: the position after three moves that the reference reports as a draw

- **What:** `1:60 4B:16,27,28,39 4C:40,41,50,52`, blue to move. The reference reports that red's 4C
  refutes all 938 blue moves (382 s on 5 threads on its machine) and that blue's `4A:49,59,70,81`
  does not lose, so the value is a draw. `frozen/par-v9.exe bench work/long/ply3_ublock.txt
  --threads 16 --tt 25`, nothing else running except this session's file edits.
- **Result:** value 0, as published. 1,765.8 M nodes, 139.8 s wall-clock on 16 threads (8 cores),
  about 37 thread-minutes. One run. This is the exact value of the whole position, both halves
  of the reference's statement in one search.

## 2026-11-15 v9 on the reference's bench files, and a second pair of ladders

- **What:** the five `bench/ply5_*` files of the reference repository (126 positions after five
  moves), each solved exactly by `frozen/base.exe` (v9, one thread, fresh table per position) and
  by `frozen/par-v9.exe --threads 16`. One run each on a quiet machine. The reference's numbers
  are the ones it publishes for its own v4 on its own machine; its solver was not run here.
- **Result:** every value correct (126 of 126, both builds).

  | File | Positions | v9 nodes | v9 seconds (1 thread) | v4 here (1 thread) | 16 threads, wall | Reference, published |
  | --- | --- | --- | --- | --- | --- | --- |
  | `ply5_blue_wins` | 24 | 23.9 M | 13.5 | 28.1 s | 2.9 s | 4.40 M nodes, 13 thread-s |
  | `ply5_blue_wins_holdout` | 24 | 23.9 M | 13.0 | 29.9 s | 2.8 s | 4.31 M, 14 |
  | `ply5_random_units` | 24 | 21.9 M | 12.0 | 21.4 s | 2.5 s | 6.16 M, 18 |
  | `ply5_other_openings` | 24 | 13.2 M | 6.3 | 13.1 s | 1.4 s | 7.45 M, 15 |
  | `ply5_ublock_refutations` | 30 | 78.7 M | 55.0 | 106.3 s | 9.2 s | 37.37 M, 137 |
  | Total | 126 | 161.5 M | 99.7 | 198.8 s | 18.8 s | 59.7 M, 197 thread-s |

- **Reading:** v9 is 1.99x faster than v4 on these files in one thread and, at face value, 2.0x
  faster than the reference's published thread-seconds. The second comparison is across two
  machines and is not a measurement. The reference still searches 2.7x fewer nodes: its move
  order and bounds are better than ours, and our nodes are about 5x cheaper. That gap is where
  the next gains are.
- **Caveat:** the first ten ublock positions and 30 ply-5 tune positions were looked at while E14
  was screened, so those ten are not unseen. The other 116 positions were not used for any choice.
- **LADDER-7B** (ply 7, confirm units 326-354, 290 positions, mean of two passes): v4 3.39 s,
  4.12 M nodes; v8 1.92 s, 4.12 M; v9 1.65 s, 3.51 M. v4 against v9: +107% [+92%, +124%] per unit;
  v8 against v9: +18.0% [+8.9%, +27.9%].
- **LADDER-6B** (ply 6, confirm units 212-229, 180 positions): v4 7.82 s, 9.16 M nodes; v8 4.48 s,
  9.18 M; v9 3.28 s, 6.70 M. v4 against v9: +139% [+108%, +175%]; v8 against v9: +36.6%
  [+18.5%, +57.4%].
- **What it says:** every version since v4 holds on units no decision touched. E14 (v9) confirms at
  +18% (ply 7) and +37% (ply 6) against its gate reading of +24.7%; the further from the end of
  the game, the more the order matters. With the first pair of ladders (v0 to v4: 3.30x at ply 7,
  5.05x at ply 6) the whole climb is about 6.8x at ply 7 and 12x at ply 6. Those two products
  multiply readings from different units and are rough.
- **Confirm units left:** none at ply 7 and one at ply 6. A later confirmation needs new positions.
