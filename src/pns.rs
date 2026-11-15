//! Depth-first proof-number search (df-pn) over the node logic of the alpha-beta solver.
//!
//! One question at a time: "does `attacker` win?". At every node the mover has a goal: a win if it
//! is the attacker, at least a draw if it is the defender. phi is the work still needed to prove the
//! mover's goal at a node and delta the work to refute it: a node's phi is the smallest delta among
//! its children, its delta the sum of their phi. The search always walks to the child that is
//! cheapest to refute and leaves a node when its numbers pass the thresholds its parent set.
//! Every decided node is decided by a proof (game over, the last pieces, the length bounds, a threat
//! left playable), so the answer is exact; only the order of the work is heuristic.

use crate::bounds::*;
use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;
use crate::solver::*;

pub const INF: u64 = 1 << 60;
const MAX_DEPTH: usize = 48;
const QKEY: [u64; 2] = [0x9E37_79B9_7F4A_7C15, 0xC2B2_AE3D_27D4_EB4F];

#[derive(Clone, Copy)]
struct PnEntry {
    key: u64,
    phi: u64,
    delta: u64,
    work: u32,
    gen: u32,
}

const PN_EMPTY: PnEntry = PnEntry { key: 0, phi: 1, delta: 1, work: 0, gen: 0 };

#[derive(Clone, Copy)]
struct Child {
    mv: u16,
    phi: u64,
    delta: u64,
}

#[derive(Clone, Copy)]
pub struct PnConfig {
    /// log2 of the number of table entries.
    pub tt_bits: u32,
    /// The child's delta threshold is the second-best delta plus 1 plus second-best / this.
    pub slack: u64,
    /// A new child's delta starts at 1 + (its rank in the move order) * rank_mul / this; 0 = always 1.
    pub rank_div: u64,
    pub rank_mul: u64,
    /// A node with at most this many pieces left (both hands) is a leaf for the alpha-beta search,
    /// asked the question of this search as a window of one step. 0 = no leaves.
    pub ab_level: u32,
    /// Alpha-beta nodes that count as one unit of a leaf's numbers: a leaf returns when the nodes
    /// spent on it reach its threshold times this, and comes back with a larger budget.
    /// 0 = a leaf is solved in one go.
    pub unit: u64,
    /// Order the children by the alpha-beta solver's model (where that solver uses it).
    pub model: bool,
    /// Only the best `width` children that are not yet refuted count at a node; 0 = all.
    pub width: usize,
    /// Read the children's numbers from the table when a node is expanded.
    pub probe_kids: bool,
    /// Share the alpha-beta solver's killers: a move that proved the mover's goal at this ply is
    /// tried first, and after a refuted move the moves that cover a cell of the refuting reply.
    pub killers: bool,
}

impl Default for PnConfig {
    fn default() -> Self {
        PnConfig { tt_bits: 21, slack: 4, rank_div: 0, rank_mul: 1, ab_level: 0, unit: 0, model: false, width: 0, probe_kids: false, killers: false }
    }
}

/// The hash of the position after the move.
#[inline]
fn child_hash(t: &Tables, pos: &Pos, m: u16) -> u64 {
    let mut h = pos.hash ^ t.zob_stm;
    if m != PASS {
        let me = pos.stm as usize;
        let p = &t.placements[m as usize];
        h ^= t.zob_piece[me][p.piece as usize];
        for j in 0..p.n as usize {
            h ^= t.zob_cell[me][p.cell_list[j] as usize];
        }
    }
    h
}

pub struct PnSolver {
    pub s: Solver,
    pub cfg: PnConfig,
    tt: Vec<[PnEntry; 2]>,
    mask: usize,
    gen: u32,
    /// Nodes visited.
    pub nodes: u64,
    /// Nodes whose moves were generated.
    pub expansions: u64,
    /// Calls of the alpha-beta search on a leaf.
    pub leaves: u64,
    node_limit: u64,
    aborted: bool,
    kids: Vec<Vec<Child>>,
    order: Vec<u64>,
}

impl PnSolver {
    pub fn new(cfg: PnConfig, mut solver_cfg: Config) -> PnSolver {
        if cfg.ab_level == 0 {
            solver_cfg.tt_bits = 4; // the alpha-beta table is not used
        }
        let buckets = 1usize << (cfg.tt_bits - 1);
        PnSolver {
            s: Solver::new(solver_cfg),
            cfg,
            tt: vec![[PN_EMPTY; 2]; buckets],
            mask: buckets - 1,
            gen: 1,
            nodes: 0,
            expansions: 0,
            leaves: 0,
            node_limit: u64::MAX,
            aborted: false,
            kids: (0..MAX_DEPTH).map(|_| Vec::with_capacity(1024)).collect(),
            order: Vec::with_capacity(1024),
        }
    }

    /// Forget everything learned from earlier positions.
    pub fn clear(&mut self) {
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            for b in self.tt.iter_mut() {
                *b = [PN_EMPTY; 2];
            }
            self.gen = 1;
        }
        self.s.clear();
    }

    /// Nodes of this search and of the alpha-beta searches on its leaves.
    pub fn total_nodes(&self) -> u64 {
        self.nodes + self.s.nodes
    }

    /// The exact value for the side to move; UNKNOWN when the node limit stopped the search.
    pub fn solve(&mut self, pos: &Pos, node_limit: u64) -> i32 {
        self.nodes = 0;
        self.expansions = 0;
        self.leaves = 0;
        self.s.nodes = 0;
        self.node_limit = node_limit;
        self.aborted = false;
        let me = pos.stm as usize;
        match self.prove(pos, me) {
            None => UNKNOWN,
            Some(true) => 1,
            Some(false) => match self.prove(pos, me ^ 1) {
                None => UNKNOWN,
                Some(true) => -1,
                Some(false) => 0,
            },
        }
    }

    /// Does `attacker` win from here? None when the node limit stopped the search.
    pub fn prove(&mut self, pos: &Pos, attacker: usize) -> Option<bool> {
        let (phi, delta) = self.mid(pos, attacker, INF, INF, 0);
        if self.aborted {
            return None;
        }
        assert!(phi == 0 || delta == 0, "df-pn left the root undecided");
        let mover_goal = phi == 0;
        Some(if pos.stm as usize == attacker { mover_goal } else { !mover_goal })
    }

    #[inline]
    fn probe(&self, key: u64) -> Option<(u64, u64)> {
        let b = &self.tt[(key as usize) & self.mask];
        if b[0].key == key && b[0].gen == self.gen {
            Some((b[0].phi, b[0].delta))
        } else if b[1].key == key && b[1].gen == self.gen {
            Some((b[1].phi, b[1].delta))
        } else {
            None
        }
    }

    #[inline]
    fn probe_entry(&self, key: u64) -> Option<PnEntry> {
        let b = &self.tt[(key as usize) & self.mask];
        if b[0].key == key && b[0].gen == self.gen {
            Some(b[0])
        } else if b[1].key == key && b[1].gen == self.gen {
            Some(b[1])
        } else {
            None
        }
    }

    #[inline]
    fn store(&mut self, key: u64, phi: u64, delta: u64, work: u64) {
        let gen = self.gen;
        let b = &mut self.tt[(key as usize) & self.mask];
        let e = PnEntry { key, phi, delta, work: work.min(u32::MAX as u64) as u32, gen };
        if b[0].key == key {
            b[0] = e;
        } else if b[1].key == key {
            b[1] = e;
        } else if b[0].gen != gen || e.work >= b[0].work {
            b[1] = b[0];
            b[0] = e;
        } else {
            b[1] = e;
        }
    }

    /// What is known about the mover's goal without a search: Some(true) it holds, Some(false) it fails.
    fn static_goal(&mut self, pos: &Pos, attacker: usize) -> Option<bool> {
        let t = self.s.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let need = if me == attacker { 1 } else { 0 };
        if pos.is_over() {
            return Some(game_result(pos) >= need);
        }
        if pos.left[me].count_ones() <= 1 && pos.left[opp].count_ones() <= 1 {
            return Some(self.s.last_pieces(pos) >= need);
        }
        let mine = bounds(t, pos, me);
        let theirs = bounds(t, pos, opp);
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
        if blo >= need {
            Some(true)
        } else if bhi < need {
            Some(false)
        } else {
            None
        }
    }

    /// The mover's moves that can still reach its goal, best-looking first; a pass if it has no move.
    /// Moves that leave a deciding opponent placement playable are left out: they fail by the bounds.
    fn generate(&mut self, pos: &Pos, attacker: usize, kids: &mut Vec<Child>) {
        let t = self.s.t;
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let need = if me == attacker { 1 } else { 0 };
        let alpha = need - 1; // a move worth this or less fails the mover's goal
        let mut th = Threats::new();
        if pos.left[opp] != 0 {
            let mut mine = Clusters::new();
            upper_bound_ex(t, pos, me, 0, Some(&mut mine));
            self.s.scan_threats(pos, &mine, alpha, &mut th);
        }
        let mut order = std::mem::take(&mut self.order);
        order.clear();
        let mut skipped = false;
        let own = pos.cells[me];
        let theirs = pos.cells[opp];
        let front = neighbours(shape_of(theirs).ends) & pos.empty();
        let level = pos.left[0].count_ones() + pos.left[1].count_ones();
        let use_model = self.cfg.model && level >= self.s.cfg.model_level;
        let w = self.s.cfg.model;
        let empty = pos.empty();
        let their_shape = shape_of(theirs);
        let their_free = neighbours(their_shape.ends) & empty & !neighbours(their_shape.body);
        let ply = (pos.ply as usize).min(MAX_PLY - 2);
        let (k1, k2) = if self.cfg.killers { (self.s.killers[ply][0], self.s.killers[ply][1]) } else { (PASS, PASS) };
        pos.for_each_move(t, me, pos.pieces_now(), |m| {
            let p = &t.placements[m as usize];
            if th.n > 0 && th.bound_for(p.cells) <= alpha {
                skipped = true;
                return;
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
                // the first eight features of the alpha-beta solver's model (the rest have weight 0)
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
            if m == k1 {
                score = 1 << 29;
            } else if m == k2 {
                score = 1 << 28;
            }
            order.push(score << 16 | m as u64);
        });
        order.sort_unstable_by(|a, b| b.cmp(a));
        let (div, mul) = (self.cfg.rank_div, self.cfg.rank_mul);
        for (i, &x) in order.iter().enumerate() {
            let mv = (x & 0xFFFF) as u16;
            let mut c = Child { mv, phi: 1, delta: if div == 0 { 1 } else { 1 + i as u64 * mul / div } };
            if self.cfg.probe_kids {
                if let Some((phi, delta)) = self.probe(child_hash(t, pos, mv) ^ QKEY[attacker]) {
                    if phi == 0 {
                        continue; // the opponent reaches its goal after this move: refuted before
                    }
                    c.phi = phi;
                    c.delta = delta;
                    if delta == 0 {
                        kids.insert(0, c); // proved before: it must be inside the window
                        continue;
                    }
                }
            }
            kids.push(c);
        }
        if order.is_empty() && !skipped {
            kids.push(Child { mv: PASS, phi: 1, delta: 1 });
        }
        self.order = order;
    }

    /// A leaf: the alpha-beta search answers the question, within a budget when `unit` is set.
    /// A leaf that is not solved gets numbers that grow with the nodes spent on it.
    fn leaf(&mut self, pos: &Pos, attacker: usize, key: u64, th_phi: u64, th_delta: u64, spent: u64) -> (u64, u64) {
        let need = if pos.stm as usize == attacker { 1 } else { 0 };
        let unit = self.cfg.unit;
        let th = th_phi.min(th_delta);
        let budget = if unit == 0 || th >= INF { u64::MAX } else { th.saturating_mul(unit).saturating_sub(spent).max(1) };
        let before = self.s.nodes;
        let global = self.node_limit.saturating_sub(self.nodes); // what the leaves may still use in all
        let limit = before.saturating_add(budget).min(global);
        self.leaves += 1;
        let v = self.s.search_more(pos, need - 1, need, limit);
        let total = spent + (self.s.nodes - before);
        if v == UNKNOWN {
            if unit == 0 || self.s.nodes >= global {
                self.aborted = true;
                return (1, 1);
            }
            let n = 1 + total / unit;
            self.store(key, n, n, total);
            return (n, n);
        }
        let r = if v >= need { (0, INF) } else { (INF, 0) };
        self.store(key, r.0, r.1, total);
        r
    }

    /// Works on a node until its phi or delta reaches its threshold; returns (phi, delta).
    fn mid(&mut self, pos: &Pos, attacker: usize, th_phi: u64, th_delta: u64, depth: usize) -> (u64, u64) {
        self.nodes += 1;
        if self.nodes + self.s.nodes > self.node_limit {
            self.aborted = true;
            return (1, 1);
        }
        let key = pos.hash ^ QKEY[attacker];
        let leaf = self.cfg.ab_level > 0 && pos.left[0].count_ones() + pos.left[1].count_ones() <= self.cfg.ab_level;
        // 1. the table; on a first visit, what is known without a search
        let mut spent = 0u64;
        if let Some(e) = self.probe_entry(key) {
            if e.phi >= th_phi || e.delta >= th_delta {
                return (e.phi, e.delta);
            }
            spent = e.work as u64;
        } else if !leaf {
            if let Some(holds) = self.static_goal(pos, attacker) {
                return if holds { (0, INF) } else { (INF, 0) };
            }
        }
        if leaf {
            return self.leaf(pos, attacker, key, th_phi, th_delta, spent);
        }
        // 2. expand
        let nodes_before = self.nodes;
        self.expansions += 1;
        let depth = depth.min(MAX_DEPTH - 1);
        let mut kids = std::mem::take(&mut self.kids[depth]);
        kids.clear();
        self.generate(pos, attacker, &mut kids);
        let t = self.s.t;
        let slack = self.cfg.slack;
        let width = if self.cfg.width == 0 { usize::MAX } else { self.cfg.width };
        let ply = (pos.ply as usize).min(MAX_PLY - 2);
        let mut answered = PASS;
        // 3. work on the child that is cheapest to refute until this node passes a threshold
        let result = loop {
            let (mut phi, mut second, mut best, mut delta) = (INF, INF, 0usize, 0u64);
            for (i, c) in kids.iter().enumerate().take(width) {
                delta = (delta + c.phi).min(INF);
                if c.delta < phi {
                    second = phi;
                    phi = c.delta;
                    best = i;
                } else if c.delta < second {
                    second = c.delta;
                }
            }
            if phi >= th_phi || delta >= th_delta || phi == 0 || delta == 0 || self.aborted {
                break (phi, delta);
            }
            let c = kids[best];
            let c_th_phi = if th_delta >= INF { INF } else { th_delta - delta + c.phi };
            let c_th_delta = th_phi.min(second + 1 + if slack == 0 { 0 } else { second / slack }).min(INF);
            let mut child = *pos;
            child.play(t, c.mv);
            let (p, d) = self.mid(&child, attacker, c_th_phi, c_th_delta, depth + 1);
            if p == 0 {
                // the opponent reaches its goal after this move: the move is refuted
                if self.cfg.width == 0 {
                    kids.swap_remove(best);
                } else {
                    kids.remove(best); // the window needs the order
                }
                if self.cfg.killers && self.cfg.width != 0 {
                    // the reply that refuted this move will meet most moves that leave it alone
                    let k = self.s.killers[ply + 1][0];
                    if k != PASS && k != answered {
                        answered = k;
                        let reply = t.placements[k as usize].cells;
                        let covers = |c: &Child| c.mv != PASS && t.placements[c.mv as usize].cells & reply != 0;
                        kids.sort_by_key(|c| !covers(c)); // stable: the covering moves first, each group in its order
                    }
                }
            } else {
                kids[best].phi = p;
                kids[best].delta = d;
            }
        };
        if self.cfg.killers && result.0 == 0 {
            // the move that proved the mover's goal is a killer for this ply, as a cut is in alpha-beta
            if let Some(c) = kids.iter().find(|c| c.delta == 0) {
                if c.mv != PASS && self.s.killers[ply][0] != c.mv {
                    self.s.killers[ply][1] = self.s.killers[ply][0];
                    self.s.killers[ply][0] = c.mv;
                }
            }
        }
        self.kids[depth] = kids;
        if !self.aborted {
            self.store(key, result.0, result.1, self.nodes - nodes_before + 1);
        }
        result
    }
}
