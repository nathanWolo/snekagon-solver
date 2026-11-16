# Ideas

The backlog. Each idea has its evidence and expected size. Status: open, passed (with the key),
failed or dropped (with the key and what would reopen it). `LOG.md` has every run.

## Open, by expected gain for the effort

0. **Finish the centre opening: paused by the owner on 2026-11-15, speed first.** `1:60` has
   172 blue replies after symmetry; 74 were refuted at the pause (11:56), none held, 6.3
   minutes a reply. Resume only on the owner's word (CLAUDE.md, "Right now"). The climb's ruler
   for proof work is `tests/proof4/` (TESTING.md).
   *Considered and not built:* a survey order, every reply gets a few red tries before any
   reply gets more (one cost term in the driver's scheduler). It would show early which replies
   are stubborn, but it is never faster to the value of `1:60`: if a reply holds, the
   refutations of the others were not needed for it. The dearest reply so far took 21 red
   tries. If every one loses, the start position is a
   win for red. The run is set up and can be resumed (CLAUDE.md, "Right now"). What decides its
   length is how soon red's winning move comes up in the order at ply 2: a red move that only
   draws costs about 20 s, the win about 5 minutes. The driver tries the last winning moves and
   their mirror images first (not measured). If a blue reply holds, showing it costs all of
   red's moves (hours); a wide window there, and a better guess of the first red move, are the
   next things to build.

0a. **Reach threats (E29, verified exact, gate pending on the base with E30).** At 4 pieces
   left or fewer with the test at node start: about 0.96x on profiled builds. At 5-6 left the
   tests that find a threat cost 23-40 thousand cycles; the cluster bound's margin predicts
   the answer and is the filter to try. Found by the study agent on 2026-11-15: the reply that
   refuted a move is a threat when the mover can no longer reach its length even alone; exact;
   nodes 0.51-0.57x on proof positions, 0.64-0.67x at plies 6-7; no time gain until the
   three-piece reach test behind it is cheaper. Branch `climb/threats`. LOG, "The study
   agent's report".
1. **More from what the search already knows.** E14 (cover the reply that refuted the last move)
   was worth 0.77x at ply 7 and 0.59x on 30 ply-5 positions for twenty lines. Six follow-ups of the
   same kind failed on node counts (E15-E17, E19-E21, below), a history table among them, and
   so did a table lookup of every child (E22). Rules of
   this kind look used up; what is left is a per-cell count of where refuting replies land.
2. **Cheapest win first.** A root pays 21-55% of its nodes before the first winning move and the win
   it finds costs about twice the cheapest (LOG, "a second look at the best few moves"). The fitted
   model is at the limit of its eight features (a refit on five times the data gave the same
   weights), and re-ranking by look-ahead features gave nothing. It needs information of another
   kind.
2b. **Cheapest win first, from what the search has seen (E30: passed, 0.796x; round two in
   work).** Left in the pot after round one (the agent's oracle): choosing between the two
   killers by record gets 0.78-0.96x where the cheaper in fact costs 0.55-0.60x. Round two:
   a budget race between candidates, more killer slots with costs, the history over every
   candidate, the history's life in the driver. The ordering study
   (LOG, 2026-11-15): a learned order pays 0.97-1.00x on held-out parents; the pot is the cost
   of the win that is taken (today pays 5.9x the cheapest win at 7 left, by standalone costs),
   and no feature of the position predicts it. Cost-aware killers and history use what
   neighbouring nodes paid. Branch `climb/cost`.
2a. **The GPU (owner's question, 2026-11-15): "could we write a solver to run on gpu and leverage
   that parallelism? could it be fast?"** Assessment, not yet a measurement. *As the search:*
   unlikely. A node here is not cheap arithmetic but a branchy proof step (reach tests 29% of the
   time, threat scans 22%); the tree is cut by order and by a shared table (red's first move cuts
   85-94% of the time), which is sequential knowledge; and plain enumeration is already not the
   cost (`perft 3`: a billion placements a second on one thread). A brute-force GPU bottom would
   have to beat alpha-beta's square-root tree, which it can only within about three plies of the
   end, and those are already closed form. *As the trainer of a move-ordering model:* this is
   where a measured pot is (Open 2: cheapest win first is 0.22-0.32x at the top of a proof, and
   a third of a proof's time goes to red tries that fail). The fitted model has 8 hand-made
   features and a refit on five times the data gave the same weights; a network over the board,
   trained on the solved positions with the owner's JAX set-up and evaluated only in the first
   plies (4% of the nodes are at six pieces left or more), is a different model class. *First
   step (no GPU needed), done 2026-11-15: no (item 2b; LOG, "The ordering study").* The plan was to size it: can any model trained on the labelled positions and the
   proof's records beat the current order on held-out positions, in nodes paid before the first
   winning move.
3. **Merge the parallel solve** (branch `parallel-v9`): exact, 3.4x wall-clock on 8 threads, about 2%
   with one thread on v4. Owner's call. The proof driver on the same branch (`prove`, E18) hands
   whole positions to workers and keeps every result in a file, so a solve can be stopped and
   resumed. It beats the shared-table solve where every root move must be refuted (42.8 s against
   51.9 s) and loses on a drawn position (191 s against 140 s), because it asks two narrow
   questions.
3a. **The driver's window.** Two narrow passes cost 1.15-1.4x on a drawn position and 1.39x when
   the first question is the wrong one; the right single pass costs 0.82-1.03x of a wide search
   (LOG, the window study). So: a wide window at the root when its moves are searched one at a
   time (safe, not built); a guess of the value to choose the first pass (the sign of the lead
   after a shallow search). "Ask for a win first" is refuted: the weaker statement is the cheaper
   proof. *Measured at depth (E23):* the right narrow question takes 0.82x the nodes of the wide
   window on 215 ply-5 positions and 0.72x on 48 proof positions four moves in. The driver's jobs
   already have it; `bench` and `solve` (and so the numbers on the reference's bench files) do
   not, and need the guess.
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
- **E18 proof driver** (`src/prove.rs` on `parallel-v9`): exact, restartable, not a speed gain yet.
- **Driver options `--split` and `--hint`** (branch `climb/driver`, not merged): exact; no gain
  that can be told from the driver's own run-to-run noise (sd 0.115 a job). *Reopen* with a
  quieter ruler or more jobs.
  See Open 3 and 3a.

## Dropped (the graveyard)

- **E23 df-pn with alpha-beta's order, killers and alpha-beta leaves** (options of `--pn` on
  `main`, off by default). Exact. Level with alpha-beta asked the same narrow question (within
  5% at ply 6 and on proof positions, 14-20% behind at ply 5), never ahead; leaves with a budget,
  the one thing only a proof-number search has, gain nothing that repeats and can cost 2.2x.
  *Why:* with a good order the search is at its best when it commits, and its numbers carry no
  prediction of cost (LOG, the pot sizing before E23). *Reopen if:* a cost predictor exists.
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
- **E19 the answer rule with the reply's whole zone** (cells next to a reply that starts a new snake
  too): nodes -0.1% at ply 7, +5.3% at ply 6.
- **E20 the answer bonus, halved, for the second killer**: nodes +4% and +14%.
- **E21 a history table of cutting moves in the score**: best weight -5.3% at ply 7 and +2.1% at
  ply 6; larger weights worse on both. Reopen if the score it is added to changes.
- **Reach test for hands of three, again on v8**: nodes -7%, time +13%. The pot is 7% at most.
- **Re-ranking the first K moves by look-ahead features**: 0.65x to 1.77x across files, no gain.
- **E22 enhanced transposition cutoffs** (look every child up in the table before the first
  move): nodes -0.5% at ply 7 and -1.0% at ply 6 at best, time +30% to +75% at those levels. The
  reference found the same. Reopen if the cutting child starts to come late in the order.

## Inbox

- The labels come from older versions of the reference solver. If our solver ever disagrees with one,
  check the position with `--no-bounds` (pure alpha-beta) before believing either. (No disagreement
  so far in about 100,000 labelled solves across versions.)
- The binaries are not bit-reproducible (the Windows linker stamps the time). Identity between builds
  is checked by node counts; a fresh build of the v4 source reads +0.04% [-0.8%, +0.9%] against the
  frozen v4 (`AA-REBUILD`).
