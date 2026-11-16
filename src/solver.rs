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
    /// E30, a study: count what the winning move of each cut node cost (`--cost-stats`).
    pub cost_stats: bool,
    /// E30: at a cut node with this many pieces left, where the first move is a killer and the other
    /// killer is a candidate too, search both and record their costs (a study; changes the tree). 0 = off.
    pub oracle_level: u32,
    /// E30 idea 1: try the second killer first when its recorded cost times this is below the first's
    /// (0 = off), at nodes with at least `kc_level` pieces left.
    pub kc_order: u64,
    pub kc_level: u32,
    /// E30 idea 1: how a killer's recorded cost is kept when it cuts again: 0 the last, 1 a running
    /// mean, 2 the minimum.
    pub kc_mode: u8,
    /// E30 idea 1: a new killer replaces the dearer of the two (not the older).
    pub kc_evict: bool,
    /// E30 idea 2: among the first `hc_k` listed candidates (after the table move) at nodes with at
    /// least `hc_level` pieces left, the move with the lowest mean cost of its recorded wins goes first
    /// (0 = off). Needs at least `hc_min` wins recorded.
    pub hc_k: u32,
    pub hc_level: u32,
    pub hc_min: u32,
    /// E30 idea 2: keep the staged first move (table move, then the killer that the history prefers).
    pub hc_staged: bool,
    /// Reach threats: after a failed move, the reply that refuted it is a threat when, with it
    /// placed, the mover can no longer build a snake as long as it (the mover alone, the opponent
    /// frozen). Zone: the reply's cells and border. Tested at nodes with between `reach_threat_min`
    /// and `reach_threat_max` pieces left (both hands).
    pub reach_threat: bool,
    pub reach_threat_min: u32,
    pub reach_threat_max: u32,
    /// The positions a reach-threat test may look at (when they run out, no threat).
    pub reach_threat_work: u32,
    /// A reach threat that grows an opponent snake also prunes the moves that touch it without
    /// covering it, when they grow one of the mover's snakes (the two then commute).
    pub reach_threat_grow: bool,
    /// Test reach threats only once this many moves have failed at the node.
    pub reach_threat_after: u64,
    /// Also test the killer replies as reach threats at the start of a node with at most this many
    /// pieces left (0 = never).
    pub reach_threat_start: u32,
    /// Study counters for reach-threat tests (`--rt-study`; costs time).
    pub rt_study: bool,
    /// Keep the "cannot reach" answers of reach-threat tests and reuse them where they still hold.
    pub reach_threat_cache: bool,
    /// Above `reach_threat_max` pieces left, a reach threat is still tested after a failed move once
    /// the node has spent at least this many nodes (by pieces left, 7 = 7 or more; MAX = never).
    pub rt_effort: [u64; 8],
    /// Skip a reach-threat test when the cluster bound after the reply is more than this above the
    /// reply's length (at most `reach_threat_max` pieces left / above it; 99 = no limit).
    pub rt_margin_lo: i32,
    pub rt_margin_hi: i32,
    /// After a failed move, which reply is taken as the one that refuted it: 0 the first killer one
    /// ply down (as before; with the cost order it need not be the reply that just cut), 1 the move
    /// that cut the child (the killer when the child did not cut by a move) for the reach-threat
    /// test, 2 that also for the answer bonus.
    pub rt_refuter: u8,
    /// The proof driver, for exact node counts: one worker thread that keeps this many solvers and
    /// hands them the jobs in turn, with this many jobs out at once (`--emulate W`). 0 = off.
    pub drv_emulate: u32,
}

/// A "cannot reach" answer: the player with the cells `own` and the pieces `left` cannot build
/// `need` cells while the opponent holds the cells `opp` (only those within reach are kept).
#[derive(Clone, Copy)]
struct NoReach {
    own: BB,
    opp: BB,
    left: u8,
    need: i32,
}

const NO_REACH_BITS: u32 = 13;

/// Reach-threat study counters, by pieces left at the node.
pub struct RtStudy {
    /// [answer: 0 none, 1 threat] -> [tests, reach positions, cycles]
    pub by_ans: [[[u64; 3]; 2]; 16],
    /// [hand of the mover 0..3][margin bucket 0..7] -> [tests, threats, positions]
    pub margin: [[[[u64; 3]; 8]; 4]; 16],
    /// [moves left outside the zone, bucket 0..6] -> [tests, threats, positions]
    pub outside: [[[u64; 3]; 7]; 16],
    /// [moves skipped only by reach threats, estimated nodes under them, nodes with such skips,
    ///  moves searched at those nodes, nodes under those moves]
    pub skip: [[u64; 5]; 16],
    pub work: u64,
    pub outside_now: u32,
    /// [effort bucket: nodes spent at the node when the test is asked, <16, <64, <256, <1024,
    /// <4096, more] -> [tests, threats, cycles, moves skipped only by reach threats, estimated nodes
    /// under them] (skips go to the bucket of the node's first test)
    pub eff: [[[u64; 5]; 6]; 16],
}

pub fn effort_bucket(e: u64) -> usize {
    match e {
        0..=15 => 0,
        16..=63 => 1,
        64..=255 => 2,
        256..=1023 => 3,
        1024..=4095 => 4,
        _ => 5,
    }
}

impl RtStudy {
    pub fn new() -> RtStudy {
        RtStudy { by_ans: [[[0; 3]; 2]; 16], margin: [[[[0; 3]; 8]; 4]; 16], outside: [[[0; 3]; 7]; 16], skip: [[0; 5]; 16], work: 0, outside_now: 0, eff: [[[0; 5]; 6]; 16] }
    }
}

pub fn outside_bucket(n: u32) -> usize {
    match n {
        0 => 0,
        1..=2 => 1,
        3..=5 => 2,
        6..=10 => 3,
        11..=20 => 4,
        21..=40 => 5,
        _ => 6,
    }
}

impl Default for Config {
    fn default() -> Self {
        Config { bounds: true, last_pieces: true, threats: true, reach: true, reach_hand: 2, reach_work: 200, model_level: 7, model: MODEL, par_level: 5, par_join: 2, tt_bits: 22, staged: true, lazy: true, narrow: true, answer_bonus: 1024, prefetch: true, tt_skip: 0, witness: true, cost_stats: false, oracle_level: 0, kc_order: 1, kc_level: 0, kc_mode: 1, kc_evict: true, hc_k: 4, hc_level: 5, hc_min: 1, hc_staged: true, reach_threat: true, reach_threat_min: 0, reach_threat_max: 99, reach_threat_work: 2000, reach_threat_grow: true, reach_threat_after: 1, reach_threat_start: 99, rt_study: false, reach_threat_cache: false, rt_effort: [u64::MAX; 8], rt_margin_lo: 99, rt_margin_hi: 99, rt_refuter: 0, drv_emulate: 0 }
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
    /// The zone for a move that grows one of the mover's snakes (a subset of `zone`).
    zone_grow: [BB; MAX_THREATS],
    /// Whether the threat is a reach threat (study counters only).
    reach: [bool; MAX_THREATS],
    value: [i32; MAX_THREATS],
    tried: [u16; MAX_TRIED],
    ntried: usize,
    /// The replies tested as reach threats.
    rtried: [u16; MAX_TRIED],
    nrtried: usize,
}

impl Threats {
    pub(crate) fn new() -> Threats {
        Threats { n: 0, zone: [0; MAX_THREATS], zone_grow: [0; MAX_THREATS], reach: [false; MAX_THREATS], value: [0; MAX_THREATS], tried: [PASS; MAX_TRIED], ntried: 0, rtried: [PASS; MAX_TRIED], nrtried: 0 }
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
    /// `bound_for` for a move with the cells `cells` that grows one of the mover's snakes (`grows`)
    /// or starts a new one.
    #[inline]
    pub(crate) fn bound_for_move(&self, cells: BB, grows: bool) -> i32 {
        let mut b = NO_BOUND;
        for i in 0..self.n {
            let z = if grows { self.zone_grow[i] } else { self.zone[i] };
            if cells & z == 0 && self.value[i] < b {
                b = self.value[i];
            }
        }
        b
    }
    /// `bound_for_move` without the reach threats (study counters only).
    fn bound_for_move_plain(&self, cells: BB, grows: bool) -> i32 {
        let mut b = NO_BOUND;
        for i in 0..self.n {
            let z = if grows { self.zone_grow[i] } else { self.zone[i] };
            if !self.reach[i] && cells & z == 0 && self.value[i] < b {
                b = self.value[i];
            }
        }
        b
    }
    #[inline]
    fn push(&mut self, zone: BB, zone_grow: BB, value: i32) {
        self.reach[self.n] = false;
        self.zone[self.n] = zone;
        self.zone_grow[self.n] = zone_grow;
        self.value[self.n] = value;
        self.n += 1;
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
    /// The last sequence a reach-threat test found, by ply (for the mover at that ply).
    rwit: [Witness; MAX_PLY],
    /// Reach-threat tests: [tests, answered by a witness, threats found].
    pub rt_stats: [u64; 3],
    pub rts: Box<RtStudy>,
    /// The nodes the current node has spent when it asks for a threat test.
    rt_effort_now: u64,
    /// The move that cut the node at each ply (PASS until one does).
    refuter: [u16; MAX_PLY],
    no_reach: Vec<NoReach>,
    node_limit: u64,
    aborted: bool,
    /// Parallel solve: the ply of the root, the nodes on the path from it by ply (hash and window at
    /// entry), and the ply to unwind to when another thread has resolved a node we are below.
    root_ply: usize,
    path: Vec<(u64, i32, i32)>,
    unwind_to: usize,
    pub(crate) killers: [[u16; 2]; MAX_PLY],
    /// E30: the nodes of the subtree with which each killer last cut (0 = not known).
    kcost: [[u64; 2]; MAX_PLY],
    /// E30: the study's counters.
    pub cs: Box<CostStats>,
    /// E30 idea 2: by side to move, pieces left and move: [wins, nodes of those wins].
    hist: Vec<[u64; 2]>,
    /// The entries of `hist` written since the last `clear()`, so that clearing costs what was used.
    hist_used: Vec<u32>,
    bufs: Vec<Vec<u64>>,
    defer_bufs: Vec<Vec<u16>>,
    replies: Vec<(u32, BB)>,
}

/// E30: what the winning moves of cut nodes cost, measured in the tree.
pub struct CostStats {
    /// By side (0 the root's mover, "red"), pieces left, where the winning move came from (0 the
    /// table move, 1 the newer killer, 2 the older killer, 3 the list) and how many moves were searched
    /// before it (0, 1, 2, 3 or more): [cut nodes, nodes under the winning move, nodes under failed moves].
    pub cut: [[[[[u64; 3]; 4]; 4]; 16]; 2],
    /// A killer that cuts again: log2 of its recorded cost (x) against log2 of this cut's cost (y), by
    /// side and pieces left: [n, sx, sy, sxx, syy, sxy].
    pub stab: [[[f64; 6]; 16]; 2],
    /// The same, cost here over cost recorded, in buckets: below 1/4, 1/2, 1, 2, 4, and above.
    pub stab_b: [[[u64; 6]; 16]; 2],
    /// The oracle: [pieces left, recorded cost of the first killer, of the other, cost of the first,
    /// of the other, first wins, other wins, other searched first, first is the newer killer].
    pub oracle: Vec<[u64; 9]>,
}

impl CostStats {
    fn new() -> CostStats {
        CostStats { cut: [[[[[0; 3]; 4]; 4]; 16]; 2], stab: [[[0.0; 6]; 16]; 2], stab_b: [[[0; 6]; 16]; 2], oracle: Vec::new() }
    }
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
            rwit: [Witness::NONE; MAX_PLY],
            rt_stats: [0; 3],
            rts: Box::new(RtStudy::new()),
            rt_effort_now: 0,
            refuter: [PASS; MAX_PLY],
            no_reach: vec![NoReach { own: 0, opp: 0, left: 0, need: 0 }; 1 << NO_REACH_BITS],
            node_limit: u64::MAX,
            aborted: false,
            root_ply: 0,
            path: vec![(0, 0, 0); MAX_PLY + 2],
            unwind_to: NO_UNWIND,
            killers: [[PASS; 2]; MAX_PLY],
            kcost: [[0; 2]; MAX_PLY],
            cs: Box::new(CostStats::new()),
            hist: if cfg.hc_k > 0 { vec![[0; 2]; 2 * 16 * tables().placements.len()] } else { Vec::new() },
            hist_used: Vec::new(),
            bufs: (0..MAX_PLY).map(|_| Vec::with_capacity(1024)).collect(),
            defer_bufs: (0..MAX_PLY).map(|_| Vec::new()).collect(),
            replies: Vec::with_capacity(1024),
        }
    }

    /// Forget everything learned from earlier positions.
    pub fn clear(&mut self) {
        self.gen = self.tt.next_generation();
        self.killers = [[PASS; 2]; MAX_PLY];
        self.kcost = [[0; 2]; MAX_PLY];
        for &i in &self.hist_used {
            self.hist[i as usize] = [0; 2];
        }
        self.hist_used.clear();
    }

    /// For a thread of a parallel solve: take up the table's present generation.
    pub fn sync(&mut self) {
        self.gen = self.tt.generation();
        self.killers = [[PASS; 2]; MAX_PLY];
        self.kcost = [[0; 2]; MAX_PLY];
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
        self.kcost = [[0; 2]; MAX_PLY];
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
    /// `deep` (after a failed move, with `reach_threat`): also test it as a reach threat.
    fn try_threat(&mut self, pos: &Pos, ply: usize, m: u16, alpha: i32, deep: bool, th: &mut Threats) {
        if m == PASS || th.n >= MAX_THREATS {
            return;
        }
        let first_time = th.ntried < MAX_TRIED && !th.tried[..th.ntried].contains(&m);
        let lv = pos.left[0].count_ones() + pos.left[1].count_ones();
        let low = lv <= self.cfg.reach_threat_max;
        let deep = deep && th.nrtried < MAX_TRIED && !th.rtried[..th.nrtried].contains(&m) && lv >= self.cfg.reach_threat_min
            && (low || self.rt_effort_now >= self.cfg.rt_effort[(lv as usize).min(7)]);
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
        let opp_len = pos.longest_after(opp, b) as i32;
        // only a bound of at most opp_len makes a threat: the flood may stop once it is above
        let mlim = if low { self.cfg.rt_margin_lo } else { self.cfg.rt_margin_hi };
        let use_m = deep && mlim < 99;
        let my_upper = if pos.left[me] == 0 {
            pos.longest[me] as i32
        } else {
            upper_bound_cells_upto(t, &BoundCtx::new(pos, me), growable(pos, me) & !b.cells, opp_len + if use_m { mlim } else { 0 })
        };
        // a reply with a wide margin rarely makes a reach threat: no test (`rt_margin_*`)
        let deep = deep && (!use_m || my_upper <= opp_len + mlim);
        let value = if my_upper < opp_len {
            -1
        } else if my_upper <= opp_len {
            0
        } else {
            2
        };
        // A reach threat: with b placed, can we still build a snake as long as the opponent's? If
        // not, a move of ours that neither covers nor touches b leaves b playable, is itself playable
        // after b, and our one-player optimum after both is no more than after b alone (any sequence
        // of our placements legal after the move and b is legal after b alone, as the move followed
        // by the rest; the opponent's snakes never shrink): the move is worth at most the threat's
        // value. Zone: b's cells and border.
        if deep && pos.left[me] != 0 && (value == 2 || (value == 0 && alpha < 0)) {
            let mut child = *pos;
            child.place_for(t, opp, m);
            self.rt_stats[0] += 1;
            let study = self.cfg.rt_study;
            let (c0, w0) = if study { (unsafe { core::arch::x86_64::_rdtsc() }, self.rts.work) } else { (0, 0) };
            let rv = if !self.threat_reach(&child, ply, me, opp_len) {
                -1
            } else if value == 2 && alpha >= 0 && !self.threat_reach(&child, ply, me, opp_len + 1) {
                0
            } else {
                2
            };
            if study {
                let cyc = unsafe { core::arch::x86_64::_rdtsc() } - c0;
                let work = self.rts.work - w0;
                let lv = ((pos.left[0].count_ones() + pos.left[1].count_ones()) as usize).min(15);
                let found = (rv <= 0) as u64;
                let a = &mut self.rts.by_ans[lv][found as usize];
                a[0] += 1;
                a[1] += work;
                a[2] += cyc;
                let exact = upper_bound_cells_upto(t, &BoundCtx::new(pos, me), growable(pos, me) & !b.cells, i32::MAX);
                let mb = ((exact - opp_len).max(0) as usize).min(7);
                let hand = (pos.left[me].count_ones() as usize).min(3);
                let g = &mut self.rts.margin[lv][hand][mb];
                g[0] += 1;
                g[1] += found;
                g[2] += work;
                let e = &mut self.rts.eff[lv][effort_bucket(self.rt_effort_now)];
                e[0] += 1;
                e[1] += found;
                e[2] += cyc;
                let ob = outside_bucket(self.rts.outside_now);
                let o = &mut self.rts.outside[lv][ob];
                o[0] += 1;
                o[1] += found;
                o[2] += work;
            }
            if rv <= 0 {
                self.rt_stats[2] += 1;
                // A move m that touches b without covering it, when m grows one of our snakes and b
                // grows one of theirs: a placement that grows a snake is legal exactly when its cells
                // are empty and the player's own cells allow it, so m and b stay legal after each
                // other and commute; any sequence of ours after m and b is one after b alone (m then
                // the rest), and the argument above holds for m too.
                let zone_grow = if self.cfg.reach_threat_grow { threat_zone(pos, opp, b) } else { b.cells | b.border };
                th.push(b.cells | b.border, zone_grow, rv);
                th.reach[th.n - 1] = true;
                return;
            }
        }
        if value <= 0 && first_time {
            let z = threat_zone(pos, opp, b);
            th.push(z, z, value);
        }
    }

    /// The reach test of a reach threat: can `me` still build `need` cells in `child` (the position
    /// with the threat placed)? A witness found earlier at this ply (by a threat test, or by an
    /// ordinary reach test here or one ply up) answers yes when it still applies: the mover's cells
    /// and pieces are those of the node, and the witness checks that its cells are empty in `child`.
    #[inline]
    fn threat_reach(&mut self, child: &Pos, ply: usize, me: usize, need: i32) -> bool {
        if self.cfg.witness
            && (self.rwit[ply].proves(child, me, need) || self.wit[ply][me].proves(child, me, need) || (ply > 0 && self.wit[ply - 1][me].proves(child, me, need)))
        {
            self.rt_stats[1] += 1;
            return true;
        }
        // A "cannot reach" found before, for the same cells and pieces of ours: see `near_cells`.
        let own = child.cells[me];
        let left = child.left[me];
        let mut slot = usize::MAX;
        let mut near = 0;
        if self.cfg.reach_threat_cache {
            let rem = hand_cells(left);
            if need > rem {
                near = near_cells(own, rem);
                let h = ((own as u64) ^ ((own >> 64) as u64).rotate_left(29) ^ left as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                slot = (h >> (64 - NO_REACH_BITS)) as usize;
                let e = &self.no_reach[slot];
                if e.own == own && e.left == left && need >= e.need && e.opp & !child.cells[me ^ 1] == 0 {
                    self.rt_stats[1] += 1;
                    return false;
                }
            }
        }
        let mut work = self.cfg.reach_threat_work;
        let r = can_reach_touching_w(self.t, child, me, need, &mut work, &mut self.rwit[ply]);
        self.rts.work += (self.cfg.reach_threat_work - work) as u64;
        if !r && slot != usize::MAX {
            self.no_reach[slot] = NoReach { own, opp: child.cells[me ^ 1] & near, left, need };
        }
        r
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
                th.push(f.1, f.1, f.2);
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
    /// E30: the move `m` cut at this node after `tried` other moves; its subtree took `win` nodes, the
    /// node's whole subtree `total`. Keeps the killers (as before, or by cost) and the counters.
    #[inline]
    fn cut_by(&mut self, pos: &Pos, ply: usize, level: usize, m: u16, tt_move: u16, tried: u64, win: u64, total: u64) {
        self.refuter[ply] = m;
        let k = self.killers[ply];
        let c = self.kcost[ply];
        if self.cfg.cost_stats {
            let side = (ply - self.root_ply.min(ply)) & 1;
            let src = if m == tt_move { 0 } else if m == k[0] { 1 } else if m == k[1] { 2 } else { 3 };
            let e = &mut self.cs.cut[side][level][src][(tried as usize).min(3)];
            e[0] += 1;
            e[1] += win;
            e[2] += total.saturating_sub(1 + win);
            let j = if m == k[0] { 0 } else if m == k[1] { 1 } else { 2 };
            if j < 2 && c[j] > 0 {
                let x = (c[j] as f64).log2();
                let y = (win as f64).log2();
                let s = &mut self.cs.stab[side][level];
                s[0] += 1.0;
                s[1] += x;
                s[2] += y;
                s[3] += x * x;
                s[4] += y * y;
                s[5] += x * y;
                let r = y - x;
                let b = if r < -2.0 { 0 } else if r < -1.0 { 1 } else if r < 0.0 { 2 } else if r < 1.0 { 3 } else if r < 2.0 { 4 } else { 5 };
                self.cs.stab_b[side][level][b] += 1;
            }
        }
        if !self.hist.is_empty() && (m as usize) < self.t.placements.len() {
            let i = (pos.stm as usize * 16 + level) * self.t.placements.len() + m as usize;
            if self.hist[i][0] == 0 {
                self.hist_used.push(i as u32);
            }
            let h = &mut self.hist[i];
            h[0] += 1;
            h[1] += win;
        }
        let merged = |old: u64| match self.cfg.kc_mode {
            0 => win,
            1 => {
                if old == 0 {
                    win
                } else {
                    (old + win) / 2
                }
            }
            _ => {
                if old == 0 {
                    win
                } else {
                    old.min(win)
                }
            }
        };
        if k[0] == m {
            self.kcost[ply][0] = merged(c[0]);
        } else if k[1] == m {
            self.killers[ply] = [m, k[0]];
            self.kcost[ply] = [merged(c[1]), c[0]];
        } else if self.cfg.kc_evict && c[1] > 0 && c[0] > c[1] {
            // the newer killer is the dearer: it goes, the older stays
            self.killers[ply][0] = m;
            self.kcost[ply][0] = win;
        } else {
            self.killers[ply] = [m, k[0]];
            self.kcost[ply] = [win, c[0]];
        }
    }

    /// Whether `m` may be searched first here: in hand, legal, and not ruled out by a threat.
    #[inline]
    fn candidate(&self, pos: &Pos, m: u16, th: &Threats, alpha: i32) -> bool {
        if m == PASS {
            return false;
        }
        let p = &self.t.placements[m as usize];
        if pos.pieces_now() >> p.piece & 1 == 0 || !pos.legal_placement(self.t, pos.stm as usize, p) {
            return false;
        }
        !(th.n > 0 && th.bound_for(p.cells) <= alpha)
    }

    fn search<const PAR: bool>(&mut self, pos: &Pos, mut alpha: i32, mut beta: i32) -> i32 {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return 0;
        }
        let ply = (pos.ply as usize).min(MAX_PLY - 2);
        self.refuter[ply] = PASS;
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
        if self.cfg.prefetch && self.cfg.staged {
            // the likely first move: its child's entry loads while the threats are tried
            let cand = if tt_move != PASS { tt_move } else { self.killers[ply][0] };
            if cand != PASS {
                self.prefetch(self.hash_after(pos, cand));
            }
        }
        if threats_on {
            let deep = self.cfg.reach_threat && (level as u32) <= self.cfg.reach_threat_start;
            self.rt_effort_now = 0;
            self.try_threat(pos, ply, self.killers[ply + 1][0], alpha, deep, &mut th);
            self.try_threat(pos, ply, self.killers[ply + 1][1], alpha, deep, &mut th);
        }
        let mut skipped = -2; // the best bound of the moves skipped because of threats
        let mut rskip = 0u64; // study: moves skipped only because of reach threats
        let mut first_eb = usize::MAX; // study: the effort bucket of the node's first reach-threat test
        let nodes_before_moves = self.nodes;
        let mut scanned = !threats_on;

        // 4a. The first move of the order is the table move, else a killer, when one of them is
        // legal here and no threat rules it out. Three in four nodes cut on it, so it is searched
        // before the list is built.
        let (mut k1, mut k2) = (self.killers[ply][0], self.killers[ply][1]);
        if self.cfg.kc_order > 0 && level as u32 >= self.cfg.kc_level {
            // E30 idea 1: the killer whose last win was cheaper goes first
            let c = self.kcost[ply];
            if c[0] > 0 && c[1] > 0 && c[1] * self.cfg.kc_order < c[0] {
                std::mem::swap(&mut k1, &mut k2);
            }
        }
        // E30 idea 2: at these nodes the killers are left to the list, where the history chooses
        let hc_on = self.cfg.hc_k > 0 && level as u32 >= self.cfg.hc_level;
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
            if hc_on && self.cfg.hc_staged && k1 != PASS && k2 != PASS {
                let np = t.placements.len();
                let base = (me * 16 + level) * np;
                let key = |m: u16, s: &Solver| {
                    let h = if (m as usize) < np { s.hist[base + m as usize] } else { [0, 0] };
                    if h[0] >= s.cfg.hc_min as u64 { h[1] / h[0] } else { u64::MAX }
                };
                if key(k2, self) < key(k1, self) {
                    std::mem::swap(&mut k1, &mut k2);
                }
            }
            for cand in if hc_on && !self.cfg.hc_staged { [tt_move, PASS, PASS] } else { [tt_move, k1, k2] } {
                if cand == PASS {
                    continue;
                }
                let p = &t.placements[cand as usize];
                if now >> p.piece & 1 == 0 || !pos.legal_placement(t, me, p) {
                    continue;
                }
                if th.n > 0 && th.bound_for_move(p.cells, p.border & pos.cells[me] != 0) <= alpha {
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
                let other = if first == k1 { k2 } else { k1 };
                let oracle = self.cfg.oracle_level as usize == level
                    && (ply - self.root_ply.min(ply)) & 1 == 0
                    && first != tt_move
                    && other != PASS
                    && other != tt_move
                    && self.candidate(pos, other, &th, alpha);
                let mut child = *pos;
                child.play(t, first);
                if self.cfg.prefetch {
                    self.prefetch(child.hash);
                }
                let before = self.nodes;
                let v;
                let mut ow = 0;
                if oracle {
                    // E30 study: search the other killer too, before or after this one by turns
                    let other_first = self.cs.oracle.len() & 1 == 1;
                    let mut oc = *pos;
                    oc.play(t, other);
                    let (mut cf, mut co, mut vo) = (0, 0, 0);
                    let mut vf = 0;
                    for step in 0..2 {
                        let b = self.nodes;
                        if (step == 0) == other_first {
                            vo = -self.search::<PAR>(&oc, -beta, -alpha);
                            co = self.nodes - b;
                        } else {
                            vf = -self.search::<PAR>(&child, -beta, -alpha);
                            cf = self.nodes - b;
                        }
                        if self.aborted {
                            return self.after_abort::<PAR>(pos, ply, alpha_in, beta_in);
                        }
                    }
                    let slot = |m: u16, s: &Solver| if m == s.killers[ply][0] { 0 } else { 1 };
                    let (sf, so) = (slot(first, self), slot(other, self));
                    self.cs.oracle.push([level as u64, self.kcost[ply][sf], self.kcost[ply][so], cf, co, (vf >= beta) as u64, (vo >= beta) as u64, other_first as u64, (sf == 0) as u64]);
                    v = vf;
                    ow = cf;
                } else {
                    v = -self.search::<PAR>(&child, -beta, -alpha);
                }
                let win = if oracle { ow } else { self.nodes - before };
                if !busy.is_null() {
                    unsafe { &*busy }.finish(move_key);
                }
                if self.aborted {
                    return self.after_abort::<PAR>(pos, ply, alpha_in, beta_in);
                }
                if v >= beta {
                    self.cut_by(pos, ply, level, first, tt_move, 0, win, self.nodes - nodes_at_start + 1);
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
                    let bound = th.bound_for_move(p.cells, p.border & pos.cells[me] != 0);
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

        // E30 idea 2: among the first few candidates after the table move, the one whose wins at this
        // level cost least goes first (the scores of those places are handed out again in that order)
        if hc_on && buf.len() > 1 {
            buf.sort_unstable_by(|a, b| b.cmp(a));
            let s = if pre.is_some() { 1 } else { 0 };
            let e = (s + self.cfg.hc_k as usize).min(buf.len());
            if e > s + 1 {
                let np = t.placements.len();
                let base = (me * 16 + level) * np;
                let mut w: Vec<(u64, usize, u64)> = buf[s..e]
                    .iter()
                    .enumerate()
                    .map(|(j, &x)| {
                        let m = (x & 0xFFFF) as usize;
                        let h = if m < np { self.hist[base + m] } else { [0, 0] };
                        let key = if h[0] >= self.cfg.hc_min as u64 { h[1] / h[0] } else { u64::MAX };
                        (key, j, x & 0xFFFF)
                    })
                    .collect();
                w.sort_by_key(|&(k, j, _)| (k, j));
                for (j, &(_, _, m)) in w.iter().enumerate() {
                    buf[s + j] = (buf[s + j] & !0xFFFF) | m;
                }
            }
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
                let mut win = 0;
                if let (1, true, Some((first, value))) = (i, first_pass, pre) {
                    // searched before the list was built
                    assert!(m == first, "the placement check and the generator disagree");
                    searched += 1;
                    v = value;
                } else {
                    if th.n > 0 {
                        let pm = &t.placements[m as usize];
                        let bound = th.bound_for_move(pm.cells, pm.border & pos.cells[me] != 0);
                        if bound <= alpha {
                            skipped = skipped.max(bound);
                            if self.cfg.rt_study && th.bound_for_move_plain(pm.cells, pm.border & pos.cells[me] != 0) > alpha {
                                rskip += 1;
                            }
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
                    let before = self.nodes;
                    v = -self.search::<PAR>(&child, -beta, -alpha);
                    win = self.nodes - before;
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
                    self.cut_by(pos, ply, level, m, tt_move, searched - 1, win, self.nodes - nodes_at_start + 1);
                    if searched == 1 {
                        self.prof[level][4] += 1;
                    }
                    break;
                }
                // The reply that refuted this move (the first killer one ply down) will meet most
                // moves that leave it alone: the moves that cover one of its cells go first.
                let refuted_by = if self.cfg.rt_refuter > 0 && self.refuter[ply + 1] != PASS { self.refuter[ply + 1] } else { self.killers[ply + 1][0] };
                if self.cfg.answer_bonus > 0 && first_pass && i + 1 < n {
                    let k = if self.cfg.rt_refuter >= 2 { refuted_by } else { self.killers[ply + 1][0] };
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
                    if self.cfg.rt_study {
                        let k = refuted_by;
                        if k != PASS {
                            let r = &t.placements[k as usize];
                            let z = r.cells | r.border;
                            self.rts.outside_now = buf[i..n].iter().filter(|e| t.placements[(**e & 0xFFFF) as usize].cells & z == 0).count() as u32;
                        }
                    }
                    self.rt_effort_now = self.nodes - nodes_at_start;
                    let tests_before = self.rt_stats[0];
                    self.try_threat(pos, ply, refuted_by, alpha, self.cfg.reach_threat && searched >= self.cfg.reach_threat_after, &mut th);
                    if self.cfg.rt_study && first_eb == usize::MAX && self.rt_stats[0] > tests_before {
                        first_eb = effort_bucket(self.nodes - nodes_at_start);
                    }
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
            if rskip > 0 {
                let under = self.nodes - nodes_before_moves;
                let s = &mut self.rts.skip[level];
                s[0] += rskip;
                s[1] += rskip * under / searched.max(1);
                s[2] += 1;
                s[3] += searched;
                s[4] += under;
                if first_eb != usize::MAX {
                    let e = &mut self.rts.eff[level][first_eb];
                    e[3] += rskip;
                    e[4] += rskip * under / searched.max(1);
                }
            }
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
