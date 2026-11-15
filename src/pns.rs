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
    /// A new child's delta starts at 1 + (its rank in the move order) / this; 0 = always 1.
    pub rank_div: u64,
}

impl Default for PnConfig {
    fn default() -> Self {
        PnConfig { tt_bits: 21, slack: 4, rank_div: 0 }
    }
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
    node_limit: u64,
    aborted: bool,
    kids: Vec<Vec<Child>>,
    order: Vec<u64>,
}

impl PnSolver {
    pub fn new(cfg: PnConfig, mut solver_cfg: Config) -> PnSolver {
        solver_cfg.tt_bits = 4; // the alpha-beta table is not used here
        let buckets = 1usize << (cfg.tt_bits - 1);
        PnSolver {
            s: Solver::new(solver_cfg),
            cfg,
            tt: vec![[PN_EMPTY; 2]; buckets],
            mask: buckets - 1,
            gen: 1,
            nodes: 0,
            expansions: 0,
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
    }

    /// The exact value for the side to move; UNKNOWN when the node limit stopped the search.
    pub fn solve(&mut self, pos: &Pos, node_limit: u64) -> i32 {
        self.nodes = 0;
        self.expansions = 0;
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
            order.push(score << 16 | m as u64);
        });
        order.sort_unstable_by(|a, b| b.cmp(a));
        let div = self.cfg.rank_div;
        for (i, &x) in order.iter().enumerate() {
            let delta = if div == 0 { 1 } else { 1 + i as u64 / div };
            kids.push(Child { mv: (x & 0xFFFF) as u16, phi: 1, delta });
        }
        if kids.is_empty() && !skipped {
            kids.push(Child { mv: PASS, phi: 1, delta: 1 });
        }
        self.order = order;
    }

    /// Works on a node until its phi or delta reaches its threshold; returns (phi, delta).
    fn mid(&mut self, pos: &Pos, attacker: usize, th_phi: u64, th_delta: u64, depth: usize) -> (u64, u64) {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return (1, 1);
        }
        let key = pos.hash ^ QKEY[attacker];
        // 1. the table; on a first visit, what is known without a search
        if let Some((phi, delta)) = self.probe(key) {
            if phi >= th_phi || delta >= th_delta {
                return (phi, delta);
            }
        } else if let Some(holds) = self.static_goal(pos, attacker) {
            return if holds { (0, INF) } else { (INF, 0) };
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
        // 3. work on the child that is cheapest to refute until this node passes a threshold
        let result = loop {
            let (mut phi, mut second, mut best, mut delta) = (INF, INF, 0usize, 0u64);
            for (i, c) in kids.iter().enumerate() {
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
                kids.swap_remove(best);
            } else {
                kids[best].phi = p;
                kids[best].delta = d;
            }
        };
        self.kids[depth] = kids;
        if !self.aborted {
            self.store(key, result.0, result.1, self.nodes - nodes_before + 1);
        }
        result
    }
}
