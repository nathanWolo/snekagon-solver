//! Exact solver: the game-theoretic value of a position for the side to move (+1 win, 0 draw, -1 loss).
//! Negamax alpha-beta over the three values. Every shortcut is a proof; nothing here is heuristic
//! except the order in which moves are tried.

use crate::bounds::*;
use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Config {
    /// Use the snake-length bounds to decide nodes.
    pub bounds: bool,
    /// Value the last pieces (at most one in each hand) without a search.
    pub last_pieces: bool,
    /// Skip moves that leave a deciding opponent placement playable.
    pub threats: bool,
    /// Near the end, ask whether each side can still reach the length it needs.
    pub reach: bool,
    /// The reach test is run for a player with at most this many pieces in hand.
    pub reach_hand: u32,
    /// Positions one reach test may look at before it gives up.
    pub reach_work: u32,
    /// With at least this many pieces left, moves are ordered by the fitted model. 99 = never.
    pub model_level: u32,
    /// Model weights (a logistic fit of "this move wins", scaled by 64): our lead after the move, piece
    /// size, grows a snake, length of the snake joined, covers a cell next to an opponent end, touches
    /// the opponent, free cells at our ends after, free cells at their ends after, their longest snake
    /// after their best reply, ours after our best next placement, their upper bound, our upper bound.
    pub model: [i32; NFEATURES],
    /// In a parallel solve, moves are shared out between threads at nodes with at least this many
    /// pieces left.
    pub par_level: u32,
    /// In a parallel solve, the first this-many plies from the root are searched by all threads
    /// together, move by move; below them a move another thread has taken is set aside.
    pub par_join: usize,
    /// log2 of the number of table entries.
    pub tt_bits: u32,
    /// Search the table move or a killer before the move list is generated.
    pub staged: bool,
    /// Where the reach test applies to a player, ask it first and skip that player's cluster bound.
    pub lazy: bool,
    /// In a threat scan, recompute only the clusters a placement covers.
    pub narrow: bool,
    /// After a failed move, the score added to the moves that cover a cell of the reply that refuted
    /// it (0 = off). 1024 is more than any other score: those moves come first, in their old order.
    pub answer_bonus: u64,
    /// Ask the processor for a child's table entry ahead of its lookup: when the move is played,
    /// and for the next move in the order while this one is searched.
    pub prefetch: bool,
    /// Nodes with at most this many pieces left (both hands) neither read nor write the table.
    pub tt_skip: u32,
    /// Study build: collect the pot measurements of `StudyStats` (observation only; the search is unchanged).
    pub study: bool,
    /// Study: the work budget of one reach test asked by the study (reach threats, one-player optimum).
    pub study_work: u32,
    /// Reach threats (an opponent placement after which our one-player optimum is too short) at
    /// nodes with at least this many pieces left. 99 = never.
    pub reach_threat: u32,
    /// The work budget of one reach-threat test.
    pub reach_threat_work: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { bounds: true, last_pieces: true, threats: true, reach: true, reach_hand: 2, reach_work: 200, model_level: 7, model: MODEL, par_level: 5, par_join: 2, tt_bits: 22, staged: true, lazy: true, narrow: true, answer_bonus: 1024, prefetch: false, tt_skip: 0, study: false, study_work: 20000, reach_threat: 99, reach_threat_work: 2000 }
    }
}

/// One searched move of a node, for the study: the move, how its child was decided, the child's
/// reply (PASS if none), the child's nodes, and the reach-threat savings counted inside the child.
#[derive(Clone, Copy)]
pub struct Rec {
    pub m: u16,
    pub kind: u8,
    pub reply: u16,
    pub nodes: u64,
    pub inner: [u64; 2],
}

/// Pot measurements (study build). Every saving is exclusive: a subtree counted under an outer
/// node is not counted again for nodes inside it.
pub struct StudyStats {
    /// Nodes under moves that failed at a node that later cut, by the side to move at that node and
    /// by the pieces left at the wasted node (outermost attribution).
    pub waste: [[u64; 16]; 2],
    /// Nodes where every move failed, by pieces left: count, moves searched, distinct replies, moves
    /// refuted by the top reply, nodes under the moves, nodes under the top reply's moves, children
    /// decided without a move, by the table, by their first move, by a later move, pass children.
    pub an: [[u64; 12]; 16],
    /// Reach threats at those nodes, by pieces left: tests, tests that give a threat, moves pruned
    /// (in order), nodes pruned (in order), work, moves pruned (oracle), nodes pruned (oracle).
    pub rt: [[u64; 8]; 16],
    /// Exclusive saved nodes of the reach threats: [in order, oracle].
    pub rt_saved: [u64; 2],
    /// One-player optimum at nodes the present bounds did not decide, by pieces left: nodes looked
    /// at, tests asked, nodes it decides, work, tests that ran out of work.
    pub opt: [[u64; 6]; 16],
    pub opt_saved: u64,
    /// Upper bound minus one-player optimum, by pieces left, by hand size of the player, slack 0..7.
    pub slack: [[[u64; 8]; 6]; 16],
    /// Separated positions (no placement of one side can touch a placement of the other): by pieces
    /// left, [nodes looked at, separated].
    pub sep: [[u64; 2]; 16],
    pub sep_saved: u64,
}

impl StudyStats {
    pub fn new() -> StudyStats {
        StudyStats { waste: [[0; 16]; 2], an: [[0; 12]; 16], rt: [[0; 8]; 16], rt_saved: [0; 2], opt: [[0; 6]; 16], opt_saved: 0, slack: [[[0; 8]; 6]; 16], sep: [[0; 2]; 16], sep_saved: 0 }
    }
}

/// What the table knows about a position: its value lies in [lo, hi]; mv was the best move.
#[derive(Clone, Copy)]
struct Entry {
    lo: i8,
    hi: i8,
    mv: u16,
}

/// The transposition table, shared by all threads of a solve. A bucket holds two entries; an entry
/// is two words, key ^ data and data, so a pair torn by two writers fails its key test and is
/// ignored. data: move (16 bits), generation (8), log2 of the work (8), lo + 1 (2), hi + 1 (2).
pub struct Table {
    slots: Vec<[AtomicU64; 4]>,
    mask: usize,
    gen: AtomicU8,
}

#[inline]
fn pack(lo: i32, hi: i32, mv: u16, work: u8, gen: u8) -> u64 {
    mv as u64 | (gen as u64) << 16 | (work as u64) << 24 | ((lo + 1) as u64) << 32 | ((hi + 1) as u64) << 34
}

impl Table {
    pub fn new(bits: u32) -> Table {
        let buckets = 1usize << (bits - 1);
        Table {
            slots: (0..buckets).map(|_| [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)]).collect(),
            mask: buckets - 1,
            gen: AtomicU8::new(1),
        }
    }

    /// Starts a new generation: entries of an older one never match. The table is wiped only when
    /// the counter wraps (generation 0 is the empty entry's).
    pub fn next_generation(&self) -> u8 {
        let mut g = self.gen.load(Relaxed).wrapping_add(1);
        if g == 0 {
            for b in self.slots.iter() {
                for w in b {
                    w.store(0, Relaxed);
                }
            }
            g = 1;
        }
        self.gen.store(g, Relaxed);
        g
    }

    pub fn generation(&self) -> u8 {
        self.gen.load(Relaxed)
    }
}

/// A pointer to the buckets of a `Table` that the holder keeps alive.
#[derive(Clone, Copy)]
struct SlotPtr(*const [AtomicU64; 4]);
unsafe impl Send for SlotPtr {}

/// Which (position, move) pairs some thread is searching right now. A thread that meets one sets the
/// move aside and comes back to it later, when the answer is likely in the table (simplified
/// ABDADA). Races here only change who searches what, never a value.
pub struct Busy {
    slots: Vec<AtomicU64>,
}

const BUSY_BUCKETS: usize = 1 << 14;
const BUSY_WAYS: usize = 4;
const MAX_DEFERRED: usize = 4096;

impl Busy {
    pub fn new() -> Busy {
        Busy { slots: (0..BUSY_BUCKETS * BUSY_WAYS).map(|_| AtomicU64::new(0)).collect() }
    }
    #[inline]
    fn bucket(h: u64) -> usize {
        ((h >> 24) as usize & (BUSY_BUCKETS - 1)) * BUSY_WAYS
    }
    #[inline]
    fn is_busy(&self, h: u64) -> bool {
        let b = Self::bucket(h);
        (0..BUSY_WAYS).any(|i| self.slots[b + i].load(Relaxed) == h)
    }
    #[inline]
    fn start(&self, h: u64) {
        let b = Self::bucket(h);
        for i in 0..BUSY_WAYS {
            if self.slots[b + i].load(Relaxed) == 0 {
                self.slots[b + i].store(h, Relaxed);
                return;
            }
        }
        self.slots[b].store(h, Relaxed);
    }
    #[inline]
    fn finish(&self, h: u64) {
        let b = Self::bucket(h);
        for i in 0..BUSY_WAYS {
            if self.slots[b + i].load(Relaxed) == h {
                self.slots[b + i].store(0, Relaxed);
            }
        }
    }
}
pub const UNKNOWN: i32 = -9;
pub const NFEATURES: usize = 12;
/// Fitted on tune positions of plies 6 and 7 (`tools/rootfit.py --win-only`, 2026-11-15). The last four
/// (one placement ahead, bounds after the move) cost a child position each and did not pay: weight 0.
pub const MODEL: [i32; NFEATURES] = [21, 17, 21, -1, 1, -3, 9, -7, 0, 0, 0, 0];
const MAX_THREATS: usize = 6;
const MAX_TRIED: usize = 8;
const MAX_FOUND: usize = 64;
const NO_BOUND: i32 = 2;

/// Opponent placements that would decide the game by the bounds if played now. A move of ours that
/// covers no cell of a threat's zone leaves it playable and is worth at most the threat's value.
pub(crate) struct Threats {
    pub(crate) n: usize,
    zone: [BB; MAX_THREATS],
    value: [i32; MAX_THREATS],
    tried: [u16; MAX_TRIED],
    ntried: usize,
    rtried: [u16; MAX_TRIED],
    nrtried: usize,
}

impl Threats {
    pub(crate) fn new() -> Threats {
        Threats { n: 0, zone: [0; MAX_THREATS], value: [0; MAX_THREATS], tried: [PASS; MAX_TRIED], ntried: 0, rtried: [PASS; MAX_TRIED], nrtried: 0 }
    }
    /// Upper bound on our value for a move covering `cells`.
    #[inline]
    pub(crate) fn bound_for(&self, cells: BB) -> i32 {
        let mut b = NO_BOUND;
        for i in 0..self.n {
            if cells & self.zone[i] == 0 && self.value[i] < b {
                b = self.value[i];
            }
        }
        b
    }
}

/// The cells our move has to cover to make the opponent placement b illegal.
#[inline]
fn threat_zone(pos: &Pos, opp: usize, b: &Placement) -> BB {
    if b.border & pos.cells[opp] != 0 {
        b.cells
    } else {
        b.cells | b.border
    }
}
pub const MAX_PLY: usize = 40;
const NO_UNWIND: usize = usize::MAX;
const TT_MOVE: u64 = 1 << 30;
const KILLER1: u64 = 1 << 29;
const KILLER2: u64 = 1 << 28;

pub struct Solver {
    pub(crate) t: &'static Tables,
    pub cfg: Config,
    tt: Arc<Table>,
    /// The table's buckets and index mask, cached: the table is owned through `tt` and never resized.
    slots: SlotPtr,
    mask: usize,
    gen: u8,
    /// Set by another thread when the answer is known: stop.
    pub stop: Option<Arc<AtomicBool>>,
    /// Shared with the other threads of a parallel solve.
    pub busy: Option<Arc<Busy>>,
    pub nodes: u64,
    /// Nodes by the number of pieces both players still hold (a profile, not used by the search).
    pub by_left: [u64; 16],
    /// By pieces left: table cuts, bound cuts, nodes searched, moves searched, nodes cut by their
    /// first move, nodes where every move failed, moves skipped by threats, nodes that scanned.
    pub prof: [[u64; 8]; 16],
    /// Reach tests by kind (0 the mover, 1 the opponent) and pieces left: [tests, decided].
    pub reach_by: [[[u64; 2]; 16]; 2],
    node_limit: u64,
    aborted: bool,
    /// Parallel solve: the ply of the root, the nodes on the path from it by ply (hash and window at
    /// entry), and the ply to unwind to when another thread has resolved a node we are below.
    root_ply: usize,
    path: Vec<(u64, i32, i32)>,
    unwind_to: usize,
    pub(crate) killers: [[u16; 2]; MAX_PLY],
    bufs: Vec<Vec<u64>>,
    defer_bufs: Vec<Vec<u16>>,
    replies: Vec<(u32, BB)>,
    /// Study build only.
    pub st: Box<StudyStats>,
    ret_kind: u8,
    ret_mv: u16,
    recs: Vec<Vec<Rec>>,
}

/// The result of a finished game for the side to move.
#[inline]
pub fn game_result(pos: &Pos) -> i32 {
    let me = pos.stm as usize;
    let lead = pos.longest[me] as i32 - pos.longest[me ^ 1] as i32;
    lead.signum()
}

impl Solver {
    pub fn new(cfg: Config) -> Solver {
        Solver::with_table(cfg, Arc::new(Table::new(cfg.tt_bits)))
    }

    /// A solver on a table that other solvers (threads) may share.
    pub fn with_table(cfg: Config, table: Arc<Table>) -> Solver {
        Solver {
            t: tables(),
            cfg,
            gen: table.generation(),
            slots: SlotPtr(table.slots.as_ptr()),
            mask: table.mask,
            tt: table,
            stop: None,
            busy: None,
            nodes: 0,
            by_left: [0; 16],
            prof: [[0; 8]; 16],
            reach_by: [[[0; 2]; 16]; 2],
            node_limit: u64::MAX,
            aborted: false,
            root_ply: 0,
            path: vec![(0, 0, 0); MAX_PLY + 2],
            unwind_to: NO_UNWIND,
            killers: [[PASS; 2]; MAX_PLY],
            bufs: (0..MAX_PLY).map(|_| Vec::with_capacity(1024)).collect(),
            defer_bufs: (0..MAX_PLY).map(|_| Vec::new()).collect(),
            replies: Vec::with_capacity(1024),
            st: Box::new(StudyStats::new()),
            ret_kind: 0,
            ret_mv: PASS,
            recs: (0..MAX_PLY).map(|_| Vec::new()).collect(),
        }
    }

    /// Forget everything learned from earlier positions.
    pub fn clear(&mut self) {
        self.gen = self.tt.next_generation();
        self.killers = [[PASS; 2]; MAX_PLY];
    }

    /// For a thread of a parallel solve: take up the table's present generation.
    pub fn sync(&mut self) {
        self.gen = self.tt.generation();
        self.killers = [[PASS; 2]; MAX_PLY];
    }

    /// The value within the window (alpha, beta): exact inside it, a bound outside. UNKNOWN when the
    /// node limit stopped the search.
    pub fn solve(&mut self, pos: &Pos, alpha: i32, beta: i32, node_limit: u64) -> i32 {
        self.nodes = 0;
        self.node_limit = node_limit;
        self.aborted = false;
        self.root_ply = (pos.ply as usize).min(MAX_PLY - 2);
        self.unwind_to = NO_UNWIND;
        let v = if self.busy.is_some() { self.search::<true>(pos, alpha, beta) } else { self.search::<false>(pos, alpha, beta) };
        if self.aborted {
            UNKNOWN
        } else {
            v
        }
    }

    /// For the proof driver: the value of one position within (alpha, beta), on the shared table,
    /// given up when `self.stop` is set. `killers`: the killer moves to start from (those of the
    /// positions searched just before this one in the tree); they come back as the search left them.
    /// Returns the value (UNKNOWN when stopped) and the table's best move for the position.
    pub fn solve_job(&mut self, pos: &Pos, alpha: i32, beta: i32, killers: &mut [[u16; 2]; MAX_PLY]) -> (i32, u16) {
        self.nodes = 0;
        self.node_limit = u64::MAX;
        self.aborted = false;
        self.root_ply = (pos.ply as usize).min(MAX_PLY - 2);
        self.unwind_to = NO_UNWIND;
        self.killers = *killers;
        let v = self.search::<true>(pos, alpha, beta);
        *killers = self.killers;
        if self.aborted {
            return (UNKNOWN, PASS);
        }
        (v, self.probe(pos.hash).map_or(PASS, |e| e.mv))
    }

    /// For the proof driver: what is already known about a position goes into the table.
    pub(crate) fn seed(&mut self, hash: u64, lo: i32, hi: i32, mv: u16) {
        self.store(hash, lo, hi, mv, 1 << 30);
    }

    /// For the proof driver: the moves of `pos` worth a search in the window above `alpha`, best
    /// first, each as score << 16 | move; the best bound of the moves a threat rules out (-2 if
    /// none); and the threats. The scores are those of the search (step 4 of `search`, repeated
    /// here so that the search itself is left as it was); `first` are the moves to put in front.
    pub(crate) fn ordered_moves(&mut self, pos: &Pos, alpha: i32, first: [u16; 3]) -> (Vec<u64>, i32, Threats) {
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let mut th = Threats::new();
        if self.cfg.threats && self.cfg.bounds && pos.left[opp] != 0 {
            let mut mine = Clusters::new();
            upper_bound_ex(t, pos, me, 0, Some(&mut mine));
            self.scan_threats(pos, &mine, alpha, &mut th);
        }
        let mut skipped = -2;
        let mut buf: Vec<u64> = Vec::new();
        let level = (pos.left[0].count_ones() + pos.left[1].count_ones()) as u32;
        let own = pos.cells[me];
        let theirs = pos.cells[opp];
        let front = neighbours(shape_of(theirs).ends) & pos.empty();
        let use_model = level >= self.cfg.model_level;
        let w = self.cfg.model;
        let empty = pos.empty();
        let their_shape = shape_of(theirs);
        let their_free = neighbours(their_shape.ends) & empty & !neighbours(their_shape.body);
        pos.for_each_move(t, me, pos.pieces_now(), |m| {
            let p = &t.placements[m as usize];
            if th.n > 0 {
                let bound = th.bound_for(p.cells);
                if bound <= alpha {
                    skipped = skipped.max(bound);
                    return;
                }
            }
            let mut score = p.n as u64 * 64;
            if p.border & own != 0 {
                score += 256;
            }
            if p.cells & front != 0 {
                score += 128;
            } else if p.border & theirs != 0 {
                score += 32;
            }
            if use_model {
                let mut joined = p.n as i32;
                for j in 0..pos.nsnakes[me] as usize {
                    if pos.snakes[me][j] & p.border != 0 {
                        joined += count(pos.snakes[me][j]) as i32;
                    }
                }
                let after = joined.max(pos.longest[me] as i32);
                let mine = shape_of(own | p.cells);
                let my_ends = count(neighbours(mine.ends) & empty & !p.cells & !neighbours(mine.body)) as i32;
                let their_ends = count(their_free & !p.cells) as i32;
                let s = w[0] * (after - pos.longest[opp] as i32)
                    + w[1] * p.n as i32
                    + w[2] * (p.border & own != 0) as i32
                    + w[3] * joined
                    + w[4] * (p.cells & front != 0) as i32
                    + w[5] * (p.border & theirs != 0) as i32
                    + w[6] * my_ends
                    + w[7] * their_ends;
                score = (s + (1 << 20)).max(0) as u64;
            }
            if m == first[0] {
                score = TT_MOVE;
            } else if m == first[1] {
                score = KILLER1;
            } else if m == first[2] {
                score = KILLER2;
            }
            buf.push(score << 16 | m as u64);
        });
        buf.sort_unstable_by(|a, b| b.cmp(a));
        (buf, skipped, th)
    }

    /// One more window search for a driver above: the table stays and `nodes` goes on counting.
    /// `node_limit` is a limit on `nodes`. UNKNOWN when it stopped the search.
    pub(crate) fn search_more(&mut self, pos: &Pos, alpha: i32, beta: i32, node_limit: u64) -> i32 {
        self.node_limit = node_limit;
        self.aborted = false;
        let v = self.search::<false>(pos, alpha, beta);
        if self.aborted {
            self.aborted = false;
            UNKNOWN
        } else {
            v
        }
    }

    #[inline]
    fn probe(&self, key: u64) -> Option<Entry> {
        // in bounds: mask is the bucket count minus one
        let b = unsafe { &*self.slots.0.add((key as usize) & self.mask) };
        for i in 0..2 {
            let d = b[2 * i + 1].load(Relaxed);
            if b[2 * i].load(Relaxed) ^ d == key && (d >> 16) as u8 == self.gen {
                return Some(Entry { lo: ((d >> 32) & 3) as i8 - 1, hi: ((d >> 34) & 3) as i8 - 1, mv: d as u16 });
            }
        }
        None
    }

    /// Starts loading the table entry of `key` into the cache.
    #[inline(always)]
    fn prefetch(&self, key: u64) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use core::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
            _mm_prefetch(self.slots.0.add((key as usize) & self.mask) as *const i8, _MM_HINT_T0);
        }
    }

    /// The hash of the position after the mover plays the placement `m`.
    #[inline(always)]
    fn hash_after(&self, pos: &Pos, m: u16) -> u64 {
        let t = self.t;
        let me = pos.stm as usize;
        let p = &t.placements[m as usize];
        let mut h = pos.hash ^ t.zob_stm ^ t.zob_piece[me][p.piece as usize];
        for j in 0..p.n as usize {
            h ^= t.zob_cell[me][p.cell_list[j] as usize];
        }
        h
    }

    #[inline(always)]
    fn store_if(&mut self, on: bool, key: u64, lo: i32, hi: i32, mv: u16, work: u64) {
        if on {
            self.store(key, lo, hi, mv, work);
        }
    }

    #[inline]
    fn store(&mut self, key: u64, lo: i32, hi: i32, mv: u16, work: u64) {
        let work = (64 - work.leading_zeros()) as u8;
        let gen = self.gen;
        // in bounds: mask is the bucket count minus one
        let b = unsafe { &*self.slots.0.add((key as usize) & self.mask) };
        let d0 = b[1].load(Relaxed);
        let k0 = b[0].load(Relaxed) ^ d0;
        let d1 = b[3].load(Relaxed);
        let k1 = b[2].load(Relaxed) ^ d1;
        // An entry of this position from this solve (another thread's): both bounds are true, so keep
        // the tighter of each. With one thread this never changes anything.
        let merged = |old: u64| -> u64 {
            if (old >> 16) as u8 != gen {
                return pack(lo, hi, mv, work, gen);
            }
            let (olo, ohi) = (((old >> 32) & 3) as i32 - 1, ((old >> 34) & 3) as i32 - 1);
            let (nlo, nhi) = (lo.max(olo), hi.min(ohi));
            if nlo > nhi {
                return pack(lo, hi, mv, work, gen);
            }
            pack(nlo, nhi, if mv == PASS { old as u16 } else { mv }, work.max((old >> 24) as u8), gen)
        };
        if k0 == key {
            let d = merged(d0);
            b[0].store(key ^ d, Relaxed);
            b[1].store(d, Relaxed);
        } else if k1 == key {
            let d = merged(d1);
            b[2].store(key ^ d, Relaxed);
            b[3].store(d, Relaxed);
        } else {
            let d = pack(lo, hi, mv, work, gen);
            if (d0 >> 16) as u8 != gen || work >= (d0 >> 24) as u8 {
                b[2].store(k0 ^ d0, Relaxed);
                b[3].store(d0, Relaxed);
                b[0].store(key ^ d, Relaxed);
                b[1].store(d, Relaxed);
            } else {
                b[2].store(key ^ d, Relaxed);
                b[3].store(d, Relaxed);
            }
        }
    }

    /// The exact value when each player has at most one piece left: the mover places its piece (or
    /// passes), then the opponent takes its longest extension that is still legal. A placement only
    /// takes opponent placements away (the cells it covers, and new snakes that would now touch it).
    pub(crate) fn last_pieces(&mut self, pos: &Pos) -> i32 {
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let occ = pos.occupied();
        let opp_now = pos.longest[opp] as u32;
        let mut replies = std::mem::take(&mut self.replies);
        replies.clear();
        if pos.left[opp] != 0 {
            pos.for_each_move(t, opp, pos.left[opp], |m| {
                let p = &t.placements[m as usize];
                let len = pos.longest_after(opp, p);
                if len > opp_now {
                    // a reply that grows a snake is stopped only by covering it; a new snake also by touching it
                    let blocked_by = if p.border & occ != 0 { p.cells } else { p.cells | p.border };
                    replies.push((len, blocked_by));
                }
            });
            replies.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
        let opp_after = |ours: BB| -> u32 {
            for &(len, blocked_by) in replies.iter() {
                if blocked_by & ours == 0 {
                    return len;
                }
            }
            opp_now
        };
        let mut best = -2;
        if pos.left[me] != 0 {
            pos.for_each_move(t, me, pos.left[me], |m| {
                if best == 1 {
                    return;
                }
                let p = &t.placements[m as usize];
                let mine = pos.longest_after(me, p) as i32;
                let v = (mine - opp_after(p.cells) as i32).signum();
                if v > best {
                    best = v;
                }
            });
        }
        if best == -2 {
            best = (pos.longest[me] as i32 - opp_after(0) as i32).signum();
        }
        self.replies = replies;
        best
    }

    /// Adds the opponent placement `m` to the threats if the bounds decide the game after it.
    fn try_threat(&self, pos: &Pos, m: u16, alpha: i32, deep: bool, th: &mut Threats) {
        if m == PASS || th.n >= MAX_THREATS {
            return;
        }
        let level = pos.left[0].count_ones() + pos.left[1].count_ones();
        let deep = deep && level >= self.cfg.reach_threat && th.nrtried < MAX_TRIED && !th.rtried[..th.nrtried].contains(&m);
        let first_time = th.ntried < MAX_TRIED && !th.tried[..th.ntried].contains(&m);
        if !first_time && !deep {
            return;
        }
        if first_time {
            th.tried[th.ntried] = m;
            th.ntried += 1;
        }
        if deep {
            th.rtried[th.nrtried] = m;
            th.nrtried += 1;
        }
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let b = &t.placements[m as usize];
        if pos.left[opp] >> b.piece & 1 == 0 || !pos.legal_placement(t, opp, b) {
            return;
        }
        let my_upper = upper_bound_ex(t, pos, me, b.cells, None);
        let opp_len = pos.longest_after(opp, b) as i32;
        let value = if my_upper < opp_len {
            -1
        } else if my_upper <= opp_len {
            0
        } else {
            2
        };
        let _ = alpha;
        // A reach threat (`--reach-threat L`): with b placed, can we still build a snake as long as
        // the opponent's? If not, a move of ours that neither covers nor touches b leaves b playable,
        // is itself playable after b, and our one-player optimum after both is no more than after b
        // alone: the move is worth at most the threat's value. Zone: b's cells and border.
        let ask = deep && pos.left[me] != 0 && (value == 2 || (value == 0 && alpha < 0));
        let mut rv = 2;
        if ask {
            let mut child = *pos;
            child.place_for(t, opp, m);
            let mut work = self.cfg.reach_threat_work;
            if !can_reach_alone(t, &child, me, opp_len, &mut work) {
                rv = -1;
            } else if value == 2 && alpha >= 0 {
                let mut work = self.cfg.reach_threat_work;
                if !can_reach_alone(t, &child, me, opp_len + 1, &mut work) {
                    rv = 0;
                }
            }
        }
        if rv <= 0 {
            th.zone[th.n] = b.cells | b.border;
            th.value[th.n] = rv;
            th.n += 1;
        } else if value <= 0 && first_time {
            th.zone[th.n] = threat_zone(pos, opp, b);
            th.value[th.n] = value;
            th.n += 1;
        }
    }

    /// Adds every opponent placement after which our value is at most alpha by the bounds.
    /// `mine`: our clusters. A placement lowers our bound only inside the clusters it covers, so it
    /// has to cover every cluster whose bound is too high: the cheap test before the exact one.
    pub(crate) fn scan_threats(&self, pos: &Pos, mine: &Clusters, alpha: i32, th: &mut Threats) {
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let my_lower = pos.longest[me] as i32;
        let strict = if alpha < 0 { 1 } else { 0 }; // a loss needs our bound strictly below their length
        // one placement joins at most two opponent snakes, which caps the opponent's length after it
        let (mut longest, mut second) = (0i32, 0i32);
        for j in 0..pos.nsnakes[opp] as usize {
            let len = count(pos.snakes[opp][j]) as i32;
            if len > longest {
                second = longest;
                longest = len;
            } else if len > second {
                second = len;
            }
        }
        let max_need = (pos.longest[opp] as i32).max(4 + longest + second) - strict;
        if my_lower > max_need {
            return;
        }
        let mut must_cover = ALL;
        if !mine.overflow {
            for i in 0..mine.n {
                if mine.upper[i] > max_need {
                    must_cover = mine.cells[i];
                    break;
                }
            }
        }
        let mut found: [(u32, BB, i32); MAX_FOUND] = [(0, 0, 0); MAX_FOUND];
        let mut nf = 0;
        // a placement changes only the clusters it covers: the others keep their bound
        let narrow = self.cfg.narrow && !mine.overflow && pos.left[me] != 0;
        let ctx = BoundCtx::new(pos, me);
        pos.for_each_move(t, opp, pos.left[opp], |m| {
            if nf >= MAX_FOUND {
                return;
            }
            let b = &t.placements[m as usize];
            if b.cells & must_cover == 0 {
                return;
            }
            let opp_len = pos.longest_after(opp, b) as i32;
            let need = opp_len - strict; // our upper bound must not exceed this
            if my_lower > need {
                return;
            }
            if !mine.overflow {
                for i in 0..mine.n {
                    if mine.upper[i] > need && mine.cells[i] & b.cells == 0 {
                        return;
                    }
                }
            }
            let my_upper = if narrow {
                let mut upper = my_lower;
                let mut covered: BB = 0;
                for i in 0..mine.n {
                    if mine.cells[i] & b.cells != 0 {
                        covered |= mine.cells[i];
                    } else if mine.upper[i] > upper {
                        upper = mine.upper[i];
                    }
                }
                if covered != 0 {
                    upper = upper.max(upper_bound_cells(t, &ctx, covered & !b.cells, None));
                }
                upper
            } else {
                upper_bound_ex(t, pos, me, b.cells, None)
            };
            let value = if my_upper < opp_len {
                -1
            } else if my_upper <= opp_len {
                0
            } else {
                1
            };
            if value <= alpha {
                let zone = threat_zone(pos, opp, b);
                found[nf] = (count(zone), zone, value);
                nf += 1;
            }
        });
        // smallest zones first: a zone that contains a kept one (with a value as low) prunes nothing more
        found[..nf].sort_unstable_by_key(|f| f.0);
        for f in &found[..nf] {
            if th.n >= MAX_THREATS {
                break;
            }
            let redundant = (0..th.n).any(|i| th.zone[i] & !f.1 == 0 && th.value[i] <= f.2);
            if !redundant {
                th.zone[th.n] = f.1;
                th.value[th.n] = f.2;
                th.n += 1;
            }
        }
    }

    /// Study: at a node the present bounds left open, would the exact one-player optimum decide it
    /// (bit 1), are the two sides separated (bit 2)? Also samples the slack of the upper bound.
    fn study_node(&mut self, pos: &Pos, level: usize, alpha: i32, beta: i32, test_me: bool, test_them: bool) -> u8 {
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let mine_now = pos.longest[me] as i32;
        let theirs_now = pos.longest[opp] as i32;
        let budget = self.cfg.study_work;
        let mut mark = 0u8;
        self.st.opt[level][0] += 1;
        let mut ask = |st: &mut StudyStats, player: usize, need: i32| -> bool {
            let mut work = budget;
            st.opt[level][1] += 1;
            let r = can_reach_alone(t, pos, player, need, &mut work);
            st.opt[level][3] += (budget - work) as u64;
            if work == 0 {
                st.opt[level][4] += 1;
            }
            r
        };
        if !test_me && pos.left[me] != 0 {
            let need = theirs_now + if alpha >= 0 { 1 } else { 0 };
            if mine_now < need && !ask(&mut self.st, me, need) {
                mark |= 1;
            }
        }
        if mark == 0 && !test_them && pos.left[opp] != 0 {
            let need = mine_now + if beta <= 0 { 1 } else { 0 };
            if theirs_now < need && !ask(&mut self.st, opp, need) {
                mark |= 1;
            }
        }
        if mark & 1 != 0 {
            self.st.opt[level][2] += 1;
        }
        // separation: whole growable areas, and only the clusters that can still reach the other's length
        self.st.sep[level][0] += 1;
        let gm = growable(pos, me);
        let go = growable(pos, opp);
        if go & (gm | neighbours(gm)) == 0 {
            self.st.sep[level][1] += 1;
        }
        {
            let mut cm = Clusters::new();
            let mut co = Clusters::new();
            upper_bound_ex(t, pos, me, 0, Some(&mut cm));
            upper_bound_ex(t, pos, opp, 0, Some(&mut co));
            if !cm.overflow && !co.overflow {
                let rel = |c: &Clusters, other_now: i32| -> BB {
                    let mut r = 0;
                    for i in 0..c.n {
                        if c.upper[i] >= other_now {
                            r |= c.cells[i];
                        }
                    }
                    r
                };
                let rm = rel(&cm, theirs_now);
                let ro = rel(&co, mine_now);
                if go & (rm | neighbours(rm)) == 0 && gm & (ro | neighbours(ro)) == 0 {
                    self.st.sep[level][1 + 0] += 0;
                    mark |= 2;
                }
            }
        }
        if mark & 2 != 0 {
            self.st.opt[level][5] += 1;
        }
        // slack of the upper bound, on a sample of the nodes
        if self.nodes & 7 == 0 {
            for player in 0..2 {
                let h = pos.left[player].count_ones() as usize;
                if h == 0 || h > 5 {
                    continue;
                }
                let upper = upper_bound(t, pos, player);
                let mut opt = pos.longest[player] as i32;
                let mut ok = true;
                while opt < upper {
                    let mut work = budget;
                    let r = can_reach_alone(t, pos, player, opt + 1, &mut work);
                    if work == 0 {
                        ok = false;
                        break;
                    }
                    if !r {
                        break;
                    }
                    opt += 1;
                }
                if ok {
                    self.st.slack[level][h][((upper - opt) as usize).min(7)] += 1;
                }
            }
        }
        mark
    }

    fn study_close(&mut self, mark: u8, s_opt0: u64, s_sep0: u64, nodes_at_start: u64) {
        let below = self.nodes - nodes_at_start;
        if mark & 1 != 0 {
            self.st.opt_saved = s_opt0 + below;
        }
        if mark & 2 != 0 {
            self.st.sep_saved = s_sep0 + below;
        }
    }

    /// Study: a node where every move failed. How the refutations look, and what reach threats (a
    /// reply of the opponent after which our one-player optimum is too short) would have pruned.
    fn study_all_node(&mut self, pos: &Pos, level: usize, alpha: i32, recs: &[Rec]) {
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let a = &mut self.st.an[level];
        a[0] += 1;
        a[1] += recs.len() as u64;
        let mut replies: Vec<(u16, u64, u64)> = Vec::new(); // reply, moves, nodes
        for r in recs {
            a[4] += r.nodes;
            match r.kind {
                0 => a[6] += 1,
                1 => a[7] += 1,
                3 => a[8] += 1,
                4 => a[9] += 1,
                _ => a[10] += 1,
            }
            if r.reply != PASS {
                if let Some(e) = replies.iter_mut().find(|e| e.0 == r.reply) {
                    e.1 += 1;
                    e.2 += r.nodes;
                } else {
                    replies.push((r.reply, 1, r.nodes));
                }
            }
        }
        a[2] += replies.len() as u64;
        if let Some(top) = replies.iter().max_by_key(|e| e.1) {
            a[3] += top.1;
            a[5] += top.2;
        }
        if pos.left[me] == 0 || pos.left[opp] == 0 {
            return;
        }
        // reach threats. A reply R is a threat when, with R placed, our one-player optimum is below
        // (or, for a draw, not above) R's length. A move of ours that leaves R legal and that would
        // itself be legal after R is then worth at most that value.
        let budget = self.cfg.study_work;
        let test = |st: &mut StudyStats, r: u16| -> Option<(BB, BB, bool, i32)> {
            let p = &t.placements[r as usize];
            if pos.left[opp] >> p.piece & 1 == 0 || !pos.legal_placement(t, opp, p) {
                return None;
            }
            let mut child = *pos;
            child.place_for(t, opp, r);
            let target = child.longest[opp] as i32;
            st.rt[level][0] += 1;
            let mut work = budget;
            let mut value = 2;
            if !can_reach_alone(t, &child, me, target, &mut work) {
                value = -1;
            } else if alpha >= 0 {
                let mut w2 = budget;
                if !can_reach_alone(t, &child, me, target + 1, &mut w2) {
                    value = 0;
                }
                work = work.min(w2);
            }
            st.rt[level][4] += (budget - work) as u64;
            if value > alpha {
                return None;
            }
            st.rt[level][1] += 1;
            let grows = p.border & pos.cells[opp] != 0;
            Some((p.cells, p.border, grows, value))
        };
        let own = pos.cells[me];
        let covered = |z: &(BB, BB, bool, i32), m: u16| -> bool {
            // true: the move m is worth at most the threat's value
            let q = &t.placements[m as usize];
            if q.cells & z.0 != 0 {
                return false;
            }
            if q.cells & z.1 == 0 {
                return true;
            }
            // m touches R: fine only if R grows (then R is still legal) and m grows (then m is legal after R)
            z.2 && q.border & own != 0
        };
        // in the search's order: a reply is tested when it first refutes a move, up to eight
        let mut zones: Vec<(BB, BB, bool, i32)> = Vec::new();
        let mut tested: Vec<u16> = Vec::new();
        for r in recs {
            if zones.iter().any(|z| covered(z, r.m)) {
                self.st.rt[level][2] += 1;
                self.st.rt[level][3] += r.nodes;
                self.st.rt_saved[0] += r.nodes - r.inner[0];
            }
            if r.reply != PASS && !tested.contains(&r.reply) && tested.len() < 8 {
                tested.push(r.reply);
                if let Some(z) = test(&mut self.st, r.reply) {
                    zones.push(z);
                }
            }
        }
        // oracle: every distinct reply tested first (up to 16 by frequency)
        let mut by_freq = replies.clone();
        by_freq.sort_unstable_by(|x, y| y.1.cmp(&x.1));
        let mut zones2: Vec<(BB, BB, bool, i32)> = Vec::new();
        for e in by_freq.iter().take(16) {
            if let Some(i) = tested.iter().position(|&x| x == e.0) {
                let _ = i;
            }
            let p = &t.placements[e.0 as usize];
            if pos.left[opp] >> p.piece & 1 == 0 || !pos.legal_placement(t, opp, p) {
                continue;
            }
            let mut child = *pos;
            child.place_for(t, opp, e.0);
            let target = child.longest[opp] as i32;
            let mut work = budget;
            let mut value = 2;
            if !can_reach_alone(t, &child, me, target, &mut work) {
                value = -1;
            } else if alpha >= 0 {
                let mut w2 = budget;
                if !can_reach_alone(t, &child, me, target + 1, &mut w2) {
                    value = 0;
                }
            }
            if value <= alpha {
                zones2.push((p.cells, p.border, p.border & pos.cells[opp] != 0, value));
            }
        }
        for r in recs {
            if zones2.iter().any(|z| covered(z, r.m)) {
                self.st.rt[level][5] += 1;
                self.st.rt[level][6] += r.nodes;
                self.st.rt_saved[1] += r.nodes - r.inner[1];
            }
        }
    }

    /// True when the table now decides the node at `ply` of the current path for its window.
    #[inline]
    fn resolved(&self, ply: usize) -> bool {
        let (hash, alpha, beta) = self.path[ply];
        match self.probe(hash) {
            Some(e) => e.lo >= e.hi || e.lo as i32 >= beta || e.hi as i32 <= alpha,
            None => false,
        }
    }

    /// After an abort below this node. While the unwinding goes on the value is ignored. If it ends
    /// here (another thread resolved this node while we were below it), the table has the answer.
    fn after_abort<const PAR: bool>(&mut self, pos: &Pos, ply: usize, alpha: i32, beta: i32) -> i32 {
        if !PAR || self.unwind_to != ply {
            return 0;
        }
        self.aborted = false;
        self.unwind_to = NO_UNWIND;
        if let Some(e) = self.probe(pos.hash) {
            let (lo, hi) = (e.lo as i32, e.hi as i32);
            if lo >= hi || lo >= beta {
                return lo;
            }
            if hi <= alpha {
                return hi;
            }
        }
        self.search::<PAR>(pos, alpha, beta) // the entry was replaced meanwhile
    }

    /// The value of a position within the window (alpha, beta): exact inside it, a bound outside.
    /// Compiled twice: PAR = false is the plain search, PAR = true adds what a parallel solve needs,
    /// so that one thread pays nothing for it.
    fn search<const PAR: bool>(&mut self, pos: &Pos, mut alpha: i32, mut beta: i32) -> i32 {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return 0;
        }
        let ply = (pos.ply as usize).min(MAX_PLY - 2);
        let (alpha_in, beta_in) = (alpha, beta);
        if PAR {
            // a parallel solve: remember the path, and now and then look whether another thread has
            // resolved a node we are below; if so, unwind to it
            self.path[ply] = (pos.hash, alpha, beta);
            if self.nodes & 255 == 0 {
                if let Some(stop) = &self.stop {
                    if stop.load(Relaxed) {
                        self.aborted = true;
                        return 0;
                    }
                }
                for p in self.root_ply..ply {
                    if self.resolved(p) {
                        self.aborted = true;
                        self.unwind_to = p;
                        return 0;
                    }
                }
            }
        }
        let level = ((pos.left[0].count_ones() + pos.left[1].count_ones()) as usize).min(15);
        self.by_left[level] += 1;
        self.ret_kind = 0;
        self.ret_mv = PASS;
        if pos.is_over() {
            return game_result(pos);
        }
        let nodes_at_start = self.nodes;
        let t = self.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;

        // 0. the last pieces: the exact value without a search
        if self.cfg.last_pieces && pos.left[me].count_ones() <= 1 && pos.left[opp].count_ones() <= 1 {
            return self.last_pieces(pos);
        }

        // 1. what the table knows: the value lies in [lo, hi]
        let (mut lo, mut hi) = (-1, 1);
        let mut tt_move = PASS;
        let use_tt = level as u32 > self.cfg.tt_skip;
        if !use_tt {
        } else if let Some(e) = self.probe(pos.hash) {
            lo = e.lo as i32;
            hi = e.hi as i32;
            tt_move = e.mv;
            if lo >= hi || lo >= beta {
                self.prof[level][0] += 1;
                self.ret_kind = 1;
                self.ret_mv = e.mv;
                return lo;
            }
            if hi <= alpha {
                self.prof[level][0] += 1;
                self.ret_kind = 1;
                self.ret_mv = e.mv;
                return hi;
            }
        }

        // 2. the snake-length bounds
        let mut my_clusters = Clusters::new();
        let mut have_clusters = false;
        let reach_on = self.cfg.reach && self.cfg.bounds;
        let test_me = reach_on && pos.left[me].count_ones() <= self.cfg.reach_hand;
        let test_them = reach_on && pos.left[opp].count_ones() <= self.cfg.reach_hand;
        let lazy = self.cfg.lazy && (test_me || test_them);
        if lazy {
            // 2'. Near the end the exact question comes first: can each side still reach the length
            // it needs, even unopposed? A side that has this test needs no cluster bound.
            let mine_now = pos.longest[me] as i32;
            let theirs_now = pos.longest[opp] as i32;
            let (a, b) = (alpha.max(lo), beta.min(hi));
            if test_me {
                // to score above a the mover must end at least this long
                let need = theirs_now + if a >= 0 { 1 } else { 0 };
                if mine_now < need {
                    let mut work = self.cfg.reach_work;
                    self.reach_by[0][level][0] += 1;
                    if !can_reach_touching(t, pos, me, need, &mut work) {
                        self.reach_by[0][level][1] += 1;
                        self.store_if(use_tt, pos.hash, lo, a, PASS, 1);
                        return a;
                    }
                }
            }
            if test_them {
                // to hold the mover below b the opponent must end at least this long
                let need = mine_now + if b <= 0 { 1 } else { 0 };
                if theirs_now < need {
                    let mut work = self.cfg.reach_work;
                    self.reach_by[1][level][0] += 1;
                    if !can_reach_touching(t, pos, opp, need, &mut work) {
                        self.reach_by[1][level][1] += 1;
                        self.store_if(use_tt, pos.hash, b, hi, PASS, 1);
                        return b;
                    }
                }
            }
            if !test_me {
                let upper = upper_bound_ex(t, pos, me, 0, Some(&mut my_clusters));
                have_clusters = true;
                hi = hi.min(if upper < theirs_now {
                    -1
                } else if upper <= theirs_now {
                    0
                } else {
                    1
                });
            }
            if !test_them {
                let upper = upper_bound(t, pos, opp);
                lo = lo.max(if mine_now > upper {
                    1
                } else if mine_now >= upper {
                    0
                } else {
                    -1
                });
            }
            if lo >= hi || lo >= beta {
                self.prof[level][1] += 1;
                return lo;
            }
            if hi <= alpha {
                self.prof[level][1] += 1;
                return hi;
            }
            alpha = alpha.max(lo);
            beta = beta.min(hi);
            // a cluster bound may have moved the window: then the question is stricter
            if test_me && alpha >= 0 && a < 0 && mine_now < theirs_now + 1 {
                let mut work = self.cfg.reach_work;
                self.reach_by[0][level][0] += 1;
                if !can_reach_touching(t, pos, me, theirs_now + 1, &mut work) {
                    self.reach_by[0][level][1] += 1;
                    self.store_if(use_tt, pos.hash, lo, alpha, PASS, 1);
                    return alpha;
                }
            }
            if test_them && beta <= 0 && b > 0 && theirs_now < mine_now + 1 {
                let mut work = self.cfg.reach_work;
                self.reach_by[1][level][0] += 1;
                if !can_reach_touching(t, pos, opp, mine_now + 1, &mut work) {
                    self.reach_by[1][level][1] += 1;
                    self.store_if(use_tt, pos.hash, beta, hi, PASS, 1);
                    return beta;
                }
            }
        }
        let mut their_clusters = Clusters::new();
        if self.cfg.bounds && !lazy {
            have_clusters = true;
            let mine = Bounds { lower: pos.longest[me] as i32, upper: upper_bound_ex(t, pos, me, 0, Some(&mut my_clusters)) };
            let rec = if test_them { Some(&mut their_clusters) } else { None };
            let theirs = Bounds { lower: pos.longest[opp] as i32, upper: upper_bound_ex(t, pos, opp, 0, rec) };
            let blo = if mine.lower > theirs.upper {
                1
            } else if mine.lower >= theirs.upper {
                0
            } else {
                -1
            };
            let bhi = if mine.upper < theirs.lower {
                -1
            } else if mine.upper <= theirs.lower {
                0
            } else {
                1
            };
            lo = lo.max(blo);
            hi = hi.min(bhi);
            if lo >= hi || lo >= beta {
                self.prof[level][1] += 1;
                return lo;
            }
            if hi <= alpha {
                self.prof[level][1] += 1;
                return hi;
            }
        }
        alpha = alpha.max(lo);
        beta = beta.min(hi);

        // 2b. near the end: can each side still reach the length it needs, even unopposed?
        if !lazy && (test_me || test_them) {
            let mine_now = pos.longest[me] as i32;
            let theirs_now = pos.longest[opp] as i32;
            let total = (pos.left[me].count_ones() + pos.left[opp].count_ones()) as usize;
            // to score above alpha the mover must end at least this long
            let need = theirs_now + if alpha >= 0 { 1 } else { 0 };
            if test_me && mine_now < need {
                let mut work = self.cfg.reach_work;
                self.reach_by[0][total][0] += 1;
                if !can_reach(t, pos, me, need, &my_clusters, &mut work) {
                    self.reach_by[0][total][1] += 1;
                    self.store_if(use_tt, pos.hash, lo, alpha, PASS, 1);
                    return alpha;
                }
            }
            // to hold the mover below beta the opponent must end at least this long
            let need = mine_now + if beta <= 0 { 1 } else { 0 };
            if test_them && theirs_now < need {
                let mut work = self.cfg.reach_work;
                self.reach_by[1][total][0] += 1;
                if !can_reach(t, pos, opp, need, &their_clusters, &mut work) {
                    self.reach_by[1][total][1] += 1;
                    self.store_if(use_tt, pos.hash, beta, hi, PASS, 1);
                    return beta;
                }
            }
        }
        let (alpha0, beta0) = (alpha, beta);

        // Study: would an exact one-player bound, or a separation of the two sides, decide this node?
        let mut study_mark = 0u8;
        let (s_opt0, s_sep0) = (self.st.opt_saved, self.st.sep_saved);
        if self.cfg.study && (4..=11).contains(&level) {
            study_mark = self.study_node(pos, level, alpha, beta, test_me, test_them);
        }
        // the study's loop-start snapshots (waste)
        let (bl0, w0) = if self.cfg.study { (self.by_left, self.st.waste) } else { ([0; 16], [[0; 16]; 2]) };
        let (mut cur_bl, mut cur_w) = (bl0, w0);
        let mut recs = if self.cfg.study { std::mem::take(&mut self.recs[ply]) } else { Vec::new() };
        recs.clear();

        // 3. threats: the opponent's refutations one ply down are the natural candidates
        let threats_on = self.cfg.threats && self.cfg.bounds && pos.left[opp] != 0;
        let mut th = Threats::new();
        if threats_on {
            self.try_threat(pos, self.killers[ply + 1][0], alpha, false, &mut th);
            self.try_threat(pos, self.killers[ply + 1][1], alpha, false, &mut th);
        }
        let mut skipped = -2; // the best bound of the moves skipped because of threats
        let mut scanned = !threats_on;

        // 4a. The first move of the order is the table move, else a killer, when one of them is
        // legal here and no threat rules it out. Three in four nodes cut on it, so it is searched
        // before the list is built.
        let (k1, k2) = (self.killers[ply][0], self.killers[ply][1]);
        let mut pre: Option<(u16, i32)> = None;
        // In a parallel solve: a move another thread is searching is set aside and taken up again
        // after the others, when its answer is probably in the table.
        // A plain pointer, not a clone of the Arc: a clone would have every thread write the same
        // reference count at every node. The Arc is owned by self and outlives this call.
        let busy: *const Busy = match &self.busy {
            Some(b) if PAR && level as u32 >= self.cfg.par_level && ply - self.root_ply.min(ply) >= self.cfg.par_join => Arc::as_ptr(b),
            _ => std::ptr::null(),
        };
        if self.cfg.staged {
            let now = pos.pieces_now();
            let mut first = PASS;
            for cand in [tt_move, k1, k2] {
                if cand == PASS {
                    continue;
                }
                let p = &t.placements[cand as usize];
                if now >> p.piece & 1 == 0 || !pos.legal_placement(t, me, p) {
                    continue;
                }
                if th.n > 0 && th.bound_for(p.cells) <= alpha {
                    continue;
                }
                first = cand;
                break;
            }
            if first != PASS {
                let move_key = pos.hash ^ (first as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                if !busy.is_null() {
                    unsafe { &*busy }.start(move_key);
                }
                let mut child = *pos;
                child.play(t, first);
                if self.cfg.prefetch {
                    self.prefetch(child.hash);
                }
                let (n_before, rt_before) = (self.nodes, self.st.rt_saved);
                let v = -self.search::<PAR>(&child, -beta, -alpha);
                if self.cfg.study {
                    let inner = [self.st.rt_saved[0] - rt_before[0], self.st.rt_saved[1] - rt_before[1]];
                    recs.push(Rec { m: first, kind: self.ret_kind, reply: self.ret_mv, nodes: self.nodes - n_before, inner });
                }
                if !busy.is_null() {
                    unsafe { &*busy }.finish(move_key);
                }
                if self.aborted {
                    return self.after_abort::<PAR>(pos, ply, alpha_in, beta_in);
                }
                if v >= beta {
                    if self.cfg.study {
                        self.study_close(study_mark, s_opt0, s_sep0, nodes_at_start);
                        self.recs[ply] = recs;
                        self.ret_kind = 3;
                        self.ret_mv = first;
                    }
                    if self.killers[ply][0] != first {
                        self.killers[ply][1] = self.killers[ply][0];
                        self.killers[ply][0] = first;
                    }
                    self.prof[level][2] += 1;
                    self.prof[level][3] += 1;
                    self.prof[level][4] += 1;
                    self.store_if(use_tt, pos.hash, lo.max(v), hi, first, self.nodes - nodes_at_start + 1);
                    return v;
                }
                pre = Some((first, v));
            }
        }

        // 4. generate and score the moves
        let mut buf = std::mem::take(&mut self.bufs[ply]);
        buf.clear();
        {
            let own = pos.cells[me];
            let theirs = pos.cells[opp];
            let front = neighbours(shape_of(theirs).ends) & pos.empty();
            let use_model = level as u32 >= self.cfg.model_level;
            let w = self.cfg.model;
            let empty = pos.empty();
            let their_shape = shape_of(theirs);
            let their_free = neighbours(their_shape.ends) & empty & !neighbours(their_shape.body);
            pos.for_each_move(t, me, pos.pieces_now(), |m| {
                let p = &t.placements[m as usize];
                if th.n > 0 {
                    let bound = th.bound_for(p.cells);
                    if bound <= alpha {
                        skipped = skipped.max(bound);
                        return;
                    }
                }
                let mut score = p.n as u64 * 64;
                if p.border & own != 0 {
                    score += 256;
                }
                if p.cells & front != 0 {
                    score += 128;
                } else if p.border & theirs != 0 {
                    score += 32;
                }
                if use_model {
                    let mut joined = p.n as i32;
                    for j in 0..pos.nsnakes[me] as usize {
                        if pos.snakes[me][j] & p.border != 0 {
                            joined += count(pos.snakes[me][j]) as i32;
                        }
                    }
                    let after = joined.max(pos.longest[me] as i32);
                    let mine = shape_of(own | p.cells);
                    let my_ends = count(neighbours(mine.ends) & empty & !p.cells & !neighbours(mine.body)) as i32;
                    let their_ends = count(their_free & !p.cells) as i32;
                    let mut s = w[0] * (after - pos.longest[opp] as i32)
                        + w[1] * p.n as i32
                        + w[2] * (p.border & own != 0) as i32
                        + w[3] * joined
                        + w[4] * (p.cells & front != 0) as i32
                        + w[5] * (p.border & theirs != 0) as i32
                        + w[6] * my_ends
                        + w[7] * their_ends;
                    if w[8] != 0 || w[9] != 0 || w[10] != 0 || w[11] != 0 {
                        // one placement ahead for each side, and both bounds, in the position after the move
                        let mut child = *pos;
                        child.play(t, m);
                        let mut their_best = child.longest[opp] as u32;
                        child.for_each_move(t, opp, child.left[opp], |r| their_best = their_best.max(child.longest_after(opp, &t.placements[r as usize])));
                        let mut my_next = child.longest[me] as u32;
                        child.for_each_move(t, me, child.left[me], |r| my_next = my_next.max(child.longest_after(me, &t.placements[r as usize])));
                        s += w[8] * their_best as i32 + w[9] * my_next as i32 + w[10] * upper_bound(t, &child, opp) + w[11] * upper_bound(t, &child, me);
                    }
                    score = (s + (1 << 20)).max(0) as u64;
                }
                if m == tt_move {
                    score = TT_MOVE;
                } else if m == k1 {
                    score = KILLER1;
                } else if m == k2 {
                    score = KILLER2;
                }
                buf.push(score << 16 | m as u64);
            });
        }

        // 5. search them, best-scored first
        self.prof[level][2] += 1;
        let mut searched = 0u64;
        let mut best = -2;
        let mut best_move = PASS;
        let mut answered = PASS;
        if buf.is_empty() && skipped == -2 {
            let mut child = *pos;
            child.play(t, PASS);
            best = -self.search::<PAR>(&child, -beta, -alpha);
        } else {
            let n = buf.len();
            let mut deferred = if !busy.is_null() { std::mem::take(&mut self.defer_bufs[ply]) } else { Vec::new() };
            deferred.clear();
            let mut d = 0usize;
            let mut i = 0; // the number of listed moves taken so far
            loop {
                let (m, first_pass) = if i < n {
                    if i < 3 {
                        let mut b = i;
                        for j in i + 1..n {
                            if buf[j] > buf[b] {
                                b = j;
                            }
                        }
                        buf.swap(i, b);
                    } else if i == 3 {
                        buf[3..].sort_unstable_by(|a, b| b.cmp(a));
                    }
                    i += 1;
                    ((buf[i - 1] & 0xFFFF) as u16, true)
                } else if d < deferred.len() {
                    d += 1;
                    (deferred[d - 1], false)
                } else {
                    break;
                };
                let v;
                if let (1, true, Some((first, value))) = (i, first_pass, pre) {
                    // searched before the list was built
                    assert!(m == first, "the placement check and the generator disagree");
                    searched += 1;
                    v = value;
                } else {
                    if th.n > 0 {
                        let bound = th.bound_for(t.placements[m as usize].cells);
                        if bound <= alpha {
                            skipped = skipped.max(bound);
                            continue;
                        }
                    }
                    let move_key = pos.hash ^ (m as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    if !busy.is_null() {
                        let busy = unsafe { &*busy };
                        if searched > 0 && self.resolved(ply) {
                            // another thread finished this node while we were searching its moves
                            self.aborted = true;
                            self.unwind_to = ply;
                            break;
                        }
                        if first_pass && searched > 0 && deferred.len() < MAX_DEFERRED && busy.is_busy(move_key) {
                            deferred.push(m);
                            continue;
                        }
                        busy.start(move_key);
                    }
                    let mut child = *pos;
                    child.play(t, m);
                    if self.cfg.prefetch {
                        self.prefetch(child.hash);
                        if i >= 3 && i + 1 < n {
                            // the next move in the order: its entry loads while this one is searched
                            let next = (buf[i + 1] & 0xFFFF) as u16;
                            self.prefetch(self.hash_after(pos, next));
                        }
                    }
                    searched += 1;
                    if self.cfg.study {
                        cur_bl = self.by_left;
                        cur_w = self.st.waste;
                    }
                    let (n_before, rt_before) = (self.nodes, self.st.rt_saved);
                    v = -self.search::<PAR>(&child, -beta, -alpha);
                    if self.cfg.study {
                        let inner = [self.st.rt_saved[0] - rt_before[0], self.st.rt_saved[1] - rt_before[1]];
                        recs.push(Rec { m, kind: self.ret_kind, reply: self.ret_mv, nodes: self.nodes - n_before, inner });
                    }
                    if !busy.is_null() {
                        unsafe { &*busy }.finish(move_key);
                    }
                    if self.aborted {
                        break;
                    }
                }
                if v > best {
                    best = v;
                    best_move = m;
                }
                if v > alpha {
                    alpha = v;
                }
                if alpha >= beta {
                    if self.killers[ply][0] != m {
                        self.killers[ply][1] = self.killers[ply][0];
                        self.killers[ply][0] = m;
                    }
                    if searched == 1 {
                        self.prof[level][4] += 1;
                    }
                    if self.cfg.study && searched > 1 {
                        // the moves before this one failed: their nodes are waste, attributed here
                        for s in 0..2 {
                            for l in 0..16 {
                                self.st.waste[s][l] -= cur_w[s][l] - w0[s][l];
                            }
                        }
                        for l in 0..16 {
                            self.st.waste[me][l] += cur_bl[l] - bl0[l];
                        }
                    }
                    break;
                }
                // The reply that refuted this move (the first killer one ply down) will meet most
                // moves that leave it alone: the moves that cover one of its cells go first.
                if self.cfg.answer_bonus > 0 && first_pass && i + 1 < n {
                    let k = self.killers[ply + 1][0];
                    if k != PASS && k != answered {
                        answered = k;
                        let reply = t.placements[k as usize].cells;
                        let bonus = self.cfg.answer_bonus << 16;
                        for e in buf[i..].iter_mut() {
                            if t.placements[(*e & 0xFFFF) as usize].cells & reply != 0 {
                                *e += bonus;
                            }
                        }
                        if i > 3 {
                            buf[i..].sort_unstable_by(|a, b| b.cmp(a));
                        }
                    }
                }
                // The refutation just found may be a threat against the remaining moves. And once the
                // first move has failed this is probably a node where every move must be refuted, so
                // look for all of the opponent's threats.
                if threats_on && first_pass && i + 1 < n {
                    self.try_threat(pos, self.killers[ply + 1][0], alpha, true, &mut th);
                    if !scanned {
                        scanned = true;
                        self.prof[level][7] += 1;
                        if !have_clusters {
                            have_clusters = true;
                            upper_bound_ex(t, pos, me, 0, Some(&mut my_clusters));
                        }
                        self.scan_threats(pos, &my_clusters, alpha, &mut th);
                    }
                }
            }
            if !busy.is_null() {
                self.defer_bufs[ply] = deferred;
            }
            best = best.max(skipped);
            self.prof[level][3] += searched;
            self.prof[level][6] += (n as u64).saturating_sub(searched);
            if alpha < beta {
                self.prof[level][5] += 1;
            }
        }
        self.bufs[ply] = buf;
        if self.aborted {
            return self.after_abort::<PAR>(pos, ply, alpha_in, beta_in);
        }

        // 6. store what we learned
        let (mut nlo, mut nhi) = (lo, hi);
        if best <= alpha0 {
            nhi = nhi.min(best);
        } else if best >= beta0 {
            nlo = nlo.max(best);
        } else {
            nlo = best;
            nhi = best;
        }
        debug_assert!(nlo <= nhi);
        if self.cfg.study {
            let cut = best >= beta0;
            if !cut && !recs.is_empty() && level >= 4 {
                self.study_all_node(pos, level, alpha0, &recs);
            }
            self.study_close(study_mark, s_opt0, s_sep0, nodes_at_start);
            self.recs[ply] = recs;
            if self.recs[ply].is_empty() {
                self.ret_kind = 6;
                self.ret_mv = PASS;
            } else if cut {
                self.ret_kind = if self.recs[ply].len() == 1 { 3 } else { 4 };
                self.ret_mv = best_move;
            } else {
                self.ret_kind = 5;
                self.ret_mv = PASS;
            }
        }
        if best_move == PASS {
            best_move = tt_move;
        }
        self.store_if(use_tt, pos.hash, nlo, nhi, best_move, self.nodes - nodes_at_start + 1);
        best
    }
}
