//! A position, the placement rules and the bit-parallel move generator.

use crate::geom::*;
use crate::pieces::*;

pub const RED: usize = 0;
pub const BLUE: usize = 1;
pub const MAX_SNAKES: usize = 8;

#[derive(Clone, Copy)]
pub struct Pos {
    pub cells: [BB; 2],
    pub snakes: [[BB; MAX_SNAKES]; 2],
    pub hash: u64,
    pub left: [u8; 2],
    pub nsnakes: [u8; 2],
    pub longest: [u8; 2],
    pub stm: u8,
    pub passes: u8,
    pub ply: u8,
}

/// What a player's cells imply for the cells around them.
pub struct Shape {
    pub t1: BB,   // cells with at least 1 own neighbour
    pub t2: BB,   // at least 2
    pub t3: BB,   // at least 3
    pub pair: BB, // cells touching two own cells that touch each other
    pub body: BB, // own cells with 2 own neighbours
    pub ends: BB, // own cells with at most 1 own neighbour
}

#[inline]
pub fn shape_of(own: BB) -> Shape {
    let mut nd = [0u128; 6];
    for d in 0..6 {
        nd[d] = shift_in(own, DIR_Q[d], DIR_R[d]);
    }
    let (mut a1, mut a2, mut a3, mut pair) = (0u128, 0u128, 0u128, 0u128);
    for d in 0..6 {
        a3 |= a2 & nd[d];
        a2 |= a1 & nd[d];
        a1 |= nd[d];
        pair |= nd[d] & nd[(d + 1) % 6];
    }
    Shape { t1: a1, t2: a2, t3: a3, pair, body: own & a2, ends: own & !a2 }
}

/// Everything the generator needs to know about a player's options, computed once per position.
pub struct Gen {
    pub free_new: BB,     // empty cells that touch no tile
    pub allowed: [BB; 3], // allowed[k]: empty cells where a piece cell with k neighbours inside its piece may go
    pub next_own: BB,     // empty cells next to an own cell
    pub own: BB,
    pub own_with_nb: BB, // own cells that already have an own neighbour
}

impl Pos {
    pub fn initial() -> Pos {
        Pos { cells: [0; 2], snakes: [[0; MAX_SNAKES]; 2], hash: 0, left: [ALL_PIECES, ALL_BUT_ONE], nsnakes: [0; 2], longest: [0; 2], stm: 0, passes: 0, ply: 0 }
            .with_derived()
    }

    fn with_derived(mut self) -> Pos {
        self.recompute();
        self
    }

    /// Recomputes snakes and the hash from cells, pieces and side to move.
    pub fn recompute(&mut self) {
        let t = tables();
        let mut h = 0u64;
        for player in 0..2 {
            self.nsnakes[player] = 0;
            self.longest[player] = 0;
            let mut rest = self.cells[player];
            while rest != 0 {
                let snake = flood(bit(first(rest)), rest);
                rest &= !snake;
                self.snakes[player][self.nsnakes[player] as usize] = snake;
                self.nsnakes[player] += 1;
                self.longest[player] = self.longest[player].max(count(snake) as u8);
            }
            let mut c = self.cells[player];
            while c != 0 {
                h ^= t.zob_cell[player][first(c)];
                c &= c - 1;
            }
            for p in 0..NPIECES {
                if self.left[player] >> p & 1 == 1 {
                    h ^= t.zob_piece[player][p];
                }
            }
        }
        if self.stm == 1 {
            h ^= t.zob_stm;
        }
        self.hash = h;
    }

    #[inline(always)]
    pub fn is_over(&self) -> bool {
        self.passes >= 2 || (self.left[0] | self.left[1]) == 0
    }
    #[inline(always)]
    pub fn occupied(&self) -> BB {
        self.cells[0] | self.cells[1]
    }
    #[inline(always)]
    pub fn empty(&self) -> BB {
        ALL & !self.occupied()
    }
    /// The pieces the side to move may place now (the very first move must be the 1-hex).
    #[inline(always)]
    pub fn pieces_now(&self) -> u8 {
        if self.ply == 0 {
            self.left[self.stm as usize] & 1
        } else {
            self.left[self.stm as usize]
        }
    }

    #[inline]
    pub fn play(&mut self, t: &Tables, m: u16) {
        if m == PASS {
            self.passes += 1;
        } else {
            self.passes = 0;
            let me = self.stm as usize;
            let p = &t.placements[m as usize];
            self.cells[me] |= p.cells;
            self.left[me] &= !(1u8 << p.piece);
            let mut h = self.hash ^ t.zob_piece[me][p.piece as usize];
            for j in 0..p.n as usize {
                h ^= t.zob_cell[me][p.cell_list[j] as usize];
            }
            self.hash = h;
            // the new piece and every own snake it touches become one snake
            let mut joined = p.cells;
            let mut kept = 0;
            for j in 0..self.nsnakes[me] as usize {
                let s = self.snakes[me][j];
                if s & p.border != 0 {
                    joined |= s;
                } else {
                    self.snakes[me][kept] = s;
                    kept += 1;
                }
            }
            self.snakes[me][kept] = joined;
            self.nsnakes[me] = (kept + 1) as u8;
            self.longest[me] = self.longest[me].max(count(joined) as u8);
        }
        self.hash ^= t.zob_stm;
        self.stm ^= 1;
        self.ply += 1;
    }

    /// Adds a placement of `player` without passing the turn; returns the snake it is now part of.
    /// For one-player look-ahead: the hash, the ply and the side to move are left alone.
    #[inline]
    pub fn place_for(&mut self, t: &Tables, player: usize, m: u16) -> BB {
        let p = &t.placements[m as usize];
        self.cells[player] |= p.cells;
        self.left[player] &= !(1u8 << p.piece);
        let mut joined = p.cells;
        let mut kept = 0;
        for j in 0..self.nsnakes[player] as usize {
            let s = self.snakes[player][j];
            if s & p.border != 0 {
                joined |= s;
            } else {
                self.snakes[player][kept] = s;
                kept += 1;
            }
        }
        self.snakes[player][kept] = joined;
        self.nsnakes[player] = (kept + 1) as u8;
        self.longest[player] = self.longest[player].max(count(joined) as u8);
        joined
    }

    /// The longest snake of `player` if it placed p now.
    #[inline]
    pub fn longest_after(&self, player: usize, p: &Placement) -> u32 {
        let mut joined = p.n as u32;
        for j in 0..self.nsnakes[player] as usize {
            if self.snakes[player][j] & p.border != 0 {
                joined += count(self.snakes[player][j]);
            }
        }
        joined.max(self.longest[player] as u32)
    }

    #[inline]
    pub fn gen_ctx(&self, player: usize) -> Gen {
        let empty = self.empty();
        let own = self.cells[player];
        let s = shape_of(own);
        let not_next_body = empty & !neighbours(s.body);
        Gen {
            free_new: empty & !neighbours(self.occupied()),
            allowed: [not_next_body & !s.t3 & !s.pair, not_next_body & !s.t2, not_next_body & !s.t1],
            next_own: empty & s.t1,
            own,
            own_with_nb: own & s.t1,
        }
    }

    /// The generator context for placements that grow an own snake only (`free_new` is left empty).
    #[inline]
    pub fn gen_ctx_grow(&self, player: usize) -> Gen {
        let empty = self.empty();
        let own = self.cells[player];
        let s = shape_of(own);
        let not_next_body = empty & !neighbours(s.body);
        Gen {
            free_new: 0,
            allowed: [not_next_body & !s.t3 & !s.pair, not_next_body & !s.t2, not_next_body & !s.t1],
            next_own: empty & s.t1,
            own,
            own_with_nb: own & s.t1,
        }
    }

    /// Whether `player` may place p now, straight from the rules (one placement at a time).
    pub fn legal_placement(&self, t: &Tables, player: usize, p: &Placement) -> bool {
        let occ = self.occupied();
        if p.cells & occ != 0 {
            return false;
        }
        let own = self.cells[player];
        if p.border & own == 0 {
            return p.border & occ == 0;
        }
        let after = own | p.cells;
        for j in 0..p.n as usize {
            let around = t.geo.neighbours[p.cell_list[j] as usize] & after;
            let degree = count(around);
            if degree > 2 {
                return false;
            }
            if degree == 2 && t.geo.neighbours[first(around)] & (around & (around - 1)) != 0 {
                return false;
            }
        }
        let mut touched = p.border & own;
        while touched != 0 {
            let i = first(touched);
            touched &= touched - 1;
            if count(t.geo.neighbours[i] & after) > 2 {
                return false;
            }
        }
        true
    }

    /// Calls f(placement id) for every legal placement of `player` among `pieces`, in id order.
    #[inline]
    pub fn for_each_move<F: FnMut(u16)>(&self, t: &Tables, player: usize, pieces: u8, mut f: F) {
        if pieces == 0 {
            return;
        }
        let g = self.gen_ctx(player);
        for piece in 0..NPIECES {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                let mut anchors = legal_anchors(o, &g);
                while anchors != 0 {
                    f(o.placement_at[first(anchors)] as u16);
                    anchors &= anchors - 1;
                }
            }
        }
    }

    /// Calls f for legal placements of `player`, largest pieces first, until f returns true.
    /// With `touching` != 0 only placements that touch those own cells are produced.
    #[inline]
    pub fn find_move<F: FnMut(u16) -> bool>(&self, t: &Tables, player: usize, pieces: u8, touching: BB, mut f: F) -> bool {
        if pieces == 0 {
            return false;
        }
        if touching != 0 {
            // only placements that grow a snake: no new-snake anchors to compute
            let mut g = self.gen_ctx_grow(player);
            g.next_own &= neighbours(touching);
            if g.next_own == 0 {
                return false;
            }
            for piece in (0..NPIECES).rev() {
                if pieces >> piece & 1 == 0 {
                    continue;
                }
                for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                    let mut anchors = grow_anchors(o, &g);
                    while anchors != 0 {
                        if f(o.placement_at[first(anchors)] as u16) {
                            return true;
                        }
                        anchors &= anchors - 1;
                    }
                }
            }
            return false;
        }
        let g = self.gen_ctx(player);
        for piece in (0..NPIECES).rev() {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                let mut anchors = legal_anchors(o, &g);
                while anchors != 0 {
                    if f(o.placement_at[first(anchors)] as u16) {
                        return true;
                    }
                    anchors &= anchors - 1;
                }
            }
        }
        false
    }

    pub fn moves(&self, t: &Tables, out: &mut Vec<u16>) {
        out.clear();
        self.for_each_move(t, self.stm as usize, self.pieces_now(), |m| out.push(m));
    }

    pub fn count_moves(&self, t: &Tables) -> u64 {
        let pieces = self.pieces_now();
        if pieces == 0 {
            return 0;
        }
        let g = self.gen_ctx(self.stm as usize);
        let mut n = 0u64;
        for piece in 0..NPIECES {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                n += count(legal_anchors(o, &g)) as u64;
            }
        }
        n
    }

    /// The number of legal placements of `player` among `pieces`.
    pub fn count_moves_for(&self, t: &Tables, player: usize, pieces: u8) -> u32 {
        if pieces == 0 {
            return 0;
        }
        let g = self.gen_ctx(player);
        let mut n = 0;
        for piece in 0..NPIECES {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                n += count(legal_anchors(o, &g));
            }
        }
        n
    }

    pub fn has_move(&self, t: &Tables, player: usize, pieces: u8) -> bool {
        if pieces == 0 {
            return false;
        }
        let g = self.gen_ctx(player);
        for piece in 0..NPIECES {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                if legal_anchors(o, &g) != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// The position text of the reference: red_blue_redpieces_bluepieces_side_ply_passes.
    pub fn to_text(&self) -> String {
        format!("{:032x}_{:032x}_{}_{}_{}_{}_{}", self.cells[0], self.cells[1], self.left[0], self.left[1], self.stm, self.ply, self.passes)
    }

    pub fn from_text(text: &str) -> Option<Pos> {
        let f: Vec<&str> = text.trim().split('_').collect();
        if f.len() != 7 {
            return None;
        }
        let mut pos = Pos::initial();
        pos.cells[0] = u128::from_str_radix(f[0], 16).ok()?;
        pos.cells[1] = u128::from_str_radix(f[1], 16).ok()?;
        pos.left[0] = f[2].parse().ok()?;
        pos.left[1] = f[3].parse().ok()?;
        pos.stm = f[4].parse().ok()?;
        pos.ply = f[5].parse().ok()?;
        pos.passes = f[6].parse().ok()?;
        pos.recompute();
        Some(pos)
    }

    /// A position from a list of moves from the start; None if a move is unknown or illegal.
    pub fn from_moves(t: &Tables, moves: &[&str]) -> Option<Pos> {
        let mut pos = Pos::initial();
        for text in moves {
            let m = parse_move(t, text)?;
            if m == PASS {
                if pos.has_move(t, pos.stm as usize, pos.pieces_now()) {
                    return None;
                }
            } else {
                let p = &t.placements[m as usize];
                if pos.pieces_now() >> p.piece & 1 == 0 || !pos.legal_placement(t, pos.stm as usize, p) {
                    return None;
                }
            }
            pos.play(t, m);
        }
        Some(pos)
    }

    /// Accepts either a position text or a whitespace-separated move list.
    pub fn parse(t: &Tables, text: &str) -> Option<Pos> {
        let text = text.trim();
        if text.contains(':') || text.is_empty() || text.starts_with("pass") {
            let moves: Vec<&str> = text.split_whitespace().collect();
            Pos::from_moves(t, &moves)
        } else {
            Pos::from_text(text)
        }
    }
}

/// The anchor cells where orientation `o` starts a new snake.
#[inline(always)]
pub fn new_anchors(o: &Orientation, g: &Gen) -> BB {
    let mut a = g.free_new;
    for k in 1..o.size as usize {
        a &= (g.free_new & o.win[k]) >> o.shift[k];
    }
    a
}

/// The anchor cells where orientation `o` grows an own snake.
#[inline(always)]
pub fn grow_anchors(o: &Orientation, g: &Gen) -> BB {
    let mut grow = g.allowed[o.inner[0] as usize];
    let mut touch = g.next_own;
    for k in 1..o.size as usize {
        grow &= (g.allowed[o.inner[k] as usize] & o.win[k]) >> o.shift[k];
        touch |= (g.next_own & o.win[k]) >> o.shift[k];
    }
    grow &= touch;
    if grow != 0 {
        for &(q, r) in &o.wedge {
            grow &= !shift_in(g.own, q, r);
        }
        for &(q, r) in &o.bridge {
            grow &= !shift_in(g.own_with_nb, q, r);
        }
    }
    grow
}

#[inline(always)]
pub fn legal_anchors(o: &Orientation, g: &Gen) -> BB {
    let a = new_anchors(o, g);
    if g.next_own == 0 {
        a
    } else {
        a | grow_anchors(o, g)
    }
}
