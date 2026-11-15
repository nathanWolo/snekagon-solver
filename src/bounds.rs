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

pub const MAX_CLUSTERS: usize = 24;

/// A player's clusters: the growable cells of each and the longest final snake possible inside it.
pub struct Clusters {
    pub n: usize,
    pub cells: [BB; MAX_CLUSTERS],
    pub upper: [i32; MAX_CLUSTERS],
    /// More clusters than the list holds: the list is not complete.
    pub overflow: bool,
}

impl Clusters {
    pub fn new() -> Clusters {
        Clusters { n: 0, cells: [0; MAX_CLUSTERS], upper: [0; MAX_CLUSTERS], overflow: false }
    }
}

/// The upper bound, cluster by cluster: regions of growable cells, merged by the snakes touching them.
#[inline]
pub fn upper_bound(t: &Tables, pos: &Pos, player: usize) -> i32 {
    upper_bound_ex(t, pos, player, 0, None)
}

/// The upper bound of `player` when the empty cells `blocked` are taken by the opponent as well;
/// with `rec`, the clusters are recorded there.
#[inline]
pub fn upper_bound_ex(t: &Tables, pos: &Pos, player: usize, blocked: BB, mut rec: Option<&mut Clusters>) -> i32 {
    if let Some(r) = rec.as_deref_mut() {
        r.n = 0;
        r.overflow = false;
    }
    if pos.left[player] == 0 {
        return pos.longest[player] as i32;
    }
    let ctx = BoundCtx::new(pos, player);
    upper_bound_cells(t, &ctx, growable(pos, player) & !blocked, rec)
}

/// What the upper bound needs to know about a player's snakes: the same for every set of cells.
pub struct BoundCtx {
    lower: i32,
    left: u8,
    ns: usize,
    snake_nb: [BB; MAX_SNAKES],
    snake_len: [i32; MAX_SNAKES],
}

impl BoundCtx {
    #[inline(always)]
    pub fn new(pos: &Pos, player: usize) -> BoundCtx {
        let ns = pos.nsnakes[player] as usize;
        let mut snake_nb = [0u128; MAX_SNAKES];
        let mut snake_len = [0i32; MAX_SNAKES];
        for j in 0..ns {
            snake_nb[j] = neighbours(pos.snakes[player][j]);
            snake_len[j] = count(pos.snakes[player][j]) as i32;
        }
        BoundCtx { lower: pos.longest[player] as i32, left: pos.left[player], ns, snake_nb, snake_len }
    }
}

/// The bound over the growable cells `rest` alone: the longest snake now, or the best cluster made
/// of those cells and the snakes touching them. `rest` must be whole clusters (minus blocked cells):
/// then the snakes touching it touch no growable cell outside it. The player has a piece left.
#[inline(always)]
pub fn upper_bound_cells(t: &Tables, ctx: &BoundCtx, mut rest: BB, mut rec: Option<&mut Clusters>) -> i32 {
    let lower = ctx.lower;
    let fill = &t.best_fill[ctx.left as usize];
    let joinable = ctx.left.count_ones() as usize + 1;
    let ns = ctx.ns;
    let snake_nb = &ctx.snake_nb;
    let snake_len = &ctx.snake_len;
    let mut upper = lower;
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
        if let Some(r) = rec.as_deref_mut() {
            if r.n < MAX_CLUSTERS {
                r.cells[r.n] = cluster;
                r.upper[r.n] = reach;
                r.n += 1;
            } else {
                r.overflow = true;
            }
        }
    }
    upper
}

/// Can `player` still build a snake of at least `target` cells? The opponent is frozen: its moves
/// only take placements away, so false is a proof for the real game too. `work` is the number of
/// positions the search may still look at; when it runs out the answer is true (no proof).
///
/// Why extending one snake is enough: the final snake is connected, so its new pieces can be put in
/// an order in which each touches what is already there (an old snake of it, or else the piece that
/// was placed first in the game). Each such placement is legal now if it was legal in the game: the
/// own cells around it are a subset of the final ones, and fewer tiles are in its way.
pub fn can_reach(t: &Tables, pos: &Pos, player: usize, target: i32, cl: &Clusters, work: &mut u32) -> bool {
    if pos.longest[player] as i32 >= target {
        return true;
    }
    // the first piece goes into a cluster that could hold such a snake
    let mut good: BB = 0;
    if cl.overflow {
        good = ALL;
    } else {
        for i in 0..cl.n {
            if cl.upper[i] >= target {
                good |= cl.cells[i];
            }
        }
    }
    good != 0 && reach_from(t, pos, player, target, 0, good, work)
}

/// The same with the clusters computed here.
pub fn can_reach_alone(t: &Tables, pos: &Pos, player: usize, target: i32, work: &mut u32) -> bool {
    let mut cl = Clusters::new();
    upper_bound_ex(t, pos, player, 0, Some(&mut cl));
    can_reach(t, pos, player, target, &cl, work)
}

/// The same without clusters. When the target is more than the cells in hand, the final snake holds
/// an old snake; its new pieces can be ordered outward from that snake, so the first piece touches an
/// own snake and each later piece touches what has been built.
pub fn can_reach_touching(t: &Tables, pos: &Pos, player: usize, target: i32, work: &mut u32) -> bool {
    if pos.longest[player] as i32 >= target {
        return true;
    }
    let left = pos.left[player];
    let mut rem = 0i32;
    for p in 0..NPIECES {
        if left >> p & 1 == 1 {
            rem += PIECE_SIZE[p] as i32;
        }
    }
    if target > rem {
        pos.cells[player] != 0 && reach_from(t, pos, player, target, pos.cells[player], ALL, work)
    } else {
        reach_from(t, pos, player, target, 0, ALL, work)
    }
}

/// `snake`: the snake being built (0 before the first piece); `good`: where the first piece may go.
fn reach_from(t: &Tables, pos: &Pos, player: usize, target: i32, snake: BB, good: BB, work: &mut u32) -> bool {
    if *work == 0 {
        return true;
    }
    *work -= 1;
    let left = pos.left[player];
    let ns = pos.nsnakes[player] as usize;
    let mut len_of = [0i32; MAX_SNAKES];
    for j in 0..ns {
        len_of[j] = count(pos.snakes[player][j]) as i32;
    }
    let pieces = left.count_ones() as usize;
    if pieces == 1 {
        // the last piece: one look at its placements
        return pos.find_move(t, player, left, snake, |m| {
            let p = &t.placements[m as usize];
            if snake == 0 && p.cells & good == 0 {
                return false;
            }
            let mut len = p.n as i32;
            for j in 0..ns {
                if pos.snakes[player][j] & p.border != 0 {
                    len += len_of[j];
                }
            }
            len >= target
        });
    }
    let mut rem = 0i32;
    for p in 0..NPIECES {
        if left >> p & 1 == 1 {
            rem += PIECE_SIZE[p] as i32;
        }
    }
    // A piece is a path: one end touches the snake being built, so it joins at most one more old
    // snake. extra[r]: the most that r further pieces can add in old snakes.
    let mut sorted = len_of;
    sorted[..ns].sort_unstable_by(|a, b| b.cmp(a));
    let mut extra = [0i32; MAX_SNAKES + 1];
    for j in 0..ns {
        extra[j + 1] = extra[j] + sorted[j];
    }
    pos.find_move(t, player, left, snake, |m| {
        let p = &t.placements[m as usize];
        if snake == 0 && p.cells & good == 0 {
            return false;
        }
        let mut len = p.n as i32;
        for j in 0..ns {
            if pos.snakes[player][j] & p.border != 0 {
                len += len_of[j];
            }
        }
        if len >= target {
            return true;
        }
        let later = pieces - 1;
        if len + rem - p.n as i32 + extra[later.min(ns)] < target {
            return false;
        }
        let mut child = *pos;
        let joined = child.place_for(t, player, m);
        if later > 1 && upper_bound(t, &child, player) < target {
            return false;
        }
        reach_from(t, &child, player, target, joined, 0, work)
    })
}

/// The longest snake `player` could build with the opponent frozen, by trying every sequence of
/// placements. Slow; `verify` checks can_reach against it.
pub fn one_player_optimum(t: &Tables, pos: &Pos, player: usize) -> i32 {
    let mut best = pos.longest[player] as i32;
    let mut moves = Vec::new();
    pos.for_each_move(t, player, pos.left[player], |m| moves.push(m));
    for m in moves {
        let mut child = *pos;
        child.place_for(t, player, m);
        best = best.max(one_player_optimum(t, &child, player));
    }
    best
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
