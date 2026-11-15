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
    /// Answer a reach test from a sequence an earlier test found (this node's or the parent's), when
    /// it still applies.
    pub witness: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { bounds: true, last_pieces: true, threats: true, reach: true, reach_hand: 2, reach_work: 200, model_level: 7, model: MODEL, par_level: 5, par_join: 2, tt_bits: 22, staged: true, lazy: true, narrow: true, answer_bonus: 1024, prefetch: false, tt_skip: 0, witness: true }
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
}

impl Threats {
    pub(crate) fn new() -> Threats {
        Threats { n: 0, zone: [0; MAX_THREATS], value: [0; MAX_THREATS], tried: [PASS; MAX_TRIED], ntried: 0 }
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
    /// Reach tests answered by a witness instead of a search.
    pub reach_reused: u64,
    /// The last sequence a reach test found, by ply and player.
    wit: [[Witness; 2]; MAX_PLY],
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
            reach_reused: 0,
            wit: [[Witness::NONE; 2]; MAX_PLY],
            node_limit: u64::MAX,
            aborted: false,
            root_ply: 0,
            path: vec![(0, 0, 0); MAX_PLY + 2],
            unwind_to: NO_UNWIND,
            killers: [[PASS; 2]; MAX_PLY],
            bufs: (0..MAX_PLY).map(|_| Vec::with_capacity(1024)).collect(),
            defer_bufs: (0..MAX_PLY).map(|_| Vec::new()).collect(),
            replies: Vec::with_capacity(1024),
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
    fn try_threat(&self, pos: &Pos, m: u16, th: &mut Threats) {
        if m == PASS || th.n >= MAX_THREATS || th.ntried >= MAX_TRIED || th.tried[..th.ntried].contains(&m) {
            return;
        }
        th.tried[th.ntried] = m;
        th.ntried += 1;
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
            return;
        };
        th.zone[th.n] = threat_zone(pos, opp, b);
        th.value[th.n] = value;
        th.n += 1;
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
                    // only whether it is at most `need` matters: the flood may stop once it is above
                    upper = upper.max(upper_bound_cells_upto(t, &ctx, covered & !b.cells, need));
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

    /// Can `player` still build a snake of `need` cells (the exact test, `can_reach_touching`)? A
    /// sequence found by an earlier test at this ply or the one above answers yes when it still applies.
    #[inline]
    fn reach(&mut self, pos: &Pos, ply: usize, player: usize, need: i32) -> bool {
        if self.cfg.witness {
            if self.wit[ply][player].proves(pos, player, need) || (ply > 0 && self.wit[ply - 1][player].proves(pos, player, need)) {
                self.reach_reused += 1;
                return true;
            }
        }
        let mut work = self.cfg.reach_work;
        can_reach_touching_w(self.t, pos, player, need, &mut work, &mut self.wit[ply][player])
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
                return lo;
            }
            if hi <= alpha {
                self.prof[level][0] += 1;
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
                    self.reach_by[0][level][0] += 1;
                    if !self.reach(pos, ply, me, need) {
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
                    self.reach_by[1][level][0] += 1;
                    if !self.reach(pos, ply, opp, need) {
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
                self.reach_by[0][level][0] += 1;
                if !self.reach(pos, ply, me, theirs_now + 1) {
                    self.reach_by[0][level][1] += 1;
                    self.store_if(use_tt, pos.hash, lo, alpha, PASS, 1);
                    return alpha;
                }
            }
            if test_them && beta <= 0 && b > 0 && theirs_now < mine_now + 1 {
                self.reach_by[1][level][0] += 1;
                if !self.reach(pos, ply, opp, mine_now + 1) {
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

        // 3. threats: the opponent's refutations one ply down are the natural candidates
        let threats_on = self.cfg.threats && self.cfg.bounds && pos.left[opp] != 0;
        let mut th = Threats::new();
        if threats_on {
            self.try_threat(pos, self.killers[ply + 1][0], &mut th);
            self.try_threat(pos, self.killers[ply + 1][1], &mut th);
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
                let v = -self.search::<PAR>(&child, -beta, -alpha);
                if !busy.is_null() {
                    unsafe { &*busy }.finish(move_key);
                }
                if self.aborted {
                    return self.after_abort::<PAR>(pos, ply, alpha_in, beta_in);
                }
                if v >= beta {
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
                    v = -self.search::<PAR>(&child, -beta, -alpha);
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
                    self.try_threat(pos, self.killers[ply + 1][0], &mut th);
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
        if best_move == PASS {
            best_move = tt_move;
        }
        self.store_if(use_tt, pos.hash, nlo, nhi, best_move, self.nodes - nodes_at_start + 1);
        best
    }
}
