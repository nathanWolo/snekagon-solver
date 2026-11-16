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

## 2026-11-15 E18: a proof driver that can be stopped and resumed (`prove`, branch `parallel-v9`)

- **Hypothesis.** A coordinator that searches the first plies and hands whole positions to
  single-thread workers wastes fewer nodes than the shared-table solve (whose nodes grow 1.9x at 8
  threads), and its results can be kept in a file so that a long solve survives a stop. Expected:
  up to 2x better use of 16 threads on positions of several minutes. Bar: exact on the labelled
  sets; faster than `frozen/par-v9.exe` on the ply-4 and ply-3 positions of the long solves.
  (Process note: this was in my working notes, not in this log, before the build.)
- **Built** (commit `a787b1f` on `parallel-v9`, `src/prove.rs`; a built copy is
  `frozen/prove.exe`): `snek prove FILE --threads N --job-threads T --depth D --out RESULTS`.
  The coordinator searches D plies with the solver's move order, threat zones, killers and the
  answer rule of E14; positions below are jobs for N workers on one shared table; every finished
  job and every finished coordinator node is appended to RESULTS, which a later run reads first.
  The search itself is untouched: one thread still gives v9's node counts (8,076,471 at ply 7).
- **Correct:** every label on ply 8 (4,000), ply 7 (590), ply 6 (200) and the five bench files
  (126), in each of the forms below. A results file cut to its first 150 lines is picked up and
  the proof finished with the same value; a complete file answers at once.
- **What was learned on the way** (16 threads, one run each, nothing else running):

  | Step | Bench files (5) | Ply-4 position (-1) | Ply-3 position (0) |
  | --- | --- | --- | --- |
  | Shared-table solve `par-v9`, one wide window (the reference point) | 18.8 s | 51.9 s, 622 M nodes | 139.8 s, 1,766 M |
  | 1. Driver, young brothers wait, wide window | 19.5 s | 58.1 s, 748 M | not run |
  | 2. Least speculative job first | not run | 43.5 s, 530 M | 528 s, 7,410 M |
  | 3. Two passes with windows of one step | 19.8 s | 45.1 s, 546 M | 191 s, 2,580 M |
  | 4. Killers travel with the tree | 18.6 s | 42.8 s, 556 M | 222 s, 2,977 M |
  | Shared-table search asked the same two narrow questions (1 job of 16 threads, depth 0) | not run | not run | 194.8 s, 2,526 M |
  | Root moves one at a time, each a job of 16 threads (depth 1) | 47.2 s | 61.4 s, 895 M | not run |

  Depth: 2 for the bench files and the ply-4 position, 3 for the ply-3 position (depth 2 there:
  222 s at step 3).
- **Step 2, why:** plain young-brothers-wait let all free workers pile onto the alternatives of
  the oldest node that needs one good move, ahead of root moves that had to be searched anyway.
  Now a job costs 1, 2, 3 ... for each node on its path where it runs beside 1, 2-3, 4-7 ...
  older moves that may still cut, and the cheapest job goes first. Nodes on the ply-4 position
  fell from 748 M to 530 M, below the shared-table solve.
- **Step 3, why:** with the window (-1, 1) at a drawn root, several drawing moves were under way
  at once, each with the window it was given before the first of them raised alpha, so each got
  the full exact proof that only one needs. 7.4 billion nodes. Two passes ("can the mover at
  least draw", "can the mover win") have no window that can move.
- **The finding that matters:** on the drawn ply-3 position the two passes cost 1.4x a single
  wide search with the same engine (194.8 s against 139.8 s), and the driver equals the engine
  when both ask the same questions (191 s against 194.8 s). So the driver's scheduling is not
  the loss; splitting a drawn position into two passes is. (My first guess at why, "the win is
  the proof the search finds faster", was tested in the next entry and is wrong.)
- **Verdict against the bar:** exact: yes. Faster than `par-v9`: yes on the ply-4 position
  (42.8 s against 51.9 s), no on the ply-3 position (191-222 s against 139.8 s). Kept on the
  branch for what it adds (a solve that can be stopped and resumed, with progress by root move);
  not a speed gain as it stands. One run per cell: the differences of 5-10% between steps 3 and
  4 are inside what two runs of a parallel search differ by, the large ones are not.
- **What would make it a gain:** see the next entry: one pass where the value is decisive and the
  first question is the right one; a wide window with one move at a time where a draw is likely.

## 2026-11-15 Study: what the search window costs (after E18)

- **Question.** E18 left a guess: two narrow passes lose to one wide search because a win is a
  cheaper proof than "at least a draw". Tested directly on labelled tune positions, by the label.
- **How:** `snek bench FILE --only LABEL --window A,B` (branch `parallel-v9`, commit `639d659`),
  v9 search, one thread, fresh table per position. Node counts are exact; the solve on ply 2 was
  running beside this, so the times are not used.

  | Positions (value for the mover) | Wide (-1, 1) | "At least a draw" (-1, 0) | "A win" (0, 1) |
  | --- | --- | --- | --- |
  | Ply 7, wins (355) | 2.43 M | 1.39 M | 1.99 M |
  | Ply 7, draws (35) | 1.62 M | 0.54 M | 1.32 M |
  | Ply 7, losses (200) | 4.03 M | 3.64 M | 2.61 M |
  | Ply 6, wins (185) | 6.05 M | 3.80 M | 5.68 M |
  | Ply 6, draws (3) | 0.27 M | 0.08 M | 0.25 M |
  | Ply 6, losses (12) | 0.81 M | 0.84 M | 0.49 M |

- **What it says.**
  1. The guess was wrong. The weaker true statement is the cheaper proof: for a won position "at
     least a draw" takes 0.57-0.63x the nodes of the wide search and "a win" 0.82-0.94x; for a
     lost one "cannot win" takes 0.60-0.65x and "cannot even draw" 0.90-1.03x.
  2. A decisive position solved by the right single pass costs 0.82-1.03x the wide search. The
     wrong pass first costs both: 1.39x for the wins at ply 7 (1.39 M + 1.99 M against 2.43 M).
  3. A drawn position needs both passes and costs 1.15x (ply 7) to 1.23x (ply 6, three positions)
     the wide search. On the drawn ply-3 position it was 1.39x (194.8 s against 139.8 s). The
     wide search shares work between the two proofs that two passes do twice.
- **For the driver:** two passes are right when the value is decisive and the first question is
  chosen well, and wrong for a draw. Where the root moves are searched one at a time (jobs of
  several threads, depth 1) a wide window at the root is safe and is the better default; the
  trouble in E18 came only from several wide-window moves under way at once.
- **For the solve of ply 2** (running): its second pass asks of each red move "does blue at least
  draw", which is the question a wide search would ask after its first drawing move, so nothing
  is lost there.

## 2026-11-15 E19, E20, E21: three more order ideas, dropped at the screen

Screened on exact node counts while the ply-2 solve was running (one thread; the counts do not
depend on load). Tune positions: ply 7, 0-589 (v9: 8,076,471 nodes) and ply 6, 0-199 (7,136,589).
The bar, as for E15-E17: 5% fewer nodes on both, or no gate. Each was built behind an option on a
copy of the search, the option off gave v9's counts exactly, and the source was reverted.

- **E19: the answer rule with the whole zone.** Hypothesis: E14 prefers moves that cover a cell of
  the reply that refuted the last move. A reply that starts a new snake is also made illegal by a
  move next to it, so those moves should get the bonus too (the zone the threat rule uses).
  Result: ply 7 8,069,607 (-0.1%), ply 6 7,512,568 (+5.3%). Dropped.
- **E20: the same bonus, halved, for the second killer one ply down.** Result: ply 7 8,402,919
  (+4.0%), ply 6 8,162,263 (+14.4%). Dropped. With the zone as well: +4.4% and +14.9%.
- **E21: a history table.** Hypothesis: the killers hold the last two moves that cut at a ply; a
  table by player and placement of how often a move cut anywhere (decayed) adds what they forget,
  as in chess engines. Added to the move score with a weight.

  | Weight | Ply 7 | Ply 6 |
  | --- | --- | --- |
  | 4 | 7,645,662 (-5.3%) | 7,287,587 (+2.1%) |
  | 16 | 8,002,285 (-0.9%) | 7,521,289 (+5.4%) |
  | 64 | 8,208,743 (+1.6%) | 7,555,596 (+5.9%) |
  | 256 | 9,136,107 (+13.1%) | 8,424,616 (+18.0%) |
  | 64, cuts counted by pieces left | 8,032,426 (-0.5%) | 7,579,846 (+6.2%) |

  Dropped: the only setting that helps one set hurts the other.
- **What they taught:** six order ideas after E14 have now failed on node counts (E15-E17, E19-E21).
  The order above the model is at a local best for rules of this kind; the next gain in nodes
  needs a different kind of information (IDEAS, "cheapest win first"), or better bounds.

## 2026-11-15 Long solve: the position after two moves is a win for red

- **What:** `1:60 4B:16,27,28,39`, red to move. The reference reports that red's `4C:40,41,50,52`
  draws here; whether red can win was open. Solved with the proof driver:
  `frozen/prove.exe prove work/long/ply2.txt --threads 1 --job-threads 16 --depth 1 --tt 27 --out
  work/long/ply2_results.txt` (build `ef60f52`): the root's moves one at a time, each solved by 16
  threads on one shared table, every root move's result appended to the results file.
- **Seed:** the results file started with one line, the value 0 of the position after red's 4C
  (the ply-3 long solve above, which agrees with the reference). That answered the first question
  ("can red at least draw") at once. A first attempt without the seed was stopped after five
  minutes inside that question.
- **Result: value +1. Red wins with `4B:70,81,82,93`**, the 72nd move in the order. The 4B piece
  goes on the far side of the centre from blue's piece and joins red's centre cell, a snake of 5.
  - Root moves 2 to 71 (4C, 4A and 4B placements): after each, blue has a reply that at least
    draws. 70 jobs, 20.6 billion nodes, 1,549 s; the cheapest 1.1 s, the median 13.8 s, the
    dearest 94 s. Blue's holding reply was one of the last two that had held in 25 of these
    (mean 11.8 s) and another one in 42 (mean 28.1 s).
  - Root move 72: every blue reply loses. 4.32 billion nodes, 318 s.
  - Whole run: 24.96 billion nodes, 1,868 s (31 minutes) on 16 threads.
- **Checks, because the result is new and matters:**
  1. `frozen/par-v9.exe bench work/long/ply3_redwin.txt --threads 16 --tt 27` (the plain
     shared-table solve: another binary, a cold table, the wide window) on the position after red's
     move: value -1 for blue. 4.95 billion nodes, 371 s.
  2. `frozen/par.exe` (the parallel solve built from v4, without E9, E10, E11, E12 and E14) on the
     same position, 16 threads: value -1 for blue. 14.73 billion nodes, 1,745 s (other runs of mine
     were going beside it for part of that time).
  3. What stands behind every engine: all labelled positions of plies 5 to 11 (about 17,000, values
     from the reference's data), and the two long solves that reproduce the reference's published
     results for this very opening (the ply-4 blue win, the ply-3 draw after 4C).
- **Not checked:** the reference solver was not run on this position (it is downloaded code and
  was not run here at all). A proof by an independent program is the check that would settle it
  for a reader who does not trust this one; the command for the owner is in the report.
- **What it means:** blue's reply `4B:16,27,28,39` to the centre opening loses. With the
  reference's result that this reply wins against red's `4A:7,8,9,20`, the reply is sound only
  against the wrong answer. Whether blue has any reply to `1:60` that holds is now the question
  that decides the centre opening, and with twelve symmetries there are about 135 replies to
  check, not 1,589. (Corrected in the next entry: 172 classes of 1,932 replies.)

## 2026-11-15 E22: a table lookup of every child before the first move (dropped at the screen)

- **Hypothesis:** high in the tree many positions are reached by the same moves in another order.
  Before a node's moves are searched, look up the position after each move in the table; one
  that is already known to be worth beta cuts without a search (the "enhanced transposition
  cutoff" of chess engines). The reference tried it and reports no gain; it was not tried here.
  Expected: a few percent of nodes at the levels where a lookup per move is cheap against a
  subtree. Bar for a gate: 5% of nodes on both sets at no cost in time.
- **Build:** `--etc L`: the lookup at nodes with at least L pieces left (99 = never, the
  default). Patch `e22_patch.py`; with the option off the node counts are the base's exactly.
- **Screen (v9, one thread, all labels correct; seconds are from a loaded machine and only
  show the direction):**

  | `--etc` | Ply 7 nodes (590 positions) | s | Ply 6 nodes (200 positions) | s |
  | --- | --- | --- | --- | --- |
  | off | 8,076,471 | 11.4 | 7,136,589 | 10.2 |
  | 8 | 8,076,471 | 11.9 | 7,136,589 | 10.3 |
  | 7 | 8,076,471 | 11.8 | 7,142,825 (+0.1%) | 10.9 |
  | 6 | 8,110,196 (+0.4%) | 12.1 | 7,121,144 (-0.2%) | 11.2 |
  | 5 | 8,068,010 (-0.1%) | 14.8 | 7,064,633 (-1.0%) | 13.3 |
  | 4 | 8,038,268 (-0.5%) | 16.7 | 7,070,840 (-0.9%) | 14.5 |
  | 3 | 8,038,315 (-0.5%) | 20.1 | 7,070,836 (-0.9%) | 16.9 |

- **Verdict:** dropped. At most 1% of nodes, and the lookups cost 30-75% more time at the
  levels where they find anything. It agrees with the reference's finding. The search already
  gets these cuts one ply later: a child that the table knows returns at once when it is
  searched, so the lookup only saves the moves ordered before it, and the order already puts
  the table move and the killers first.
- **Reopens if** the order of moves gets worse for another reason (so that the cutting child
  comes late), or in the proof driver's top plies, where one saved move is minutes and the
  lookup is free. The driver already does that with its results file ("known results first").

## 2026-11-15 Long solve under way: the centre opening `1:60`

- **Question:** does every blue reply to `1:60` lose? Then the centre opening is a win for red,
  and so is the start position (red chooses the opening). Expected from the solve above: about
  5 minutes for a reply whose refutation is tried early, hours for a reply that holds (all of
  red's moves must then be shown not to win).
- **Run:** `frozen/prove.exe prove work/long/ply1.txt --threads 1 --job-threads 16 --depth 2
  --tt 27 --out work/long/ply1_results.txt` (build `22d36ad`), started at 04:18. The
  coordinator holds the first two plies (blue's reply, red's answer); each position after them
  is one job for 16 threads on the shared table. The results file was seeded with the results
  of the solve above (73 positions), so the reply `4B:16,27,28,39` was done at once.
- **New in this build, not measured against the build without them:** moves that a symmetry of
  the position maps onto each other are searched once; a move that cut at a coordinator ply is
  tried first at its sibling nodes, and so are its twelve mirror images ("winners").
- **The count, checked without the solver:** `tools/certificate.py` builds the replies from the
  piece shapes: 1,932 blue replies (a first piece may not touch the centre tile) in 172
  classes under the twelve symmetries. The driver searches 172 root moves. This corrects
  the entry above, which guessed "about 135 replies, not 1,589": 1,589 is the number of red
  moves in the position after two moves.
- **Progress at 06:33: 31 of 172 classes are refuted, 141 are open.** No reply holds so far.
  133 minutes and 102.4 billion nodes for the 30 replies done in this
  run, 4.4 minutes a reply.

  | Blue's reply (cells) | Red wins with | Red moves tried | Seconds |
  | --- | --- | --- | --- |
  | 9,19,28,29 | 4B:36,37,48,49 | 3 | 174 |
  | 16,27,28,39 | 4B:70,81,82,93 | seed (72, 31 min) |  |
  | 17,19,28,29 | 4C:61,63,72,73 | 1 | 136 |
  | 17,27,36,37 | 4C:29,30,39,50 | 1 | 188 |
  | 18,20,29,30 | 4C:61,63,72,73 | 2 | 158 |
  | 18,28,37,38 | 4B:41,42,50,51 | 3 | 208 |
  | 19,29,38,39 | 4B:36,37,48,49 | 1 | 185 |
  | 20,30,39,40 | 4B:36,37,48,49 | 4 | 248 |
  | 27,28,37 | 4B:41,42,50,51 | 1 | 203 |
  | 27,37,46,47 | 4C:29,30,39,50 | 5 | 247 |
  | 28,30,39,40 | 4C:51,52,61,63 | 1 | 400 |
  | 28,38,47,48 | 4C:29,30,39,50 | 3 | 357 |
  | 38,39,48 | 4C:71,72,83,93 | 4 | 322 |
  | 38,48,57,58 | 4C:27,28,39,49 | 2 | 200 |
  | 40,42,51,52 | 4C:27,28,39,49 | 2 | 500 |
  | 48,58,67,68 | 4B:18,28,39,49 | 2 | 168 |
  | 51,53,62,63 | 4B:18,28,39,49 | 1 | 191 |
  | 53,62,63,72 | 4B:71,81,92,102 | 8 | 555 |
  | 62,72,81,82 | 4C:51,52,61,63 | 8 | 264 |
  | 63,72,73,82 | 4C:71,81,92,93 | 2 | 339 |
  | 63,73,82,83 | 4C:71,81,92,93 | 1 | 220 |
  | 72,81,82,91 | 4B:30,39,40,49 | 1 | 211 |
  | 72,82,91,92 | 4B:36,37,48,49 | 1 | 100 |
  | 73,82,83,92 | 4B:18,28,39,49 | 1 | 205 |
  | 73,83,92,93 | 4B:36,37,48,49 | 1 | 77 |
  | 80,82,91,92 | 4B:30,39,40,49 | 2 | 218 |
  | 81,82,83 | 4B:36,37,48,49 | 1 | 420 |
  | 81,83,92,93 | 4B:57,58,69,70 | 2 | 289 |
  | 81,91,100,101 | 4B:30,39,40,49 | 1 | 94 |
  | 81,92,103,114 | 4B:18,28,39,49 | 21 | 840 |
  | 91,93,102,103 | 4B:71,80,81,90 | 1 | 237 |

  The winning move was red's first try for 14 of the 30 replies done in this run (tries,
  sorted: 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 5, 8, 8, 21). The first reply of a new kind is the dear one: against the
  first straight four (`4I:81,92,103,114`) red's win was the 21st try, 14 minutes. Red's winning
  moves so far: `4B:36,37,48,49` (6), `4B:18,28,39,49` (4), `4C:29,30,39,50` (3), `4B:30,39,40,49` (3). All of them grow the centre tile with a piece of four.
- **What this does not show:** nothing about the 141 open classes. The order has taken the
  four-cell replies first; whether the small pieces are as quick to refute is not known. At the
  rate so far the rest is about 10 hours of this PC, more if a reply holds.
- **To go on:** the run can be stopped at any time and the same command continues from the
  results file. `python tools/certificate.py work/long/ply1_results.txt` lists what is proved
  and says so when every class has a reply that loses.
- **Before the result is claimed:** each line of the certificate is a position that another
  program can check (after `1:60`, blue's reply and red's move, blue to move loses). With
  `frozen/par-v9.exe bench` that is about 172 times 5 minutes; the reference solver would be
  the independent check.

## 2026-11-15 Pot sizing: would a proof-number search with better heuristics beat alpha-beta? (no build)

- **Question** (the owner's, 2026-11-15): alpha-beta leads through its move order and its
  depth-first commitment, and a proof-number search can be given both. E5 was the textbook form
  on v2 (start numbers from the reply count and the rank, every child expanded once), so its
  28-60x says little about a good one. Asked before building: with the same order, what is left
  for a proof-number search to win?
- **What it adds over the same order:** it moves effort between the candidates of a node as their
  numbers rise. That pays when the first move in order is dear and a later one is cheap. Two
  readings of recorded data, no new solves.
- **1. Spread against commit** (`python tools/spreadsim.py work/rootmoves_ply06.csv ...` on the
  four files of `snek rootmoves`; won positions only; moves in the model's order; each move's
  nodes are those of its own solve; moves of unknown value count as not winning). "Spread k"
  keeps the k best unsolved moves alive at the same rate, a refuted move leaves and the next
  joins, and it stops at the first proof: a proof-number search with a window of k that knows
  nothing of costs and pays nothing to come back to a node.

  | Policy | Ply 6 (37 positions) | Ply 6b (37) | Ply 7 (49) | Ply 8 (101) |
  | --- | --- | --- | --- | --- |
  | Commit: in order, each to the end (alpha-beta) | 2.60 M nodes | 1.15 M | 0.52 M | 0.16 M |
  | Spread 2 | 1.22x | 1.62x | 1.27x | 1.22x |
  | Spread 3 | 1.47x | 2.16x | 1.50x | 1.46x |
  | Spread 5 | 1.98x | 3.12x | 1.89x | 1.87x |
  | Spread 3, each place a quarter of the rate above | 1.11x | 1.23x | 1.11x | 1.10x |
  | Only the first winning move in the order | 0.45x | 0.79x | 0.39x | 0.37x |
  | Only the cheapest winning move | 0.23x | 0.32x | 0.22x | 0.22x |

  The model's first move wins in 62%, 76%, 67% and 64% of these positions.
- **2. Where the centre-opening run spends its time** (`python tools/jobsplit.py
  work/long/ply1_results.txt`, the 70 replies refuted in the run by about 11:50): red's winning
  tries 70 jobs, 17,856 s (67%), median 208 s, quartiles 142 and 322 s, dearest 733 s; red's
  failed tries 158 jobs, 8,728 s (33%), median 32 s, dearest 322 s; 41 failed tries took more
  than 60 s (5,844 s together). The first try won for 34 of the 70.
- **Reading.**
  1. Spreading without knowledge of costs loses at every window, 10% in its mildest form. With
     the model's order a proof-number search is at its best when it commits as alpha-beta does.
  2. The pot is real and it is a pot for prediction: a perfect guess of win or loss is worth
     1.3-2.7x, a perfect guess of the cheapest win 3-4.5x (the earlier study, against the order
     of that time, said 5-8x at ply 6). Proof numbers are one such guess. Their native signal, the reply count,
     goes with the log of the cost among winning moves at +0.40 within a position
     (`tools/rootcost.py`), and as a move order it lost time (E5b).
  3. At the top of the long proof a failed try costs a fifth to a sixth of a winning one (mean,
     median), and a third of the
     time goes to failed tries. Leaving a dear failing try early can recover at most part of that
     third.
- **Limits.** The simulation treats the moves of a node as independent (no shared table) and
  spreads blindly; a real search lies between "spread" and "cheapest win", nearer the first unless
  its numbers predict cost. The positions are plies 6-8. Nothing here is a timed run of a
  proof-number search with the model's order: this is a prediction (near parity with alpha-beta
  at best), not a result.
- **From the owner's link** (minimax.dev, a solver for Ultimate Tic Tac Toe after Pawlewicz's
  Hex solver): a proof-number tree kept in memory at the top, df-pn workers below with a work
  budget, "virtual" proof numbers so that free workers pick different nodes, and a node split
  into its children once enough work has gone into it. The proof driver has the same skeleton
  with alpha-beta workers. It lacks the work budget (a job runs to its end), the split by work
  done (the depth is fixed) and many single-thread jobs at a node whose children are all needed
  (the long run uses one job of 16 threads at a time; on the ply-4 position 16 single-thread
  workers took 42.8 s against 51.9-61.4 s, E18).
- **Would settle it:** df-pn on v9 with start numbers from the model, a window on the best few
  children, and alpha-beta below six pieces, screened on node counts at plies 6 and 7 against v9.
  Not built; the owner's call.

## 2026-11-15 The centre-opening run is paused; speed comes first

- **Owner, 11:56:** "you can pause the currently running super long job. im more interested in us
  hill climbing our efficiency to get faster proofs right now than just brute forcing by running
  the pc for days".
- **State at the pause:** 74 of 172 classes refuted, 0 hold, 98 open; 27,481 s (7.6 hours) on 16
  threads, 6.3 minutes a reply. Nothing is lost: `work/long/ply1_results.txt` (copy at the pause:
  `ply1_results_paused.txt`), the first run's progress lines in `work/long/ply1_run1.err`.
- **What the run leaves for the climb:** 302 solved positions three moves in with their values
  and their cost on 16 threads (70 that blue loses, median 208 s; 158 that blue holds, median
  32 s). These are the real work of a proof and can serve as its benchmark.

## 2026-11-15 E23: df-pn with the model's order, a window and alpha-beta leaves

- **Hypothesis** (the owner's; written before the build): the proof-number search of E5 lost
  because it had neither alpha-beta's move order nor its commitment. Given the model's order
  for its start numbers, a window on the best few children, and alpha-beta below a level, it
  should close the gap, and its one own ability (to leave a move that turns out dear) may then
  win.
- **Expected** (from the pot sizing above): with leaves solved in one go the upper layer does
  what alpha-beta does and the nodes come out within about 10% of v9; with leaves that return
  after a budget the search spreads effort and loses 10-25%. A gain would be a surprise and
  would show most at ply 5.
- **Build** (branch `main`, options of `--pn`, off by default; `--pn` alone must give the old
  counts): `--pn-ab L` a node with at most L pieces left is a leaf solved by the v9 search with
  the window of the question; `--pn-unit U` a leaf returns after its budget and its numbers are
  the nodes spent so far divided by U (the budget comes from the thresholds); `--pn-model` the
  children in the model's order; `--pn-width W` only the best W unrefuted children count;
  `--pn-probe` the children's numbers are read from the table when a node is expanded.
- **Test:** labels on plies 8-11 first. Then tune units, one thread, a fresh table for each
  position, the machine quiet: ply 7 positions 0-589, ply 6 positions 0-199, ply 5 positions
  0-29, against `frozen/base.exe` (v9). Seconds and nodes (the search's own count plus the
  leaves' alpha-beta nodes).
- **Bar:** at least 5% less time than v9 on ply 6 and on ply 5 for a gate. Otherwise it is
  dropped as a speed gain; the options stay if they beat the old `--pn`.
- **Built** as planned, plus `--pn-killers` (the proof-number layers share the alpha-beta
  solver's killers: a move that proved the mover's goal at a ply goes first, and after a refuted
  move the moves that cover a cell of the refuting reply) and `--pn-mul` (the slope of the start
  numbers). `snek children FILE` lists the positions one move after each position of a file.
- **Identity:** without `--pn` the counts are v9's (8,076,471 on ply 7 positions 0-589, 7,136,589
  on ply 6 positions 0-199); `--pn` alone gives the old counts (733,362 on ply 7 positions 0-7).
- **Correct:** every label on plies 8-11 (16,000 positions, leaves at 5 pieces with a unit of 40
  nodes so that leaves stop and resume), ply 7 0-589, ply 6 0-379, ply 5 0-244, and 48 proof
  positions in eight variants. No wrong value.
- **A new benchmark, made for this: `tests/proof4/`** (`tools/mkproof.py`): 592 positions four
  moves in, eight children of each of the 74 positions that the paused run searched to a loss
  for blue. Each is a red win that the proof of the centre opening has to show. v9 needs about
  5 M nodes and 5 s for one. Positions 0-47 were used here; the rest are unused.
- **Results** (one thread, a fresh table for each position; nodes are exact; the seconds of the
  two right-hand columns were taken with six to eight runs side by side, so only their order
  counts). "Narrow" is the v9 search asked the question of a proof-number search ("does the mover
  win", then if not "does the opponent win") with windows of one step: `--pn --pn-ab 12`. The
  hybrids have `--pn-model --pn-probe --pn-killers --pn-width 3 --pn-rank 1`.

  | Search | Ply 6, 0-199 (alone) | Ply 5, 30-244 | Proof positions 0-47 |
  | --- | --- | --- | --- |
  | v9, wide window | 7.14 M, 3.77 s | 103.0 M, 92.8 s | 247.9 M, 246.8 s |
  | v9, narrow | 7.21 M, 3.67 s | 84.3 M, 76.8 s | 178.4 M, 183.4 s |
  | Hybrid, proof numbers at the root only | 7.12 M, 3.61 s | 95.7 M, 83.8 s | 174.6 M, 180.3 s |
  | Hybrid, two layers | 7.09 M, 3.78 s | 100.9 M, 90.1 s | 187.9 M, 196.7 s |
  | Hybrid, three layers | 8.37 M, 5.54 s | not run | 169.5 M, 174.0 s |
  | Two layers, leaves with a budget (unit 10,000) | 7.29 M at unit 3,000 | 98.8 M, 89.5 s | 179.3 M, 179.3 s |
  | Root only, leaves with a budget (unit 10,000) | 8.94 M at unit 3,000 | not run | 383.9 M, 387.5 s |

  Before the killers the hybrid took 7.38 M nodes at ply 6 (root only) and 8.75 M (two layers).
  The old `--pn` takes 52.5 M nodes and 67 s on the first 30 positions of ply 7, where v9 takes
  about 0.25 s.
- **Reading.**
  1. With alpha-beta's order, its killers and alpha-beta below, the proof-number search went
     from 60 times slower to level with alpha-beta: within 5% at ply 6 and on the proof
     positions, 14-20% behind at ply 5. It does not get ahead. The first reading on ply 5
     (positions 0-29: 4% fewer nodes, 14% fewer with a budget) did not hold on the 215 fresh
     ones.
  2. The part that only a proof-number search has, leaves that return after a budget so that
     effort can move, gains nothing that repeats (-5% to +2% with two layers) and can cost 2.2x.
     This is what the pot sizing said.
  3. **The gain found on the way is the question, not the search:** asked "does the mover win"
     with a window of one step, v9 itself takes 18% fewer nodes at ply 5 and 28% fewer on the
     proof positions than with the wide window. The window study after E18 had this (0.82-0.94x
     for a won position); it grows with depth. The proof driver already asks its jobs narrow
     questions, so the paused run had this gain; `bench` and `solve` do not.
- **Verdict against the bar** (5% less time than v9's best on ply 6 and ply 5): not met; no
  gate. The options stay, off by default, as the proof-number search of this project
  (`--pn --pn-ab 9 --pn-model --pn-probe --pn-killers --pn-width 3 --pn-rank 1`).
- **Reopen if:** a cost predictor exists that the numbers could carry (see "cheapest win
  first"), or positions appear where the first moves in order are dear and wrong.

## 2026-11-15 E24: a proof job as many single-thread jobs, not one job of 16 threads

- **Hypothesis** (written before the run; the idea is the two-level search of the owner's
  link): the dear jobs of the paused proof are positions where every blue move must be refuted
  (about 900 moves, each about 4 s on one thread). The run gave each such position to one
  shared-table search on 16 threads, whose nodes grow about 2x at 16 threads. The driver with a
  coordinator ply and 16 single-thread workers on different blue moves wastes less.
- **Expected:** 1.2-1.4x less wall-clock (E18 on the ply-4 position: 42.8 s against 51.9-61.4 s).
- **Test:** the three cheapest winning jobs of the run (77, 78 and 88 s there), each solved from
  a cold table by `frozen/prove.exe` four ways with 16 threads in all: one job of 16 threads
  (`--depth 0`, what the run did), 16 single-thread workers at depth 1 and at depth 2, four
  workers of four threads at depth 1. One at a time on a quiet machine, `--tt 27`.
- **Bar:** at least 10% less wall-clock than the job of 16 threads on each of the three.

## 2026-11-15 E25, E26, E27: three cheap node-speed tries (hypotheses, written before the timing)

Asked by the owner: the search visits 1.4-2.1 M nodes a second on one thread, engines for chess
or Ultimate Tic Tac Toe 10 M and more; is there low-hanging fruit? Where the time is: a node here
carries proofs (IDEAS, Open 4). A profile of three proof positions (v9, 9.94 M nodes): 46% of the
nodes have five pieces left, 20% three, 19% four, 11% two; 6.35 M reach tests, 4.31 M of them
decide their node; at five pieces left 32% of the nodes end on the table, 49% on a reach test.
`perft 3` counts 316 M placements in 0.31 s: generating is not the cost.

- **E25, prefetch** (`--prefetch`): ask for a child's table entry when its move is played, and for
  the next move's entry while this one is searched (from the fourth move on, where the order is
  fixed). Same nodes. Expected: 2-5% less time, more at depth where the table is far larger than
  the cache (the same build makes 1.9 M nodes a second at ply 6 and 1.4 M at ply 5).
- **E26, no table at the lowest levels** (`--tt-skip L`): a node with at most L pieces left
  neither reads nor writes the table. At three pieces left 9% of the lookups hit. Expected:
  about even at L = 3 (a lookup saved on 20% of the nodes against 6% more nodes at ply 7), a
  loss at L = 4; worth one screen because it also keeps cheap entries out of the table.
- **E27, a profile-guided build:** same source, the compiler told which branches are taken
  (typically 5-15%). **Blocked:** the GNU toolchain (`stable-x86_64-pc-windows-gnu`) ships
  without the profiler runtime (`can't find crate for profiler_builtins`), and the MSVC toolchain
  that is installed cannot link here (no Visual Studio build tools). The LLVM tools component
  was installed on the owner's word and is not needed otherwise. Open again if the owner
  installs the build tools.
- **Test:** screens on tune units, one thread, alone on a quiet machine: ply 6 positions 0-199,
  ply 5 positions 0-29, proof positions 0-23. Three runs an arm, interleaved. Then the gate on
  fresh proof positions through `tools/run.py` (unit: 2 positions) if a screen shows 2% or more.
- **Bar:** `SPRT[0, 0.05]` as before; the guard on total time.

## 2026-11-15 Pot sizing: the same position twice in a proof, up to symmetry (nothing there)

- **Idea:** with three coordinator plies the driver would solve positions four moves in one by
  one; key its results by the canonical position (the least of the twelve mirror images), so
  that a position reached under two blue replies is solved once.
- **Measured without building** (`python tools/sharepot.py work/long/ply1_results_paused.txt`):
  the 74 positions searched to a loss for blue have 66,585 children. 66,567 are distinct as they
  stand, 66,433 up to symmetry over all parents (0.2% repeat), and no parent has a symmetry of
  its own. Red's answers differ from reply to reply, so the transpositions do not meet.
- **Verdict:** dropped before a build. Reopen only if red's answers are chosen to coincide.
- **First three jobs** (`frozen/prove.exe`, one run each, quiet machine; seconds of wall-clock):

  | Job | One job of 16 threads | 16 workers, depth 1 | 16 workers, depth 2 | 4 workers of 4 threads, depth 1 |
  | --- | --- | --- | --- | --- |
  | a | 77.0 | 55.5 | 55.4 | 57.5 |
  | b | 74.1 | 67.4 | 73.1 | 73.2 |
  | c | 87.1 | 78.0 | 45.5 | 78.6 |
  | all | 238.2 | 200.9 (0.84x) | 174.0 (0.73x) | 209.3 (0.88x) |

  The bar (10% on each job) is not met on job b. Seven more jobs were started and stopped: I
  compiled while the first was being timed, which spoiled it, and stopping the task left the
  script and a solver running (found and killed by hand: stopping a background shell does not
  stop the solver it started). The base's three settings are being run again on four jobs kept
  for the deciding read (g to j), every run under the run lock.

## 2026-11-15 The climb moves to `parallel-v9`, with sub-agents

- **Owner:** "keep hill climbing the performance of the prover. act as a coordinator with opus
  sub-agents for implementations."
- **Set-up:** `main` merged into `parallel-v9` (commit `f4e5efc`: the E23 options, the proof
  benchmark, `--prefetch`, `--tt-skip`); identity and values checked on the merged build
  (8,076,471 and 7,136,589 nodes; the bench file through `prove`: wrong 0). The build is kept as
  `frozen/climb-base.exe`, the base of this climb. `tools/locked.sh` is the machine's run lock;
  `tools/run.py` knows the set `proof4` (unit: 2 positions; tune 0-87, gate 88-441) and takes
  `--base`, and runs every arm under the lock.
- **Three agents (Opus), each in its own worktree and branch:** `climb/search` (one-thread
  speed of the search on proof positions), `climb/driver` (wall-clock of `prove` on proof jobs),
  `climb/study` (where a 2x could come from: counts, no shipping). They return evidence; the
  coordinator gates on positions and jobs they were told not to touch (proof positions 48 and
  later; jobs g to j).
- **Worktrees:** the app's own worktree isolation fails here (the folder's path is too long for
  git), so the worktrees were made by hand in a shorter place.
- **Gate for the driver:** `tools/provegate.py` reads base against candidate on whole proof jobs
  (wall-clock, a job a unit, fixed length, every solve under the lock). Reserved for it: jobs g to
  j and eight more made today from the paused proof's results (`work/e24/k.pos` to `r.pos`, the
  next eight by the time the proof spent on them: 113 to 142 s). Twelve jobs cannot feed a
  sequential test (it takes no decision before 30 units), so a driver read is of fixed length
  and passes when its 95% interval excludes zero.
- **Owner, on the build tools for profile-guided optimisation (E27):** "I'll install them". E27
  is open again once the C++ linker is on the machine. It will be two reads, not one: the MSVC
  toolchain against the GNU one (the environment pin changes), then the profile on top.

## 2026-11-15 E27 open again: the plan, written before the build tools are in

- **Owner:** asked whether I could install the build tools myself; told what it takes (the
  package manager, Microsoft's licence accepted on the owner's behalf, the administrator prompt
  theirs to click, a few gigabytes): "Yes, install it". The installer runs holding the run lock
  (kept fresh every four minutes), and the three agents were told that the lock is held long.
- **Two questions, kept apart.** Both toolchains are rustc 1.97.1.
  - **E27a, the toolchain:** the unchanged source built for `x86_64-pc-windows-msvc` against the
    GNU build. Same compiler and LLVM; the linker and the C runtime differ. Expected: within 2%
    either way. It is read because E27b's base has to be the plain MSVC build.
  - **E27b, the profile:** the MSVC build made with a profile of the tune positions (proof
    positions 0-23 with `--window 0,1`, and ply 7 positions 0-99) against the plain MSVC build.
    Expected: 5-15% less time. Node counts must be the base's exactly (same source).
- **How it would be used.** A profiled build is packaging, not a change of the program: day to
  day, candidate and base are both plain builds, so a gate keeps comparing like with like. The
  profile goes onto the build that does the long proofs, and its gain is read again at each
  freeze.
- **Test:** screen on proof positions 0-23, three runs an arm, interleaved, one thread, under
  the lock. Gate on fresh proof units through `tools/run.py --set proof4` if the screen shows
  2% or more. **Bar:** `SPRT[0, 0.05]`, wrong values 0, node counts identical.

## 2026-11-15 E24 result: sixteen single-thread workers beat one job of sixteen threads, unevenly

- **Reserved jobs g to j** (`frozen/climb-base.exe`, one run each, every run under the lock;
  seconds of wall-clock; `work/e24c/`):

  | Job | One job of 16 threads | 16 workers, depth 1 | 16 workers, depth 2 | Beside it |
  | --- | --- | --- | --- | --- |
  | g | 92.4 | 82.6 | 102.5 | nothing |
  | h | 98.4 | 96.2 | 87.3 | nothing |
  | i | 127.6 | 114.6 | 108.5 | the build-tools installer, from the second run on |
  | j | 126.0 | 121.0 | 93.9 | the installer (first run), two compiles of mine (the others) |

- **Clean jobs only** (a, b, c from the first batch, g, h): one job of 16 threads 429.0 s;
  16 workers at depth 1 379.7 s (0.89x); at depth 2 363.8 s (0.85x). With i and j: 682.6,
  615.3 (0.90x), 566.2 (0.83x).
- **By job the ratio runs from 0.52x to 1.11x.** The bar of the hypothesis (10% on each job) is
  not met; on the total both settings are 10-17% ahead of the shared-table job. Depth 1
  against depth 2 is not decided: they trade places from job to job (g: 82.6 against 102.5; c:
  78.0 against 45.5), and one run a job cannot tell the setting from the run. Depth 2 throws
  work away (55 to 152 jobs stopped a solve), depth 1 none.
- **Verdict:** a long proof should use single-thread workers, not the shared table (this is what
  the paused proof used: `--job-threads 16`). Which depth, and whether the spread between jobs
  can be removed (work thrown away, workers idle at the end of a root), is the driver agent's
  question. Not known yet: how much two runs of the same setting differ; that A/A read comes
  before the driver's gate.
- **Mistake, twice:** timed runs with something beside them. The installer was started outside
  the lock because the owner was waiting for its prompt (right call, known cost); my two
  compiles during job j were not needed then. Compiles are allowed beside one-thread timings;
  beside a 16-thread timing they take a sixteenth of the machine. Rule from here: no compile
  while a `prove` timing of mine is running.

## 2026-11-15 E27: the build tools are in; identity and the screen

- **Install:** Visual Studio Build Tools 2022 (17.14.45, the C++ workload), by `winget`, on the
  owner's word; seven minutes (13:15 to 13:22).
- **The linker and this folder:** `link.exe` fails with LNK1104 when the target directory is
  inside this repo (the path is longer than 260 characters). An MSVC build needs a short
  `--target-dir` (the scratchpad). The GNU build is not affected.
- **Identity:** the plain MSVC build and the profiled build both give 1,724,647 nodes on ply 7
  positions 0-99, as the base does; all values right.
- **The profile:** the instrumented build on proof positions 0-23 (`--window 0,1`; 75.4 M nodes)
  and ply 7 positions 0-99, merged with `llvm-profdata`.
- **Screen** (proof positions 24-35, not in the profile; `--window 0,1`; 62,315,717 nodes on
  every run; seconds, in the order run; the machine was idle except for the agents' compiles):

  | Run | GNU base | MSVC plain | MSVC profiled |
  | --- | --- | --- | --- |
  | 1 | 61.7 | 53.0 | 54.3 |
  | 2 | 63.5 | 61.1 | 55.8 |
  | 3 | 59.9 | (cut off) | (cut off) |

  Repeats of one build differ by up to 15%, so the screen says only "both look ahead of the
  GNU build; the profile's own share is not visible". The screen ran without the run lock, on
  purpose: the lock was stuck (below) and nothing could be timed beside it.
- **Gates started:** `E27A` (MSVC plain against the GNU base) and `E27B` (profiled against MSVC
  plain), each `SPRT[0, 0.05]` on fresh proof units, cap 60 units.
- **A stuck lock:** the search agent force-killed a timing round that held the run lock
  (13:27). The lock frees itself after 20 minutes. The agent's own attempt to remove it was
  refused by the permission check and it asked whether I would; I did not (a refused action is
  not done for an agent by someone else). The profile run and the screen used the gap. Told to
  all agents' future briefs: stop the solver, never the script that holds the lock.

## 2026-11-15 E27 result: both gates pass; the climb's base is now a profiled MSVC build

- **E27A, the toolchain** (plain MSVC build against the GNU base; proof units 44-73, `--window 0,1`):
  +4.9% [+2.1%, +7.8%] where it stopped, sd 0.076 per unit, `SPRT[0, 0.05]` PASS at 30 units;
  total time 175.2 s against 168.2 s (0.960x); node counts identical (183,799,694).
- **E27B, the profile** (profiled against plain MSVC; proof units 74-103): +6.3% [+3.0%, +9.8%]
  where it stopped, sd 0.090, PASS at 30 units; total time 162.3 s against 153.3 s (0.944x);
  node counts identical (177,240,303).
- **Not measured:** the two together against the GNU base. Passes do not add; the product of
  the two totals (0.907x) is an expectation, not a reading. Both sizes are read where a
  sequential test stopped and lean high.
- **Hypotheses against results:** the toolchain was expected "within 2% either way" and gave
  about 5%: wrong, in our favour. The profile was expected at 5-15% and gave about 6%.
- **Frozen:** `frozen/climb-base.exe` = the profiled build that passed (`231876efcb8b`, source
  of `f4e5efc`). Beside it: `climb-base-plain.exe` (plain MSVC, `7321efa41a10`) and
  `climb-base-gnu.exe` (the GNU build, `18c5a5c49d58`).
- **The build is a script now:** `sh tools/build.sh OUT.exe` (plain MSVC) and
  `sh tools/build.sh --pgo OUT.exe` (profiled; the profile is always collected on proof
  positions 0-23 with `--window 0,1` and ply 7 positions 0-99). It builds in a short directory
  because the linker fails on this folder's long path. Two plain builds of the same source
  differ in 23 bytes (the linker's time stamps): the code is the same.
- **What changes for the climb** (the skill's rule 2: test the build production runs):
  - A deciding read compares two profiled builds, base and candidate each built by
    `tools/build.sh --pgo`. A hand-tuned branch or layout that pays on a plain build may be
    worth nothing once the compiler has the profile.
  - Screens may use plain MSVC builds on both arms (a build in 20 s instead of 3 minutes).
  - The agents now at work started from GNU builds. Their screens stand as screens; their
    changes are gated on profiled builds.
- **Gate units:** the proof set's gate range has 117 units left (104-220).

## 2026-11-15 The proof driver against itself: two runs of one setting differ by 12% (sd)

- **Run** `DRV-AA` (`tools/provegate.py`): the profiled base against itself, 16 single-thread
  workers at depth 1, jobs g to j, every solve under the lock.

  | Job | First run (s) | Second run (s) | Ratio | Nodes, first and second |
  | --- | --- | --- | --- | --- |
  | g | 77.2 | 75.5 | 0.978 | 1,023 M and 1,028 M |
  | h | 76.2 | 71.5 | 0.939 | 1,004 M and 946 M |
  | i | 98.5 | 99.3 | 1.008 | 1,198 M and 1,277 M |
  | j | 91.1 | 110.9 | 1.217 | 1,102 M and 1,383 M |

- **Reading:** the sd of the log ratio is 0.115 a job. The time follows the nodes: which
  worker finishes first changes what the others know, and job j searched a quarter more
  positions on its second run. So one run a job cannot rank two settings that differ by
  less than about a quarter, which is what E24's table looked like.
- **What a driver read resolves:** twelve jobs, one run an arm: a 95% interval of about plus or
  minus 7%. Two runs an arm: about 5%. A driver change has to be worth 10% to be seen with the
  jobs at hand; smaller ones need more jobs (62 more proved positions exist in the paused
  proof's results, at 2 to 12 minutes a solve).
- **The profiled build on whole jobs** (not a paired read: other runs, other hours): g 77.2 and
  75.5 s where the GNU build took 82.6 s with the same setting; h 76.2 and 71.5 s against
  96.2 s.
- **The spoiled arms of E24** (jobs i and j) are not run again: the base has changed build since,
  and a driver read pairs candidate and base anyway.

## 2026-11-15 The machine is a third slower today than last night (the ruler, not the solver)

- **What:** asked how we compare with the reference, I timed the profiled base on its five bench
  files: 142.6 s on one thread, 161.5 M nodes, all 126 values right (16 threads: 19.9 s). Last
  night v9 took 99.7 s for the same 161.5 M nodes.
- **Check:** four builds side by side on `bench_ply5_other_openings` (24 positions, 13,223,437
  nodes each), three runs, under the lock: v9 of last night (`frozen/base.exe`) 8.68, 8.30,
  8.40 s; the climb base built with GNU 8.20, 8.62, 8.62; plain MSVC 8.06, 8.07, 8.03; profiled
  8.26, 7.34, 8.01. Last night `frozen/base.exe` took 6.3 s on this file.
- **Reading:** the same executable is 1.33x slower today, so today's absolute times cannot be
  set beside last night's. Paired reads (candidate and base in the same minutes) are not
  touched by this. Side by side the profiled base is about 0.93x of v9 on this file.
- **Why, not settled:** the machine now runs three agents, their builds and the desktop
  app's sessions beside every timing: 3.1 GB of 15.9 GB free, 22 GB committed, the system
  compressing memory, power plan "Balanced". Earlier today the same was seen and not
  explained (ply 5 positions 0-29: 8.2 s last night, 11.3-11.8 s today). A long proof should
  run with the machine to itself; what a quiet machine is worth is a measurement to take when
  the agents are done.

## 2026-11-15 E28: the search agent's bundle (hypothesis, before the gate)

- **From:** branch `climb/search` (Opus sub-agent), eight commits on `f4e5efc`, tip `8507875`.
  Seven change the code that runs by default, all claimed to keep every answer: a reach test
  can answer from the placements an earlier test found (a witness; `--no-witness`); the last
  piece of a two-piece reach test is generated only where it can close the gap, and without
  copying the position; threat scans and killer threats stop their bound as soon as it is
  above what the threat needs; cheaper grow anchors. The eighth extends `--prefetch`.
- **The agent's screens** (tune positions; median of 5 rounds; node counts identical, 75,382,224
  on proof positions 0-23): GNU builds 0.880x, with `--prefetch` 0.830x; profiled MSVC builds
  0.915x, with `--prefetch` 0.877x. The profile takes back about a third of what the hand
  changes show on the GNU build. Its profile of the base: reach tests 40% of the time (the
  two-piece "no" answers alone 26%), threat scans 17%.
- **Hypothesis E28:** the seven changes together, profiled build against profiled base, take
  less time on fresh proof positions; expected 0.92x. Same nodes. `SPRT[0, 0.05]`, cap 60.
- **Hypothesis E25 (logged earlier, now on this code):** prefetch on by default (commit
  `36892b4` on `climb/search-pf`, profiled with it on) against the E28 candidate; expected
  0.95x. Same nodes. `SPRT[0, 0.05]`, cap 60.
- **Before a freeze, besides the gates:** the coordinator's own identity and label runs (three
  identity sets, ply 8, the five bench files, `verify 100`, one 16-thread run), and an
  independent agent (Opus, branch `climb/verify`) that tries to refute each change and runs old
  and new code side by side. The witness is the one to worry about: it is the only change
  that carries an answer from one position to another.
- **Not split by commit:** the agent's early rounds were lost; the bundle is gated as one. If it
  fails, nothing is learned about its members; if the review breaks one member, that one is
  taken out and the rest gated again under a new key.

## 2026-11-15 The study agent's report: where the next large gain is

Branch `climb/study` (Opus sub-agent; study build, counts exact; raw outputs in `tools/study/` on
that branch). Four pots, ranked:

| Pot | Size | What it needs |
| --- | --- | --- |
| 1. **Reach threats** (new, exact, built): the reply R that refuted a move is a threat when, with R placed, the mover can no longer reach R's snake length even alone; then only moves that cover or touch R are searched | nodes 0.51x on proof positions 0-11 tested at node start, 0.57x tested after a failed move; 0.67x at ply 7, 0.64x at ply 6; every label right (plies 6-11, proof positions). **Time: no gain yet** (base 39.4-39.9 s; 38.8-41.1 s) | a cheaper test: 1.59 M tests build 33 M reach positions to save 17.5 M nodes; 24 M of them at six pieces left (a hand of three, about 42 positions a test) |
| 2. Red's wrong moves: nodes under a red move that failed at a node that later cut | 22.6% and 40.7% of the nodes on the two halves of proof positions 0-47; 32.7% at ply 6. Sits under red nodes with 5 and 7 pieces left | an order that finds the cut sooner (IDEAS 2, 2a); no new lever found |
| 3. The one-player optimum for hands of 3-4 where no reach test is asked | a ceiling of 5-11% of the nodes; overlaps pot 1 | the same cheaper three-piece test |
| 4. Slack of the cluster bound over the exact one-player optimum | tight high in the tree (slack 0 in 95% at 7-9 left), loose low (slack of 3 or more in 25-50% at 4-6 left), where the reach test already covers it | nothing beyond pot 3 |

- **Why reach threats are exact** (the agent's argument): a move m that neither covers nor
  touches R leaves R legal, and any sequence of the mover's placements that is legal after m
  and R is legal after R alone, as m followed by the rest; so the mover's one-player optimum
  after m and R is at most the optimum after R alone, and the opponent's snakes never shrink.
  `verify` on that branch checks this monotonicity against brute force.
- **Where the gain lives:** at 5-6 pieces left (nodes at 5 left 19.0 M to 9.3 M). At 7 or more
  left nothing (0.99x nodes): there blue can still reach red's length, the refutation is a
  two-player one. At 3-4 left only: 0.89x nodes, no time.
- **Nothing found in:** separation of the two sides (0.0-0.03% of nodes), blue's wrong moves
  (0% on proof positions), a smaller work budget per test.
- **At nodes where every move fails** (proof 0-47): 8 left: 153-158 moves, about 5 distinct
  refuting replies, the top one refutes 73-77% of the moves; 6 left: 35 moves, 1.4 distinct
  replies, 77-79% of children decided at once.
- **Next (E29, hypothesis):** reach threats on top of the search branch, made to pay with that
  branch's witness cache and cheaper last-piece generation, tested only where they decide
  (5-6 left). Expected: the node gain of 0.55x turned into 0.7x time or better on proof
  positions. An Opus agent has it (branch `climb/threats`, from `climb/search-pf` with
  `parallel-v9` merged). The rule gets its own independent check before any gate.

## 2026-11-15 Second batch of agents (all Opus)

- **`climb/threats` (E29):** make reach threats pay, on top of the search branch.
- **`climb/order` (study, nothing ships):** how much of the wrong-move pot a learned order can
  get on positions it has not seen, at 7 and at 5 pieces left, with reach threats on and off,
  and what it may cost per node. This is the owner's GPU question turned into a measurement.
  Data rule set in the brief: 24 of the 74 parents of the proof benchmark are never looked
  at, so that a later gate of a learned order can use positions whose parents were not in
  training.
- **`climb/verify`:** independent check of the search branch's eight changes (reasoning, and
  old and new code run side by side).
- **Still out from the first batch:** `climb/driver`.

## 2026-11-15 E28 before its gate: the coordinator's checks and the independent verifier

- **Coordinator's runs** (plain MSVC build of `8507875`, every run under the lock): node counts
  as the base on the three identity sets (8,076,471; 7,136,589; 13,150,272), also with
  prefetch on by default (`36892b4`); every label right on ply 8 (4,000 positions) and on the
  reference's five bench files (126 positions, node counts as the base file by file);
  `verify 100` passes; ply 6 positions 0-199 on 16 threads: all right. The profiled builds
  give 1,724,647 nodes on ply 7 positions 0-99.
- **Independent verifier** (Opus, branch `climb/verify`, commits `5abb59f` and `005f91a`): a
  build with `--features shadow` in which every changed routine also runs the body it had at
  `f4e5efc` and stops on any difference. Runs: `verify 200`, proof positions 0-11, ply 6
  0-199 (also with 8 threads), ply 7 0-589, `bench_ply5_other_openings`, ply 8 (all). No
  mismatch in: 5.0e9 comparisons of the grow anchors, 50 M of `find_move` with a touching set,
  62 M of the last-piece generator, 86 M of the early-stopping bound, 74 M threat-scan and
  13 M killer-threat decisions, 52 M reach-test answers (and work left), 32 M reach-test "no"
  answers against the brute-force optimum, 6.1 M witness answers checked three ways and
  19.7 M recorded witnesses replayed.
- **Its verdict:** six commits proven, the two prefetch changes cannot change an answer (a
  masked index, nothing read). The argument that settles the witness: a reach test prunes
  only when it answers no; a witness can only turn an answer into yes, which skips a cut; so
  even a wrong witness could cost nodes, never a value. And the witness's claim itself holds:
  the legality of a growing placement depends only on the own cells and on its cells being
  empty, a new snake also on its border, and the border is among the cells a witness needs
  empty; the own cells, the whole hand and the length are compared on reuse. The witness
  store belongs to a thread's solver (not shared in the parallel solve).
- **Not checked by it:** runs with prefetch on under the shadow build (argued only),
  `--reach-hand` above 2, the proof-number and driver code paths.
- **The shadow build stays** on `climb/verify` as a tool for the next change to these routines.

## 2026-11-15 E28 and E25: both gates pass; merged; guards running before the freeze

- **E28** (the search branch's seven changes, profiled `8507875` against the profiled base;
  proof units 104-133, `--window 0,1`): +8.0% [+5.4%, +10.7%] where it stopped, sd 0.070 a
  unit, LLR +16.4, PASS at 30 units; total time 213.5 s against 195.6 s (0.916x); node counts
  identical (242,995,080). The agent's screen had said 0.915x.
- **E25** (prefetch on by default, profiled `36892b4` against the E28 candidate; proof units
  134-163): +5.8% [+3.2%, +8.4%] where it stopped, LLR +9.8, PASS at 30 units; total time
  266.5 s against 252.8 s (0.949x); node counts identical (339,005,081). Expected 0.95x.
- **Merged** into `parallel-v9` (`bd41644`). Not frozen yet.
- **Guards before the freeze:** `E25-UP` (ply 5 tune units, the next harder file: must not show
  a higher total time) and `E25-DRV` (whole proof jobs a-d on 16 single-thread workers
  through `tools/provegate.py`: prefetch has not been timed with 16 threads sharing the
  memory; four jobs resolve about 12%, so this guard can only catch a large loss).
- **Gate units:** the proof set's gate range has 57 units left (164-220). More positions are
  needed before the reach-threat gate: a second sample of children from the same parents.
- **More gate units:** `tests/proof4b/` (1,184 positions, 16 children a parent, seed 2, none of
  them in `tests/proof4/`; `tools/mkproof.py --exclude`). `tools/run.py --set proof4b`: tune
  units 0-87, gate 88-442. A first draw of it overlapped the first set in 13 positions,
  because my patch of the sampler had not applied; redrawn, overlap 0.

## 2026-11-15 E28 and E25 frozen: the climb's base is the search bundle with prefetch on

- **Guards, both held:**
  - `E25-UP` (ply 5, the tune range's three units, 30 positions): total time 10.38 s against
    8.61 s (0.83x), node counts identical (15,824,832).
  - `E25-DRV` (whole proof jobs on 16 single-thread workers, depth 1, seconds, base then
    candidate): a 50.8 / 43.7, b 62.9 / 53.7, c 64.1 / 58.5, d 127.5 / 77.0; total 305.2 s
    against 232.9 s. Job d's base run is an outlier of the kind the A/A showed (the driver's
    own sd is 0.115 a job), so the size is not to be read off this; the guard asked only
    whether 16 workers lose with prefetch on, and they do not.
- **Frozen:** `frozen/climb-base.exe` = `3d3bd213049b` (profiled MSVC build of `36892b4`, in
  `parallel-v9` as `bd41644`); `frozen/climb-base-plain.exe` = the plain MSVC build of the same
  source (`6729272a400c`). The base before it is `frozen/E27B-231876efcb8b.exe`.
- **Since this morning, on proof positions, one thread** (each read against the base of its
  time, not against one fixed base; passes do not add): MSVC toolchain 0.960x, profile 0.944x,
  the search bundle 0.916x, prefetch 0.949x. The product is 0.79x; a direct read of the
  morning's GNU build against today's base has not been taken.
- **Lessons:**
  - The agent's screen on profiled builds (0.915x and 0.953-0.958x) predicted both gates
    (0.916x, 0.949x). Its GNU screens (0.880x) would have overstated the bundle: a third of
    the hand-tuning's gain was something the compiler finds by itself with a profile.
  - The independent verifier found the argument that makes the riskiest change safe in a
    sentence (a witness can only say yes, and only a no prunes). Asking a second agent to
    break a change cost 35 minutes and is now a tool (`--features shadow`).

## 2026-11-15 The driver agent's report: two exact options, no gain that can be told from noise

Branch `climb/driver` (Opus sub-agent), commits `8f36221` and `ce2d0af`. Both options are off by
default; with both off the driver does what the base does.

- **`--split S`:** when a worker would be idle, the job whose root move has been under way
  longest (at least S seconds) is stopped and its position becomes a coordinator node, its
  moves going out as jobs. On job d it removed the idle tail (87.7 s of wall-clock for 1403
  thread-seconds), but the base ran 88.8 s that hour without a late giant job; on job b no gain
  (70.0 s against 60.8 and 67.2).
- **`--hint`:** before a job goes out the coordinator works out the move it would search first
  there and gives it to the job as its first killer; a move that wins a job is recorded as a
  winner at its ply (before, at depth 1, that list stayed empty). Job c: 976 and 1064
  thread-seconds against 1098 and 1204 (0.89x); elsewhere inside the noise. Limited to jobs at
  most two plies below the root after it made the coordinator the bottleneck at depth 3 (127 s
  against 15 s on a bench file).
- **Correct:** the five bench files at depths 1-3 with both options on, splitting as hard as
  possible: all values right; a cut results file is picked up and finished.
- **The agent's reading, which I share:** one run cannot tell settings apart unless they differ
  by 15-30%; depth 1 and depth 2 trade places from job to job. Its study of the morning's
  results files: runs at depth 1 and depth 2 found different red refutations for 30-95% of
  blue's moves; at depth 2 the coordinator sometimes tries hundreds of red moves (110 and 347
  tries, 50 s and 150 s) where one job did it in 9 s and 47 s, because a failing first move is
  cheap inside the solver (threat zones) and dear at the coordinator.
- **Decision:** not gated. The effect the screens suggest (0 to 10%) is below what twelve jobs
  resolve (7% with one run an arm, 5% with two), and a read of that size costs an hour of the
  machine. The branch is kept apart, unmerged. *Reopen* when the driver's noise is lower or
  the jobs are more (62 proved positions are left in the paused proof's results), or if a
  long proof shows idle workers.
- **Recommended setting for a long proof, as it stands:** `--threads 16 --job-threads 1 --depth 1`
  (no work thrown away, the coordinator stays out of red's choices).
- **What the driver may be losing, being sized now:** separate workers do not share a table.
  One solve of job a's position on one thread with one table tells how many nodes the same
  proof needs when everything is shared (the workers needed 642-685 M).

## 2026-11-15 Pot sizing: the driver's jobs need no more nodes than one sequential search

- **Run:** proof job a's position solved by one thread (`frozen/climb-base.exe bench
  work/e24/a.pos --tt 27 --window -1,0`): value -1, 721.6 M nodes, 600.3 s.
- **Against the driver on the same position** (16 single-thread workers, depth 1): 642 M to
  685 M nodes in five runs, 834 thread-seconds, 50.8 to 57.9 s of wall-clock.
- **Corrected the same day.** This entry first said the workers each have their own table and
  concluded that a shared table would win nothing. That premise was wrong: `prove` makes one
  table and every worker's solver is built on it (`Solver::with_table`), and a job does not
  clear it. So the workers already share a table, and nothing here says what an unshared
  design would cost.
- **What the run does show:** splitting the proof into 675 jobs on 16 threads costs no nodes
  against the sequential search (it needs a few fewer). The driver turns one thread's 600 s
  into about 51 to 58 s on 8 cores (10 to 12 times), and its 1.39x in thread-seconds is the
  price of two threads a core. At depth 1 the driver is close to what this machine can give:
  the remaining gains are in the search (nodes, and the cost of a node), not in scheduling.
- **Closed:** more scheduling work on single jobs. *Reopen* the driver only for what a long
  proof with many roots shows (idle workers between roots, the order of roots).

## 2026-11-15 The ordering study: a learned order is worth nothing here; the pot is the cost of the win chosen

Branch `climb/order` (Opus sub-agent; commits `508d81d`, `239ad89`; tools in `tools/order/`,
results in `tools/order/results/`). The owner's GPU question, measured. Data rule kept: 24 of the
74 parents never dumped, labelled or fitted; the other 50 split 38 train and 12 test by parent;
all training and test positions are children that are not in `tests/proof4`.

- **Verdict: not worth building, and nothing for a GPU to do.** Every learned order (the 8-feature
  linear model refitted in distribution, an extended logistic model, a cost-aware "win
  probability per unit of cost" model, networks of 32 and 64 units, one over a board encoding)
  pays 0.97x to 1.00x of today's nodes on held-out parents at 7 and at 5 pieces left (792 and
  248 test nodes with reach threats on). In sample 0.96-0.99x. A model of this size fits on
  the processor in seconds.
- **Why:** today's first move already wins 84-94% of the time at these nodes (98-99% of the red
  nodes that reach the move loop cut on their first move), so failed moves are only 3-10% of
  what a node pays. A perfect win/lose classifier would still pay 0.95-0.99x.
- **Where the pot is:** in *which* win is taken. Against "cheapest winning move first" today's
  order pays 5.9x at 7 left, 1.8x at 5 left, 7.9x at 9 left (reach threats on; costs are each
  move's own solve with a fresh table, which overstates: 1.3-1.7x against the tree, and killers
  look dear alone). The cheapest win among today's first three moves would pay 0.53-0.61x at
  7 left. No cheap feature of the position predicts which win is cheap: the cost-aware model's
  winning pick is the cheapest win at 24-43% of the nodes at 7 left.
- **The pot by the level of the node that chose the move** (share of all nodes under failed
  moves at red cut nodes; reach threats off / on): 11 left (the root) 16.8% / 16.7%; 9 left
  9.7% / 12.1%; 7 left 4.4% / 6.4%; 5 left 1.9% / 3.2%. The first study counted by the level of
  the wasted node, which is why it said "5 and 7".
- **Budget:** at 7 left a 1.2% gain is about 0.5 microseconds a red node, 3 ns a move; at 5 left
  70 ns a node.
- **Not measured:** any order inside the solver; the root; level 9 is partial; test sets are five
  parents, so a few percent are not resolved.
- **What it points to (E30, hypothesis):** the search itself sees what a win cost at the nodes
  next to this one. A killer today is the last move that cut; the cheapest move that cut is
  information of another kind than a feature of the position. Expected: unknown; earlier
  order ideas from the search's own knowledge gave 0.77x once (E14) and nothing five times
  (E15-E17, E20, E21). Screened in the tree on node counts, which are exact.

## 2026-11-15 E29, first report: reach threats ported and exact; nodes 0.56x, time unchanged

Branch `climb/threats` (Opus sub-agent), commits `f59006f` to `efa65ec`, on top of the search
bundle. The option is `--reach-threat`, off by default; with it off the node counts are the
base's.

- **Nodes** (proof positions 0-23, `--window 0,1`): 75,382,224 off; 44.66 M with the study's rule
  using the witness cache; 42.38 M with a refined zone (when R grows a snake, a move that
  touches R without covering it is also pruned if it grows a snake of the mover; the argument
  is in the commit, and `verify` checks it against brute force).
- **Time** (plain MSVC builds, arms interleaved, seconds): off 59.5 / 60.6 (min / median of 5);
  on 59.2 / 60.4 (3 reads). No gain: the tests cost what the nodes save.
- **By level, one read each:** testing only at 5 pieces left or more is the slowest arm (62.6 s
  for 50.8 M nodes): most of the nodes saved there are cheap table cuts. Testing only at 4 or
  fewer keeps a fifth of the node saving (66.6 M) at no cost (59.8, 60.0 s), and with the
  replies also tested at node start there: 65.0 M nodes, 57.2 s (one read, 0.94x). This is the
  reverse of the study's guess that the gain lives at 5-6 left.
- **Witnesses** answer 11% of the threat tests; the last piece of a three-piece test without
  copying the position changed nothing in time.
- **Correct:** every label on ply 6 (0-199), ply 7 (0-589), ply 8 (all), one bench file, proof
  positions 0-23 in every variant; `verify 100` with the new checks.
- **Not done:** profiled builds, positions 24-47, repeats.
- **Sent back with two steps:** secure the setting at 4 or fewer (three rounds on both halves,
  then a profiled pair with the rule on by default, since the profile run uses the defaults);
  then the 5-6 level with counters first: what a test costs by answer, what a found threat
  saves, and whether a cheap signal (the cluster bound's margin over R's length, the moves
  left outside the zone) predicts the answer before the test.
- **A slip of the agent's:** one command ran in the main checkout (`git checkout src`, on a
  clean tree: no effect). Checked: the main checkout is clean and at the right commit.

## 2026-11-15 Does halving the nodes save memory? (owner's question) No

- **Owner:** "In the 0.56x nodes case, even if it's not saving on time, is it at least saving on
  memory? Would it let us hold onto a larger tree".
- **How memory is used:** the search is depth-first and holds no tree; what it keeps is the
  table, of a size fixed by `--tt` (16 bytes an entry; 64 MB by default, 2 GB in proof runs,
  one table for all workers). So memory does not follow the node count.
- **Could fewer nodes let the table cover more of the tree?** Measured: proof positions 0-3,
  `--window 0,1`, reach threats at every level against off (`rt5.exe`), nodes:

  | `--tt` | Off | On | Ratio |
  | --- | --- | --- | --- |
  | 16 (1 MB) | 15,180,345 | 8,197,177 | 0.540 |
  | 19 | 13,363,696 | 7,305,782 | 0.547 |
  | 22 | 13,150,272 | 7,192,988 | 0.547 |
  | 26 (1 GB) | 13,150,845 | 7,187,958 | 0.547 |

  The ratio does not move with the table's size, and a table a thousand times smaller costs
  15% more nodes. The table is not the scarce resource on these positions.
- **Where it would count:** a written-out proof tree for an independent checker would be about
  0.56x the size (a pruned move is justified by one reach test the checker can repeat).

## 2026-11-15 E29, second report: reach threats at 4 pieces left or fewer, about 0.96x

Branch `climb/threats`, commits `19d4dad` (the setting), `e75f776` (counters), `09807d5` (a
cache, off). The rule is now on by default on the branch, at nodes with at most 4 pieces left,
and the killer replies are also tested as threats when such a node starts.
`--no-reach-threat` gives the base's node counts.

- **Plain builds, three rounds, arms interleaved** (total time, min / median):
  positions 0-23 0.93x / 0.94x (65,005,807 nodes against 75,382,224); positions 24-47
  0.95x / 0.96x (91,751,245 against 103,032,394).
- **Profiled pair** (base `229cf3e`, candidate `19d4dad`, each with its own profile):
  0-23 0.93x / 0.95x; 24-47 0.975x / 0.98x. So the expectation for a gate on profiled builds
  is about 0.96x, less than the plain builds say (as with E28, the profile finds part of it).
- **Labels:** all right with the final defaults; `verify 100` passes.
- **Why 5-6 pieces left does not pay** (counters, proof position 0 only): the cost is in the
  tests that *find* a threat (23 to 40 thousand cycles each) and not in the ones that answer
  "can reach" (5 to 9 thousand). The cluster bound's margin over the reply's length predicts
  the answer: at 5 left a margin of 1 is a threat 82% of the time, 7 or more 31%; at 4 left a
  margin of 7 or more is a threat 2% of the time. A cache of "cannot reach" answers hits
  0.25% (left off). Filters on the margin are not tried yet.
- **All levels:** nodes 0.56x, time about 1.00x, as before.
- **Not done:** an independent check of the rule's exactness (a second agent, started now);
  the margin filters.

## 2026-11-15 Order of the next two gates: E30 first, then E29 on top

- The cost-aware ordering agent (E30, `climb/cost`) reported screens far larger than E29's:
  nodes 0.70x / 0.64x / 0.58x on proof positions 0-23 / 24-47 / 48-87 for its best variant,
  and a first timing round of 50.4 s against 62.6 s (0.80x). It is ordering only (the set of
  moves searched at a node is unchanged), so it cannot change a value.
- Both change which nodes are searched, so the two reads will not add. E30 is gated first
  against the frozen base; E29 is then gated against the base with E30 in it, after the
  verifier has cleared the rule.
- The agent was sent back to finish its timing rounds and to commit one variant on by default
  (the profile is collected with the defaults), with one option that switches all of it off.

## 2026-11-15 E30: the cheapest win from what the search has seen (report, and the hypothesis before the gate)

Branch `climb/cost` (Opus sub-agent), commits `7394610` (counters and options, all off),
`2b86d29`, `8f6adce` (one variant on by default; `--no-cost-order` gives v9's order and the
base's node counts). Every option only reorders moves, so no value can change.

- **What it does** (the variant on by default: `--kc-order 1 --kc-mode 1 --kc-evict --hc 4
  --hc-level 5 --hc-staged`):
  - each killer carries the size of the subtree it last cut with (a running mean when it cuts
    again); the killer with the cheaper record is tried first; a new killer replaces the
    dearer of the two, not the older;
  - a history by (side to move, pieces left, move) of wins and the nodes they cost; at nodes
    with 5 pieces left or more it picks between the two killers for the staged first move,
    and when that fails it reorders the first 4 moves of the list by mean cost of their wins.
- **The pot, counted in the tree** (proof positions 0-23, `--window 0,1`): red's first move
  wins 97-99% of the time at 5, 7 and 9 left, so the order of *trying* has nothing left; the
  nodes are under the wins (62 M of 68 M at 9 left). Where both killers are candidates the
  other one also wins 92-98% of the time, and the cheaper of the two in fact costs 0.55-0.60x
  of today's choice. A killer's recorded cost predicts its cost at a sibling with a
  correlation of 0.60-0.69 in log (red) and 0.71-0.79 (blue): enough to stand on. Choosing by
  record gets 0.78x at 5 left, 0.86x at 7, 0.96x at 9, of a possible 0.55-0.60x.
- **Screen** (nodes against the base; proof blocks 0-23 / 24-47 / 48-87, then ply 6 and 7):

  | Variant | 0-23 | 24-47 | 48-87 | ply 6 | ply 7 |
  | --- | --- | --- | --- | --- | --- |
  | cheaper killer first | 0.840 | 0.849 | | 0.868 | 0.893 |
  | + running mean | 0.808 | 0.799 | | 0.859 | 0.882 |
  | + minimum (instead of mean) | 0.960 | 0.935 | | 1.012 | 0.986 |
  | + evict the dearer | 0.762 | 0.762 | | 0.859 | 0.849 |
  | killers: mean + evict (`kme`) | 0.771 | 0.714 | 0.668 | 0.834 | 0.824 |
  | history alone, 4 moves, 5+ left | 0.771 | 0.653 | | 0.806 | 0.785 |
  | `kme` + history 4, 7+ left | 0.698 | 0.644 | 0.576 | 0.816 | 0.792 |
  | **`kme` + history 4, 5+ left, staged (chosen)** | 0.747 | 0.712 | 0.617 | 0.819 | 0.793 |
  | `kme` + history 8, 5+ left | 0.711 | 0.642 | 0.656 | 0.787 | 0.788 |

  Every variant beats the base on every block; the top five are within block-to-block noise
  of each other. The minimum loses because one lucky cheap cut stays as the record for ever.
- **Time** (plain MSVC build, positions 0-23, four arms interleaved, three rounds, sums): off
  183.1 s; `kme` 151.9 s (0.830x); `kme` + history at 7+ 150.6 s (0.823x); the chosen variant
  145.4 s (0.794x). The 7+ variant has the fewest nodes and pays 2.86 microseconds a node
  against 2.58, because it gives up the staged first move there and sorts the whole list.
- **Other files, nodes with the defaults against off:** the five reference ply-5 files 0.60x
  to 0.68x (99.5 M against 161.5 M in all), every label right; ply 8 right.
- **Driver smoke test** (job a, 16 single-thread workers, one run an arm): 347 M nodes and
  26.9 s against 691 M and 46.5 s. One run against a driver sd of 0.115 a job says only that
  it does not lose there. The agent noticed that the off run's results file has one line
  fewer. Checked: not a fault. The driver does not record a job of fewer than 2,000 nodes
  (`MIN_RECORD`), and that job was answered from the table in one run and not in the other.
- **In the driver** a worker's history is not cleared between its jobs (the killers' costs
  are). Whether a reset or a decay is better there is not measured.
- **The coordinator's own checks of `8f6adce`** (plain build `46f3a9fb8d8e`): with
  `--no-cost-order` ply 7 0-589 = 8,076,471, ply 6 0-199 = 7,136,589, proof 0-3 narrow =
  13,150,272 (the base's counts); with the defaults ply 7 6,404,487 and ply 6 5,846,840
  nodes, ply 8 and the ply-5 files right.
- **Hypothesis (E30):** the base tries the newest killer first and so pays for whichever win
  it meets; the candidate prefers the move whose wins were cheap at neighbouring nodes and
  takes less time for the same values. Expected: about 0.80x total time on proof positions
  (the plain-build read; an ordering change does not lean on the compiler, so a profiled pair
  should read much the same). Test: `SPRT[0, 0.05]` on unused `proof4b` gate units, both
  arms profiled builds with `--window 0,1`, the cap fixed from a tune screen before the first
  gate unit. Guards before a freeze: ply 5 (the next harder file) and whole proof jobs on 16
  single-thread workers.
- **Not chosen now, kept for later:** the 7+ variant with a cheaper window (its nodes at the
  staged path's cost a node), the history over every candidate, win rate times cost as the
  key, the history's life in the driver.

## 2026-11-15 E30: the screen on profiled builds, and the gate's cap

- **Profiled candidate:** `work/e30/cand.exe` = `03e5963b7ba5` (commit `8f6adce`, profile
  collected with the defaults on). With `--no-cost-order` ply 7 0-99 = 1,724,647 (the base's).
  All five ply-5 files, ply 8, `verify 100` and ply 6 on 16 threads: every value right.
- **Screen `E30-S`** (`proof4b` tune units 0-19, 40 positions, against `frozen/climb-base.exe`,
  `--window 0,1`): total time 66.7 s against 87.2 s (0.765x), nodes 0.703x; log ratio
  +0.305 +/- 0.156, sd 0.356 a unit; 4 of 20 units slower (1.10x to 1.42x), the best 0.35x.
- **Cap, fixed now:** 120 units. At sd 0.356 `SPRT[0, 0.05]` needs about 30 units if the
  effect is what the screen read, 60 if it is half that, and 270 if it is only 5%; 120 covers
  a true effect down to about 0.10. An effect smaller than that would end undecided, which
  for a change that screened at 0.77x would itself be news.
- **Gate `E30`:** `proof4b` gate units from 88, profiled builds, `--window 0,1`.

## 2026-11-15 E29: the independent verifier found no way to break reach threats

A second Opus agent that did not write the rule (branch `climb/verify-rt`, commits `56f1b58`,
`7c11643`, on `climb/threats` at `09807d5`) was asked to break it. It found no
counterexample and gives an argument for every variant.

- **The argument, in short.** R is legal where it is tested, so it does not matter that it
  came from a sibling. A move m of the mover outside R's cells and border leaves R legal, and
  R leaves m legal (adjacency is symmetric); the mover's placements after (m, R) are
  placements after R alone, so the one-player optimum after (m, R) is at most that after R,
  which is below the opponent's length with R. Opponent placements only take placements
  away. So m loses; with alpha >= 0 the same gives "at most a draw", a bound that is true in
  any window. In the grow refinement m and R both grow snakes on disjoint cells; a grow is
  legal by empty cells and the player's own cells alone, so they commute. Every pruned move
  has a bound that is true whatever the window, path, killers or thread, so what is stored
  at such a node stays valid. Witnesses can only say yes, and only a no prunes.
- **In-tree check** (`--features shadow`, `src/shadow.rs`): every move pruned by a reach
  threat alone is solved exactly with the rule off, on a table of its own, and the run stops
  if its value beats the bound the rule assumed; each node with such a prune is also solved
  exactly and compared with the value returned and the bounds stored; every "cannot reach"
  is asked again with unlimited work. Five settings (default, all levels, each without the
  grow refinement, all levels with the cache) on ply 6 (0-199), ply 7 (0-589), ply 8 (all),
  one ply-5 file, proof positions 0-23 narrow and 0-2 with the full window, and four runs on
  8 threads: about 190 million pruned moves and 9 million nodes checked, **0 mismatches**.
  At the defaults every pruned move was checked; at all levels about 99% (sampled above 5
  pieces left).
- **The check can fail:** with the zone cut down to R's cells it stopped at
  `00098250821c80901a01800000000000_0000418851004809811040100c000000_18_18_1_11_0`, move
  `4I:8,19,30,41` (assumed -1, true value 1) while every root value of that run stayed
  right. That is the case the root labels cannot see.
- **Values below the root:** 2,703 positions sampled from inside the trees (2 to 11 pieces
  left, both sides, 1,619 of the lines from nodes where the rule had pruned), each solved with
  the rule off and then on in four settings and three windows on fresh tables: about 50,400
  solves, 0 mismatches.
- **Not covered:** proof positions 24-87 and `proof4b`; `--reach-threat-min`, other work
  limits, `--reach-threat-after` above 1 (each only prunes less); `prove` and PN search do
  not use the rule.
- **Fragile though correct** (the verifier's list): the zones rest on two facts of the rules
  (a grow needs only empty cells and one's own cells; opponent cells never enable a
  placement); "touches own cells, so it grows" is computed by hand at three call sites; the
  cache reads `need >= e.need` and is never cleared (sound because entries are facts);
  `try_threat` now serves two paths behind two flags.
- **Verdict:** cleared for its speed gate, which waits for E30 (the base will change).

## 2026-11-15 E30: the gate passes (0.796x total time on fresh proof positions)

- **Gate `E30`** (`proof4b` gate units 88-117, 60 positions, profiled builds, `--window 0,1`):
  **pass** at the minimum of 30 units, LLR +15.1 against a bound of 2.94. Total time 178.3 s
  against 224.0 s (**0.796x**), nodes 0.735x; log ratio 0.276 +/- 0.057 (a geometric mean of
  0.76x a unit), sd 0.158 a unit; 2 of 30 units slower. Every value right.
- **The screen predicted it:** 0.765x on 20 tune units, 0.794x in the agent's three rounds on
  plain builds. The gate's spread (sd 0.158) is under half the screen's (0.356), so the cap of
  120 was generous.
- **A fault of the coordinator's during the gate:** a 17-second compile (the trial merge of
  the two branches) ran while chunk 1 (units 98-107) was being timed. One core beside a
  one-thread run, but it should not have happened. Sensitivity: chunk 1 read 0.813x, the
  least favourable of the three (0.779x, 0.813x, 0.780x); without it the 20 remaining units
  read 0.780x with the same mean log ratio (0.277 +/- 0.051). The verdict does not hang on it.
  Rule from now: nothing is compiled while a gate of mine is being timed.
- **The size:** an SPRT's estimate leans toward the bound it crossed, less so when it stops at
  its minimum as here. Three reads agree on 0.77x to 0.80x.
- **Next:** guards `E30-UP` (ply 5) and `E30-DRV` (whole proof jobs, 16 workers), then merge
  and freeze.

## 2026-11-15 E30 frozen: the climb's base now chooses the cheaper win

- **Guards, both held:**
  - `E30-UP` (ply 5, the tune range's three units, 30 positions, full window): total time
    7.30 s against 8.68 s (0.840x), nodes 0.802x.
  - `E30-DRV` (whole proof jobs on 16 single-thread workers, depth 1, seconds, base then
    candidate): a 46.2 / 27.0, b 58.3 / 43.9, c 57.3 / 40.7, d 94.2 / 58.5; total 255.9 s against 170.1 s
    (**0.665x**), nodes 0.659x. One run an arm and a driver sd of 0.115 a job, so
    four jobs resolve about 12%: the gain on whole proofs is real and at least as large as on
    single positions. The driver gains more than one thread does, plausibly because a
    worker's history carries over its jobs, which are siblings (not shown).
- **Frozen:** `frozen/climb-base.exe` = `03e5963b7ba5` (profiled MSVC build of `8f6adce`, in
  `parallel-v9` as `98e8167`); `frozen/climb-base-plain.exe` = `46f3a9fb8d8e`, the plain build of
  the same source. The base before it is `frozen/E25-3d3bd213049b.exe`.
- **`tools/build.sh` now takes the run lock for the compile** (the fault during the gate).
  A build waits while a gate runs.
- **Since this morning on proof positions, one thread** (each read against the base of its
  time; passes do not add): toolchain 0.960x, profile 0.944x, search bundle 0.916x, prefetch
  0.949x, cost-aware order 0.796x. The product is 0.63x; a direct read of the morning's
  build against today's has not been taken.
- **Lessons:**
  - The ordering study (a learned order pays 0.97-1.00x) looked like a dead end and was the
    opposite: it said *which* pot was full. The first move already wins; what varies is what
    the win costs, and no feature of the position predicts that, but a sibling's cost does
    (correlation 0.6-0.8 in log). Counting in the tree before building took the agent an hour
    and picked the idea.
  - The variant with the fewest nodes was not the fastest (0.698x nodes and 0.823x time
    against 0.747x and 0.794x): it gave up the staged first move. Third time today that
    nodes and time disagree; the screen on time chose.
  - A gate on 30 units sufficed because the effect was six times the test's gap. The cap is
    for the case where it is not.

## 2026-11-15 E29 on the new base: 0.966x in the screen; the gate is put off until the rule is larger

- **Candidate:** branch `climb/both`, commit `6057bd7` (the merge of `climb/threats` into E30),
  profiled build `f8417c7b1b1f`. With `--no-reach-threat` it gives the base's counts (ply 7
  0-589: 6,404,487); with the rule 5,255,948. Plain build: each rule alone gives its own
  branch's node counts, both off the old base's, and with both on every label is right (ply
  6, 7, 8, proof 0-47, the five ply-5 files, 16 threads; `verify 100` ok).
- **Screen `E29-S`** (`proof4b` tune units 20-49, 60 positions, against the frozen base with
  E30, `--window 0,1`): total time 124.3 s against 128.8 s (**0.966x**), nodes 0.880x; log
  ratio 0.042 +/- 0.059; sd 0.164 a unit. Two units went the wrong way in nodes (1.32x and
  1.16x; time 1.54x and 1.35x): the rule changes which moves feed the order's records.
- **Plan** (`hc.py plan`): `SPRT[0, 0.05]` needs about 58 units at either hypothesis and 94
  halfway. That is affordable (325 units left), but the truth is probably 3-4%, inside the
  test's gap: there the test passes a real gain only 64-88% of the time, and a fail would
  close the idea at this setting.
- **Decision:** no gate now. The rule has more in it than this setting uses (all levels:
  nodes 0.56x before E30, time unchanged), so its agent goes a third round on the merged
  base before the one gate: trigger the test at 5 pieces left and more by the effort the
  node has already cost (a found threat costs 4-6 nodes' time; it pays where the pruned
  moves are dear), the margin filter, the settings at 4 and fewer re-tuned for the new tree,
  and the two units that got worse. Target: 0.93x or better on profiled builds on every
  block. If round three ends at 0.96x, the choice is a gate with that risk or leaving it.
- **Not a change of the bar:** the bounds stay `[0, 0.05]`. What is declined is a gate whose
  likely outcome depends on luck.
