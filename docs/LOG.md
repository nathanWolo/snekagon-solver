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
