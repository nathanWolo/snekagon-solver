//! Sound bounds on a player's final longest snake.
//!
//! Lower bound: the longest snake now (snakes never shrink).
//! Upper bound, why it holds:
//!  - A player can never again use a cell next to one of its body cells, so all its future cells lie
//!    in growable = empty cells not next to its body.
//!  - The final longest snake is connected, so it lies inside one connected cluster of own + growable.
//!  - New cells come in whole pieces and each piece lies inside one region of growable cells, so a
//!    cluster gains at most best_fill(sum over its regions of best_fill(region)).
//!  - Old snakes enter the final snake whole, alternating with new pieces, so with k pieces left only
//!    the k + 1 longest snakes of a cluster can count.

use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;

#[derive(Clone, Copy)]
pub struct Bounds {
    pub lower: i32,
    pub upper: i32,
}

/// The cells `player` may still occupy.
#[inline]
pub fn growable(pos: &Pos, player: usize) -> BB {
    pos.empty() & !neighbours(shape_of(pos.cells[player]).body)
}

/// The upper bound, cluster by cluster: regions of growable cells, merged by the snakes touching them.
#[inline]
pub fn upper_bound(t: &Tables, pos: &Pos, player: usize) -> i32 {
    let lower = pos.longest[player] as i32;
    let left = pos.left[player];
    if left == 0 {
        return lower;
    }
    let fill = &t.best_fill[left as usize];
    let joinable = left.count_ones() as usize + 1;
    let ns = pos.nsnakes[player] as usize;
    let mut snake_nb = [0u128; MAX_SNAKES];
    let mut snake_len = [0i32; MAX_SNAKES];
    for j in 0..ns {
        snake_nb[j] = neighbours(pos.snakes[player][j]);
        snake_len[j] = count(pos.snakes[player][j]) as i32;
    }
    let mut upper = lower;
    let mut rest = growable(pos, player);
    let mut used = 0u32; // snakes already inside a cluster
    while rest != 0 {
        // one cluster: flood a region, pull in the snakes touching it, flood the regions they touch
        let mut cluster = flood(bit(first(rest)), rest);
        let mut capacity = fill[count(cluster) as usize] as usize;
        rest &= !cluster;
        let mut lens = [0i32; MAX_SNAKES];
        let mut nl = 0;
        let mut changed = true;
        while changed {
            changed = false;
            for j in 0..ns {
                if used >> j & 1 == 0 && snake_nb[j] & cluster != 0 {
                    used |= 1 << j;
                    lens[nl] = snake_len[j];
                    nl += 1;
                    let mut more = snake_nb[j] & rest;
                    while more != 0 {
                        let region = flood(bit(first(more)), rest);
                        rest &= !region;
                        more &= !region;
                        capacity += fill[count(region) as usize] as usize;
                        cluster |= region;
                        changed = true;
                    }
                }
            }
        }
        let mut own = 0;
        if nl > joinable {
            lens[..nl].sort_unstable_by(|a, b| b.cmp(a));
            for l in &lens[..joinable] {
                own += l;
            }
        } else {
            for l in &lens[..nl] {
                own += l;
            }
        }
        let reach = own + fill[capacity.min(GRID)] as i32;
        if reach > upper {
            upper = reach;
        }
    }
    upper
}

#[inline]
pub fn bounds(t: &Tables, pos: &Pos, player: usize) -> Bounds {
    Bounds { lower: pos.longest[player] as i32, upper: upper_bound(t, pos, player) }
}

/// The same upper bound the slow, obvious way: flood own + growable cells directly.
pub fn upper_bound_reference(t: &Tables, pos: &Pos, player: usize) -> i32 {
    let own = pos.cells[player];
    let lower = pos.longest[player] as i32;
    let left = pos.left[player];
    if left == 0 {
        return lower;
    }
    let fill = &t.best_fill[left as usize];
    let grow = growable(pos, player);
    let mut upper = lower;
    let mut rest = own | grow;
    while rest != 0 {
        let cluster = flood(bit(first(rest)), rest);
        rest &= !cluster;
        let mut g = cluster & grow;
        if g == 0 {
            continue;
        }
        let mut capacity = 0usize;
        while g != 0 {
            let region = flood(bit(first(g)), g);
            g &= !region;
            capacity += fill[count(region) as usize] as usize;
        }
        let mut lens: Vec<i32> = Vec::new();
        let mut o = cluster & own;
        while o != 0 {
            let snake = flood(bit(first(o)), o);
            o &= !snake;
            lens.push(count(snake) as i32);
        }
        lens.sort_unstable_by(|a, b| b.cmp(a));
        let joinable = (left.count_ones() as usize + 1).min(lens.len());
        let reach: i32 = lens[..joinable].iter().sum::<i32>() + fill[capacity.min(GRID)] as i32;
        upper = upper.max(reach);
    }
    upper
}
