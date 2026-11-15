# Ideas

The backlog. Each idea has its evidence and expected size. Status: open, running, passed (with the
key), failed (with the key and what would reopen it), dropped.

## Open

Ordered by expected gain for the effort. "Reference" means the idea is in the reference solver, whose
README reports 58x fewer nodes and 12x less time from v3 to v4 with items 1-4 together.

1. **E1 last pieces in closed form.** With at most one piece each, the value follows from the two
   move lists without a search. Evidence: profile P0, 64% of v0's nodes have at most two pieces left.
   Expected: 2-3x fewer nodes, 1.5-2x less time.
2. **E2 threat zones.** An opponent placement that decides the game by the bounds if played now must be
   covered (or touched, if it starts a new snake); only moves through every such zone are searched.
   Reference: the main part of its v4 gain at nodes where the loser must answer every move.
3. **E3 tight bounds near the end.** Guaranteed growth one and two plies ahead, and the one-player
   optimum (longest snake if the opponent never moved again). Reference: used with at most 5 pieces left.
4. **E4 ordering by a cheap evaluation of the child** for the side that looks winning. Reference uses a
   learned logistic evaluation with seven or more pieces left; the winner's first move is right about
   90% of the time there. A candidate here: difference of upper bounds and territory after the move.
5. **E5 proof-number search (df-pn)** over the same node logic, asking "does the mover win" and "does
   the mover lose" separately. Evidence for: the tree is very uneven (one move for the winner, every
   move for the loser), and a wrong first move at a winner's node costs a whole refutation. Evidence
   against: the reference reports first-move success near 90%. Owner asked for it by name.
6. **E6 symmetry-canonical table keys** up to ply 6. Small: test positions start at ply 5 or later.
7. **E7 speed of a node.** Incremental bounds (flood only what a move touched), cheaper move lists
   (score lazily, generate the table move first), prefetching the child's table entry.
8. **E8 a parallel solve** over root moves with a shared table, for the hard units.
9. **E9 enhanced transposition cutoffs.** The reference tried it and found no gain. Low priority.
10. **E10 null-window questions** ("does the mover win?") as the unit of work in proofs: cheaper than
    the exact value. Needed for any proof of an opening.

## Dropped (the graveyard)

None yet.

## Inbox

- The labels come from older versions of the reference solver. If our solver ever disagrees with one,
  check the position with `--no-bounds` (pure alpha-beta) before believing either.
