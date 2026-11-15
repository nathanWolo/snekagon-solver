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
