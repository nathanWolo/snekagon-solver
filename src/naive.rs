//! A deliberately naive implementation of the rules, written from the rulebook with coordinates and
//! lists instead of bitboards. It exists only to check the fast code against.

use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;

fn nbrs(c: usize) -> Vec<usize> {
    let (q, r) = (cell_q(c), cell_r(c));
    let mut v = Vec::new();
    for (dq, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)] {
        if on_board(q + dq, r + dr) {
            v.push(cell_index(q + dq, r + dr));
        }
    }
    v
}

fn to_set(b: BB) -> Vec<bool> {
    (0..GRID).map(|i| b >> i & 1 == 1).collect()
}

/// Every cell has at most two own neighbours and no three cells are mutually adjacent.
fn valid_snakes(own: &[bool]) -> bool {
    for c in 0..GRID {
        if !own[c] {
            continue;
        }
        let nb: Vec<usize> = nbrs(c).into_iter().filter(|&n| own[n]).collect();
        if nb.len() > 2 {
            return false;
        }
        if nb.len() == 2 && nbrs(nb[0]).contains(&nb[1]) {
            return false;
        }
    }
    true
}

pub fn legal(pos: &Pos, player: usize, p: &Placement) -> bool {
    let me = to_set(pos.cells[player]);
    let opp = to_set(pos.cells[player ^ 1]);
    let cells: Vec<usize> = p.cell_list[..p.n as usize].iter().map(|&c| c as usize).collect();
    if cells.iter().any(|&c| me[c] || opp[c]) {
        return false;
    }
    let mut touches_any = false;
    let mut touches_me = false;
    for &c in &cells {
        for n in nbrs(c) {
            if cells.contains(&n) {
                continue;
            }
            if me[n] {
                touches_me = true;
            }
            if me[n] || opp[n] {
                touches_any = true;
            }
        }
    }
    if touches_any && !touches_me {
        return false;
    }
    let mut after = me.clone();
    for &c in &cells {
        after[c] = true;
    }
    valid_snakes(&after)
}

/// All legal placements of the side to move, in id order.
pub fn moves(t: &Tables, pos: &Pos) -> Vec<u16> {
    let player = pos.stm as usize;
    let pieces = pos.pieces_now();
    let mut out = Vec::new();
    for (id, p) in t.placements.iter().enumerate() {
        if pieces >> p.piece & 1 == 1 && legal(pos, player, p) {
            out.push(id as u16);
        }
    }
    out
}

/// The longest connected group of a player's cells.
pub fn longest(b: BB) -> u32 {
    let own = to_set(b);
    let mut seen = vec![false; GRID];
    let mut best = 0;
    for c in 0..GRID {
        if !own[c] || seen[c] {
            continue;
        }
        let mut stack = vec![c];
        seen[c] = true;
        let mut n = 0;
        while let Some(x) = stack.pop() {
            n += 1;
            for y in nbrs(x) {
                if own[y] && !seen[y] {
                    seen[y] = true;
                    stack.push(y);
                }
            }
        }
        best = best.max(n);
    }
    best
}
