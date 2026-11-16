//! E32: an evaluation of the position after a move, used as the move order where many pieces are
//! left (`--eval-order L`). It decides nothing; it only sorts the listed moves.
//!
//! The estimate: each side's snakes grow into the free cells it reaches first. Distances are walked
//! from the free cells next to each side's snake ends, through the cells that side may still occupy
//! (empty and not next to its body), each side on its own; a cell goes to the side that reaches it
//! in fewer steps, the side to move taking the ties. A snake's estimated final length is its
//! length plus the cells in hand, capped by the cells of its side connected to its ends; a side's
//! estimate is its best snake's, capped by the cluster bound.
//!
//! The terms, each as the mover's value minus the opponent's (the mover is the side that made the
//! move, the opponent is to move in the position evaluated):
//!   est   estimated final longest snake
//!   ub    the cluster upper bound (`upper_bound`)
//!   long  longest snake now
//!   terr  cells reached first (at most 40)
//!   hand  cells in hand
//!   ends  snake ends with a free cell the side may use
//!   short cells in hand that the best snake has no room for
//!   fresh room for a new snake: the largest group of cells that touch no tile and that the other
//!         side does not reach first, at most the cells in hand
//! The order score is the weighted sum of these terms and of the model's score of the move.
//! The weights are fitted on our own solved positions (LOG, E32).

use crate::bounds::*;
use crate::geom::*;
use crate::pos::*;
use crate::pieces::*;

pub const NFEAT: usize = 8;
pub const NAMES: [&str; NFEAT] = ["est", "ub", "long", "terr", "hand", "ends", "short", "fresh"];

/// The fitted weights, in the order of `NAMES`, then the weight of the model's score (`MODEL`, the
/// order below these levels; 64 with every other weight 0 is today's order).
pub const WEIGHTS: [i32; NFEAT + 1] = [0, 0, 0, 0, 0, 0, 0, 0, 64];

fn largest_group(mut b: BB) -> i32 {
    let mut best = 0;
    while b != 0 {
        let g = flood(bit(first(b)), b);
        b &= !g;
        best = best.max(count(g) as i32);
    }
    best
}

/// The terms of `child`, the position after a move, from the point of view of the side that moved.
pub fn features(t: &Tables, child: &Pos) -> [i32; NFEAT] {
    let s = child.stm as usize; // to move now
    let m = s ^ 1; // made the move
    let empty = child.empty();
    let occ = child.occupied();
    let sh = [shape_of(child.cells[0]), shape_of(child.cells[1])];
    let mut room = [0u128; 2];
    let mut wave = [0u128; 2];
    let mut seen = [0u128; 2];
    for p in 0..2 {
        room[p] = empty & !neighbours(sh[p].body);
        wave[p] = if child.left[p] != 0 { neighbours(sh[p].ends) & room[p] } else { 0 };
        seen[p] = wave[p];
    }
    // first arrival; the side to move takes the ties
    let mut mine = [0u128; 2];
    mine[s] = wave[s];
    mine[m] = wave[m] & !wave[s];
    for _ in 0..40 {
        if wave[0] | wave[1] == 0 {
            break;
        }
        for p in 0..2 {
            wave[p] = neighbours(wave[p]) & room[p] & !seen[p];
            seen[p] |= wave[p];
        }
        let taken = mine[0] | mine[1];
        let ns = wave[s] & !taken;
        mine[m] |= wave[m] & !taken & !ns;
        mine[s] |= ns;
    }
    let mut est = [0i32; 2];
    let mut short = [0i32; 2];
    let mut fresh = [0i32; 2];
    let mut ends = [0i32; 2];
    let mut ub = [0i32; 2];
    let free = empty & !neighbours(occ);
    for p in 0..2 {
        let hand = hand_cells(child.left[p]);
        let long = child.longest[p] as i32;
        let mut best = long;
        let mut best_room = 0;
        for j in 0..child.nsnakes[p] as usize {
            let sn = child.snakes[p][j];
            let start = neighbours(sn & sh[p].ends) & mine[p];
            let r = if start != 0 { count(flood(start, mine[p])) as i32 } else { 0 };
            let e = count(sn) as i32 + r.min(hand);
            if e > best || (e == best && r > best_room) {
                best = e;
                best_room = r;
            }
        }
        ub[p] = upper_bound(t, child, p);
        fresh[p] = if hand > 0 { largest_group(free & !mine[p ^ 1]).min(hand) } else { 0 };
        est[p] = best.max(fresh[p]).min(ub[p]).max(long);
        short[p] = (hand - best_room).max(0);
        ends[p] = count(sh[p].ends & neighbours(room[p])) as i32;
    }
    let d = |a: [i32; 2]| a[m] - a[s];
    [
        d(est),
        d(ub),
        child.longest[m] as i32 - child.longest[s] as i32,
        (count(mine[m]) as i32).min(40) - (count(mine[s]) as i32).min(40),
        hand_cells(child.left[m]) - hand_cells(child.left[s]),
        d(ends),
        d(short),
        d(fresh),
    ]
}

/// The order score of a move: higher first.
#[inline]
pub fn score(t: &Tables, child: &Pos, w: &[i32; NFEAT + 1]) -> i32 {
    let f = features(t, child);
    let mut s = 0;
    for i in 0..NFEAT {
        s += w[i] * f[i];
    }
    s
}
