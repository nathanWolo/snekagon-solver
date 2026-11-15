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
