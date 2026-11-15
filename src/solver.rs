//! Exact solver: the game-theoretic value of a position for the side to move (+1 win, 0 draw, -1 loss).
//! Negamax alpha-beta over the three values. Every shortcut is a proof; nothing here is heuristic
//! except the order in which moves are tried.

use crate::bounds::*;
use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;

#[derive(Clone, Copy)]
pub struct Config {
    /// Use the snake-length bounds to decide nodes.
    pub bounds: bool,
    /// Value the last pieces (at most one in each hand) without a search.
    pub last_pieces: bool,
    /// log2 of the number of table entries.
    pub tt_bits: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { bounds: true, last_pieces: true, tt_bits: 22 }
    }
}

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    lo: i8,
    hi: i8,
    work: u8,
    gen: u8,
    mv: u16,
}

const EMPTY: Entry = Entry { key: 0, lo: -1, hi: 1, work: 0, gen: 0, mv: PASS };
pub const UNKNOWN: i32 = -9;
const MAX_PLY: usize = 40;
const TT_MOVE: u64 = 1 << 30;
const KILLER1: u64 = 1 << 29;
const KILLER2: u64 = 1 << 28;

pub struct Solver {
    t: &'static Tables,
    pub cfg: Config,
    tt: Vec<[Entry; 2]>,
    mask: usize,
    gen: u8,
    pub nodes: u64,
    /// Nodes by the number of pieces both players still hold (a profile, not used by the search).
    pub by_left: [u64; 16],
    node_limit: u64,
    aborted: bool,
    killers: [[u16; 2]; MAX_PLY],
    bufs: Vec<Vec<u64>>,
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
        let buckets = 1usize << (cfg.tt_bits - 1);
        Solver {
            t: tables(),
            cfg,
            tt: vec![[EMPTY; 2]; buckets],
            mask: buckets - 1,
            gen: 1,
            nodes: 0,
            by_left: [0; 16],
            node_limit: u64::MAX,
            aborted: false,
            killers: [[PASS; 2]; MAX_PLY],
            bufs: (0..MAX_PLY).map(|_| Vec::with_capacity(1024)).collect(),
            replies: Vec::with_capacity(1024),
        }
    }

    /// Forget everything learned from earlier positions.
    pub fn clear(&mut self) {
        // entries of an older generation never match; the table is wiped only when the counter wraps
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            for b in self.tt.iter_mut() {
                *b = [EMPTY; 2];
            }
            self.gen = 1;
        }
        self.killers = [[PASS; 2]; MAX_PLY];
    }

    /// The value within the window (alpha, beta): exact inside it, a bound outside. UNKNOWN when the
    /// node limit stopped the search.
    pub fn solve(&mut self, pos: &Pos, alpha: i32, beta: i32, node_limit: u64) -> i32 {
        self.nodes = 0;
        self.node_limit = node_limit;
        self.aborted = false;
        let v = self.search(pos, alpha, beta);
        if self.aborted {
            UNKNOWN
        } else {
            v
        }
    }

    #[inline]
    fn probe(&self, key: u64) -> Option<Entry> {
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
    fn store(&mut self, key: u64, lo: i32, hi: i32, mv: u16, work: u64) {
        let work = (64 - work.leading_zeros()) as u8;
        let gen = self.gen;
        let b = &mut self.tt[(key as usize) & self.mask];
        let e = Entry { key, lo: lo as i8, hi: hi as i8, work, gen, mv };
        if b[0].key == key {
            b[0] = e;
        } else if b[1].key == key {
            b[1] = e;
        } else if b[0].gen != gen || work >= b[0].work {
            b[1] = b[0];
            b[0] = e;
        } else {
            b[1] = e;
        }
    }

    /// The exact value when each player has at most one piece left: the mover places its piece (or
    /// passes), then the opponent takes its longest extension that is still legal. A placement only
    /// takes opponent placements away (the cells it covers, and new snakes that would now touch it).
    fn last_pieces(&mut self, pos: &Pos) -> i32 {
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

    fn search(&mut self, pos: &Pos, mut alpha: i32, mut beta: i32) -> i32 {
        self.nodes += 1;
        if self.nodes > self.node_limit {
            self.aborted = true;
            return 0;
        }
        self.by_left[((pos.left[0].count_ones() + pos.left[1].count_ones()) as usize).min(15)] += 1;
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
        if let Some(e) = self.probe(pos.hash) {
            lo = e.lo as i32;
            hi = e.hi as i32;
            tt_move = e.mv;
            if lo >= hi || lo >= beta {
                return lo;
            }
            if hi <= alpha {
                return hi;
            }
        }

        // 2. the snake-length bounds
        if self.cfg.bounds {
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
            lo = lo.max(blo);
            hi = hi.min(bhi);
            if lo >= hi || lo >= beta {
                return lo;
            }
            if hi <= alpha {
                return hi;
            }
        }
        alpha = alpha.max(lo);
        beta = beta.min(hi);
        let (alpha0, beta0) = (alpha, beta);

        // 3. generate and score the moves
        let ply = (pos.ply as usize).min(MAX_PLY - 1);
        let mut buf = std::mem::take(&mut self.bufs[ply]);
        buf.clear();
        {
            let own = pos.cells[me];
            let theirs = pos.cells[opp];
            let front = neighbours(shape_of(theirs).ends) & pos.empty();
            let (k1, k2) = (self.killers[ply][0], self.killers[ply][1]);
            pos.for_each_move(t, me, pos.pieces_now(), |m| {
                let p = &t.placements[m as usize];
                let mut score = p.n as u64 * 64;
                if p.border & own != 0 {
                    score += 256;
                }
                if p.cells & front != 0 {
                    score += 128;
                } else if p.border & theirs != 0 {
                    score += 32;
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

        // 4. search them, best-scored first
        let mut best = -2;
        let mut best_move = PASS;
        if buf.is_empty() {
            let mut child = *pos;
            child.play(t, PASS);
            best = -self.search(&child, -beta, -alpha);
        } else {
            let n = buf.len();
            for i in 0..n {
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
                let m = (buf[i] & 0xFFFF) as u16;
                let mut child = *pos;
                child.play(t, m);
                let v = -self.search(&child, -beta, -alpha);
                if self.aborted {
                    break;
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
                    break;
                }
            }
        }
        self.bufs[ply] = buf;
        if self.aborted {
            return 0;
        }

        // 5. store what we learned
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
        self.store(pos.hash, nlo, nhi, best_move, self.nodes - nodes_at_start + 1);
        best
    }
}
