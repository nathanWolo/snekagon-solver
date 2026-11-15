# Ideas

The backlog. Each idea has its evidence and expected size. Status: open, passed (with the key),
failed or dropped (with the key and what would reopen it). `LOG.md` has every run.

## Open, by expected gain for the effort

1. **Cheapest win first.** The study of root moves (LOG, 2026-11-15) puts the ideal order at 3-6 times
   fewer nodes than the model of E4 at every upper level (ply 6: 0.67 M against 1.84 M out of sample;
   static was 3.39 M). The model predicts "wins"; what is missing is "wins cheaply". Within a position
   a win is cheap when the move leaves few replies after threat zones (correlation +0.44 to +0.50 with
   log nodes) and when our best next extension is large (-0.40). Fitting the cost directly overfitted
   200 positions; it needs thousands (the tool makes about 300 moves a second at ply 6), per level,
   and a two-stage score (cheap features for all moves, look-ahead features for the best 20).
2. **Merge the parallel solve** (branch `parallel`, E8): exact, 3.9x wall-clock on 8 cores, costs about
   2% with one thread. Owner's call. Further: its nodes grow 1.7x at 8 threads; the waste is threads
   on alternatives at winner's nodes below the two joint plies. Node types by a guess of the value
   (join where the mover is expected to win, share out where it is expected to lose) should cut it.
3. **Stronger threats.** With six pieces left the side being refuted still tries 16 moves a node. A
   threat is found today by the cluster bound after one opponent placement; the reach test (hands of
   one or two) or a two-placement threat would find more, so zones shrink. Cost: a test per candidate.
4. **A closed form with three pieces left** (two against one). After E3 the leaves with two pieces
   left and their parents are still 60% of the nodes at ply 6. The list of the opponent's extensions
   can be built once for the parent instead of once a child.
5. **Speed of a node (E7).** `Clusters::new()` zeroes about 1 KB twice a node; the move generator
   builds its context twice at nodes that scan threats; a child position is a 300-byte copy.
6. **Symmetry-canonical table keys** up to ply 6 (E6). Nothing for the test positions (ply 5 and
   later); needed before any search from the start position.
7. **Null-window questions as the unit of work** (E10) for a proof of an opening: "does the mover win"
   is cheaper than the exact value and is what a proof tree needs.
8. **A per-level model.** E4's weights come from roots with nine and eight pieces left; at ply 5 (ten
   left) the gain was 0.92x against 0.72x at ply 6. Root data at ply 5 costs minutes a position.

## Passed

- **E1 last pieces in closed form** (gate `E1`, ply 7): time 0.61x. v1.
- **E2 threat zones** (gate `E2`, ply 7): time 0.63x; 0.54x at ply 6. v2.
- **E3 reach test for hands of one or two pieces** (gate `E3`, ply 6): time 0.75x; 0.66x at ply 5. v3.
- **E4 upper moves ordered by a fitted model** (gate `E4`, ply 6): time 0.72x; 0.92x at ply 5. v4.

## Kept apart

- **E8 parallel solve**: see Open 2.

## Dropped (the graveyard)

- **E5 proof-number search (df-pn)**, `--pn` on both branches. Exact (12,100 labelled positions) and
  far slower than alpha-beta: 28-60x at ply 7 in its best setting. Not gated: the screen was decisive.
  *Why:* most moves at a winner's node are wins that are expensive to prove or losses that are cheap
  to refute; df-pn spends on every candidate before it commits, and re-expands a node on each return.
  *Reopen if:* positions appear where the first moves in order are expensive losses, or as a top
  layer over alpha-beta with a prior from the model (start numbers from win probability).
- **E5b fewest replies first** (`--order-level`, removed): nodes -7% to -18%, time worse (a threat scan
  per move). The count predicts the cost of a win, not whether the move wins. Its signal lives on
  in Open 1.
- **E5c budgeted rounds** (`--budget-level`, removed): 7-18x more nodes. Same reason as E5.
- **Reach test for hands of three** (part of E3): 10% of tests decide; 4.46 s against 4.18 s. *Reopen
  if* a cheaper three-piece test exists.
- **Look-ahead features in the model** (part of E4): fewer nodes (2.70 M against 2.92 M at seven or
  more pieces left) but a child position per move: 3.99 s against 2.59 s. *Reopen* as stage two of a
  two-stage score (Open 1).
- **E9 enhanced transposition cutoffs.** The reference tried it and found no gain. Not tried here.

## Inbox

- The labels come from older versions of the reference solver. If our solver ever disagrees with one,
  check the position with `--no-bounds` (pure alpha-beta) before believing either. (No disagreement
  so far in about 100,000 labelled solves across versions.)
- The binaries are not bit-reproducible (the Windows linker stamps the time). Identity between builds
  is checked by node counts; a fresh build of the v4 source reads +0.04% [-0.8%, +0.9%] against the
  frozen v4 (`AA-REBUILD`).
