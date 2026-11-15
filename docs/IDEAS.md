# Ideas

The backlog. Each idea has its evidence and expected size. Status: open, passed (with the key),
failed or dropped (with the key and what would reopen it). `LOG.md` has every run.

## Open, by expected gain for the effort

1. **More from what the search already knows.** E14 (cover the reply that refuted the last move)
   was worth 0.77x at ply 7 and 0.59x on 30 ply-5 positions for twenty lines. Three follow-ups
   failed (E15-E17, below). Not yet tried: the same rule for the second killer; more than two killers
   a ply; a per-cell count of where refuting replies land, as a score.
2. **Cheapest win first.** A root pays 21-55% of its nodes before the first winning move and the win
   it finds costs about twice the cheapest (LOG, "a second look at the best few moves"). The fitted
   model is at the limit of its eight features (a refit on five times the data gave the same
   weights), and re-ranking by look-ahead features gave nothing. It needs information of another
   kind.
3. **Merge the parallel solve** (branch `parallel-v9`): exact, 3.4x wall-clock on 8 threads, about 2%
   with one thread on v4. Owner's call. Its nodes grow 1.9x at 8 threads: the waste is threads on
   alternatives at winner's nodes. A driver that hands whole subtrees (the children of a node that
   must all be refuted) to single threads with private tables would waste less and could be stopped
   and resumed; a solve of the start position needs that anyway.
4. **Node speed, what is left** (v8, share of time): reach tests 29%, threat scans 16%, last pieces
   10%, killer threats 6%, generating 5%, the rest (table, copy, play) 28%. The generator's
   per-orientation cost is behind most of these. A witness kept from a reach test that said "yes"
   could answer the same question at the child (about a third of the tests are such repeats).
5. **Stronger threats.** A threat is found by the cluster bound after one opponent placement. The
   exact reach test would find more at three and four pieces left, at about six times the cost of
   a scan: not worth it as it stands.
6. **Symmetry-canonical table keys** up to ply 6. Nothing for the test positions (ply 5 and later);
   needed before any search from the start position.
7. **More test positions.** The ply-6 gate range is used up and ply 7 has about 90 gate units left.
   New positions can be labelled by two frozen versions that agree (v0 and the base).

## Passed

- **E1 last pieces in closed form** (gate `E1`, ply 7). v1.
- **E2 threat zones** (gate `E2`, ply 7). v2.
- **E3 reach test for hands of one or two pieces** (gate `E3`, ply 6). v3.
- **E4 upper moves ordered by a fitted model** (gate `E4`, ply 6). v4.
- **E9 the first move before the move list** (gate `E9`, ply 6): same nodes, time 0.895x. v5.
- **E10 the exact reach question first, no cluster bound for that side** (gate `E10`, ply 6): time 0.857x. v6.
- **E11 a scan recomputes only the covered clusters** (gate `E11-P7`, ply 7): same nodes, time 0.954x. v7.
- **E12 the reach test starts from the longest snake** (gate `E12`, ply 7): same nodes, time 0.800x. v8.
- **E14 after a failed move, cover the refuting reply first** (gate `E14`, ply 7): nodes 0.77x on tune. v9.

The sizes to quote are the ladder's (LOG, "Confirmation"), not the gates'.

## Kept apart

- **E8 parallel solve**: see Open 3.

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
- **E13 a distance check before a reach test that needs a second snake**: same nodes, no change in
  time. Two pieces reach nine steps on a board eleven across. *Reopen if* a tighter test than
  distance is found.
- **E15 the last known refuting reply used before the first move**: nodes +0% to +4%.
- **E16 a counter-move table**: nodes +2% to +10% after the killers, twice the nodes before them.
- **E17 moves that overlap a blocked killer first**: nodes +4% to +26%.
- **Reach test for hands of three, again on v8**: nodes -7%, time +13%. The pot is 7% at most.
- **Re-ranking the first K moves by look-ahead features**: 0.65x to 1.77x across files, no gain.
- **Enhanced transposition cutoffs.** The reference tried it and found no gain. Not tried here.

## Inbox

- The labels come from older versions of the reference solver. If our solver ever disagrees with one,
  check the position with `--no-bounds` (pure alpha-beta) before believing either. (No disagreement
  so far in about 100,000 labelled solves across versions.)
- The binaries are not bit-reproducible (the Windows linker stamps the time). Identity between builds
  is checked by node counts; a fresh build of the v4 source reads +0.04% [-0.8%, +0.9%] against the
  frozen v4 (`AA-REBUILD`).
