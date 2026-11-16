# Snekagon solver: how this project works

This file is the orientation. Details live in `docs/`. Keep it short and true.

## The goal

- **Target:** an exact solver for the board game Snekagon (rules: `docs/research/rules-and-prior-work.md`),
  fast enough that the start position can one day be solved. The score is thread-seconds to solve a
  position exactly, with the value correct. Lower is better.
- **Production:** the real work a full solution needs: positions after about five moves that come up in
  the proof of an opening. The reference repository's `bench/ply5_*` units stand in for it.
- **Limits:** this PC (Ryzen 7 5800XT, 16 threads, 16 GB). Rust only (std, no crates). Toolchain:
  `stable-x86_64-pc-windows-msvc` on branch `parallel-v9` (E27); `main`'s base is a GNU build.
- **The bar:** owner's words, 2026-11-14: "an ultra fast solver ... with the long term goal of solving it
  from the start position. Good incremental progress would be having a very fast proof number search
  implementation that can solve non-start pos positions fairly quickly."
- **Today:** see "Right now".

## Right now

Update this section in the same change that freezes a version.

- **Paused on 2026-11-16 at 10:37 at the owner's request ("lets pause the climbing agents
  for now im just gonna play with the gui for a bit"). No agent is running; do not restart the
  climb without the owner's word.** The three lines each have a current `HANDOVER.md` on their
  branch (`git show BRANCH:HANDOVER.md`): `climb/failtry` (E34, `450dfa5`), `climb/tightq`
  (E33, `b1ba8c4`), `climb/evalorder` (E32, `9a382fa`). Copies of the candidate executables
  are in `work/exes/`. While the owner uses the board (port 8770) no timing counts: look at
  `work/gui/runs.log`. After stopping agents, look at `ps -ef` for replay loops and lock
  waiters that outlived them, then at `.git/run.lock` and `.git/run.queue`. LOG, "Paused
  again at 10:37".

- **Base for new work:** `frozen/base.exe` = v9 (alpha-beta with a table, the last pieces in closed
  form, threat zones, exact reach tests asked before the cluster bounds, model ordering, the first
  move searched before the list is generated, and after a failed move the moves that cover the
  refuting reply first). Id, commit and date: `frozen/BASE.txt` and `docs/SHIPLOG.md`. Branch `main`.
- **Base of the climb:** `frozen/climb-base.exe` on branch `parallel-v9`: a profiled MSVC build
  (E27: the MSVC toolchain +4.9% over the GNU build, the profile +6.3% on top) of v9 plus the
  search agent's same-node speed changes (E28, 0.916x) with table prefetch on by default (E25,
  0.949x), and a cost-aware move order (E30, 0.796x: killers and a history prefer the move
  whose wins were cheap at neighbouring nodes; `--no-cost-order` switches it off), and reach threats at
  every level (E29 round three, 0.856x with 0.53x the nodes; exact, cleared by an independent
  verifier; `--no-reach-threat` switches it off). All gated
  on proof positions. Build it and every candidate with `sh tools/build.sh [--pgo] OUT.exe`
  (the compile takes the run lock); a deciding read compares two `--pgo` builds.
  `frozen/climb-base-plain.exe` is the same source without the profile, for screens.
- **In work (sub-agents, Opus):**
  - The top of the proof (branch `climb/replay`, `a762d50`): `prove --replay` proves a file of
    replies in the proof's order (`sh tools/replay.sh EXE FILE DIR`, `tests/replay/tune25.txt`;
    `tools/replay_sum.py FILE DIR` sums a run and checks its certificates). **On the 25 tune
    replies base 4 takes 373 s a reply, no less than the paused run (326 s): each try is
    cheaper but failed tries take about 70% of the wall-clock** (recorded arms in
    `work/replay/base4` and `work/replay/turns`; LOG, "The top of the proof, fourth report").
    `--win-turns` read 0.771x there (35% better a reply, interval [+0.04%, +83%]): held, not
    gated, 49 replies cannot decide it. The other 49 refuted replies are kept for a gate.
  - Ending a failed try sooner (E34, branch `climb/failtry`): a driver change, read on the replay.
    Measured: 83% of the failed tries' thread-seconds are red-win jobs under the same try, on a
    full machine. `prove --hold S` is built and checked; its screen at S=10 was cut at root 6
    (0.89x on roots 0-5, no read); S=30 is next.
  - The field study is done (branch `climb/field`, `5e73da7`; LOG, "The field study"). The
    reference's lead is its move order near the root (a learned child evaluation); its tight
    bounds cut nodes and cost time. **The port of its evaluation (`--ref-eval L`) is a
    yardstick only: its repository has no licence, so its code and weights do not enter the
    base without the owner's word.**
  - Our own child evaluation as the order in the search (branch `climb/evalorder`): stopped.
    The port reads 0.69x cold on proof4b and 1.02x on failing jobs; every weight set of ours
    fitted on 48 roots read 1.04-1.09x out of sample. Its study of the top of the proof found
    that the model's own first move is a known winner in 15 of 25 tune replies where the base
    replay won first try in 7: the image bonus and the handed-down killers push the winner
    back. Next there: label the model's 7 unlabelled first moves, then a driver option that
    keeps the model's first move ahead of them (IDEAS 0b).
  - The one tight question, made cheap (branch `climb/tightq`): a change inside a pruning
    rule. It pays only asked at exactly 6 pieces left (0.873x nodes, about 0.89x time, single
    reads). `cargo test` passes and the exactness argument is written in its `HANDOVER.md`.
    Not verified: no check build, no `verify`, no independent judge yet.
  - Extra killer slots (E31, branch `climb/xk`, `381560f`): 0.845x nodes on cold single
    positions, 1.003x inside the driver. **Held, not gated** (LOG, "Ordering, round three").
  - Spent: mechanical cuts inside the reach test (branch `climb/threats`, five options, all off,
    none above 3%; LOG, "E29, sixth report", which also holds the hand-over note for that code).
    `--emulate W` (a driver run with exact node counts) is on `climb/emulate` and in `climb/replay`.
  Kept apart: the driver's `--split` and `--hint` (`climb/driver`), the ordering study
  (`climb/order`). Shadow builds that re-search what a rule pruned: `climb/verify`,
  `climb/verify-rt`, `climb/verify-rt3` (`--features shadow`).
- **A change inside a pruning rule is judged in place, never by labels.** A planted fault in the
  reach test gave 0 wrong labels and fewer nodes on the proof positions. Compare every answer
  with the unchanged computation in a check build (`--features ubcheck`, the shadow builds),
  run `verify 100 1 8 8`, and give it to an agent who did not write it.
- **An ordering change needs a driver read before its gate.** `tests/proof4` positions are
  the driver's jobs solved cold (no killers handed down, no history from sibling jobs, an
  empty table). E30 gained on both; E31 only cold. Read it with `prove --emulate 16` (exact
  nodes) on the tune jobs `work/e24/a.pos` to `f.pos`; jobs g to r are for driver gates.
- **Gate units:** `tests/proof4` has 57 unused gate units (164-220); `tests/proof4b` (1,184
  positions, none shared) has 295 (148-442). A learned order is gated only on positions whose
  parents were kept out of its training.
- **A change is not a faster proof until the replay says so.** `tests/proof4`, `proof4b` and the
  proof jobs are red-win jobs and winning tries: they measure what a try costs, not how many
  tries a reply takes or what a failed try wastes.
- **Absolute times drift with the machine:** with agents at work the same executable ran a
  third slower than the night before. Only paired reads count; a long proof wants the machine
  to itself.
- **Parallel solve:** branch `parallel-v9` (`bench --threads N`, `src/par.rs`), the port of branch
  `parallel` (built from v4) onto v9. Exact; 3.4x wall-clock on 8 threads; on v4 the plumbing cost
  about 2% with one thread, which is why it is not merged. A built copy is `frozen/par-v9.exe`.
  Merging is the owner's call (asked 2026-11-15).
- **The analysis board (owner's request, 2026-11-15):** `python tools/gui.py`, then
  `http://localhost:8765`: set up a position and run the frozen solver on it (`gui/`, no
  change to `src/`). Its rules live in `gui/rules.js` and are checked against the solver by
  `python tools/gui_check.py`; `tools/gui_test.py` exercises the server. A run from the page
  goes next in the lock's queue (a priority ticket) or, with "Run now", beside whatever
  runs; `work/gui/runs.log` records each run and whether it held the lock. Look there
  before trusting a timing that the owner's solves may have overlapped. **Prove** runs
  `frozen/prove-replay.exe` (`--prover`) on the position's replies, one root after another, and
  keeps its proof in `work/gui/proofs/<position>/`; the position after `1:60` starts from a copy
  of `work/long/ply1_results.txt` (74 of 172 refuted) and goes on from there.
- **Found about the game:** after `1:60 4B:16,27,28,39` red wins with `4B:70,81,82,93` (LOG,
  "the position after two moves is a win for red"; three runs and two engine generations agree;
  the reference solver has not been asked).
- **The centre opening `1:60` is paused, and speed comes first.** Owner, 2026-11-15 11:56: "you
  can pause the currently running super long job. im more interested in us hill climbing our
  efficiency to get faster proofs right now than just brute forcing by running the pc for days".
  State at the pause: 172 blue replies after symmetry (of 1,932), each needs a red win; 74
  refuted, 0 hold, 98 open after 7.6 hours on 16 threads (6.3 minutes a reply). What is proved:
  `python tools/certificate.py work/long/ply1_results.txt`. To go on, when the owner says so:
  `frozen/prove.exe prove work/long/ply1.txt --threads 1 --job-threads 16 --depth 2 --tt 27 --out work/long/ply1_results.txt`
  continues from the results file. The first run's progress lines are in
  `work/long/ply1_run1.err`, a copy of the results at the pause in `ply1_results_paused.txt`.
- **Proof driver:** also on `parallel-v9`: `snek prove FILE --threads N --job-threads T --depth D
  --out RESULTS` (`src/prove.rs`, a built copy is `frozen/prove.exe`). A coordinator searches the
  first D plies and hands positions to workers; every result goes to RESULTS and a later run
  starts from it. Exact; faster than the shared-table solve on the ply-4 position, slower on the
  drawn ply-3 position (LOG, E18). `--wide` searches with one window; moves that a symmetry of the
  position maps onto each other are searched once. Long solves and their results files are in
  `work/long/`.
- **Proof-number search:** `--pn` (df-pn, `src/pns.rs`). Alone it is 28-60x slower than
  alpha-beta. With alpha-beta's order and killers above and alpha-beta below
  (`--pn --pn-ab 9 --pn-model --pn-probe --pn-killers --pn-width 3 --pn-rank 1`) it is level
  with alpha-beta and not ahead (LOG, E23; `docs/IDEAS.md`, graveyard).
- **Proof benchmark:** `tests/proof4/positions.txt`: 592 red wins four moves in, children of the
  positions the paused proof searched to a loss for blue; about 5 s each for v9 on one thread.
  This is the ruler for "faster proofs". Positions 0-47 are used (E23); a deciding read takes
  fresh ones. On it the narrow question ("does the mover win", `--pn --pn-ab 12`) takes 0.72x
  the nodes of the wide window.
- **State:** the end of `docs/LOG.md`, and the table in `docs/SHIPLOG.md`.
- **What a gate resolves:** timing noise is about 1-2% per unit (A/A). A change that keeps the node
  counts has sd 0.02-0.08 per unit, a pruning change 0.2, an ordering change 0.25-0.45. Plan with
  `python tools/hc.py plan` on the screen's records.
- **Gate units:** the ply-6 gate range is used up. Gates run at ply 7 (about 90 unused gate units
  on 2026-11-15); `tools/run.py` refuses a gate with fewer than 30. More positions are needed soon.

## Standing rules

- **Ask the owner first** before pushing, publishing, or running the reference C++ solver (downloaded
  code). The owner allowed one thing on 2026-11-16 ("Yes, run it", to "Build and run the reference
  solver on this PC for a same-machine comparison?"): its `benchset` on its own bench files, as in
  LOG, "Same machine, the reference solver" (WSL, `/opt/snekref`, no network). Its other modes, and
  any other downloaded code, still need asking. Standing permissions, in the owner's words: "dont block on anything that needs interaction from
  me" (2026-11-14, for the overnight run): local commits and heavy CPU use are fine; nothing outward.
- **A wrong value is the only unforgivable result.** Every run checks every position against its label.
  A candidate that gives one wrong value is blocked, whatever its speed.
- **The base is frozen.** Experiments change `src/`; `frozen/base.exe` changes only by `run.py ... freeze`
  after a passed gate.
- **Test data.** Screen on the tune units. Gate once, on unused gate units. Confirm on unused confirm
  units. Nothing is tuned on held-out units. `tests/ledger.jsonl` records every deciding read.
- **Environment pin:** `-C target-cpu=native`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`.
  On `main`: `cargo build --release` (GNU toolchain). On `parallel-v9`: `sh tools/build.sh` (MSVC
  toolchain; `--pgo` for anything that is gated or frozen). Timed runs: one process at a time,
  nothing else heavy running.

## How we test

The contract is `docs/TESTING.md`. In short:

1. Write the hypothesis, expected size and bar in `docs/LOG.md`.
2. Make the one change in `src/` (behind a flag when cheap), `cargo build --release`.
3. Correctness: `target/release/snek verify 100` and the labels (the runner checks them on every unit).
   With a flag: the flag off must give the base's node counts exactly.
4. Screen: `python tools/run.py KEY-S tune --set plyNN --units 20` (node counts are exact; times are not).
5. Gate once: `python tools/run.py KEY gate --set plyNN --hypothesis "..."`.
6. Pass: commit, `python tools/run.py KEY freeze`, update docs. Fail or undecided: `git checkout src/`, log it.

## Working in this repo

- **The climb is on branch `parallel-v9`** (search, parallel solve, proof driver, proof-number
  options). Owner, 2026-11-15: "keep hill climbing the performance of the prover. act as a
  coordinator with opus sub-agents for implementations." The coordinator owns the deciding
  measurements and the freezes; sub-agents build in their own git worktrees and branches and
  return evidence. Nobody gates their own change.
- **One machine, one run lock.** Every timed run and anything that uses a processor for more
  than about 15 seconds goes through `sh tools/locked.sh <command>` (one lock for all worktrees;
  at most 15 minutes a command). `tools/build.sh` takes the lock for its compile by itself (do
  not wrap it). Quick checks do not need it. The lock is first come first served
  (`tools/lockfair.sh` in the main checkout; `tools/locked.sh` in every checkout is a stub that
  calls it): a script that chains locked commands goes to the back of the queue after each one.
- **Never kill the script that holds the lock.** Stop the solver it runs (the child process):
  the script then ends and gives the lock back. A lock left behind blocks everyone for 20
  minutes (it happened on 2026-11-15).
- **A build with the MSVC toolchain needs a short path:** the linker fails inside this folder
  (LNK1104). `tools/build.sh` builds in a short directory; a plain `cargo +stable-x86_64-pc-windows-msvc build`
  works only in a checkout whose path is short.
- **A worktree has no ignored files.** The labelled sets (`tests/set1/*.txt`), `frozen/` and
  `work/` exist only in the main checkout: read them there by absolute path, write your own
  scratch files in your worktree's `work/`.

- Tools run from the repo root with `python tools/<name>.py`. `tools/hc.py` is the statistics tool.
- The test set is not in git: rebuild it with `python tools/mktests.py <checkout of BoltKey/snekagon-lab at 61eb3a4> tests/set1`
  and compare with `tests/set1/MANIFEST.txt`.
- Shell quirks: Git Bash heredocs mangle backslashes; write scripts as files.
- Leave no processes behind. Kill only your own.
- A timed run needs a quiet machine: no builds, studies or other solves beside it.
- Build experiments that must not disturb a running `target/release/snek.exe` with
  `cargo build --release --target-dir work/target`.
- Several versions on the same fresh units: `python tools/ladder.py KEY --set plyNN --units N v3=frozen/v3-....exe v4=frozen/base.exe`.
- A study build that times the sections of a node: the patch scripts named in `docs/LOG.md` (`timing2_patch.py`), applied to a copy, built into `work/target`, never committed.
- Order ideas are screened on node counts first (exact, free of load): `snek bench FILE --from A --to B --quiet` with the option on and off.

## Where things are

| You need | Read |
| --- | --- |
| The rules, the reference solver and its numbers | `docs/research/rules-and-prior-work.md` |
| The testing contract | `docs/TESTING.md` |
| Every run, in order | `docs/LOG.md`, `runs.jsonl`, `runs/` |
| What we have learned | `docs/LESSONS.md` |
| The backlog | `docs/IDEAS.md` |
| Every frozen version | `docs/SHIPLOG.md`, `frozen/` |
| Code | `src/` (`geom`, `pieces`, `pos`, `bounds`, `solver`, `pns`, `study`, `naive`, `main`; `par` on branch `parallel`) |
| The analysis board | `tools/gui.py`, `gui/`, `tools/gui_check.py`, `tools/gui_test.py` |
| What predicts a good move | `tools/rootstudy.py`, `tools/rootfit.py`, `tools/rootcost.py` on the output of `snek rootmoves` |
