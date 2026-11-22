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

## 2026-11-15 Ordering, round two (report): the two-killer pot is spent; two more killer slots are next (E31)

Agent's report on branch `climb/cost`, commit `ddd4688` (every new option off; with them off
the counts are `8f6adce`'s: proof 0-23 56,301,796, ply 6 5,846,840, ply 7 6,404,487). Its
numbers, not yet mine.

- **Pot left between the two killers, with E30 on** (proof 0-23, both killers searched at the
  same red node): choosing by record now pays 0.957x / 0.999x / 1.161x at 5 / 7 / 9 pieces
  left (0.78 / 0.86 / 0.96x before E30). The cheaper killer in fact would still pay 0.76 /
  0.74 / 0.72x, but the record no longer predicts which it is (correlation of the log
  ratios 0.40 / 0.39 / 0.12). What a record can tell between two killers is used up.
- **Where the nodes are now** (red nodes, proof 0-23, 56M): under wins 46.0M at the root,
  43.3M at 9 left, 40.8M at 7, 14.7M at 5 (E30 took a third off each); under failed moves
  10.3M at the root and 5.7M at 9 left, unchanged, now 18% of all nodes. Wins that come from
  the list at 7 and 9 left are few and dear (25k cuts, 4.1M nodes at 7; 434 cuts, 5.8M at 9).
- **Probe then commit (a budget race between the killers): dropped on an offline
  simulation.** On the oracle's pairs, budgets of f times the record, doubling: the
  pessimistic figure (work set aside is lost) is never below 0.993x and the optimistic one
  never below 0.990x, against a bound of 0.73-0.76x. The cheaper killer is often the one
  that costs more than its record, so a budget cuts the cheap wins' tails as often as the
  dear ones. Not built.
- **Built and screened on nodes** (ratio to the defaults; proof blocks 0-23 / 24-47 / 48-87,
  then ply 6, ply 7):
  - `--xk 2` (two more killer slots: a move pushed out of the two killers is kept with its
    cost; all four are tried as staged first moves, in the history's order, then by cost):
    0.958 / 0.798 / 0.824, 0.913, 0.935; **0.845x over proof 0-87**; ply 8 0.949x; the five
    ply-5 files 0.938x in total, the worst 1.07x (`other_openings`). No block above 0.97.
  - `--xk 1`: 0.959 / 0.852 / 0.877, 0.909, 0.933.
  - `--xk 2 --hc-key 2` (the history's key counts the nodes of failures too): 0.860 / 0.822 /
    0.923, 0.879, 0.915; 0.883x over 0-87; `other_openings` 1.27x.
  - A wider history window (`--hc 8`, 16, 32, the whole list), `--hc-all`, keys 1 and 2
    alone: 0.86-1.02 on single blocks, nothing on ply 6 and 7, some blocks above 1.
- **Time** (plain build, proof 0-23, three interleaved rounds): `--xk 2` 0.952x (nodes 0.957x),
  `--xk 2 --hc-key 2` 0.857x (nodes 0.860x). Time per node is the same in all arms, so time
  follows nodes; 24-87, where `--xk 2` gains most, was not timed.
- **The driver** (jobs a, b, c, 16 workers, two runs an arm): keeping the history between a
  worker's jobs (today) is best; clearing it costs 1.07x nodes, halving it 1.10x (1.37-1.54x
  on job a). `--xk 2` in the driver read 0.996x nodes and 1.06x time, which three jobs
  cannot tell from no change or from the bench's gain (sd 0.115 a job; about +/-8%). The
  agent's guess at why: `solve_job` clears the killers' costs and the extra slots for every
  job. One run had a 25-minute gap under the lock with no solver running.
- **A rule broken, by the agent's own account:** its first run of the simulation read 4M
  rows outside the lock for about two minutes; stopped, and sampled after that.

**What goes to a gate: E31 = `--xk 2` on by default.** Branch `climb/xk`, commit `381560f` (the
coordinator's flip of the default on top of `ddd4688`; `--xk 0` and `--no-cost-order` switch
it off).

- **Hypothesis:** with two killers, a move that won cheaply and was pushed out by a later
  cut is forgotten and must be found again through the list, after dearer moves. Keeping it
  in a third and fourth slot, with its cost, lets the staged first move choose among four
  candidates by the same records. Expected on fresh proof positions, one thread: total time
  about 0.85-0.90x (the tune blocks say 0.845x in nodes; the least of them 0.958x).
- **Why `--xk 2` and not the variant with key 2:** key 2 is ahead on one block and ply 6/7,
  behind on the largest block (0.923 against 0.824) and bad on one ply-5 file. One change at
  a time; key 2 can be asked again on top.
- **Test:** `SPRT[0, 0.05]` on log time ratio, unused `proof4b` gate units from 118, profiled
  builds, `--window 0,1`, against `frozen/climb-base.exe`; the cap from `hc.py plan` on the
  screen `E31-S` (`proof4b` tune units 50-79, which no choice in this round has seen).
  Guards after a pass: ply 5 (`E31-UP`) and whole proof jobs (`E31-DRV`).
- **Open before the gate:** what the driver gets from it. A pass on single positions does
  not show a gain for `prove` on 16 workers; the agent's next round is the driver.

## 2026-11-15 E29, third report: reach threats at every level, 0.81-0.92x, through a cheaper reach test

Agent's report on branch `climb/threats`, commit `f24a2d2` (on top of `climb/both` `6057bd7`).
Its numbers, not yet mine.

- **What changed.** Not the triggers the brief asked for. A cheaper reach test
  (`--reach-start3`, `f368e74`): with three or more pieces in hand and a target above the cells
  in hand, the first piece is generated only touching an old snake that can be the final
  snake's longest old one (its length + the cells in hand + the k longest other snakes no
  longer than it >= the target, k the number of pieces). With it the test at 5 pieces left
  costs 1,391 cycles for "can reach" and 14,828 for a threat (7,312 and 35,248 before), and
  the rule pays at every level. New defaults in `f24a2d2`: the rule at every level
  (`reach_threat_max 99`), also tested at node start at every level (`reach_threat_start 99`),
  start3 on. `--reach-threat-max 4 --reach-threat-start 4 --no-reach-start3` gives `6057bd7`'s
  counts; `--no-reach-threat` the frozen base's.
- **Profiled pair against the frozen base** (three rounds, alternated, `--window 0,1`): proof4
  0-23 0.831x, 24-47 0.832x; proof4b positions 0-49 0.848x, 50-99 0.915x (medians); 0.867x
  on the summed medians. Nodes 0.46x, 0.49x, 0.53x, 0.57x.
- **Labels right** with the new defaults on ply 6 (16 threads), 7, 8, the five ply-5 files,
  proof4 0-47, proof4b 0-99; `verify 100` passes.
- **Why the brief's ideas did not apply:** at 5 and 6 left almost every test is asked before
  the node has spent 16 nodes (it comes right after the first failed move), so an effort
  trigger has nothing to select on. The margin filter and `--reach-threat-after 2` cost
  nodes. `--rt-refuter` changes nothing: with the cost order the first killer one ply down
  already is the move that cut.
- **The two units that got worse in `E29-S`** (old setting): the regression comes through
  E30's cost history (with `--hc 0` positions 75 and 97 read 1.07x and 0.92x in nodes
  instead of 1.56x and 1.55x; with `--no-cost-order` 0.94x and 0.78x). Why is not found; not
  rechecked under the new defaults.
- **Not known:** what the tests at 4 left and fewer cost outside the reach search (the reach
  cycles are 1.3% of the run); the per-unit picture of the weakest block (0.915x).
- **Not covered by the independent check:** the verifier's shadow runs (`climb/verify-rt`)
  were on the earlier code. start3 is a new pruning inside the reach test, which every
  "cannot reach" answer in the solver depends on (threats, and the reach tests before the
  cluster bounds). The labels and `verify 100` are all that cover it. With start3 on and the
  rule at every level the nodes went *up* (27,984,567 to 28,463,674 on proof 0-23), which an
  exact speed-up of a test with a work limit does not obviously do.

**Before any gate:** an independent agent checks start3 and the new defaults the way the
first rule was checked (the argument in its own words, the reach test's answers with start3
against without on queries from real searches, the shadow re-search of what the rule
pruned, a planted fault), and explains the node increase. A wrong "cannot reach" is a wrong
value somewhere. Then: merge onto the base of that time (E31 is at its gate first), my own
identity and label checks, a profiled pair, a tune screen, the cap, the one gate.

## 2026-11-15 Ordering, round three (report): the driver gets nothing from the extra slots; E31 is held

Agent's report, branch `climb/cost`, commit `1b2a3b6` (on `climb/xk` `381560f`; new options off,
counts with them off equal a build of `381560f`: proof 0-3 12,078,999, ply 6 5,336,972, ply 7
5,990,249). Its numbers.

- **A driver read with exact node counts: `--emulate W`.** One thread holds W solvers that take
  the jobs in turn; the coordinator still keeps W jobs out, so killers are handed down as with
  W workers and the table is shared. Deterministic: job a, base against base, 344,011,725
  nodes twice, every job identical. A whole job is 5 minutes (a) to 14 (d) on one thread, two
  at a time (a `--tt 27` table is 2 GB). What is left is the spread from job to job: an order
  change moves the total by 0.85x to 1.07x between jobs (sd of the log about 0.08), so six
  tune jobs resolve about 6-8%, and 3% would need about 30 jobs. The approximation: solvers
  take jobs strictly in turn, real workers as they come free.
- **`--xk 2` inside the driver, exact:** a 1.013x, b 1.067x, c 0.924x; total 1,595.5M nodes
  against 1,591.2M, **1.003x**. The real driver in round two read 0.996x on the same jobs. On
  cold single positions the same option reads 0.845x (88 tune positions).
- **Why, as far as known:** not the clearing of the costs and slots for every job (bench
  clears them for every position and still gains). A driver job starts from the killers its
  siblings proved, handed down by the coordinator; a bench position starts with none. Not
  counted directly.
- **Candidates in the driver** (exact, emulated, nodes against the defaults):
  - `--drv-carry` (a job keeps the costs and extra slots its solver had for the handed
    killers that are the same moves): a 0.937, b 0.852, c 1.055, d 1.040; total 0.978x. The
    jobs disagree in sign.
  - `--drv-share` (the history merged between workers at job boundaries): job a 1.091x; with
    carry 0.971x. A worker's own recent history is better than everyone's.
- **Recommendation:** no driver gate; nothing here can be shown.
- **Breaches, by the agent's account:** one pair of runs held the lock for 13-14 minutes; a
  launch of five 2 GB provers at once was stopped while it still waited for the lock; it
  could not stop its own loop shell (refused by the permission check) and shortened the
  loop's script instead.

**Decision: E31 is not gated now.**

- The goal is the prover (owner, 2026-11-15: "keep hill climbing the performance of the
  prover"). The three jobs put the extra slots' effect in the driver at 1.00x with a
  standard error of about 4-5% on the log: a few percent either way is open, the bench's
  0.845x is not (3.6 standard errors away).
- A gate on cold single positions would pass on a gain the prover does not get, and would
  move the base under the reach-threat candidate for nothing. The candidate stays on
  `climb/xk` (`381560f`); my checks of it are right so far (flags off: the base's counts;
  `--no-cost-order`: v9's; proof 0-23 53,909,813 and 24-47 58,520,632 with every label right).
  The screen `E31-S` was already queued and is recorded as the cold-solve number.
- **Reopen:** a driver read over six or more jobs that shows a gain; or cold single solves
  become a target of their own (the analysis board solves that way).
- **What this says about the ruler.** `tests/proof4` positions are the driver's jobs solved
  cold: no killers handed down, no history from sibling jobs, an empty table. E30 gained on
  both (0.796x cold, 0.665x in the driver). E31 gains only cold. A change that works by
  remembering more about sibling nodes is exactly what the driver's hand-down already does,
  so for this class the cold ruler overstates. From here on an ordering change needs a
  driver read (emulated, exact nodes, the tune jobs) *before* its gate, not as a guard
  after; changes that prune or that speed up a node are not affected in the same way, but
  get the same read once.

## 2026-11-15 Ordering, round four (report): the order is used up inside the driver

Agent's report. Its numbers.

- **`--emulate W` on a branch of its own:** `climb/emulate`, commit `ab3bf7e`, parent `8f6adce`
  (the frozen base's source); 18 lines in three files. Off: the base's counts (proof 0-23
  56,301,796, ply 7 6,404,487, ply 6 5,846,840). On: job a with 16 solvers 339,620,578 nodes
  twice, every job identical, root -1 and every recorded child 1.
- **Where the driver's nodes go** (job a, emulated, the base's defaults, the agent's cost
  counters; "cold" is every fourth child of job a, 169 positions, through `bench`):

  | Left | First try wins, driver / cold | Nodes under failed moves, driver / cold | Driver's win nodes from handed-down killers |
  | --- | --- | --- | --- |
  | 11 (the job's root) | 96.3% / 92.9% | 7.9% / 13.6% | 88% |
  | 9 | 99.5% / 98.5% | 4.5% / 7.2% | 64% |
  | 7 | 99.7% / 99.5% | 3.0% / 3.6% | 1% |
  | 5 | 98.5% / 98.1% | 1.4% / 1.7% | 0% |

  A driver job costs 0.74x of the same position solved cold (108.4M against 146.9M nodes on
  those 169).
- **Why the extra killer slots gain cold and not in the driver:** at 9 and 11 left a job
  starts from killers its siblings proved. `--xk 2` orders all killers by the history's key
  and then by recorded cost; a handed-down killer has no recorded cost and sorts after the
  extra slots. With the slots on, the handed-down share of win nodes at 9 left falls from
  64% to 53% and the nodes under failed moves rise from 4.5% to 5.5%. Cold there is nothing
  handed down to displace. (A fix to E31, not built: handed-down killers ahead of extras of
  unknown cost.)
- **What is left for the order in the driver:** failed first tries hold at most 7.9% of the
  nodes at the job's root, 4.5% at 9 left, 3.0% at 7, 1.4% at 5 (nested; the union is below
  their sum). The largest piece is 25 of 675 jobs whose first try fails. Which win is taken
  remains (the cheaper killer in fact 0.74x), and nothing measured predicts it. The agent's
  answer: used up for rules of this kind; move to another line.
- **Not counted:** job b; the coordinator's order of jobs; times (the emulation gives nodes).

**A fault in `tools/build.sh`, found by the agent and fixed.** The script checked only that
`snek.exe` exists in the build directory, and a pipeline hid cargo's exit status. After a
compile error the exe of an earlier build was copied out and its hash printed as if the build
had worked. No gated build was affected as far as the records show: every candidate's counts
were checked with its new option on, which an older exe would not have. Now cargo's exit
status decides (it passes through the lock script; checked). The test of the fix (a good
build, then a compile error with the earlier exe still in place) is queued behind the lock
as this is written; its result gets its own line below when it is in.

**Next line for this agent: the top of the proof.** In the paused run (old build) the 74
refuted replies took 29,491 s. 35 needed one red try (mean 249 s); the other 39 needed 232
failed tries on top, so about 11,000 s, 37% of the time, went to red moves at ply 2 that did
not win, and four replies hold 133 of those 232 tries. That is a pot at the top of the real
proof that no test position sees: red's choice of move against a reply, and what a failed
try costs. First a profile with today's base on replies already refuted (fresh output files;
the paused proof itself is not touched), then the pot by simulation on the recorded tries.

## 2026-11-15 The analysis board (owner's request), and the build script's test

Owner, 2026-11-15: "Can you make me like an interactive gui, with an analysis board? where I
can set up a position and run the solver on it?" Built by an Opus agent on branch `gui`
(`ab9ce31`, `9a6b6bb`, `b531c89`), merged as `35d81c7`. No change to `src/`.

- **What it is:** `python tools/gui.py` serves one page (`gui/`) on `http://localhost:8765`
  (`--port`, `--solver PATH`; default `frozen/climb-base.exe`, so it follows every freeze).
  Standard library only, nothing fetched from the internet, bound to 127.0.0.1.
- **The page:** the board, each side's pieces and longest snake; pieces are placed by
  dragging across cells (red's first by a click); undo, redo, a move list to jump in; load
  and copy a position text or a move list. "Solve" runs the position and its children in
  parallel and stops at the first winning move; "All moves" gives every move's value. A time
  limit per position, a process count, Cancel; "not solved within the limit" is a result.
- **The rules are in the page** (`gui/rules.js`) and are checked against the solver by
  `python tools/gui_check.py` (agent's run: perft 1 and 2 agree, 91 and 194,388; 4,503
  positions from random games at plies 0-17, 1,419,535 children compared as position texts,
  472 of them after a forced pass, 300 ended games; 0 disagreements). `tools/gui_test.py`
  exercises the server (agent's run: 0 failures).
- **The lock:** a run from the page takes a priority ticket (`0-<ns>-<pid>`, which sorts
  before every other ticket in `tools/lockfair.sh`'s queue), so it waits only for the command
  that holds the lock now; "Run now" starts without the lock. Every run is appended to
  `work/gui/runs.log` with whether it held the lock.
- **My own click-through** (the merged version, from the main checkout): placing by click and
  drag; loading a position eight moves in, "All moves": "Red wins with 4C:42,43,54,64" (the
  set's label is a red win); a proof position four moves in, "Solve" with the lock: waited
  20 s for the run in progress, then "Red wins with 4I:46,47,48,49" in 1.9 s on 16
  processes (label: red win); playing that move, undo, redo. Not tried by me: the limit
  being hit, copy, Cancel during a run, a run of more than 15 minutes.
- **Breaches, by the agent's account:** 16 solver processes outside the lock for 56 s
  (23:25:54 to 23:26:50, a race at page load in its own test, since fixed); one check held
  the lock for ten minutes doing nothing (a headless browser that never answered; the wait
  is now two minutes). No timed run of mine was under way then (the `E31-S` screen had not
  started); the reach-threat agent was told the window.

**`tools/build.sh`, the test of the fix** (`work/buildfix_test.txt`): a good build, then the
same tree with a compile error while the earlier exe is still in the build directory. The
script printed cargo's errors and "the build failed", returned 1, and copied no exe out.

## 2026-11-16 Same machine, the reference solver: ours takes 0.555x of its time on its own bench files

Owner, 2026-11-16: "And have you measured how we compare to the solver in the original yt
video speed wise?" Not on one machine until now. Asked "Build and run the reference solver
on this PC for a same-machine comparison?", the owner answered "Yes, run it".

- **What ran:** `github.com/BoltKey/snekagon-lab` at `61eb3a4` (the checkout the test data came
  from; nothing fetched), built in WSL2 (Ubuntu 24.04, g++ 13.3, `-O3 -march=native -std=c++20
  -pthread`, the README's flags) in `/opt/snekref`; `BENCH_FRESH=1 ./build/snek benchset
  bench/ply5_<file>.txt 1` (one thread, every position from an empty table), without network
  (`unshare -n`) as user `nobody`. Before building, its source was read for process, network
  and file calls: the network code is in its `worker` and `serve` modes, which were not run;
  `benchset` reads a file and prints.
- **Against:** `frozen/climb-base.exe` (`03e5963b7ba5`, the base with E30; a profiled MSVC
  build, native Windows), `bench tests/set1/bench_ply5_<file>.txt --quiet`, one thread, every
  position from an empty table.
- **How:** the two solvers alternate file by file, each run alone under the run lock, two
  rounds with the order swapped (`work/ref/`). No run from the analysis board fell in the
  window (00:10 to 00:40).

  | File | Positions | Ours, s (two rounds) | Reference, thread-s (two rounds) | Reference / ours | Our nodes | Its nodes |
  | --- | --- | --- | --- | --- | --- | --- |
  | `ply5_other_openings` | 24 | 4.06, 3.98 | 12.1, 12.2 | 3.02x | 8.42 M | 7.45 M |
  | `ply5_blue_wins` | 24 | 9.42, 9.72 | 10.8, 10.7 | 1.12x | 14.43 M | 4.40 M |
  | `ply5_blue_wins_holdout` | 24 | 9.23, 9.26 | 10.8, 10.9 | 1.17x | 14.31 M | 4.31 M |
  | `ply5_random_units` | 24 | 8.62, 8.76 | 13.3, 13.6 | 1.55x | 13.75 M | 6.16 M |
  | `ply5_ublock_refutations` | 30 | 46.32, 46.96 | 93.2, 94.0 | 2.01x | 53.58 M | 37.37 M |
  | Total | 126 | **77.6, 78.7** | **140.2, 141.4** | **1.80x** | 104.49 M | 59.71 M |

- **Result:** ours takes 0.554x and 0.556x of the reference's time (1.80x faster) on these
  126 positions. It searches 1.75x more nodes and spends 0.75 microseconds a node against
  2.36. Two rounds agree to 1%.
- **Values:** all 126 agree. On the four all-win files both give 24 wins; on the refutation
  file both give 27 losses and the same three draws (positions 6, 15, 23), compared one by
  one.
- **The reference reproduces its published counts exactly** (59,709,026 nodes here; 59.7 M
  published) and takes 141 thread-seconds here against the 197 it publishes for its
  authors' machine.
- **What this is not:** a like-for-like build. Theirs is g++ in a WSL2 virtual machine, ours
  a profiled native build; they publish a MinGW build on Windows, which was not made here.
  One thread only; their parallel solve and ours were not compared. These are positions
  five moves in, their benchmark; nothing here says how the two compare on a whole proof.
- **Where the gap is:** on the two `blue_wins` files it searches 3.3x fewer nodes and we are
  barely ahead (1.12x, 1.17x). Its move order and bounds there are better than ours. The
  reach-threat candidate (`f24a2d2`, unverified) reads 53.2 M nodes on these files in its
  agent's label runs, which would be below the reference's count for the first time.
- **A slip on the way:** the first build went to `/tmp` in WSL, which was gone by the next
  call (the distribution had stopped in between), so the first comparison script timed only
  ours and was stopped. Rebuilt in `/opt/snekref`. Also new: `RUN_LOCK_FIRST=1` in
  `tools/lockfair.sh` puts a caller ahead of the queue, for what the owner is waiting on.

## 2026-11-16 E31 (held): the checks that ran, and the screen that did not

- **My checks of `climb/xk` `381560f`** (plain build `b6fadcbe7879`, `work/e31/checks.txt`): with
  `--xk 0` the frozen base's counts (proof 0-23 56,301,796, ply 7 6,404,487, ply 6 5,846,840);
  with `--no-cost-order` v9's (proof 0-3 13,150,272, ply 7 8,076,471); with the defaults
  every label right on proof 0-23 (53,909,813 nodes) and 24-47 (58,520,632), ply 7
  (5,990,249), ply 6 (5,336,972), ply 8 (5,699,363), the five ply-5 files (13,239,959,
  12,960,170, 12,884,337, 8,974,161, 49,994,059) and `verify 100`. These are the agent's counts.
- **Not run:** the 16-thread label check, the profiled build and the screen `E31-S`. I stopped
  the script while it waited in the queue: the candidate is held, the screen would only have
  given the cold-solve time, and nine runs were waiting for the lock, the verifier of the
  reach test among them. The earlier entry said the screen would be recorded; it was not.

## 2026-11-16 E29, fourth report: no cheaper node yet, but the halving reaches the driver (0.85-0.89x)

Agent's report, branch `climb/threats`, head `394b50d`; the defaults are still exactly
`f24a2d2`, which stays the candidate. Its numbers.

- **The driver, tune jobs a, b, c** (`prove --threads 16 --job-threads 1 --depth 1 --tt 27`, the
  frozen base against the profiled candidate `58e025ada122`, one run an arm, alternated,
  one job at a time under the lock; root -1 and every child 1 in every run):

  | Job | Base, s | Candidate, s | Time | Nodes |
  | --- | --- | --- | --- | --- |
  | a | 27.49 | 23.98 | 0.872x | 0.476x |
  | b | 49.02 | 41.69 | 0.850x | 0.509x |
  | c | 43.30 | 38.41 | 0.887x | 0.551x |

  Unlike the extra killer slots, this gain is in the prover, at about the size it has on
  single positions. One 1.2 s run from the analysis board without the lock fell inside job
  b's candidate read (so that read may be a little slow, which is against the candidate).
- **Where the candidate's time goes** (a study build with cycle counters, proof 0-23, total
  151.3 G cycles against the base's 181.4 G; a second read after the unlocked window agreed
  within 4% in every section): the reach-threat tests are 40.5 G, 27% of the candidate's
  time, and nothing else grew: every other section is smaller in total than at the base.
  Two thirds of the tests' cost is one case: "cannot reach" with a hand of three pieces at
  6 left, 27.2 G over 807 thousand tests, about 34 thousand cycles each. Without the threat
  tests the candidate would stand at about 0.61x of the base. That is the pot.
- **Mechanical cuts with the nodes unchanged:** none that pays (child copy 0.4% at most, a
  lighter child 0.2%; the bound flood decides the threat's value and cannot be skipped).
  Not measured: generating only the covering placements in `scan_threats` (14% of the time).
- **Two cuts inside the three-piece test** (`--reach3-cap`, `d959291`; `--reach3-cap2`,
  `394b50d`; both off, same nodes): no time gain in two of three rounds (one round read
  27.7 s against 36.5 s and is not explained). They change how "cannot reach" is produced
  and would need the verifier; `verify` covers three-piece hands only thinly (1 position in
  40). Not adopted.
- **Next lever, the agent's words:** a cheaper exact proof of "cannot reach" for a
  three-piece hand at 6 left (18% of the candidate's time): profile inside that test, then
  one search that answers both targets at once.

**Order from here.** `f24a2d2` goes to its gate as it is, once the verifier has cleared
start3 and the new defaults; it is not held back for a cheaper test. My own checks, the
profiled pair and the tune screen are prepared meanwhile (they do not depend on the
verdict; the gate and the freeze do). The cheaper test is the agent's round five, on top,
and each of its changes goes to the verifier before it can be a default.

## 2026-11-16 The top of the proof, first report: replies are 2-5x faster than at the pause; the pot is red's order, bounded at 21%

Agent's report (the ordering agent's round five). Nothing built, nothing committed. Its
numbers; one run a reply.

- **Six refuted replies re-proved with today's base**, each as its own root (red to move
  after `1:60` and the reply), `prove --threads 16 --job-threads 1 --depth 2 --tt 27
  --upper-first`, a fresh results file each (the paused proof's files untouched):

  | Reply (blue cells) | Paused run: tries, s | Today: tries, wall s | Today / then |
  | --- | --- | --- | --- |
  | 10,20,29,30 | 1, 113 | 1, 58.2 | 0.52x |
  | 18,28,37,38 | 3, 208 | 1, 90.7 | 0.44x |
  | 20,30,39,40 | 4, 248 | 1, 94.1 | 0.38x |
  | 69,80,91,102 | 5, 465 | 2, 144.5 | 0.31x |
  | 57,68,79,90 | 19, 1227 | 4, 228.0 | 0.19x |
  | 81,91,100,101 | 1, 94 | 11 or more, stopped at 836 | worse than 8.9x |

  Then against now mixes three things: the build, the driver setting (16 single-thread
  workers instead of one job of 16 threads) and the order of red's tries, which today
  starts cold at every root (no winners from earlier replies). Five replies are two to five
  times faster. The sixth shows what the missing history costs: the paused run found its
  winner at the first try, an image of an earlier winner; cold, eleven tries had not found it.
- **`--upper-first` matters here:** without it the first reply also ran a (-1,0) pass and took
  113.9 s instead of 58.2 s. Red's node at ply 2 needs only "does red win".
- **A failed try is not searched further than the proof needs** (`src/prove.rs`: the child
  window at line 440, the job's window at 615, what is stored at 188-196): a red try gets
  (-1,0) and stops as soon as blue is shown not to lose. 231 of the 232 failed tries in the
  paused file are stored as [0,1]. Nothing exact to cut there.
- **What is lost today instead:** red tries the coordinator starts beside the first and
  stops when the first wins: 3-26% of the job seconds on the five good replies. At a single
  reply's root nothing else waits for the workers, so the whole proof would lose less.
- **The pot in the order of red's tries** (simulated online on the paused run's recorded
  tries of the 25 tune replies; a rule can only rank moves that were tried, which flatters
  it): the winner first would pay 0.788x. Images of earlier winners by count 0.954x;
  nearness of the reply 0.980x; exact repeats 0.971x. 21 of the 25 winners are images of an
  earlier winner, but so were 36 of the 42 failed tries, so the image bonus is also what
  puts the failures first.
- **The agent's recommendation:** build the measurement before any rule: a `prove` option
  that keeps the winners (and the known results) across the roots of one position file, so
  the tune replies can be re-proved in the proof's order with today's base and today's full
  rule. Then two candidates: the image bonus capped, and no second try at a red ply-2 node
  while other work waits.

**Decision.** The tool is built now (it costs no machine time); its runs wait until the
reach-threat gate is through, because a replay of 25 replies is about an hour of all 16
threads an arm and the lock is the bottleneck tonight. The replay is also the missing ruler
for the proof itself: seconds a reply, in the proof's order, with everything the driver
knows. Any frozen change should be read on it once.

## 2026-11-16 E29, the verifier's verdict on round three: start3 and the new defaults cleared, with limits

Agent's report (the independent verifier; branch `climb/verify-rt3`, commits `6480f52`,
`ba13096`, `74df585`; run outputs in its worktree, `work/g1` to `work/g7`). Its numbers.
It did not build the change and does not gate it.

**Verdict: no wrong value and no wrong "cannot reach" answer found. Every check passed.**

- **The argument, checked against the code.** The mover alone, opponent frozen, ends with a
  snake F of at least the target. F holds old snakes; take S, a longest of them. F's new
  pieces can be ordered outward from S so that each placement is legal now. Every piece is
  an induced path, so it touches at most two components outside itself, one of them what is
  built: each piece brings in at most one more old snake. So F is at most len(S) + the cells
  in hand + the k longest other snakes no longer than S (k pieces in hand). Ties are
  counted (`<=`), a piece joining two snakes is counted, a ring brings in none, an empty
  start set answers "cannot". At the boundary (target = cells in hand + 1) start3 does nothing.
- **Differential check inside every reach test** (shadow build at the new defaults; each
  query in start3's domain asked in the other mode with the same work limit, in both modes
  with unlimited work, and a sample against an independent brute force). Over ply 6 0-199,
  ply 7 0-589, ply 8, `ply5_other_openings`, proof4 0-23 narrow, proof 0-2 full window and
  three 8-thread runs:

  | Hand | Queries | start3 "cannot", plain "can" | The other way | Unlimited: start3 differs | Brute-force checked |
  | --- | --- | --- | --- | --- | --- |
  | 3 | 9,303,444 | 0 | 0 | 0 | 1,163,623 |
  | 4 | 366,073 | 163 | 0 | 0 | 45,812 |
  | 5 | 97 | 0 | 0 | 0 | 0 |
  | 6, 7 | 0 | | | | |

  All 163 changed answers are "cannot" by the unlimited test in both modes. No brute-force
  check disagreed.
- **Shadow re-search at the new defaults**, same sets: 128.7 M moves pruned by a reach bound,
  127.6 M of them (99.1%) solved exactly with the rule off; 3.86 M nodes where the rule
  pruned solved exactly against the value returned and the bounds stored; all 6.58 M
  "cannot reach" answers asked again with unlimited work, start3 off and on. **0
  mismatches.** The shadow build's node counts equal the candidate's (proof 0-23:
  26,095,441 in both).
- **Values below the root:** 1,880 positions from inside the trees (levels 2-11), each
  solved with the rule off and then in 4 settings x 3 windows on fresh tables: 22,560
  solves, 0 mismatches.
- `verify 100` passes. **Proof driver** (16 single-thread workers, depth 1): job a root -1
  and all 675 root moves -1; job b root -1 and all 838 root moves -1.
- **Planted faults.** No other-snakes term: caught (differential, re-search, `verify`).
  Ties not counted: caught. **k-1 other snakes instead of k: caught by nothing**, not by
  any in-tree check and not by `verify 100`; node counts identical to the unfaulted run. A
  position built by hand shows it is a real fault
  (`000100000100200478800801002084e0_00000000000000000000000000000000_14_14_0_12_0`,
  target 21: plain start3 and the brute force say "can", the faulted builds "cannot"). The
  final snake there needs S plus three other snakes with three pieces, which never arises
  in these trees. So the term k is right by argument and by that one case only.
- **What root labels cannot see:** with the first fault planted, proof 0-23 gave 0 wrong
  labels and 19.8 M nodes instead of 26.1 M. A wrong reach "cannot" looks like a 0.76x
  speed-up at the root. Only the in-tree checks see it.
- **The node increase, explained:** it happens only in `f368e74`'s setting (tested at node
  start at 4 left or fewer): 27,984,567 off, 28,463,674 on, reproduced. 39 queries (all
  with 4 pieces in hand) turn from "ran out of its 2,000 work, said can" into a proved
  "cannot", all correct; nothing changes the other way. Position 23 alone grows by 479,683
  because the extra correct prunes change what feeds E30's cost history (with
  `--no-cost-order` it is 1,396,848 against 1,405,619). With unlimited reach work start3 on
  and off give identical node counts everywhere. At the new defaults start3 lowers the
  nodes (26,238,711 to 26,095,441).
- **Not covered:** proof4 24-47 and `proof4b`; four of the five ply-5 files; the full window
  beyond 3 proof positions; driver jobs other than a and b; hands of 5 to 7 in start3's
  domain (97 queries, all agreed); the `--rt-effort*`, `--rt-margin-*` and `--rt-refuter`
  options were read, not run.
- **Fragile points it names:** the k term needs a unit test on a constructed position; the
  4-slot `top` array relies on snakes + pieces in hand being at most 8; a bench run carries
  state across positions, so node counts compare only between runs over the same range
  (position 13: 1,535,040 when the run starts at 12, 1,563,463 from 0).
- **Its slips:** a mistyped `cd` queued a build in the ordering agent's worktree; it was
  stopped while waiting for the lock, nothing compiled or written. An early smoke run
  outside the lock was refused.

**Decision.** `f24a2d2` goes to its one gate as it is. Two conditions on the freeze, neither
changing the search: a unit test of the k term on the constructed position, and a guard on
the `top` array, go in with the merge. What the verifier did not cover stays listed here.

## 2026-11-16 The top of the proof, second report: the replay is built; "images first" costs more often than it pays

Agent's report (the ordering agent's round six), branch `climb/replay` at `7068a18`
(this branch's source, plus `--emulate` cherry-picked as `9c368c8`, plus the replay
commit). Its numbers. The 25-reply replay was not run.

- **`prove --replay`:** the roots of a file are proved as the replies of one proof. Kept
  from root to root: the table (a new generation only at the first root), the winners that
  feed the image bonus, the killers, the workers' solvers (their history). A root's line
  also ends in the job seconds lost on stopped jobs. **`prove --deadline S`:** no new root
  after S seconds, so a replay can be cut into pieces.
- **Run:** `sh tools/replay.sh EXE FILE DIR` (`--threads 16 --job-threads 1 --depth 2 --tt 27
  --upper-first`; each piece under the lock, no new root after 600 s; resumes from
  `DIR/results.txt`); `tools/replay_sum.py` gives one line a root and the totals and checks
  every root as `tools/certificate.py` does. Files: `tests/replay/tune25.txt` (the 25 tune
  replies in the paused proof's order), `tests/replay/smoke3.txt` (replies 12, 9, 3).
  Nothing touches `work/long/`.
- **What the replay is not:** in the real proof idle workers take other replies' tries;
  here they start more red tries of the same reply, lost when a try wins (1,860-1,950 of the
  smoke test's 7,169-7,502 job-seconds). The 49 gate replies in between are missing (their
  winners, table entries, history). A resumed piece starts without the earlier pieces'
  table and history. The resume path was not exercised.
- **Identity with `--replay` off:** proof 0-23 narrow 56,301,796; ply07 6,404,487; ply06
  5,846,840.
- **Smoke test, base against base, two runs:**

  | Reply | Tries (failed) | Run A: wall s | Run B: wall s | Winning move (both) |
  | --- | --- | --- | --- | --- |
  | 20,30,39,40 | 1 (0) | 78.3 | 76.7 | 4B:17,28,38,49 |
  | 18,28,37,38 | 3 (2) | 360.4 | 339.4 | 4B:70,79,80,89 |
  | 10,20,29,30 | 1 (0) | 30.2 | 32.0 | 4B:70,79,80,89 |
  | Total | 5 (2) | 468.9 | 448.1 | certificate checks ok |

  The runs differ by 4.5% in total, 1-6% a reply; tries and winners identical.
- **The image bonus both ways, inside these three replies:** reply 18,28,37,38 is 3.8x
  slower than proved cold (91 s, 1 try): the bonus put two images of the previous reply's
  winner first and both failed. Reply 10,20,29,30 gained (30 s against 58 s cold): won with
  an image of the previous winner.
- **From the recorded data:** reply 81,91,100,101's winner (an image of five earlier
  winners, the paused run's first try, 94 s) is not among the first 17 tries of today's
  cold order. On the four replies where the cold order wins at try 1, 1, 2 and 4, the
  paused run never tried the cold winner: all 3, 4 and 5 tries, and 13 of 19, were images
  of earlier winners (of up to 18 replies each). None of those four cold winners is an
  image of an earlier winner.
- **The agent's reading:** "images first" puts every image of every earlier winner ahead of
  the model's order, and the set grows with every reply proved. It wins when a reply
  resembles an earlier one and costs whenever the winner is a new idea.

**Decision.** Orders of red's tries that bound the loss both ways are built behind options
now (compiles only): the two orders taken in turn, a cap on the images, images of recent
winners only. No long run before the gate of `f24a2d2` is through. The first long replay
is `tune25.txt` with the base frozen then. It serves twice: as the ruler's baseline, and as
today's estimate of what the 98 replies left of the centre-opening proof would cost.
That estimate goes to the owner; resuming is the owner's call.

## 2026-11-16 E29, fifth report: the test of the k term is written; no cut inside the dear reach test yet

Agent's report (the reach agent's round five). Its numbers. The candidate is still exactly
`f24a2d2`.

- **The test commit, `64d2bf4`, branch `climb/threats-tests`, on top of `f24a2d2`** (the
  verifier's condition for the freeze):
  - `bounds::tests::start3_counts_one_other_snake_per_piece`: the verifier's position, red,
    target 21 must be "can" with start3 off and on; the same position against the brute
    force (one-player optimum 24) for every target from the longest snake + 1 to 26; two
    neighbours of it (a fifth red snake of 5, so S is longer than four snakes that tie; a
    blue tile in the ring's gap), checked the same way. No hand-of-four case: the brute
    force is too slow there.
  - With k changed to k-1 locally the test fails ("start3 true: red can reach 21"); restored,
    it passes.
  - `debug_assert!(ns + pieces <= 8)` before the 4-slot `top` array, with the reason.
  - `cargo test` passes; a plain build gives proof 0-23 narrow 26,095,441 and ply07 0-589
    4,174,807, as `f24a2d2`.
- **Inside the three-piece reach tests** (study build, proof 0-23): the bound of the child
  after the first piece is 14.5 G cycles over 16.0 M calls (906 each), about 39% of the dear
  tests and 10% of the run; the last piece 7.0 G; copies 1.4 G. "Cannot" at 6 left is
  806,506 tests and 28.0 G, spread over many middling proofs (most visit 8 to 255
  positions); "can" answers are cheap (72% visit one position at 5 left).
- **The same question rarely comes back:** an exact repeat of a "cannot" 2,878 times in 4.12 M
  tests; a monotone cache would answer 6,151 (0.15%). Dropped.
- **`--reach-ub-upto` (`df82bf5`, off):** the prune after a piece uses the early-stopping
  flood. Same nodes everywhere (26,095,441; 36,202,062; ply06 3,669,218; ply07 4,174,807;
  ply08 4,223,127). Plain builds, three rounds: 0.96-0.97x on medians, 0.99-1.01x on
  minima, and the arm that ran first was faster in every round. Within the order effect;
  not a candidate.
- **Ruled out with a number or an argument:** one search for both targets (ceiling 4%, the
  second test runs only after a "can"); a "cannot" kept from the parent (the mover differs,
  and our cells change in between); first-piece order (a "cannot" must try every branch).
- **`snek verify GAMES SEED HAND3 THREAT3` (`ba347dc`):** how often three-piece hands are
  checked; the defaults (40, 20) give the old sequence; `verify 100 1 8 8` passes (4,108
  reach checks against 3,271).
- **What is left, its words:** the child's bound computed from the parent's instead of from
  scratch. Mechanical if exact, but needs care.

## 2026-11-16 The top of the proof, third report: three orders of red's tries are built, none run

Agent's report (the ordering agent's round seven), `climb/replay` at `2149110`, all options
off by default. No replay was run (the lock was kept for the reach-threat gate).

- **`--win-turns`:** where the image bonus applies, the model's order (no bonus) and the
  image order take turns, the model's first. **`--win-cap N`:** only the N images that most
  winners map onto keep the bonus. **`--win-recent K`:** only the K most recent winners count.
- **Identity, options off:** proof 0-23 narrow 56,301,796; ply07 0-589 6,404,487. That the
  smoke replay's tries are unchanged is argued (same image set, same list), not run.
- **The replay's tools:** `--deadline` now stops a piece only after one root that needed
  work; the resume path was run on three tiny roots with `--deadline 0` (three pieces, one
  new root each, certificate checks ok); `tools/replay_sum.py --pairs CAND_DIR BASE_DIR`
  writes one pair a root for `tools/hc.py report --ratio`. Smoke run A against B through
  it: -0.8% [-7.2%, +6.1%], sd 0.059 a root.
- **Simulation on the paused run's recorded tries** (25 tune replies, 8,145 recorded seconds;
  a variant is ranked only where its winner was tried, so the cap is flattered):

  | Variant | Replies ranked | Cost against recorded | Unranked replies (recorded s) |
  | --- | --- | --- | --- |
  | Images by count | 25 | 0.954x | 0 |
  | Oracle | 25 | 0.788x | 0 |
  | recent 1 / 2 / 3 / 5 / 10 | 13 / 19 / 20 / 22 / 24 | 0.930 / 0.982 / 0.975 / 0.993 / 1.000x | 12 / 6 / 5 / 3 / 1 |
  | cap 1 / 2 / 4 / 8 | 16 / 20 / 24 / 24 | 0.968 / 0.915 / 0.896 / 0.896x | 9 / 5 / 1 / 1 |
  | turns | cannot be ranked | | the model's own first tries were never recorded |

- **Its choice: `--win-turns` first.** On the five replies where both orders are known the
  cold winners would come at tries 1, 1, 3 and 7 (3, 4, 5 and 19 in the paused run) and the
  image winner at try 2 instead of 1: about 14 tries against 32. Second: `--win-cap 4`.
- **Its doubt:** turns re-scores a node's moves as ranks, so after a failed try the moves
  lifted by the answer rule jump the whole list, where today the bonus band keeps images
  above them. Untested.

**Decision.** Nothing here is gated or run yet. After the reach-threat gate: one replay of
`tune25.txt` with the base of that moment (the baseline and the estimate for the owner),
then `--win-turns` on the same file as a screen. The 49 other replies stay unused for a gate.

## 2026-11-16 E29 round three: my checks, the screen, and the gate's hypothesis and cap

Written before the first gate unit. Candidate: `f24a2d2` (`climb/threats`), built by me:
plain `b630a2fab0bf`, profiled `440dfbb757aa` (`work/e29r3/`, record in
`work/e29r3/checks.txt`). Base: `frozen/climb-base.exe` `03e5963b7ba5`.

- **Identity, the rule off** (`--no-reach-threat`): proof 0-23 narrow 56,301,796; ply07
  0-589 6,404,487; ply06 0-199 5,846,840: the frozen base's counts. **The old setting**
  (`--reach-threat-max 4 --reach-threat-start 4 --no-reach-start3`): 49,334,737 and
  5,255,948: `6057bd7`'s counts. The profiled build: 6,404,487 with the rule off.
- **Labels with the new defaults,** all values right, none unknown, every count equal to
  the agent's: proof 0-23 narrow 26,095,441; 24-47 36,202,062; proof 0-11 full window
  19,741,708; ply07 4,174,807; ply06 3,669,218; ply08 4,223,127; the five ply-5 files
  6,839,041 / 6,914,528 / 8,005,501 / 4,585,644 / 26,853,486 (53,198,200 together; the
  reference solver needs 59,709,026). `verify 100` passes. 16 threads on ply06: 9,624,368
  nodes, all right.
- **Screen `E29R3-S`** (proof4b tune units 50-79, profiled builds, `--window 0,1`): total
  time 117.57 s against 107.55 s, **0.915x**; nodes 0.557x; per unit the log ratio is
  +0.115 +/- 0.099, **sd 0.277 a unit**. 25 units faster (down to 0.57x), 5 slower. Two
  outliers carry the spread: unit 75 at 1.47x with half the nodes (the tests cost more than
  they save there), and unit 55 at 2.68x with 1.39x the NODES (8,993,217 to 12,512,261).
  The extra prunes are exact, so the larger tree there is a worse order: what the verifier
  found at proof position 23, where the prunes change what feeds E30's cost history. No
  run from the analysis board without the lock fell in the screen's window.
- **`hc.py plan`** on the screen: SPRT[0, 0.05] needs about 163 units if the truth is either
  hypothesis and 266 halfway. 325 gate units of `proof4b` are unused (118-442).

**Hypothesis.** Reach threats at every level, tested at node start too, with the
three-piece reach test from the longest snake, cut the time of fresh proof positions (one
thread, profiled builds, `--window 0,1`): at least 5% against no gain. Expected: about
0.89x a unit (the screen), 0.83-0.92x in total (the agent's profiled pairs and the
screen). **Test:** `E29R3`, one gate, SPRT[0, 0.05] on the per-unit log ratio of time,
`proof4b` gate units from 118, **cap 266**. On a pass: the guards (ply 5 scaling on 10 tune
units; driver jobs a-d), then the freeze with the test commit `64d2bf4`. On a fail or at
the cap undecided: no freeze, no second gate; the cost of the reach test (round six) and
the order effect seen at unit 55 are then the things to work on, and what comes back is a
new change on new units.

## 2026-11-16 E29 round three: the gate passes (0.856x); frozen as climb base 4

- **Gate `E29R3`** (`proof4b` gate units 118-147, 60 fresh positions, profiled builds, one
  thread, `--window 0,1`; base `03e5963b7ba5`, candidate `440dfbb757aa`): SPRT[0, 0.05]
  accepted H1 at unit 30 of the cap's 266, LLR +5.91. Per unit the log ratio is +0.140 +/-
  0.061 (sd 0.171 a unit); **total time 128.31 s against 109.83 s, 0.856x; nodes 0.533x.** A
  size read where a sequential test stops leans high: quote 0.86x as the gate's, 0.915x as
  the screen's, and expect between.
- **Guards.** `E29R3-UP` (ply 5, full window, the set's 3 tune units): 0.774x time, nodes
  0.470x. `E29R3-DRV` (whole proof jobs, 16 single-thread workers, depth 1, one run an arm):

  | Job | Base, s | Candidate, s | Ratio |
  | --- | --- | --- | --- |
  | a | 24.9 | 21.2 | 0.849 |
  | b | 42.3 | 36.8 | 0.868 |
  | c | 42.7 | 35.4 | 0.829 |
  | d | 59.1 | 49.4 | 0.836 |
  | Total | 169.1 | 142.8 | **0.844** |

  Log ratio 0.168 +/- 0.020 over the four jobs. The gain is in the prover at the size it has
  on single positions (unlike E31). No run from the analysis board started solvers without
  the lock after 23:58 the evening before (one at 00:05 was cancelled before any started).
- **Freeze.** `climb/threats-tests` (`f24a2d2` + the unit test `64d2bf4`) merged into
  `parallel-v9` as `6d08294`; the merge's source equals the test commit's; a plain build of
  the merge (`ab6e9bb8f5d9`) gives 26,095,441 on proof 0-23 narrow and 4,174,807 on ply07
  0-589; `cargo test` passes (the test of the k term). `frozen/climb-base.exe` is now the
  gated executable `440dfbb757aa` (kept as `frozen/E29R3-440dfbb757aa.exe`);
  `frozen/climb-base-plain.exe` is my plain build `b630a2fab0bf` of `f24a2d2`. Back:
  `cp frozen/E30-03e5963b7ba5.exe frozen/climb-base.exe` and revert the merge `6d08294`.
- **The new base's counts** (one thread): proof 0-23 narrow 26,095,441; 24-47 36,202,062;
  proof 0-11 full window 19,741,708; ply07 0-589 4,174,807; ply06 0-199 3,669,218; ply08
  4,223,127; the five ply-5 files 6,839,041 / 6,914,528 / 8,005,501 / 4,585,644 /
  26,853,486. With `--no-reach-threat` it gives the previous base's counts; with
  `--reach-threat-max 4 --reach-threat-start 4 --no-reach-start3` those of `6057bd7`.
- **Gate units left:** `proof4b` 148-442 (295), `proof4` 164-220 (57).
- **What the gate did not cover** stays as the verifier listed it: hands of 5 to 7 in start3's
  domain, and the k term, which stands on its argument and the unit test.
- **Known cost:** on some positions the extra prunes give a worse order (screen unit 55: 2.68x
  time with 1.39x the nodes; proof position 23 in the verifier's analysis). The mean pays for
  them; they are a pot of their own.

## 2026-11-16 E29, sixth report: the child's bound from the parent's is exact and worth 1-2%; not a candidate

Agent's report (the reach agent's round six), `climb/threats` at `9bf07c4`, off by default.

- **`--reach-ub-inc`:** in the three-piece reach test the parent's clusters are recorded once
  and the prune "is the child's bound below the target" is decided from them, cheapest part
  first, flooding only the placed piece's own cluster. Only the decision is used.
- **In-place check** (`--features ubcheck`, not in the release path): 35.7 M decisions and
  values compared against the full bound (proof 0-23 16,028,321; 24-47 18,999,177; ply07
  711,226; ply08 914,471; `verify 100 1 8 8` 2,105): 0 differences. Node counts identical
  with the option on and off on all five label sets.
- **Time, plain builds, three rounds:** 0.986x and 0.976x on medians, 1.004x and 1.019x on
  minima. Below the 3% bar; no profiled pair. The child bound falls from 13.0 G to 9.2 G
  cycles but recording the parent's clusters costs 2.1 G, and the piece's own cluster is
  usually the big one.
- **The rest of the three-piece test** (proof 0-23, 36.4 G inclusive over 2.95 M calls): child
  bound 13.0 G; the two-piece level 16.5 G (its own setup and loop 10.3 G, the last piece
  6.25 G); first-piece generation and setup 5.6 G; copies 1.24 G. The one cut left is
  mechanical and worth 1-2%.
- **Its hand-over note** (for whoever touches the reach test next): running out of work
  answers "can", so any change in the positions visited can change node counts; labels
  cannot see a wrong "cannot" (judge by the `ubcheck` pattern, `verify` against brute force
  and `cargo test`); `verify 100 1 8 8` for three-piece hands; the global switches
  (`START3`, `CAP3`, `UB_UPTO`, `UB_INC`) are toggled by tests, which must stay
  single-threaded; a bench run carries state across positions.

**Decision.** The mechanical cuts inside the reach test are spent: five options, none above
3%. The reach agent's line of work ends here. The pots that remain are of another kind:
fewer tests asked (a cheaper sufficient condition before the search), the order effect the
prunes cause on some positions, the top of the proof, and what the field study finds.

## 2026-11-16 Climb base 4 on the reference's bench files: 0.842x of base 3, fewer nodes than the reference

The old and the new base, alternating file by file under the run lock, one thread, one run
each (`work/ref/base4_on_ref_files.txt`, 02:30-02:35). The reference was not run again; its
figures are those of 00:10-00:40 (`work/ref/`).

| File | Base 3, s | Base 4, s | Base 4 nodes | Reference nodes | Reference, thread-s (then) |
| --- | --- | --- | --- | --- | --- |
| `ply5_other_openings` | 3.74 | 2.84 | 4,585,644 | 7,452,999 | 12.1, 12.2 |
| `ply5_blue_wins` | 8.74 | 7.48 | 6,839,041 | 4,403,893 | 10.8, 10.7 |
| `ply5_blue_wins_holdout` | 8.56 | 7.15 | 6,914,528 | 4,313,743 | 10.8, 10.9 |
| `ply5_random_units` | 8.03 | 6.73 | 8,005,501 | 6,164,249 | 13.3, 13.6 |
| `ply5_ublock_refutations` | 43.29 | 36.73 | 26,853,486 | 37,374,142 | 93.2, 94.0 |
| Total | 72.35 | **60.94** | **53,198,200** | 59,709,026 | 140.2, 141.4 |

- Base 4 takes **0.842x** of base 3's time on these files with 0.509x the nodes. All 126
  values right.
- Against the reference: base 3 took 0.555x of its time when the two were run side by side;
  times 0.842 that is **about 0.47x, or 2.1x faster**. (Set directly beside its earlier
  figure the new total is 0.43x, but the machine was busier then: base 3 read 77.6 s at
  that hour and 72.4 s now.)
- **Nodes: 53.2 M against its 59.7 M**, below the reference for the first time. It is still
  ahead on three files (1.55x, 1.60x and 1.30x fewer nodes than base 4), which is what the
  field study is sizing.

## 2026-11-16 The field study: the reference's lead is move order near the root; its tight bounds cut nodes and cost time

Agent's report (the field study, `climb/field` at `5e73da7`, worktree `wt/f`, outputs in
`work/field/`). Its numbers, plain builds, most timings one read each under the lock. Study
options, all off by default: `--ref-eval L` (the reference's learned child evaluation orders
the moves where L or more pieces are left), `--tight N` with `--tight-mode` and `--tight-min`
(its bounds at N pieces or fewer), `--root-oracle` (the winning root move first). With the
options off its builds give the base's counts (26,095,441; 4,174,807; 6,839,041).

**Technique table, where one side is stronger.**

| Technique | Theirs | Ours | Stronger |
| --- | --- | --- | --- |
| Order at 7 or more pieces left | learned evaluation of every child (territory, "who gets where first"), after the table move or killer failed | linear model of move features | theirs |
| Tight bounds at 5 or fewer left | the mover must beat the opponent's unblockable next-move length, asked of the one-player optimum | reach tests against the lengths now | theirs in nodes, not in time |
| Rest bounds at 6 left | tight bounds over the remaining moves after the first fails | none | theirs, not measured |
| One-player optimum | every hand, 20,000-node budget, memo per cluster | hand of 2 or fewer, work 200, witnesses, lazy | theirs in scope, ours in cost |
| Threats | killers, one scan; exact only at tight nodes | killers, one scan, reach threats at every level, grow zones | ours |
| Killers | 2, plain replacement | 2 with cost order and history (E30) | ours |
| Answer bonus | none | moves covering the refuter go first | ours (worth about 25% of nodes under their order too) |
| Symmetric hash, ETC, table of last pieces | present (ETC off) | absent | negligible at ply 5 |

**Per position, ours over theirs** (one position a run): blue_wins 1.553 in total (geomean
1.267, ours worse on 17 of 24); holdout 1.603 (1.755, 18 of 24); random_units 1.299 (1.356,
13 of 24); other_openings 0.615 (0.452); ublock 0.718 (0.714). Single ratios run from 0.03 to
22.6. The ten worst all start `1:60 4B:16,27,28,39`, blue to move and winning: six are
root-order misses (the winner first rescues them), four are misses below the root; their
evaluation repairs nine of the ten. Failed first moves at the root cost 2,644,672 of
blue_wins' 6,839,041 nodes and 2,973,146 of other_openings' 4,585,644, so a file's ratio is
mostly the luck of the root order.

**The pots, counted in our solver.**

| File | Base 4 nodes (s) | Root oracle | `--ref-eval 7` nodes (s) |
| --- | --- | --- | --- |
| blue_wins | 6,839,041 (6.86) | 4,720,151 | 3,025,433 (4.61) |
| blue_wins_holdout | 6,914,527 (6.80) | 4,886,090 | 3,553,807 (5.31) |
| random_units | 8,005,501 (6.37) | 3,080,135 | 1,785,431 (2.58) |
| other_openings | 4,585,644 (3.28) | 1,433,493 | 4,987,855 (4.09) |
| ublock_refutations | 26,849,229 (29.75) | | 28,762,201 (35.67) |

All 126: 53.19 M to 43.11 M nodes, time 53.06 s to 52.26 s. Tight bounds on blue_wins
(base 6,839,041 nodes, 7.5 s): their setting 4,028,340 nodes in 10.63 s; the one question
"can the mover still beat the opponent's best next placement that no single move of ours
blocks" alone (mode 1) 4,214,434 in 10.36 s. That question decides 513,751 of 677,852 asked
nodes at 5 left and costs 1.4x a node.

**On what the prover runs** (proof4 0-47, `--window 0,1`; base 62,297,503 nodes, 82.8 s):

| Variant | Nodes 0-23 | Nodes 24-47 | Nodes | Time |
| --- | --- | --- | --- | --- |
| ref-eval 7 | 27,657,133 | 20,852,372 | 0.779x | 0.917x |
| ref-eval 8 | 29,553,730 | 23,804,995 | 0.857x | 0.890x |
| ref-eval 9 | 28,164,972 | 25,070,967 | 0.855x | 0.849x |
| ref-eval 11 | 31,904,889 | 20,401,752 | 0.840x | 0.833x |
| ref-eval 7, no answer bonus | 39,128,470 | 24,638,476 | 1.024x | 1.09x |
| tight 5, mode 1, work 200 | 21,526,173 | 28,598,554 | 0.805x | 1.25x |
| tight 5, their setting | 22,777,707 | 30,872,844 | 0.861x | 1.44x |

Every evaluation variant loses on 0-23 (1.06-1.22x) and wins on 24-47 (0.56-0.69x); single
positions run from 0.08x to 2.37x. Screens, not reads: 48 positions decide nothing at that
spread.

**Its ranking.** 1. The child evaluation as the order at the top levels. 2. The tight
question at 5 or fewer left, if a tight node can be made to cost no more than about 1.2x.
3. Root order alone is covered by 1. Not worth building: ETC, symmetric hashing, dropping
the answer bonus, their threat machinery.

**Its doubts and slips.** The port may differ from theirs where our cluster bound differs.
Whole-file runs carry state between positions. `--tight` is checked by labels only (0
wrong), which my rule says is no check of a pruning rule. A stray `cat`, a `sed` that
touched a line in `last_pieces` (reverted before any measured build), one batch that held
the lock about 100 s.

**Decisions.**

- **The reference's repository carries no licence file.** The study port copies its
  evaluation and its weight table (`eval.hpp`, `eval_weights.hpp`). It stays on
  `climb/field` as a yardstick and is not merged: whether its code may enter our base is the
  owner's call. The candidate is **our own evaluation of the same idea** (who reaches which
  free cells first, turned into an estimate of each side's final longest snake), written
  from scratch and fitted on our own solved positions, measured against the port.
- **Two lines of work, one change each, on separate branches from base 4:**
  `climb/evalorder` (the order) and `climb/tightq` (the one tight question, made cheap). The
  second is a change inside a pruning rule: judged in place by an agent who did not write
  it, never by labels.
- The evaluation is also the first thing to try on **red's tries at the top of the proof**
  (the replay tool), where a failed try costs the most. After the two replays that are
  running.

## 2026-11-16 E32 and E33: hypotheses, written before the agents start

**Where the prover's time goes** (the baseline replay of the 25 tune replies on base 4, 22
replies in, `work/replay/base4/results.txt`): 44,235 jobs where red wins took 90,225
thread-seconds (50.9 G nodes); 97 jobs where red does not win took 7,135 (3.4 G nodes, 35 M
each). So `tests/proof4` and `tests/proof4b` (red wins) are 93% of the job time, and the
other kind is 7% with no test set of its own yet.

**E32, an evaluation of the child position as the move order at the top levels** (branch
`climb/evalorder`).

- *Hypothesis:* ordering the moves by an estimate of both sides' final longest snake (who
  reaches which free cells first) takes cheaper wins and fewer failed first moves than the
  8-feature move model.
- *Expected,* from the port of the reference's evaluation: 0.83-0.92x time on proof positions
  (48 positions, single positions 0.08x to 2.37x: a screen), about half the nodes where blue
  has to find the holding move.
- *Against it:* the ordering study (2026-11-15) found no learned order above 0.97x on held-out
  nodes with our own features, and said the pot is which win is taken.
- *Test:* first the port on the 176 tune positions of `proof4b` (units 0-87). If it does not
  gain there the line stops. Then our own evaluation, written and fitted by us; a screen
  against the base and against the port; the driver read under `--emulate 16`; one gate,
  SPRT[0, 0.05] on unused `proof4b` gate units, cap from `hc.py plan` on the screen.

**E33, the tight question at 5 or fewer pieces left** (branch `climb/tightq`).

- *Hypothesis:* the opponent's best next placement that no single move of the mover blocks
  is a length the opponent is sure of. If the mover alone cannot beat it the mover cannot
  win; if the mover cannot reach it the mover loses. Asked where today's reach test asks
  about the lengths now, it decides nodes the search otherwise expands.
- *Expected,* from the port: 0.81x nodes on proof positions, 0.62x on blue-win positions. In
  time a gain only if an asked node costs no more than about 1.2x (the port: 1.4x, total
  time 1.25x).
- *Test:* a check build that searches every node the rule decides without the rule and
  compares; `verify 100 1 8 8`; an agent who did not write it; a screen on tune units; one
  gate as above. Labels and node counts are no evidence for it.

## 2026-11-16 The top of the proof, fourth report: the 25 tune replies replayed; a reply costs what it did at the pause, and failed tries are why

Agent's report (the ordering agent's round eight), `climb/replay` at `a762d50` (merge
`ac67ec6` of climb base 4, then two fixes of `replay_sum.py`). Profiled build
`c9cf438f95c7`; with every replay option off it gives the base's counts (26,095,441 and
4,174,807) and `cargo test` passes. Both arms: `--threads 16 --job-threads 1 --depth 2 --tt 27
--upper-first --replay` on `tests/replay/tune25.txt`, in pieces of at most 900 s under the
lock, **one run an arm**. Outputs: `work/replay/base4`, `work/replay/turns`.

| | Tries (failed) | Wall s | Thread-s | Lost on stopped jobs, s |
| --- | --- | --- | --- | --- |
| Base 4 as it is (images of earlier winners first) | 170 (145) | 9,316 | 144,240 | 13,906 |
| `--win-turns` (the model's order and the images in turn) | 95 (70) | 7,179 | 110,491 | 9,941 |
| The paused run, same 25 replies (recorded) | 67 (42) | 8,145 | | |

`--win-turns` over the base: 0.771x in total wall-clock; paired by reply **35.3% better,
interval [+0.04%, +82.9%], sd 0.77 a reply** (thread-seconds: 34.5% [+0.2%, +80.6%]). A screen.

Per reply, in the proof's order (tries/failed, wall s; base, then turns):

| # | Reply | Base | Turns | Turns/base |
| --- | --- | --- | --- | --- |
| 0 | 91,93,102,103 | 2/1, 96 | 2/1, 104 | 1.09 |
| 1 | 72,81,82,91 | 4/3, 211 | 2/1, 121 | 0.57 |
| 2 | 81,91,100,101 | 1/0, 67 | 2/1, 166 | 2.49 |
| 3 | 63,72,73,82 | 27/26, 809 | 2/1, 171 | 0.21 |
| 4 | 20,30,39,40 | 4/3, 243 | 3/2, 117 | 0.48 |
| 5 | 38,39,48 | 1/0, 94 | 1/0, 83 | 0.89 |
| 6 | 81,82,83 | 23/22, 1523 | 2/1, 221 | 0.15 |
| 7 | 28,38,47,48 | 4/3, 206 | 3/2, 154 | 0.75 |
| 8 | 18,28,37,38 | 1/0, 214 | 1/0, 83 | 0.39 |
| 9 | 27,28,37 | 2/1, 243 | 1/0, 101 | 0.41 |
| 10 | 51,53,62,63 | 3/2, 386 | 3/2, 788 | 2.04 |
| 11 | 17,19,28,29 | 2/1, 68 | 1/0, 76 | 1.13 |
| 12 | 80,81,90 | 1/0, 119 | 1/0, 228 | 1.91 |
| 13 | 69,80,91,102 | 3/2, 162 | 1/0, 106 | 0.66 |
| 14 | 57,68,79,90 | 13/12, 432 | 3/2, 282 | 0.65 |
| 15 | 36,46,55,56 | 1/0, 51 | 1/0, 70 | 1.36 |
| 16 | 10,20,29,30 | 1/0, 25 | 1/0, 43 | 1.69 |
| 17 | 7,9,18,19 | 1/0, 124 | 1/0, 135 | 1.09 |
| 18 | 41,43,52,53 | 3/2, 315 | 1/0, 140 | 0.45 |
| 19 | 32,42,51,52 | 15/14, 591 | 1/0, 112 | 0.19 |
| 20 | 82,84,93,94 | 4/3, 518 | 10/9, 521 | 1.01 |
| 21 | 83,93,102,103 | 2/1, 287 | 2/1, 157 | 0.55 |
| 22 | 74,83,84,93 | 3/2, 430 | 2/1, 266 | 0.62 |
| 23 | 73,75,84,85 | 17/16, 822 | 33/32, 2151 | 2.61 |
| 24 | 93,95,104,105 | 32/31, 1277 | 15/14, 782 | 0.61 |

- Under turns one move (`4C:102,104,113,114`) won 12 of the 25 replies.
- Where turns lost: a different winner that costs more to prove (replies 2, 11, 12, 15-17);
  reply 10, the same winner at try 3 with 4,874 s on stopped jobs against 1,668; reply 23,
  33 tries, three pieces cut off. The agent's doubt: under turns the answer rule lifts moves
  above the whole list after a failed try.
- Pieces cut at 900 s: 4 of 12 (base), 4 of 9 (turns); a cut loses the jobs under way, the
  table and the workers' history. `--win-cap 4` was not run (stopped for time).
- What the replay leaves out (the agent's list): the 49 other replies in between (more
  images: more failures under today's rule; more table entries: a little cheaper); idle
  workers taking other replies' tries (about 10% of the thread-seconds went to stopped
  jobs here); resumed pieces start cold; the 98 open replies are not a sample like these.

**My checks.** `replay_sum.py` on both directories gives the totals above and "all ok" for
the certificates (every root a red win; the position after its winning move searched to a
loss for blue). `hc.py report --ratio --lower-is-better` on the agent's pair files gives its
intervals. `work/long/` is untouched (74 lose, 0 hold, 98 open).

**What the result files say (mine, from `results.txt` of both arms).**

| | Jobs where red wins | Jobs that refute a try (red does not win) |
| --- | --- | --- |
| Base: count, thread-s | 52,016, 112,585 | 144, 10,765 |
| Base: mean, median, 90%, 99% | 2.16 s, 1.21 s, 4.5 s, 14.8 s | 74.8 s, 55 s, 158 s, max 490 s |
| Base: over 20 s | 279 | 135 of 144 (none under 5 s) |
| Turns: count, thread-s | 38,578, 91,419 | 69, 6,625 |
| Turns: mean, median | 2.37 s, 1.30 s | 96 s, 70 s |

- **A reply costs today what it cost at the pause.** 373 s a reply in the base arm; the
  paused run recorded 326 s a reply on the same 25 (370 s over all 74). Every try is
  cheaper (55 s against 122 s), which is what the gates of the last two days bought, and
  the run makes 2.5 times the tries (170 against 67).
- **Failed tries are most of the wall-clock.** A reply won at the first try costs 25 to 228 s
  (mean 99 s over the base's 7, 107 s over turns' 11). At about 105 s a win the 25 winning
  tries are about 2,600 s: failed tries take about 6,700 of the base's 9,316 s (72%) and
  about 4,500 of turns' 7,179 s (63%), 46 s and 65 s each. Reply 3 says the same by
  difference: 25 more failed tries cost 640 s and 9,350 thread-seconds (26 s and 370
  thread-s each); reply 6: 62 s and 950 thread-s each.
- **Why a failed try is dear.** It ends when its one refuting job ends, and that job is a
  single-thread search of about a minute (35 M nodes), 35 times the average job. Until it is
  back the workers prove red wins under the same try and under the tries beside it. Of a
  failed try's 370-950 thread-seconds the refuting job is 75.
- **The rulers measured the parts.** `tests/proof4`, `proof4b` and the proof jobs a-r are
  red-win jobs and winning tries: what a try costs. They say nothing about how many tries a
  reply takes or what a failed try wastes, and that is where the top of the proof is.

**Decisions.**

- **`--win-turns` is not gated now.** At sd 0.77 a reply `hc.py plan` asks about 1,255 replies
  for SPRT[0, 0.05]; there are 49. A gate bound to end undecided only burns them. It stays
  behind its option on `climb/replay`. *Reopen* after E34: what an order of tries is worth
  depends on what a failed try costs. (Other bounds or a fixed batch are the owner's call.)
- **The estimate for the owner:** 98 open replies at 373 s are about 10 hours of the whole
  machine with base 4 as it is (about 8 with `--win-turns`, on one run). The omissions push
  both ways and the open replies may be harder than these. Resuming is the owner's call.
- **Next at the top: E34, below.** The replay is its ruler.

## 2026-11-16 E34: end a failed try sooner (hypothesis, before the agent starts)

- *Hypothesis:* at the top of the proof a failed red try cannot end before its one refuting
  job does (a single-thread search, median 55 s), and until then the other workers prove red
  wins that the failure throws away. Ending failed tries sooner cuts the 63-72% of the
  wall-clock they take. Ways, to be chosen from a measurement: the likely refuting reply
  first and nearly alone until it is back or has run S seconds; a job that runs long split
  into its moves (if it is a refutation every one of them is needed, so they divide between
  the workers) or helped by other workers on the shared table.
- *Expected:* if a failed try cost a quarter of today's, 0.46x wall-clock on the base replay
  (0.52x under turns). The bound, failed tries for free: 0.28x.
- *Against it:* the sharing rule already starts the first replies of several tries side by
  side; a split throws away the job's own work and makes many small jobs (depth 3 once made
  the coordinator the bottleneck); 279 red-win jobs also run over 20 s and would be split
  too; the refuting reply may not be early in blue's order.
- *Test:* first a job log (start, end, try, place in blue's order) on the first roots of
  `tune25`: where a failed try's seconds go. If that is not what the hypothesis says, the
  line stops there. Then one change behind an option (off: the base's tries and counts);
  certificates on every replay and all values right on the bench files at depths 1-3; a
  screen on roots 0-9 of `tune25` against the recorded arm (3,706 s; runs of one build
  differ by 4.5%, so under 10% is no read). The gate is mine: the 49 other refuted replies
  in the proof's order, pairs by reply, SPRT[0, 0.05], cap from `hc.py plan` on the screen.

## 2026-11-16 Paused for a restart of the PC (owner's request, 10:03)

- *Why:* the owner's screen, mouse and keyboard had taken no input since 01:17 (Windows itself
  was healthy). Owner, from their phone: "The PC is still frozen. Can you gracefully pause your
  work and I'll restart it".
- *What was stopped:* the three Opus agents (E32, E33, E34) stopped their own runs, committed
  their work and wrote a `HANDOVER.md` on their branches; the analysis board's server was
  stopped. At 10:03 no solver, build or script of this project was running. `work/long/` was
  not touched.
- *What a restart can lose:* the worktrees and the scratch executables were in the session's
  temporary folder. Every commit is in this repository. The candidate executables are copied to
  `work/exes/` and the coordinator's patch scripts to `work/scratch-tools/` (both ignored by
  git). If the worktrees are gone: `git worktree prune`, then `git worktree add DIR BRANCH`.
- *To go on:* one fresh Opus agent a line, started from `git show BRANCH:HANDOVER.md`.

All numbers below are interim: one run each, tune data, read from the agents' hand-over notes
and not yet checked by the coordinator.

**E34, end a failed try sooner (`climb/failtry`, `284dd01`).**
- The measurement is done (job log, roots 0-3 of `tune25`, 30 tries, 26 failed, 1,115 s against
  1,233 s recorded, certificates all ok). The 26 failed tries used 8,200 thread-s: the refuting
  jobs themselves 1,214, red-win jobs under the same failed try 6,822 (1,827 before the
  refuting job started, 4,995 while it ran), 83%. The machine was full throughout (15 of 16
  workers busy), so wall-clock follows thread-seconds, not idle time. The dear failures come
  while the root has fewer than two failed tries: the refuting reply is then deep in blue's
  order (places up to 513) and up to 15 workers prove red wins under the try. The hypothesis
  holds; the stop rule did not fire.
- The change: `prove --hold S` (off by default): a coordinator node with a job that has run S
  seconds or more opens no new move while other work can be handed out. Only the order of
  hand-outs changes. Chosen over a split and over helper threads because the waste is work on a
  full machine, which a shorter refuting job does not remove. Its cost: a winning try with a
  long red-win job is held too.
- Checks done: option off 26,095,441 (proof4 0-23) and 4,174,807 (ply07, `--to 590`);
  `cargo test`; with `--hold 0.001` all values right on the five bench files at depths 1-3; a
  cut results file resumed right.
- The screen at S=10 was cut by the pause inside root 6: roots 0-5 took 1,353 s against 1,520 s
  (0.89x, inside the 10% band that is no read); by root 1.02, 0.97, 2.35, 0.78, 0.59, 1.25.
  Root 2 lost because the winner was held behind its own 60 s and 94 s red-win jobs. Of the
  jobs that run past S seconds in the base arm, 11% are refutations at S=10, 33% at 20, 48% at
  30: S=30 is the second setting. `work/failtry/hold10/STOP` must be deleted before that
  replay goes on. Locked time used: about 1 h 40 min of the two hours given.

**E33, the tight question (`climb/tightq`, `895536e`).**
- It pays only when asked at exactly 6 pieces left: `--tight-q 6 --tight-q-min 6
  --tight-q-hand 3` searches 22,770,019 nodes on proof4 0-23 (0.873x of 26,095,441) in about
  0.88-0.90x of the time (single reads; the base drifted between 33 and 37 s). Asked at 5 it is
  neutral, at 3-4 or 7 it loses; at every level up to 5: 0.911x nodes, about 1.02x time.
- An asked node is dear (about 4,100 cycles for the bound, about 20,000 for a reach question
  with a hand of 3); 52% of asked nodes are decided and each saves a subtree.
- Not done: `verify 100 1 8 8`, the check builds, proof4 24-47, proof4b, ply 5, the fail jobs,
  `cargo test` on the current source. Unexplained: with a margin limit of 3 the candidate
  searched 26,443,223 nodes, more than the base. It is a pruning rule: zero wrong labels is no
  evidence, and an agent who did not write it judges it before any gate.

**E32, an evaluation as the move order (`climb/evalorder`, `07c92bd`).**
- No setting of ours is worth a gate yet. The yardstick (the port of the reference's
  evaluation, not ours to ship) on proof4b tune units 0-175, cold: 0.685x time a unit
  [0.586, 0.801] at 9 pieces left and above, 0.683x nodes, no wrong value. On the 20 cheapest
  failing jobs: 0.956x nodes, 1.02x time. Two thirds of its gain is a cheaper win at the root
  of the position, one third fewer failed root moves.
- Our own terms: a coordinate search on 48 roots overfitted (0.77x in sample, 0.935x on 48
  other roots, 1.038x on proof4b). Logistic weights from the level-9 labels read 0.656x on the
  fit roots and are untested out of sample: that run was cut by the pause. No driver read of a
  candidate yet (base only: job a 152,253,809 nodes, job b 326,125,056 under `--emulate 16`).
- The compiler seen outside the lock at 04:46 was `cargo check` (a type check, no link, a few
  seconds), run several times by this agent. Builds went through `tools/build.sh`.

## 2026-11-16 Resumed after the restart (10:25)

- Owner: "pc is back, can you start the gui for me". The analysis board runs again on port
  8770; checked by loading the first position of `tests/set1/ply08.txt` and solving it (blue
  wins, its label).
- The restart lost nothing: the worktrees, the scratch executables and the repository are as
  they were at 10:03; no process of ours was running and the lock was free.
- Three fresh Opus agents go on from the hand-over notes. Their aims:
  - **E34 (`climb/failtry`):** finish the screen of `--hold 10` (roots 6-9 of `tune25`), then
    `--hold 30` on roots 0-9, both against the recorded base arm. If neither reads beyond the
    10% band and the job logs point at one sharper rule, one variant behind its own option,
    with its hypothesis written down first.
  - **E33 (`climb/tightq`):** correctness before speed: identity with the option off on the
    current source, `cargo test`, the check build, `verify 100 1 8 8`, the unexplained node
    count at margin 3, and the argument why the rule is exact written out for a verifier. Then
    the wider screen of the one setting that paid (asked at exactly 6 left).
  - **E32 (`climb/evalorder`):** first a size-the-pot study at the top of the proof, then the
    logistic weights out of sample.
- *Hypothesis for the E32 study (before it runs):* the base replay made 170 red tries for 25
  replies (145 failed) where the paused run made 67, and failed tries are about 70% of the
  wall-clock. An evaluation that ranks red's winning try earlier than the model order does
  would cut the failed tries directly, which no test set of red-win jobs can show. *Test:*
  offline, on the 25 tune replies only: every red try in the recorded arms (`work/replay/base4`,
  `work/replay/turns`, and the paused run's lines for the same replies) is labelled won or
  failed; for each order (the model, the port as a yardstick, our fitted weights) the place of
  the first known winner among red's moves and the number of known failures ranked before it.
  *Against it:* 25 replies are few; untried moves have no label; the 49 other refuted replies
  stay unused for a gate. If no order ranks the winners earlier than today's, the line stops.

## 2026-11-16 Paused again at 10:37 (owner's request: the board has the machine)

- Owner, 10:36: "and lets pause the climbing agents for now im just gonna play with the gui
  for a bit". Before that: "for the limit setting on the gui, is it possible to set it to
  unlimited?" Done in `719ffd7`: a limit of 0 means no limit (the page sends 0 as typed, the
  server accepts it; an empty box is still 30 s). Checked on the restarted server: a ply-8
  position and a `proof4` position solved right with limit 0 (`work/gui/runs.log`, runs 1-2
  of server 14916).
- The board ran 16 solver processes beside the lock from 10:23:55 to 10:32:10 and from
  10:32:41 to 10:35:41 (`lock=none` in `work/gui/runs.log`). Timings in those windows are
  void. Only E34 had any: pieces 3 and 4 of its `--hold 10` replay, now kept apart in
  `work/failtry/hold10_void/`; `hold10/` is back at the end of piece 2 (roots 0-5).
- The three agents stopped, committed and updated their `HANDOVER.md`. What the 15 minutes
  gave (interim, not checked by the coordinator):

**E32 (`climb/evalorder`, `9a382fa`): our own evaluation as the order in the search is stopped.**
- The logistic weights out of sample, 48 validation roots, node counts: lgA 1.050x in total
  (mean log +0.040 +-0.263), lgB 1.093x (+0.071 +-0.285), no wrong value. The 0.656x on the fit
  roots was an overfit, like the tuned weights before. What would reopen it: labels at the root
  level, or many more fit roots than 48.
- The study at the top of the proof (hypothesis in "Resumed after the restart"): the labels
  from the three recorded arms agree with them (145, 70 and 42 failed tries; 59 known winners;
  no contradiction). **The model's own first move is a known winner in 15 of the 25 replies**
  (a known failure in 3, no label in 7), where the base arm won at the first try in 7. For
  the other orders the first move is a known winner in 2 to 5 replies and has no label in 17
  to 21, so the table cannot rank them: every arm that made the labels ran a model-based
  order. The hypothesis as stated (an evaluation ranks the winner earlier) has no support.
- What it found instead: **what pushes red's winner back at a root is the image bonus and the
  killers handed down from earlier roots, not the model.** A one-move-at-a-time simulation of
  the driver's order gives 97 failed tries plus 20 without a label against the recorded 145
  (the driver also opens tries beside the winner once a root has two failures).
- How the driver orders red's tries today (`src/prove.rs`, `new_node`): the static score plus
  the model's, with the hint and the killers first (under `--replay` the killers are the last
  two root winners); images of every earlier root winner get +2^24 (`--win-cap`,
  `--win-recent`, `--win-turns` change that); after a failed try the answer rule adds 1024 to
  moves that cover the refuting reply, more than the model's whole range (22 to 158).
  `--eval-order` does not reach this node.
- Next, when the climb goes on: label the model's 7 unlabelled first moves
  (`work/evalorder/top/unl_model.txt` on the branch; 50-230 s each on 16 threads), then a
  driver option that keeps the model's first move ahead of the images and the handed-down
  killers at a root. It belongs with `--win-turns` (IDEAS 0b), read on the replay.

**E33 (`climb/tightq`, `b1ba8c4`).** `cargo test` passes on the current source (3 tests); the
executables `tq-v3.exe` and `tqchk-v3.exe` are the current source (a rebuild gave the same
hashes); the argument why the rule is exact is written in its `HANDOVER.md`, with the weakest
step named (the generation of our blocking moves inside `sure_next_length`: one missed legal
move would make the bound too high). `verify` on that branch takes only games and seed. Not
done: option-off identity on the current source, the check-build runs, `verify`, the margin-3
node count, the wider screen (job files `e33/jobs6.txt`, `e33/jobs7.txt` are ready). It built
through its own script under `tools/locked.sh` because the check build needs a feature flag.

**E34 (`climb/failtry`, `450dfa5`).** Nothing new that counts: `--hold 10` stands at roots 0-5
(0.89x, no read), `--hold 30` was not started, no variant was built.

- *Leftovers after the stop, for the next time:* stopping an agent's background task did not
  stop the `tools/replay.sh` loop under it: the loop and a lock waiter for the next piece were
  still alive two minutes later, and would have started a 16-thread piece once the lock came
  free. The coordinator stopped them (and an idle wait loop) at 10:40. A killed holder had
  also left the lock directory and a dead ticket behind; with no solver of ours running and the
  owner's board solve waiting behind them, the coordinator removed both at 10:40. After any
  stop: `ps -ef` for replay loops and waiters, then `.git/run.lock` and `.git/run.queue`.
- *State:* no agent is running. The board's server runs on port 8770. The owner started a
  no-limit solve of the position after `1:60` from the board at 10:40 (14 processes under the
  lock: one cold `solve` for each of blue's 1,932 replies).

## 2026-11-16 The board gets the proof driver as a backend (owner's request, about 11:05)

- The owner chose to keep the board's own no-limit solve of the position after `1:60` (one
  cold `solve` for each of blue's 1,932 replies), cancelled it after 707 s with none solved,
  cleared the standing goal of the climb (`/goal clear`), and asked: "can we have the gui use
  the proof driver as the backend? i want to be able to watch the opening get solved". The
  climb stays paused; this is the owner's word to go on with the centre opening, through the
  board.
- *Design:* a Prove mode on the board (branch `gui-prove`, an Opus agent builds it). For a
  position P its roots are P's children, one a symmetry class; the driver proves them one after
  another in the configuration measured on the replay: `prove ROOTS --threads 16
  --job-threads 1 --depth 2 --tt 27 --upper-first --replay --win-turns --out RESULTS`. A proof
  store for each P under `work/gui/proofs/`; for the position after `1:60` it starts from a
  copy of `work/long/ply1_results.txt` (74 of 172 refuted), and `work/long/` itself is never
  written. The page lists the replies with their state, red's answer, tries and seconds, and
  can follow the reply in work on the board. A reply that red does not win stops the run and
  is shown plainly.
- *The driver used:* `frozen/prove-replay.exe`, a copy of `work/replay/replay-c9cf438f95c7.exe`
  (the profiled build of `climb/replay` at `a762d50`: base 4 plus the replay options). Its
  source is not merged into `parallel-v9` yet.
- *Why `--win-turns`:* it is held, not gated (0.771x on one run of 25 replies, a reply +35%
  better with interval [+0.04%, +83%]). It changes only the order of red's tries, never a
  value, and the one read we have favours it; for a run the owner watches that is the better
  bet. The estimate for the 98 open replies: about 8 hours of the whole machine with it, about
  10 without (one replay run each, 25 replies).
- *What this run is not:* a measurement. Its replies include the 49 kept for a top-of-proof
  gate; once the board proves them they are used up as gate data, and a later driver change
  has to be judged on the 25 tune replies and whatever new openings give.

## 2026-11-16 The board's Prove mode is live (11:30)

- Built by an Opus agent on `gui-prove` (`b88d460`), merged into `parallel-v9` (`81e6fe6`), the
  board's server restarted with it. `tools/gui.py` runs `frozen/prove-replay.exe prove roots.txt
  --threads N --job-threads 1 --depth 2 --tt 27 --upper-first --replay --win-turns --out
  results.txt` on the replies of the position on the board, one a symmetry class; the store of
  a position is `work/gui/proofs/<16-character hash>/` (`roots.txt`, `results.txt`,
  `meta.json` with the position text). The position after `1:60` starts from a copy of
  `work/long/ply1_results.txt`: 74 of 172 refuted, shown without a search; the driver decides
  a root known from that file at once (checked by the agent on two proved replies: 0 jobs,
  0.006 s). The 98 open replies are taken in the solver's static order, not the driver's own
  (the driver prints no move list).
- *The agent's checks:* `tools/gui_test.py` with new prove tests (a winning ply-3 try proved
  to 767 of 767 with a cancel and a resume in the middle; a failed try stopped at the reply
  red does not win, confirmed by `solve`; stores readable without a run and after a server
  restart; no driver left after cancel and server exit), the lock yield with a planted waiter,
  and the page in headless Edge. A six-minute run on the opening in its worktree finished no
  try, so a failed try on the opening was not seen live.
- *A fault the coordinator's browser check and the owner both hit:* the store's folder was
  named by the full position text, and under this checkout's long path the files passed
  Windows' 260 characters ("WinError 3" on Prove). The agent's tests ran in a short temporary
  folder and could not see it. Fixed in `0fe09ff` (a 16-character hash as the folder name).
- *The coordinator's checks on the live board (port 8770):* the position after `1:60` shows
  "refuted 74 of 172" with its rows; a click on a refuted row plays the reply and shows red's
  answer; Prove on the winning ply-3 try ran live (90 of 767 after 6 s, the row in work on
  top, the estimate) and Cancel stopped it with no driver process and no lock left.
- *Not done:* `tools/gui_test.py` was not run again after the folder-name fix (it takes the
  lock and 16 processes for some minutes, and the owner was about to start the proof); the
  source of the driver (`climb/replay`) is not merged into `parallel-v9`; job counts of the
  reply in work arrive only once a minute.
- *A lesson for tests of anything that writes files here:* run them once from the main
  checkout, whose path is 189 characters long, not only from a short scratch folder.

## 2026-11-16 The board from the owner's phone; the opening's proof runs without a page (11:48-12:00)

- Owner, away from the PC: "Could you make the gui accessible from my phone so I can watch on
  the bus?" Earlier: a wish to see, in follow mode, red's tries in work and how close the reply
  is to being refuted (an agent is building that on `gui-prove`; hypothesis-free display work).
- *The proof had stopped.* The owner's run of the opening ended 31 s after its start at 11:40:
  a run belongs to the page that started it and is cancelled 4 s after that page's event stream
  goes away. Restarted at 11:48 through the board's API with no owning page (run 6), so it no
  longer depends on any page; it holds the run lock, 16 processes.
- *Access:* `tools/gui_gate.py` on 127.0.0.1:8781 in front of the board: a request passes only
  with the token (`work/gui/gate.token`; first as `?k=`, then as a cookie), and the requests
  that start or stop runs are refused ("watch only"). In front of the gate a Cloudflare quick
  tunnel (`cloudflared tunnel --url http://127.0.0.1:8781`, the program was already installed;
  the owner's own tunnel service was not touched). Checked: 403 without the token and with a
  wrong one, the page and the API with it, `/api/run` and `/api/cancel` refused, both locally
  and through the tunnel.
- *The tunnel holds the event stream back* (a 2 s read through it gave nothing; locally the
  pings arrive). The page now asks `/api/status` every 5 s while the stream is silent. One
  answer is 40 KB with the opening's 172 rows.
- *For a phone:* a page opened while a proof runs starts on the proof's position; under 900 px
  the run's rows and the list sit directly under the board. Checked in the browser pane at
  375 x 812 through the tunnel: the proof shows without typing anything, and follow moves the
  board to the reply in work.
- *Both processes (gate, tunnel) are background shells of the coordinator's session:* they end
  with it, and the tunnel's address changes when it is started again. Commit `4c34a0c`.

## 2026-11-16 The tries of the reply in work, on the board (12:35)

- Owner: "in follow mode, it would be cool to also see what the like follow up moves its
  currently exploring are, or like how close to being refuted the currently followed one is".
  Built by the Opus agent on `gui-prove` (`4b01220`, `25d9339`), merged as `0f1a55d`, live on
  the board since 12:35.
- *What it shows* when the reply in work is on the board (follow, or a click on its row): an
  outline on each of red's tries in work, stronger the more of blue's replies to it are shown
  lost; a list of those tries with a bar and done/total, then the failed tries with the reply
  that refuted each; a bar on the reply's own row for its best try. A click on a try plays it.
- *Where it comes from:* the results file gets a line for every finished job and every
  decided try. The server builds a reply's tries from the file once when the driver takes the
  reply up, then reads only new bytes, at most twice a second. `total` is the number of blue's
  replies at the try, one a symmetry class: an upper bound of the jobs the try needs, so a bar
  can end before it is full. A try can also sit at 1104 of 1105 for minutes on one hard reply.
- *For the phone:* the tries are part of the state `/api/status` returns; the rows of the
  replies list carry a version and are left out of a poll when the page has them: one poll of
  the opening is about 850 bytes instead of 40,000.
- *The agent's checks:* a stand-in driver (`tools/fake_prover.py`) that plays back two recorded
  replies: states, counts and totals of the tries match an independent count; a file read in
  two halves cut inside a line gives the same tries; cancel and Prove again rebuild them;
  state events at least a second apart. The whole of `tools/gui_test.py` from a copy under a
  191-character path: 47 ok, 0 failures. It also found that the folder-name fix of `0fe09ff`
  had missed one lookup (a store other than the opening's could not be read without a run
  after a restart) and fixed it.
- *The coordinator's checks on the live board:* after the restart the proof went on at 77 of
  172; with follow ticked the board showed the reply in work, its leading try outlined (730
  of 1015) and three fainter ones, the list with bars, and the bar on the row; the same at
  375 x 812 through the tunnel, with polls carrying the rows' version.
- *The restart's cost:* done 58 s into a reply. The driver's jobs in flight, its table and its
  history were lost; everything in the results file was kept.
- *A leftover to know:* stopping the board's server while its run holds the lock leaves
  `.git/run.lock` behind (a killed process cannot give it back); the next run waits for it
  until it goes stale after 20 minutes. Removed by hand at 12:34 after checking that no
  driver or solver process was alive.

## 2026-11-16 The owner allows the reference solver (about 13:15)

- Asked whether a finished run would mean the game is solved, the coordinator answered: yes in
  the weak sense, pending a check by a second program, and named the reference solver as one
  way to check the 172 certificate lines. Owner: "You have my ok going forward to run the
  reference solver".
- Read as its local solving modes on any position, no network (`serve`, `worker` stay out), no
  newer download, no code or weights into our source. Written into the standing rules.
- Its source has what a check needs: `snek solvepos SECONDS THREADS MOVE...` solves a position
  given as moves with several threads, and with `SOLVER_LOSS_TEST=1` only proves or refutes
  that the side to move loses, which is exactly one certificate line (`1:60`, blue's reply,
  red's answer; blue to move loses). `benchset FILE THREADS` with `BENCH_WINTEST=1` does the
  same for a file of move lists.
- Not started: the board's proof has the machine (78 of 172 at 13:02). The plan when it ends
  or pauses: time one line first, then all 172 (and say what it costs before running them).

## 2026-11-16 An explainer of the prover (owner's request, written by 13:40); the table's key is 38 bits strong

- Owner: "Can you make an artifact that fully explains how the current prover works? In the
  style of like in depth documentatioj and exposition similar to the page from the nelhage
  uttt solver". Written from the source (`climb/replay` at `a762d50` for the driver) and this
  log; published as a private artifact, source kept as `docs/prover-explained.html` (page
  content only; the artifact tool adds the document skeleton). Figures are drawn by the
  page's script from the rules (board, images of a reply, dead cells), so they cannot drift
  from the text. State at the time: 79 of 172 refuted, 0 held, 93 open.
- The reply `4I:15,25,35,45` fell to `3I:57,67,77` after 2,707 s and 38 failed tries: the
  only winner so far that is not a piece of four (of 81 winners at 13:40: 4B 60, 4C 19,
  4I 1, 3I 1).
- *Found while writing, not measured:* a table hit compares the full 64-bit key, and the
  bucket index already fixes its low 26 bits (`--tt 27`: 2^26 buckets of two entries). So a
  probe is told apart from another position in its bucket by 38 bits: about 2 / 2^38 =
  7e-12 a probe for a false match. The whole opening proof is of the order of 6e11 to 8e11
  nodes (3.4e9 to 4.6e9 a reply by the paused run and the replay), so the expected number of
  false matches is about one to seven. Most would change nothing (the wrong entry has to
  decide the window, and the error has to reach a job's answer), but the claim "every value
  in the table is a proof" stands on the key. The driver's own map of results (a few hundred
  thousand 64-bit keys) is not at risk.
- *What would cover it:* the independent check of the certificate (the reference solver on
  each of the 172 lines), or a second run with other Zobrist numbers, or a wider check in
  the entry (a second hash word; costs table space). Not acted on: the climb is paused and
  this is the owner's call. Written into `docs/LESSONS.md`, "Open questions for the owner".

## 2026-11-16 A page to play against the proof: the plan, and the solver in a browser (about 15:00-15:40)

- Owner: "Would it be realistic to make like a blog post where embedded in the blog post someone
  could "play against" the proof?" and, after the answer, "Yeah makes sense. The reference solver
  is created by the games publisher. I would probably reach out to him and maybe make a PR to his
  repo once we have a proof for the game". I read the first words as a yes to the timing step I
  had offered (my reading). The outreach and a public page are the owner's, after the proof.
- *What the proof's file can play:* red's second move against every reply (the certificate), and
  red's third move from the job lines four moves in (49,936 such lines at 15:10, none without a
  move). The 74 replies of the first run have no such lines. Deeper, the trees are not kept: the
  page needs the solver.
- *Built:* branch `wasm` (off `parallel-v9`): a crate `wasm/` of its own (a `cdylib` that takes
  `../src` by path, no dependencies, the main build untouched), and in `src/solver.rs` the cycle
  counter of the study counters behind a `cfg` (it was x86 only). 215,130 bytes. New on this PC:
  `rustup target add wasm32-unknown-unknown` (`rustup target remove wasm32-unknown-unknown`
  undoes it). Not merged; before a merge, show the native node counts unchanged.
- *Measured* (the app's browser pane on this PC, one worker thread, an empty table for each
  position, 2^22 entries, the question "does red win", while the proof used all 16 threads; one
  run; `wasm/test/`):
  - 11 red-win jobs four moves in, at the 10% to 99% points by nodes: all value 1; 0.9 to
    16.7 s; 2.26 to 3.30 microseconds a node, 2.80 over all, against 1.61 recorded by the driver
    over all its jobs: about 1.7x.
  - six moves in (after the recorded red move, every eighth blue reply of three of those jobs,
    132 positions): all value 1; medians 0.4, 5.1 and 25.9 ms; the slowest 104 ms.
- *Reading:* behind a book for red's second and third moves the solver answers in well under a
  tenth of a second on a desktop. Without the book red's third move takes 1 to 17 s for 99% of
  the positions and minutes for the rest (the dearest job in the file has 163 M nodes).
- *Not known:* a phone; whether a Claude artifact lets WebAssembly run (a private timing page
  with no game on it was published for the owner to open; the browser pane is not signed in, so
  I could not look). The dearest 1% of the jobs were not timed.
- *Both answered by the owner's screenshot of that page on a phone (about 16:00):* the artifact
  runs the solver (WebAssembly in a worker, its files fetched beside the page), and the phone
  is faster than the PC's browser was under the proof's load. The first five jobs (the 10% to
  60% points): 0.53, 0.67, 0.62, 1.69, 3.83 s, 1.33 to 1.92 microseconds a node (the PC's
  browser: 0.9, 1.0, 0.9, 2.8, 7.0 s). Six moves in, 43 positions: median 3 ms, slowest 29 ms.
  The node counts differ from the PC's run by under 1% because that page uses a table of 2^21
  entries, not 2^22. One run, one phone, model not noted.

## 2026-11-16 A private page that plays red against the proof so far (15:45-16:15)

- Owner, asked "Should I build a private prototype now, on the replies already proved, or wait
  until the proof is done?": "Sure go ahead".
- *Built* (branch `wasm`, commits `1e2261f` and `06fd595`; not merged, nothing pushed):
  - `tools/mkbook.py STORE OUT.json`, the book. For every reply that the results file proves
    lost (the certificate's rule: the position after red's answer must be on file as a loss for
    blue): red's answer; and for every red win four moves in under that answer: blue's second
    move and red's third. A move is its text, and the page looks a position up under the twelve
    symmetries, so the book does not depend on the solver's placement numbers. At 15:53: 107
    replies proved, 29,939 third moves, 160 KB. Of the 54,232 red wins four moves in on file,
    24,298 are not under the winning answer of a proved reply (failed tries, replies still
    in work). 4 pairings were dropped: blue's second piece would start a new snake next to a
    red tile (the same cells are legal with blue's two pieces in the other order).
  - `wasm/src/lib.rs`: `snek_best` (the value by two window searches, then a move that reaches
    it, confirmed by a search of the position after it, because a node decided by a bound has
    no move in the table) and `snek_info` (longest snakes, side to move, game over).
  - `wasm/play/`: `play.html` (the reader is blue and taps cells to shape a piece; Undo and
    New game stop a running solve by starting the worker again), `worker.js` (solver, book,
    red's move), `selftest.html`. The built `snek.wasm` and `book.json` are not in git.
  - Published as a private artifact with the book of 15:53:
    https://claude.ai/artifact/ENmNkdWWKGVoFCUqXeUFJH . The publisher has not been contacted
    and a public page waits for his OK (the owner's call).
- *Checked* (the app's browser pane on this PC, beside the proof; the pane was hidden and its
  two test workers together got about one core, so these times are upper bounds and do not
  compare with the timing at 15:30):
  - every move of the book is legal where the book plays it, by the solver's own move
    generator: 107 answers, 29,939 third moves, no fault (the first export had the 4 pairings
    above; this check found them);
  - 2,000 random games through the book (37 replies with third moves, each turned by a random
    symmetry; blue's second move drawn from those the book answers, then random legal moves):
    red won all 2,000, by 1 to 21; 10,488 moves from the solver, each a proven win; six moves
    in a move took 16 ms on average and 375 ms at most;
  - 138 random games from all 107 replies with any legal second move: red won all 138; in 91
    of them red's third move came from the solver: about 2.0 M nodes and 9.3 s on average,
    88 s at most here. 40 more such games from the replies with third moves: red won all 40;
  - 12 third moves drawn from the book, each followed by a search of blue's position: lost
    for blue all 12 times (16.5 M nodes);
  - in the page at phone width, by clicks and by a script that clicks: a reply and red's answer from the book, a third
    move solved live (7.4 s), a whole game (red 21, blue 11), Undo from the end, New game,
    a reply that is not proved yet (refused, with one line of text), Undo while red thinks.
- *What the book covers:* for the 33 replies proved by the board's run (74 to 106 in the
  proof's order) it answers all of blue's second moves but one or two in a thousand (1,087 of
  1,087; 1,062 of 1,063; 1,007 of 1,009). The 74 replies of the first run have no jobs four
  moves in on file (4 of them have one or two through another order of moves), so there red's
  third move is solved in the page: seconds, sometimes a minute or more.
- *Not checked:* the published page itself (the browser pane is not signed in, so only the
  owner can open it); a blue that plays well. Random blue loses by a wide margin, so these
  games test the plumbing (symmetries, book, legality, the browser build's values), and the
  claim that red always wins rests on the solver's values, as the proof does. The 65 open
  replies cannot be played yet.
- *To publish again* (in a checkout of `wasm`): `cd wasm && cargo build --release --target
  wasm32-unknown-unknown --target-dir SHORTDIR`, copy `snekwasm.wasm` from its
  `wasm32-unknown-unknown/release/` to `wasm/play/snek.wasm`; `python tools/mkbook.py
  STORE wasm/play/book.json` with STORE the proof's folder in the main checkout's
  `work/gui/proofs/`; publish `wasm/play/play.html` with `worker.js`, `book.json` and
  `snek.wasm` (type `application/wasm`) as its files. To test first: serve `wasm/play/` on
  localhost and open `selftest.html?games=2000&pairs=with&second=book` (the page itself needs
  a wrapper with a doctype around `play.html`; `local.html` there is such a copy, not in git).
- *When the proof ends:* export the book again and publish it to the same link. Solving the
  third moves of the first 74 replies would take about 67,000 jobs (74 replies at the 907 a
  reply of the other 33), a few hours of the PC: an estimate, and the owner's call.

## 2026-11-16 The centre opening is proved: all 172 replies refuted (23:09)

- The board's Prove run on the position after `1:60` ended at 23:09:23 (`work/gui/runs.log`:
  `status=done refuted=172 held=0 open=0 searched=95 nodes=363238270863 seconds=38140.7`,
  begun 12:33:42 with 77 refuted). Owner at 23:10: "Did we just do it? Did we solve the game?"
- `python tools/certificate.py` on the results file: 172 replies known to lose, none held;
  1,932 replies in 172 classes by its own count from the piece shapes, 172 lose, 0 open; no
  reply without its record. "by this file, 1:60 is a win for red".
- *What it says:* the first move of a game must be the single hex, red may put it in the
  centre, and after that red wins against every reply. So the start position is a win for the
  first player. The file holds red's second move against every reply; deeper the strategy is
  what the solver computes move by move (the play page does that), the trees are not kept.
  Red's other first moves were not looked at.
- *What it rests on:* one program in two builds (replies 1-74 by `frozen/prove.exe` on
  2026-11-15, the rest by `frozen/prove-replay.exe`). Not checked by a second program yet.
  The certificate script checks the top of the file only: for a reply of the board's run the
  loss three moves in is the coordinator's line (it stands for the red-win jobs under it,
  and jobs under 2,000 nodes are not written), so the file alone cannot be re-added by
  hand. Known soft spot: the table's key check is 38 bits strong at `--tt 27` (the entry
  above, "the table's key is 38 bits strong").
- *What it cost:* about 19 hours of this PC on 16 threads over two days (7.6 h for the first
  74 replies, 45 minutes and then 10.6 h on the board for the other 98). The winning lines
  alone: 245 G nodes and 5.3 h of wall-clock for the first 74 (16 threads on one position
  each), 192 G nodes and 88 thread-hours of red-win jobs for the other 98.
- *Saved:* `proofs/centre-opening/` (`results.txt`, 17 MB, sha256 `cc29e5142c1fb2c0...`;
  `meta.json` with the path of the seed made relative; `certificate.txt`), commit `dd4c361`.
- *The play page:* book exported again from the finished file (172 replies, 94,016 third
  moves, 418 KB; 67 of the first run's 74 replies still have no third move on file) and
  published to the same link as version 3, after the self-test on a quiet machine: every
  book move legal (172 answers, 94,016 third moves, no fault); 2,000 random games from the
  105 replies with third moves, red won all, 10,521 solver moves, six moves in 7 ms on
  average and 143 ms at most. (The engine loads in 75 ms on a quiet machine: the 12 s seen
  in the afternoon were the proof's load.)
- *The check by the reference solver, begun 23:14* (the plan of 13:15: one line first, then
  say what all 172 would cost): `SOLVER_LOSS_TEST=1 ./build/snek solvepos 840 16 1:60
  4A:82,83,94,105 4C:67,77,88,89` under the lock, no network, as `nobody`. The line is our
  cheapest by the nodes of its jobs (403 M nodes, 702 thread-seconds). Output:
  `work/refcheck/line98.txt`. Noted while reading its source: it has `refute` and
  `proveline` modes of its own for proving a line reply by reply.
- *Its answer (23:18): the reference solver agrees on that line.* `ply 3 stm 1: loses
  (777/777 root moves solved, 382256217 nodes, 211.4s)`: every one of blue's moves loses.
  Its 777 root moves are as many as the jobs our file holds under that answer. Kept as
  `proofs/centre-opening/refcheck/line98.txt`. It took 211 s on 16 threads: 0.95x our nodes
  for the line and 4.8x our thread-seconds (3,382 against 702; 113,000 nodes a
  thread-second in WSL against our 574,000).
- *All 172 lines, estimated from this one:* by nodes (ours 437 G, times 0.95, at 113,000 a
  thread-second) about 64 hours on 16 threads; by thread-seconds (ours 621,000, times 4.8)
  about 52 hours. One line, and the cheapest one: it could be off by a factor of two either
  way. Not started: the owner's call. It can run in pieces, because `PROGRESS=<file>` keeps
  every solved root move and a rerun goes on from there.

## 2026-11-21 A blog post on the result, first draft (owner's request, 16:40-17:25)

- Owner: "Now that we solved the game, I want to write a nice blog post to share the result
  with the internet. I'm thinking it could start with an intro to the game, some exposition
  on the methods we used at a high level, some exposition on the previous work done by the
  games creator and how far they got, and what low level improvements we were making to make
  the solving feasible in a reasonable amount of time".
- *Written:* `docs/blog/solving-snekagon.html`, page content for an artifact (no byline, no
  name of the owner). Sections: the result and what has checked it; the game; what was known
  (the designer's video and repository); what a proof looks like; making it fast enough; the
  run; what to trust; play against it; what is still open; how this was made.
  `python tools/mkblog.py PLAY_DIR [--preview OUT_DIR]` fills two generated parts in place:
  the 172 lines of the certificate with their seconds (the bar chart and the wall of 172
  small boards are drawn from them), and the play page's script from `wasm/play/play.html`
  on branch `wasm`, started when the board comes near the screen. The worker, the solver and
  the book are published beside the page; they are not in this branch.
- *Published privately:* https://claude.ai/artifact/8t491NKw9H9EAkqStawz4v (version 1). The
  publisher has not been contacted and nothing is public. The play section in particular
  waits for his OK, like the play page.
- *Numbers:* every figure in the text was read against this log, SHIPLOG, LESSONS and the
  proof's files; `mkblog.py` prints the ones that come from the certificate (172 replies;
  median 237 s; slowest `3V:8,9,20`, 4,807 s; one try for 100 replies, 84 at most; 46
  distinct answers, 4C 92 and 4B 71; `4C:101,103,112,113` 31 times; 84 answers touch the
  centre). Corrected on that pass: the total is 19.5 hours when the 33 minutes of the solve
  that seeded the first reply are counted (7.6 h + 0.56 h + 0.75 h + 10.6 h; the entry of
  11-16 says "about 19"); 0.555x of the reference's time was climb base 3, and 0.47x is that
  times 0.842; the jobs on file (153,736) are the second run's; "every pruning rule was
  verified" became "the last pruning rule".
- *Checked* (the app's browser pane, a local copy with the same three files): no sideways
  scroll at 375 px and at 1,100 px, light and dark; 172 bars, 172 boards, 8 pieces; no
  console error. On the board inside the post: `4B:16,27,28,39`, red answered
  `4B:70,81,82,93` from the proof; then `2:105,115`, red solved `4C:74,85,94,95` in 2.1 s.
- *Not checked:* the published page itself (the pane is not signed in). The video: only its
  description and the repository's README were read, so the section on the designer's work
  and the words about "a saving move the engine ranked only 76th" need the owner's eye. That
  the 76th move is `4C:40,41,50,52` is an inference and the post does not say it.
- *For the owner:* the voice ("we", and "I" in the last section), the paragraph on how it was
  made, the placeholder for the repository's link, and the paragraph on what has checked the
  proof, which says one line of 172 and changes if the reference runs all of them.

## 2026-11-21 The designer's attempt in the blog post; an earlier proof found (owner's request, 18:20-19:30)

- Owner: "Can we add some more detail on what Španěl's attempt at solving the game accomplished?
  Like how far he got, and what his roadblocks were?" (Before that, the same evening: the bullet
  on the closed form reworded to the owner's wording, commit `c50fe9a`.)
- **Found on the way: the game had already been proved a first-player win, a month before us.**
  Issue #3 on `BoltKey/snekagon-lab`, "Snekagon is solved", opened 2026-10-19 21:03 UTC by the
  GitHub user K0tovsk1y; open, no comments on 11-21. It says its author watched the video, let
  Claude work on the solver's code for a day, spread the computation over a few machines and
  "first proved that Blue cannot win and then that Red wins by opening in the center". Its
  repository `K0tovsk1y/snekagon-solution` (one commit, 2026-10-19, C++, built on the
  designer's engine, no licence) holds `proof/index.txt` with one red answer for each of the
  172 classes of blue's reply to `1:60`, 172 `.lvb` files with red's fifth move for 166,912
  classes of blue's fourth move (the solver proves the rest), a checker (`snek proofcheck proof
  16`, about 26 laptop-hours) and a page to play against it. Its notes: search 16 to 19 October
  2026, about 1.2e13 nodes, about 160 laptop-hours (an hour of an 8-core laptop), the 172
  winning solves alone about 55; 123 replies fell to the first answer tried, the hardest took
  about 200 tries; no second program checked it. Against `4B:16,27,28,39` it plays
  `4I:71,82,93,104` (ours: `4B:70,81,82,93`).
  This project never read it: the research of 11-14 read the README at `61eb3a4` and not the
  issues, and no doc here mentions it before today. It was read today through web pages only;
  nothing of it was downloaded or run.
- *Sources for the designer's attempt:* the checkout at `61eb3a4` (README lines 311-472;
  `video/script_snapshot.md`, the narration with a fact sheet "as of 8 October 2026";
  `video/snekagon_en.vtt`, the final subtitles; the git log and two commit messages).
- *What they say.* Adam Španěl (git "Adam Spanel", commits 09-30 to 10-12, with Claude Opus
  5.5) had tried minimax and asked language models before; in 2026 he had a model build the
  tools. First test: one position after four moves, all 956 red moves lose, 11 billion
  positions, about 10 thread-hours. He then tried to prove that blue wins after `1:60
  4B:16,27,28,39` (the engine's favourite reply, about 65 % for blue): 1,589 red answers, each
  needing one blue move with 600 to 1,100 red follow-ups, positions after five moves. After a
  week on his laptop: 16 answers proved (15 by the engine's first blue move), 13 to 241
  thread-hours each, about 60 on average, about 1,300 thread-hours and 18,463 jobs in all (7
  October): one percent, "a year or two" for the line, "a decade or more" for the game. Red's
  `3V:40,41,50` resisted: 40 blue moves refuted (31 losses, 9 draws), 282 thread-hours, 992 of
  1,032 untried; a sample of 24: 14 refuted, 10 unresolved. "Move 76" is blue's
  `4I:46,57,68,79` after `1:60 4B:16,27,28,39 3V:40,41,50 4C:49,59,70,71 4I:52,62,72,82`: it
  holds an exact draw (17:17) and the engine ranked it 76th, the 75 before it lose. It is not
  red's U `4C:40,41,50,52`; the first draft was right not to equate them.
- *After the video.* Solver v4 (threat zones, the last pieces in closed form, tight bounds,
  win tests, `refute`, `proveline`) is commit `184001d` by **Darragh Griffin**, 2026-10-16,
  pull request #1; the network engine (`351bbe2`, pull request #2) is his too. So the
  "reference solver" we measured against and ran is his version, in the designer's
  repository. His notes: blue's only win against the stubborn answer is `3V:9,20,21` (ranked
  932nd of 1,032 by the policy, 964th by the 1-ply evaluation; `351bbe2` message), and red's
  U `4C:40,41,50,52` leaves blue no win in 938 moves (382 s on 5 threads; the commit calls
  this "the known verdict"), so the zig-zag does not beat the centre. The repository's last
  commit is 2026-10-17 and it names no winner.
- *Written* (`docs/blog/solving-snekagon.html`): "What was known" rewritten with these parts:
  the first solver; one line, from blue's side; what stood in the way (the price, the answer
  that would not fall, move 76); after the video; a proof, a month before ours. A new figure:
  1,589 squares, 16 blue and one red. **The post no longer says it is the first proof:** the
  lede, the second paragraph, "What to trust" (a bullet on the second proof, and the closing
  claim is now "two proofs by two programs agree"), "What is still open" (a cross-check), the
  thanks (Griffin) and the footer (links to the first proof). One quotation from the video is
  left, its closing words; the two from the video's description are gone.
- *Checked:* two workflows. Five readers (history, the distributed solve and its records, the
  video's material, the rest of the README, the live pages) gave 268 quoted facts; then five
  checkers tried to refute 42 claims of the new text against the checkout and the live pages,
  and one reader looked for unfairness and omissions. 33 claims were supported as written
  and 9 in part, none refuted; corrected after them: the next reply's rating (the narration says "around 50", the screen 54: now
  "well ahead of every other reply"); "he set out to prove the engine right" (the video: blue
  "had the better chances, so we tried"); the decade is "if blue wins", and a draw is worse;
  the sample of 24 left 10 unresolved; the estimate of 10,000 to 30,000 is "earlier", from the
  first test's cost for one answer; "blue's only win" is Griffin's phrase and is given as
  his; both loose ends are credited to his notes; the three ideas our solver shares with v4
  are credited to him; the figure's caption no longer reads as if we were first to win
  against the zig-zag; "the sixteen proofs still stand"; the heading "What stopped it" became
  "What stood in the way" (no source says he stopped). Page viewed at 1,024 px and 375 px: no
  sideways scroll, 1,589 squares, 172 bars, 172 boards, no console error. Published as
  version 3 of https://claude.ai/artifact/8t491NKw9H9EAkqStawz4v .
- *Not checked:* that the other proof is correct (its files were not read beyond the index);
  its play page; the video itself (its subtitles and script are in the repository).
- *For the owner:* (1) how to frame the post now that the result is not new; the sentence
  "Ours was made without it" is what this project's record shows, and is theirs to change;
  (2) a cross-check is possible both ways: our solver on their 172 answers needs only their
  `index.txt` as data (a run of some hours: the owner's call), their checker on our answers
  means running their code (the owner's word, as for any downloaded code); (3) the outreach:
  the designer has had that issue on his repository since 19 October; (4) the post names two
  more people, Darragh Griffin and K0tovsk1y.

## 2026-11-21 The first proof's answers, checked with our solver (owner: "Yes, check them")

- Asked after the entry above: "Should our solver check the earlier proof's 172 answers, a run
  of some hours?" Owner: "Yes, check them".
- **Hypothesis** (written before the run): every line of `K0tovsk1y/snekagon-solution`'s
  `proof/index.txt` is true by our solver: after `1:60`, their blue reply Y and their red
  answer Z, blue to move loses. Expected: 172 of 172, in 4 to 8 hours on 16 threads (a line
  is one winning try of our own proof, 46 s for a cheap one).
- *The list.* Read from the public file by three independent readers (a workflow of three
  agents over the same page); the three readings are equal, 172 lines. Kept as read in
  `work/k0check/index.txt`. Nothing else of that repository was fetched and none of its code
  was run. `python tools/crosscheck.py prepare work/k0check/index.txt work/k0check` checks
  it against our own count of the replies (`certificate.py`): every Y and Z is a placement of
  the piece it names, every Y is a legal reply, and the 172 lines cover our 172 classes
  exactly once. **14 answers are the same move as in our proof** (after the symmetry that
  maps the two replies onto each other), **158 are different.** `work/k0check/map.tsv`.
- *The question.* One root per line, `-1 1:60 Y Z` (expected value -1: blue, to move, loses),
  for the proof driver of the final proof's build:
  `sh tools/locked.sh frozen/prove-replay.exe prove work/k0check/lines.txt --threads 16
  --job-threads 1 --depth 1 --tt 27 --out work/k0check/results.txt`. Depth 1 from a root
  three moves in gives the same jobs as the proof itself: positions four moves in, red to
  move, "does red win". The driver finds the exact value and exits 1 on any value that is
  not the expected one. The run is restartable from its results file.
- **Controls first** (`work/k0check/controls.txt`, 19:29-19:32, 171 s): a line of our own
  proof (`4A:17,18,29,40` answered by `4C:101,103,112,113`) came out -1, ok, in 45.9 s with
  729 jobs (the proof's own record: 46 s). Red's U against the zig-zag
  (`1:60 4B:16,27,28,39 4C:40,41,50,52`) came out 0, WRONG, with blue's `4A:49,59,70,81` as
  the move that holds, which is what the designer's repository says. A failed red try from
  our results file came out +1 for blue, WRONG. Exit code 1. So the check can fail, and it
  tells a draw from a loss.
- **Started 19:33:02**, all 172 lines (the 14 shared ones too, so that one run covers the
  whole list). Progress: one line per root in `work/k0check/run.log`; times in
  `work/k0check/run.times`; `python tools/crosscheck.py report work/k0check` sums it up.
- **Result: 172 of 172 confirmed, none wrong, none unknown** (ended 2026-11-22 01:26:04,
  exit 0). Every line has the value -1 for blue: after `1:60`, their reply and their answer
  to it, blue to move loses. 5.88 hours on 16 threads (21,181 s; 86.4 thread-hours; 169.1 G
  nodes; 166,854 jobs). A line took 28.6 to 440.1 s, median 109.5 s, and ten took more than
  300 s. The slowest: `3V:28,29,40` answered by `4A:7,8,9,20` (440 s), `4I:15,25,35,45` by
  `4A:57,67,77,88` (381 s), `2:16,17` by `4I:53,63,73,83` (360 s). The 14 lines whose answer
  is also ours took 1,712 s of it, the 158 others 19,469 s. `run.err` has no error line. The
  hypothesis held, and inside its 4 to 8 hours. One row per line:
  `work/k0check/summary.tsv`, written by `python tools/crosscheck.py report work/k0check`.
- *What it shows.* By our solver, every answer at the top of the earlier proof wins: its 172
  lines cover the 172 classes of replies once each (`prepare`, by our own count of the
  replies), and each answer has the value -1 for blue. Our solver shares no code with the
  program that made that proof. It does share the rules as the designer's repository
  implements them, and it has its own soft spot, the 38-bit key check. On 158 of the 172
  replies their answer is a different move from ours under every symmetry that maps the two
  replies onto each other. By our solver both win, so by our solver red has at least two
  winning answers to each of those replies. On the other 14 replies the two proofs give the
  same answer: those 14 lines of our proof are claimed by their program too, as published
  (it was not run here). The line the reference solver checked is not one of the 14.
- *What it does not show.* It is no check of our own proof: it is the same program, the same
  table (`--tt 27`, so the same 38-bit key check) and the same reading of the rules, and 158
  of its lines are not in our proof at all. Agreement that 172 more positions are wins could
  not reveal a win reported in error, which is the error that would break our proof. It did
  not test the earlier proof below its answers (the positions it stores under them), its
  checker or its play page. None of that repository's code has been downloaded or run; only
  `proof/index.txt` was read. Their checker on our 172 answers would be the other half. It
  is downloaded code, so it waits for the owner's word.
- *After the run.* The README's lines on what has been checked and the notes in CLAUDE.md
  state the result. The run's files stay in `work/k0check/`, which is not in git: they hold
  the earlier proof's list of answers, and whether a copy of that list goes into this
  repository is the owner's to decide. The post's draft is not in the public copy; it is
  updated outside this push. The page to watch the run is down (the next entry).
- *The push.* This entry goes to the public copy with the one push the owner allowed: asked
  on 2026-11-21 at 21:33 "May I push that result to the public repository when the run
  ends?", the owner said "Yes, push it". It carries the result and the notes on it, nothing
  else. Any later push needs the owner's word again.

## 2026-11-21 A page to watch the cross-check (owner: "can you put up the browser progress thing for this run")

- `python tools/crosscheck_watch.py [--port 8772] [DIR]`: a server on 127.0.0.1 with one page
  (`/`) and its data (`/api/status`), read from the run's folder (`lines.txt`, `map.tsv`,
  `run.log`, the tail of `run.err`, `run.times`). It starts, stops and writes nothing. The
  page asks every 4 s (no event stream: the tunnel holds those back) and shows: lines done
  of 172, the time spent and an estimate of what is left (the mean of the finished lines),
  the line in work on a board with the count of blue's moves refuted so far, and the list of
  lines with verdict and seconds. A tap on a line shows that line.
- Put up as the board was on 2026-11-16: the page's server (launch entry `crosscheck-watch`),
  in front of it `python tools/gui_gate.py --port 8781 --board 127.0.0.1:8772` (watch only,
  the token in `work/gui/gate.token`), and a Cloudflare quick tunnel to the gate only (its
  address is in `work/k0check/tunnel.log` and changes with every start). The board itself
  (8770) is not running; nothing but the gate is on the tunnel.
- Checked: at the pane's width (871 px) and at 375 px there is no sideways scroll, no row
  wider than the list and no console error. Locally and through the tunnel alike: 403
  without a token, with a wrong one, and for `/api/status` without the cookie; 200 with the
  token; `POST /api/run` refused.
- The run outlived its tenth minute (the shell that started it had a time limit set): at
  19:46, 4 of 172 lines were done, all four confirmed, about 158 s a line, so about 7.5
  hours in all by that mean.
- To stop when the owner no longer needs them: the tunnel, the gate and the page's server
  (two background shells and a preview server of the session), and the launch entry.
- **Stopped on 2026-11-22 at 01:30**, four minutes after the run ended (the owner had been
  told on 11-21 that they would go down with the run): the tunnel, the gate, the page's
  server, and the launch entry. Checked: nothing listens on 8772 or 8781, and the tunnel's
  address no longer answers.

## 2026-11-21 A public copy of the repository (owner: "Can you put all the snekagon work up on a public repo on my github?")

- **Audit first** (a workflow of 13 agents, 19:55-20:20: six readers, one per risk, over all 30
  branches and the whole history; a second agent re-checked each reader's serious findings; a
  critic looked for gaps). No secret anywhere: no token, key, tunnel address or `?k=` link in
  any commit, and the gate's token file was never tracked. What it did find:
  - *Blocking:* `climb/field` (commit `5e73da7`, on that branch only) holds `src/refeval.rs`, a
    port of the reference solver's learned evaluation with its 14 weights, and ports of its
    tight bounds. That repository has no licence. On `climb/evalorder` two files hold that
    port's scores for 39,240 positions (`work/evalorder/top/port.txt`, `unl_port.txt`), from
    which the weights could be fitted again.
  - *The owner's rule on commits* (2026-11-21: no Claude attribution, only the owner's
    authorship): 232 of the 236 commits ended with a `Co-Authored-By: Claude ...` line.
  - *Stale text a reader meets first:* the research note still called the opening's value
    open and the reference solver never run; CLAUDE.md called the reference solver's author
    the publisher; LESSONS and IDEAS likewise. Corrected or dated at the tip (commit
    `088b666` and the next one). The same lines stay as they were on the older branches,
    which are snapshots of their day.
  - *For the owner to know:* the log and CLAUDE.md quote the owner's messages and the plan
    to write to the designer; 37 files named folders of this PC (the Windows user name, the
    app's session folder); two private artifact links; commit times carry the time zone.
  - Nothing of K0tovsk1y's is tracked, no labelled position of the reference repository is
    tracked, there is no binary in any commit, and quotes from other people's text are short
    and attributed (the longest from the video is 10 words).
- **The copy is made by a tool, and this repository is never pushed itself:**
  `tools/publish.py` (docstring; the options in CLAUDE.md, "The public copy"). It pipes
  `git fast-export` into `git fast-import`, takes the attribution lines out of the messages,
  leaves out chosen branches and files, replaces this machine's folders in file contents, and
  then checks every commit of the copy against its original (people, dates, parents, message,
  files). Its self-test: with nothing to change, all 236 hashes come out unchanged. Two runs
  give the same hashes, so a later push is a fast-forward. The copy's branch `commit-map`
  maps its hashes to the ones this log cites.
- **Left out of the copy:** `climb/field`; the two score files; `wasm` (the play page: the
  project's rule is that a public playable page waits for the designer's OK); the blog draft
  `docs/blog/solving-snekagon.html` (the owner's to publish, and it embeds the play
  section); the three `worktree-agent-*` branches (leftovers, no commit of their own). In:
  25 branches, 234 commits.
- **A README** for readers (`README.md`): the result, that it is the second proof, how far it
  has been checked, how to build and run, credit, what the copy leaves out.
- **Checked by a second workflow** (3 agents, 21:07-21:16) before any push: the README's 38
  factual claims against the repository (32 supported; the others reworded, among them "about
  19.5 hours", the certificate command, the files under `work/`); its claims about people
  and its tone (fuller credit to K0tovsk1y, Nelson Elhage added, the one checked line named
  as the cheapest, the shared reading of the rules, the meaning of the 38-bit key); and the
  copy itself against this repository (refs, the 14 weights and the port's names absent from
  every commit, the score files' contents absent, no attribution line, trees equal on all
  commits, no secret at any tip). That check is what found the blog draft and the folders.
- `python tools/certificate.py proofs/centre-opening/results.txt` run again on the committed
  file: its output is byte for byte the committed `certificate.txt`.
- The blog's sentence on the first proof's announcement now says "the solver's code", as the
  issue does (it said "speed"); private artifact version 4. The sentence about the credits
  of the video stands: the end card in the designer's repository credits "engine, solver and
  the Manim code of this video" to Claude Opus 5.5 (`video/chapters2.py:1444`, read on
  2026-11-21).
- **State:** the repository `nathanWolo/snekagon-solver` was created on GitHub as PRIVATE and
  the copy pushed to it, default branch `parallel-v9`. It becomes public on the owner's
  word. Open for the owner: whether the blog draft and the play page go in, a licence (none
  chosen; the owner's other solvers carry MIT), and whether the copy keeps the original
  messages after all.
- Not done: no build from a fresh clone was tried (the cross-check run has the machine), and
  the README's commands other than `info`, `text`, `verify` and the certificate were read
  from the source, not run today.
- **Made public, about 21:30.** The owner was told what the staged copy holds, what it leaves
  out and what it still shows (the owner's quoted messages and the plan to write to the
  designer first, the uncorrected lines on the older branches, no licence), and asked "Make
  the repository public as it is staged?". Owner: "Yes". This note was pushed to the staged
  repository first, as an ordinary fast-forward, and the visibility changed after it. The
  blog draft, the play page and a licence stay out until the owner says otherwise, and a
  later push (the cross-check's result, for one) needs the owner's word.
