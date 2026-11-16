//! A proof driver for positions too large for one search. A coordinator searches the first plies of
//! the tree (alpha-beta, with the solver's move order and threat zones) and hands whole positions at
//! the frontier ("jobs") to worker threads. A worker solves its job alone, on a table all workers
//! share.
//!
//! Sharing rule: the least speculative work goes out first. At a node where every move must be
//! refuted, all moves are work that has to be done, and they are divided between the workers. At a
//! node that needs one good move, a second move searched beside the first is a guess that is wasted
//! if the first one cuts; it costs 1, the next two cost 2, the next four 3. A job is handed out
//! only when no job with a smaller sum of costs along its path exists. A node counts as "every move
//! must be refuted" once enough of its moves have come back without a cut: one at a node whose
//! parent looks like a node that needs one good move, six otherwise.
//!
//! The value is found by two searches with a window of one step each ("can the mover at least
//! draw", "can the mover win"), so that no window moves while a node's moves are under way.
//!
//! Every result (a job, or a node of the coordinator's tree) is appended to a results file. A later
//! run reads the file first, so a proof that was stopped goes on where it was. Every value in the
//! file and in the table is a proof, so the answer is exact whatever the timing.

use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;
use crate::solver::*;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub struct ProveCfg {
    pub cfg: Config,
    /// Jobs that run at once.
    pub threads: usize,
    /// Threads that solve one job together, as in a parallel solve (`par.rs`). 1 = a job is one
    /// plain search.
    pub job_threads: usize,
    /// The coordinator searches this many plies from the root; positions below are jobs.
    pub depth: u32,
    /// The results file (read at the start, appended to), if any.
    pub out: Option<String>,
    pub quiet: bool,
    /// Plain young-brothers-wait: after the first move, any number side by side.
    pub plain: bool,
    /// Search every move, also those a symmetry of the position maps onto one already listed.
    pub no_symmetry: bool,
    /// Ask "can the mover win" before "can the mover at least draw".
    pub upper_first: bool,
    /// One search with the full window instead of the two questions. A node whose window is wide
    /// has one move under way at a time, so no move is searched with a window that has since moved.
    /// Cheaper for a drawn position (LOG, the window study); the workers are idle more.
    pub wide: bool,
}

pub struct ProveResult {
    pub value: i32,
    pub best: u16,
    pub jobs: u64,
    pub stopped: u64,
    pub reused: u64,
    pub nodes: u64,
    pub thread_seconds: f64,
    pub seconds: f64,
}

/// What is known about a position: its value lies in [lo, hi]; mv was its best move.
#[derive(Clone, Copy)]
struct Known {
    lo: i8,
    hi: i8,
    mv: u16,
}

pub struct Prover {
    c: ProveCfg,
    table: Arc<Table>,
    known: HashMap<u64, Known>,
    log: Option<std::fs::File>,
}

const NONE: usize = usize::MAX;
const ROOT: usize = 0;
/// Killer moves by ply. They travel with the tree: a node starts from its parent's, a job from its
/// node's, and what a finished move learned goes back up to the parent, where the next move starts
/// from it. So a search begins with the killers of the positions next to it, as in one thread.
type Killers = [[u16; 2]; MAX_PLY];
const MAX_BUDGET: u32 = 40;
const NEVER: u32 = 1000;
/// Added to the score of a move that is a mirror image of a move that has cut at the same ply:
/// above the model and the answer rule, below the killers.
const WINNER_BONUS: u64 = 1 << 24;
/// Jobs smaller than this many nodes are not kept.
const MIN_RECORD: u64 = 2000;

struct Node {
    pos: Pos,
    parent: usize,
    /// The move that led here.
    via: u16,
    /// The window the node was made with.
    window: (i32, i32),
    alpha: i32,
    beta: i32,
    best: i32,
    best_move: u16,
    /// The best bound of the moves left out because of a threat.
    skipped: i32,
    /// score << 16 | move, best first from `next` on.
    moves: Vec<u64>,
    next: usize,
    th: Threats,
    kids: Vec<usize>,
    /// Moves that have come back without a cut, and how many of them make this a node where every
    /// move has to be searched.
    finished: u32,
    need: u32,
    /// The last reply the remaining moves were re-ordered for.
    answered: u16,
    killers: Killers,
    leaf: bool,
    /// A leaf whose job has gone out, and one whose job has not come back.
    started: bool,
    running: bool,
    /// Decided when it was made (the game is over, or the result was known): nothing to record.
    given: bool,
    done: bool,
    dead: bool,
    stop: Option<Arc<AtomicBool>>,
}

struct Job {
    id: usize,
    pos: Pos,
    alpha: i32,
    beta: i32,
    killers: Killers,
    stop: Arc<AtomicBool>,
}

struct Done {
    id: usize,
    value: i32,
    best: u16,
    killers: Killers,
    nodes: u64,
    micros: u128,
}

struct Queue {
    jobs: Mutex<(VecDeque<Job>, bool)>, // the jobs, and "no more will come"
    ready: Condvar,
}

struct Driver<'a> {
    c: &'a ProveCfg,
    t: &'static Tables,
    frontier: u32,
    nodes: Vec<Node>,
    free: Vec<usize>,
    /// Slots given up during a walk of the tree; they are used again only after the walk.
    freed: Vec<usize>,
    /// The coordinator's solver: move order, threat zones and the table.
    top: Solver,
    /// By ply: the moves that have cut at a coordinator node of that ply. In an opening with a
    /// symmetric start the same idea wins again turned or mirrored, so the images of these moves
    /// under the board's symmetries are tried before the rest of the order.
    winners: Vec<Vec<u16>>,
    known: &'a mut HashMap<u64, Known>,
    log: &'a mut Option<std::fs::File>,
    start: Instant,
    /// Jobs out and not yet back.
    busy: usize,
    res: ProveResult,
}

/// The cells of `bb` after symmetry `s` of the board.
fn sym_cells(t: &Tables, mut bb: BB, s: usize) -> BB {
    let mut out: BB = 0;
    while bb != 0 {
        out |= bit(t.geo.sym[s][first(bb)] as usize);
        bb &= bb - 1;
    }
    out
}

fn bounds_of(alpha: i32, beta: i32, v: i32) -> (i32, i32) {
    if v <= alpha {
        (-1, v)
    } else if v >= beta {
        (v, 1)
    } else {
        (v, v)
    }
}

impl<'a> Driver<'a> {
    fn remember(&mut self, pos: &Pos, window: (i32, i32), v: i32, best: u16, nodes: u64, millis: u128) {
        let (lo, hi) = bounds_of(window.0, window.1, v);
        let e = self.known.entry(pos.hash).or_insert(Known { lo: -1, hi: 1, mv: PASS });
        e.lo = e.lo.max(lo as i8);
        e.hi = e.hi.min(hi as i8);
        if best != PASS {
            e.mv = best;
        }
        if let Some(f) = self.log.as_mut() {
            writeln!(f, "{} {} {} {} {} {}", pos.to_text(), lo, hi, nodes, millis, move_to_string(self.t, best)).expect("cannot write the results file");
        }
    }


    /// A new node for `pos`. One that is decided at once comes back marked done.
    fn new_node(&mut self, pos: Pos, parent: usize, via: u16, alpha: i32, beta: i32) -> usize {
        let mut n = Node {
            pos,
            parent,
            via,
            window: (alpha, beta),
            alpha,
            beta,
            best: -2,
            best_move: PASS,
            skipped: -2,
            moves: Vec::new(),
            next: 0,
            th: Threats::new(),
            kids: Vec::new(),
            killers: if parent == NONE { [[PASS; 2]; MAX_PLY] } else { self.nodes[parent].killers },
            finished: 0,
            need: if parent == NONE {
                2
            } else if self.nodes[parent].finished >= self.nodes[parent].need {
                6
            } else {
                1
            },
            answered: PASS,
            leaf: false,
            started: false,
            running: false,
            given: false,
            done: false,
            dead: false,
            stop: None,
        };
        if pos.is_over() {
            n.done = true;
            n.given = true;
            n.best = game_result(&pos);
        } else {
            let mut hint = PASS;
            if let Some(k) = self.known.get(&pos.hash).copied() {
                let (lo, hi) = (k.lo as i32, k.hi as i32);
                if lo >= hi || lo >= beta {
                    n.done = true;
                    n.best = lo;
                } else if hi <= alpha {
                    n.done = true;
                    n.best = hi;
                }
                if n.done {
                    n.given = true;
                    n.best_move = k.mv;
                    self.res.reused += 1;
                    if lo >= beta && k.mv != PASS {
                        // a cut that was known: its move is a killer and a winner like any other
                        let ply = (pos.ply as usize).min(MAX_PLY - 1);
                        let ks = &mut n.killers[ply];
                        if ks[0] != k.mv {
                            ks[1] = ks[0];
                            ks[0] = k.mv;
                        }
                        if !self.winners[ply].contains(&k.mv) {
                            self.winners[ply].push(k.mv);
                        }
                    }
                }
                hint = k.mv;
            }
            if !n.done {
                if pos.ply as u32 >= self.frontier {
                    n.leaf = true;
                } else {
                    let ks = n.killers[(pos.ply as usize).min(MAX_PLY - 1)];
                    let (moves, skipped, th) = self.top.ordered_moves(&pos, alpha, [hint, ks[0], ks[1]]);
                    n.moves = moves;
                    n.skipped = skipped;
                    n.th = th;
                    let ply = (pos.ply as usize).min(MAX_PLY - 1);
                    if !self.winners[ply].is_empty() {
                        let mut images = std::collections::HashSet::new();
                        for &w in &self.winners[ply] {
                            for s in 0..NSYM {
                                images.insert(self.t.sym_placement[w as usize][s]);
                            }
                        }
                        for e in n.moves.iter_mut() {
                            if *e >> 16 < WINNER_BONUS && images.contains(&((*e & 0xFFFF) as u16)) {
                                *e += WINNER_BONUS << 16;
                            }
                        }
                        n.moves.sort_unstable_by(|a, b| b.cmp(a));
                    }
                    // Moves whose result is already known come first, a cut among them before the
                    // rest: they cost nothing, and after a restart they bring the proof back to
                    // where it was. (The window of a child is (-beta, -alpha).)
                    let t = self.t;
                    let known = &*self.known;
                    n.moves.sort_by_cached_key(|e| {
                        let m = (*e & 0xFFFF) as u16;
                        if m == PASS {
                            return 2u8;
                        }
                        let mut child = pos;
                        child.play(t, m);
                        match known.get(&child.hash) {
                            Some(k) if k.hi as i32 <= -beta => 0,
                            Some(k) if k.lo >= k.hi || k.lo as i32 >= -alpha => 1,
                            _ => 2,
                        }
                    });
                    // Moves that a symmetry of the position maps onto each other lead to the same
                    // game: the first of each such set in the order is searched, the others not.
                    // (After the first move at the centre this leaves a twelfth of the replies.)
                    if !self.c.no_symmetry {
                        let stab: Vec<usize> = (0..NSYM).filter(|&s| sym_cells(t, pos.cells[0], s) == pos.cells[0] && sym_cells(t, pos.cells[1], s) == pos.cells[1]).collect();
                        if stab.len() > 1 {
                            let mut seen = std::collections::HashSet::new();
                            n.moves.retain(|e| {
                                let m = (*e & 0xFFFF) as u16;
                                if m == PASS {
                                    return true;
                                }
                                if seen.contains(&m) {
                                    return false;
                                }
                                for &s in &stab {
                                    seen.insert(t.sym_placement[m as usize][s]);
                                }
                                true
                            });
                        }
                    }
                    if n.moves.is_empty() && skipped == -2 {
                        n.moves.push(PASS as u64);
                    }
                }
            }
        }
        if let Some(i) = self.free.pop() {
            self.nodes[i] = n;
            i
        } else {
            self.nodes.push(n);
            self.nodes.len() - 1
        }
    }

    fn release(&mut self, n: usize) {
        if n != ROOT && !self.nodes[n].running {
            self.freed.push(n);
        }
    }

    /// What it costs to work on a move of node `n` that has `ahead` moves under way before it.
    fn cost(&self, n: usize, ahead: usize) -> u32 {
        let node = &self.nodes[n];
        if ahead > 0 && node.beta - node.alpha > 1 {
            return NEVER; // a wide window: the next move waits for this one's value
        }
        if self.c.plain {
            // plain young-brothers-wait: the first move alone, then any number
            return if ahead == 0 || node.finished >= 1 { 0 } else { 99 };
        }
        if ahead == 0 || node.finished >= node.need {
            0
        } else {
            usize::BITS - ahead.leading_zeros()
        }
    }

    /// The next job to hand out below node `n` whose cost is at most `budget`.
    fn find_work(&mut self, n: usize, budget: u32) -> Option<usize> {
        {
            let node = &mut self.nodes[n];
            if node.done || node.dead {
                return None;
            }
            if node.leaf {
                if node.started {
                    return None;
                }
                node.started = true;
                return Some(n);
            }
        }
        // work inside the moves already under way comes first, the oldest move first
        let mut i = 0;
        loop {
            if self.nodes[n].done || self.nodes[n].dead {
                return None;
            }
            let Some(&k) = self.nodes[n].kids.get(i) else { break };
            let cost = self.cost(n, i);
            if cost <= budget {
                if let Some(j) = self.find_work(k, budget - cost) {
                    return Some(j);
                }
            }
            if self.nodes[n].kids.get(i) == Some(&k) {
                i += 1; // k is still under way; otherwise it is gone and the next has taken its place
            }
        }
        // then a new move, if the budget allows one more
        loop {
            let node = &self.nodes[n];
            if node.done || node.dead || node.next >= node.moves.len() {
                return None;
            }
            let cost = self.cost(n, node.kids.len());
            if cost > budget {
                return None;
            }
            let m = (node.moves[node.next] & 0xFFFF) as u16;
            let (alpha, beta) = (node.alpha, node.beta);
            self.nodes[n].next += 1;
            if m != PASS && self.nodes[n].th.n > 0 {
                // the window may have risen since the list was made
                let bound = self.nodes[n].th.bound_for(self.t.placements[m as usize].cells);
                if bound <= alpha {
                    let s = self.nodes[n].skipped.max(bound);
                    self.nodes[n].skipped = s;
                    self.settle(n);
                    continue;
                }
            }
            let mut child = self.nodes[n].pos;
            child.play(self.t, m);
            let c = self.new_node(child, n, m, -beta, -alpha);
            self.nodes[n].kids.push(c);
            if self.nodes[c].done {
                let (v, mv) = (self.nodes[c].best, self.nodes[c].best_move);
                self.finish(c, v, mv);
                continue;
            }
            self.settle(c);
            if let Some(j) = self.find_work(c, budget - cost) {
                return Some(j);
            }
            // the child was settled while we looked: on to the next move
        }
    }

    /// If node `n` has nothing left to search or to wait for, it is finished with its best value.
    fn settle(&mut self, n: usize) {
        let node = &self.nodes[n];
        if !node.done && !node.dead && !node.leaf && node.next >= node.moves.len() && node.kids.is_empty() {
            let v = node.best.max(node.skipped);
            let mv = node.best_move;
            self.finish(n, v, mv);
        }
    }

    fn cancel(&mut self, n: usize) {
        self.nodes[n].dead = true;
        if let Some(stop) = &self.nodes[n].stop {
            stop.store(true, Relaxed);
        }
        let kids = std::mem::take(&mut self.nodes[n].kids);
        for k in kids {
            self.cancel(k);
        }
        self.release(n);
    }

    /// Node `n` has the value `v` (exact inside the window it was made with, a bound outside);
    /// `reply` is its best move.
    fn finish(&mut self, n: usize, v: i32, reply: u16) {
        {
            let node = &mut self.nodes[n];
            node.done = true;
            node.best = v;
            node.best_move = reply;
        }
        if !self.nodes[n].leaf && !self.nodes[n].given {
            let (pos, window) = (self.nodes[n].pos, self.nodes[n].window);
            self.remember(&pos, window, v, reply, 0, 0);
        }
        self.release(n);
        let p = self.nodes[n].parent;
        if p == NONE {
            return;
        }
        let via = self.nodes[n].via;
        self.nodes[p].kids.retain(|&k| k != n);
        if self.nodes[p].done || self.nodes[p].dead {
            return;
        }
        let s = -v;
        let ply = (self.nodes[p].pos.ply as usize).min(MAX_PLY - 1);
        let learned = self.nodes[n].killers;
        let pn = &mut self.nodes[p];
        pn.killers = learned;
        pn.finished += 1;
        if s > pn.best {
            pn.best = s;
            pn.best_move = via;
        }
        if s > pn.alpha {
            pn.alpha = s;
        }
        let cut = pn.alpha >= pn.beta;
        let (best, mv) = (pn.best, pn.best_move);
        if p == ROOT && !self.c.quiet {
            let pn = &self.nodes[p];
            eprintln!(
                "# {:8.1} s  root move {} of {}  {}  value {}  best so far {}",
                self.start.elapsed().as_secs_f64(),
                pn.finished,
                pn.moves.len(),
                move_to_string(self.t, via),
                s,
                best
            );
        }
        if cut {
            if via != PASS && !self.winners[ply].contains(&via) {
                self.winners[ply].push(via);
            }
            let ks = &mut self.nodes[p].killers[ply];
            if via != PASS && ks[0] != via {
                ks[1] = ks[0];
                ks[0] = via;
            }
            let kids = std::mem::take(&mut self.nodes[p].kids);
            for k in kids {
                self.cancel(k);
            }
            self.finish(p, best, mv);
            return;
        }
        // The reply that refuted this move will meet most moves that leave it alone: the moves that
        // cover one of its cells go first (the search does the same).
        let bonus = self.c.cfg.answer_bonus << 16;
        if bonus > 0 && reply != PASS && reply != self.nodes[p].answered {
            let cells = self.t.placements[reply as usize].cells;
            let t = self.t;
            let pn = &mut self.nodes[p];
            pn.answered = reply;
            let next = pn.next;
            for e in pn.moves[next..].iter_mut() {
                let m = (*e & 0xFFFF) as u16;
                if m != PASS && t.placements[m as usize].cells & cells != 0 {
                    *e += bonus;
                }
            }
            pn.moves[next..].sort_unstable_by(|a, b| b.cmp(a));
        }
        self.settle(p);
    }
}

impl<'a> Driver<'a> {
    /// A job has come back.
    fn take(&mut self, done: Done) {
        self.busy -= 1;
        self.res.nodes += done.nodes;
        self.res.thread_seconds += done.micros as f64 / 1e6;
        let id = done.id;
        self.nodes[id].running = false;
        self.nodes[id].killers = done.killers;
        if done.value == UNKNOWN {
            self.res.stopped += 1;
            if self.nodes[id].dead {
                self.freed.push(id);
            } else {
                self.nodes[id].started = false; // not stopped by us: hand it out again
            }
            return;
        }
        self.res.jobs += 1;
        let (pos, window) = (self.nodes[id].pos, self.nodes[id].window);
        if done.nodes >= MIN_RECORD {
            // a smaller job is cheaper to run again than to keep
            self.remember(&pos, window, done.value, done.best, done.nodes, done.micros / 1000);
        }
        if self.nodes[id].dead {
            self.freed.push(id);
        } else {
            self.finish(id, done.value, done.best);
        }
    }

    /// One pass: the value of `root` within the window (alpha, beta). With beta = alpha + 1 no
    /// node's window moves while its moves are under way; with a wider one, `cost` sees to that.
    fn pass(&mut self, root: &Pos, alpha: i32, beta: i32, queue: &Queue, rx: &mpsc::Receiver<Done>) -> (i32, u16) {
        self.nodes.clear();
        self.free.clear();
        self.freed.clear();
        let root_id = self.new_node(*root, NONE, PASS, alpha, beta);
        assert!(root_id == ROOT);
        self.settle(ROOT);
        let mut last_report = Instant::now();
        loop {
            // hand out work while a worker is free
            while self.busy < self.c.threads && !self.nodes[ROOT].done {
                let mut freed = std::mem::take(&mut self.freed);
                self.free.append(&mut freed);
                let Some(j) = (0..=MAX_BUDGET).find_map(|budget| self.find_work(ROOT, budget)) else { break };
                let stop = Arc::new(AtomicBool::new(false));
                let n = &mut self.nodes[j];
                n.stop = Some(stop.clone());
                n.running = true;
                let job = Job { id: j, pos: n.pos, alpha: n.alpha, beta: n.beta, killers: n.killers, stop };
                if let Some(k) = self.known.get(&job.pos.hash).copied() {
                    // a bound from an earlier run that does not decide this window still helps
                    self.top.seed(job.pos.hash, k.lo as i32, k.hi as i32, k.mv);
                }
                queue.jobs.lock().unwrap().0.push_back(job);
                queue.ready.notify_one();
                self.busy += 1;
            }
            if self.nodes[ROOT].done {
                break;
            }
            assert!(self.busy > 0, "the proof driver has no work and no answer");
            let done = rx.recv().expect("the workers are gone");
            self.take(done);
            if !self.c.quiet && last_report.elapsed().as_secs() >= 60 {
                last_report = Instant::now();
                let r = &self.nodes[ROOT];
                eprintln!(
                    "# {:8.1} s  window ({},{})  jobs {}  stopped {}  reused {}  nodes {:.1} M  thread-s {:.0}  root: {} of {} moves done, {} under way",
                    self.start.elapsed().as_secs_f64(),
                    alpha,
                    beta,
                    self.res.jobs,
                    self.res.stopped,
                    self.res.reused,
                    self.res.nodes as f64 / 1e6,
                    self.res.thread_seconds,
                    r.finished,
                    r.moves.len(),
                    r.kids.len()
                );
            }
        }
        // the pass is over: stop what is still running and wait for it (its results are still true)
        for n in self.nodes.iter() {
            if let Some(stop) = &n.stop {
                stop.store(true, Relaxed);
            }
        }
        while self.busy > 0 {
            let done = rx.recv().expect("the workers are gone");
            self.take(done);
        }
        (self.nodes[ROOT].best, self.nodes[ROOT].best_move)
    }
}

impl Prover {
    pub fn new(c: ProveCfg) -> Prover {
        let t = tables();
        let mut known = HashMap::new();
        if let Some(path) = c.out.as_deref() {
            if let Ok(text) = std::fs::read_to_string(path) {
                for line in text.lines() {
                    let f: Vec<&str> = line.split_whitespace().collect();
                    if f.len() < 6 {
                        continue;
                    }
                    let (Some(pos), Ok(lo), Ok(hi)) = (Pos::from_text(f[0]), f[1].parse::<i8>(), f[2].parse::<i8>()) else { continue };
                    if lo < -1 || hi > 1 || lo > hi {
                        continue;
                    }
                    let e = known.entry(pos.hash).or_insert(Known { lo: -1, hi: 1, mv: PASS });
                    e.lo = e.lo.max(lo);
                    e.hi = e.hi.min(hi);
                    if let Some(m) = parse_move(t, f[5]) {
                        if m != PASS {
                            e.mv = m;
                        }
                    }
                }
                if !c.quiet {
                    eprintln!("# {} positions read from {}", known.len(), path);
                }
            }
        }
        let log = c.out.as_deref().map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).expect("cannot open the results file"));
        let table = Arc::new(Table::new(c.cfg.tt_bits));
        Prover { c, table, known, log }
    }

    /// The exact value of `root` for the side to move. Two questions, each a search with a window of
    /// one step: can the mover at least draw, and can the mover win. Asked as one search with the
    /// full window, several drawing moves of a node would each get their costly exact proof side by
    /// side where one is enough.
    pub fn prove(&mut self, root: &Pos) -> ProveResult {
        let start = Instant::now();
        self.table.next_generation();
        let queue = Arc::new(Queue { jobs: Mutex::new((VecDeque::new(), false)), ready: Condvar::new() });
        let (tx, rx) = mpsc::channel::<Done>();
        let c = &self.c;
        let mut d = Driver {
            c,
            t: tables(),
            frontier: root.ply as u32 + c.depth,
            nodes: Vec::new(),
            free: Vec::new(),
            freed: Vec::new(),
            top: Solver::with_table(c.cfg, self.table.clone()),
            winners: vec![Vec::new(); MAX_PLY],
            known: &mut self.known,
            log: &mut self.log,
            start,
            busy: 0,
            res: ProveResult { value: UNKNOWN, best: PASS, jobs: 0, stopped: 0, reused: 0, nodes: 0, thread_seconds: 0.0, seconds: 0.0 },
        };
        let table = &self.table;
        let (mut lo, mut hi) = (-1, 1);
        let mut best = PASS;
        std::thread::scope(|scope| {
            // --emulate W: one worker thread, W solvers taking the jobs in turn (exact node counts)
            let emulate = c.cfg.drv_emulate as usize;
            for _ in 0..(if emulate > 0 { 1 } else { c.threads }) {
                let queue = queue.clone();
                let tx = tx.clone();
                let nsolvers = if emulate > 0 { emulate } else { c.job_threads.max(1) };
                let mut solvers: Vec<Solver> = (0..nsolvers).map(|_| Solver::with_table(c.cfg, table.clone())).collect();
                let mut turn = 0usize;
                scope.spawn(move || loop {
                    let job = {
                        let mut g = queue.jobs.lock().unwrap();
                        loop {
                            if let Some(j) = g.0.pop_front() {
                                break Some(j);
                            }
                            if g.1 {
                                break None;
                            }
                            g = queue.ready.wait(g).unwrap();
                        }
                    };
                    let Some(mut job) = job else { break };
                    let t0 = Instant::now();
                    let (value, best, nodes) = if job.stop.load(Relaxed) {
                        (UNKNOWN, PASS, 0)
                    } else if solvers.len() == 1 || emulate > 0 {
                        turn += 1;
                        let k = (turn - 1) % solvers.len();
                        let solver = &mut solvers[k];
                        solver.stop = Some(job.stop.clone());
                        let (v, m) = solver.solve_job(&job.pos, job.alpha, job.beta, &mut job.killers);
                        (v, m, solver.nodes)
                    } else {
                        // all threads of this worker search the job together; the first to finish
                        // has the answer and stops the others
                        let busy = Arc::new(Busy::new());
                        let answer = AtomicI32::new(UNKNOWN);
                        let nodes = AtomicU64::new(0);
                        let found: Mutex<Option<(u16, Killers)>> = Mutex::new(None);
                        let start_killers = job.killers;
                        std::thread::scope(|inner| {
                            for solver in solvers.iter_mut() {
                                let (job, busy, answer, nodes, found) = (&job, &busy, &answer, &nodes, &found);
                                inner.spawn(move || {
                                    solver.stop = Some(job.stop.clone());
                                    solver.busy = Some(busy.clone());
                                    let mut killers = start_killers;
                                    let (v, m) = solver.solve_job(&job.pos, job.alpha, job.beta, &mut killers);
                                    nodes.fetch_add(solver.nodes, Relaxed);
                                    if v != UNKNOWN {
                                        let mut g = found.lock().unwrap();
                                        if g.is_none() {
                                            *g = Some((m, killers));
                                            answer.store(v, Relaxed);
                                        }
                                        job.stop.store(true, Relaxed);
                                    }
                                });
                            }
                        });
                        let v = answer.load(Relaxed);
                        let mut m = PASS;
                        if let Some((mv, killers)) = found.into_inner().unwrap() {
                            m = mv;
                            job.killers = killers;
                        }
                        (v, m, nodes.load(Relaxed))
                    };
                    if tx.send(Done { id: job.id, value, best, killers: job.killers, nodes, micros: t0.elapsed().as_micros() }).is_err() {
                        break;
                    }
                });
            }
            drop(tx);
            let passes: &[(i32, i32)] = if c.wide {
                &[(-1, 1)]
            } else if c.upper_first {
                &[(0, 1), (-1, 0)]
            } else {
                &[(-1, 0), (0, 1)]
            };
            for &(a, b) in passes {
                if lo >= hi || lo >= b || hi <= a {
                    continue; // already answered
                }
                let (v, mv) = d.pass(root, a, b, &queue, &rx);
                let (l, h) = bounds_of(a, b, v);
                lo = lo.max(l);
                hi = hi.min(h);
                if (v >= b || (c.wide && v > a)) && mv != PASS {
                    best = mv;
                }
                if !c.quiet {
                    eprintln!("# {:8.1} s  window ({},{}) gave {}: the value lies in [{}, {}]", start.elapsed().as_secs_f64(), a, b, v, lo, hi);
                }
            }
            // let the workers go
            let mut g = queue.jobs.lock().unwrap();
            g.0.clear();
            g.1 = true;
            drop(g);
            queue.ready.notify_all();
        });
        assert!(lo == hi, "the two passes left the value open");
        d.res.value = lo;
        d.res.best = best;
        d.res.seconds = start.elapsed().as_secs_f64();
        d.res
    }
}
