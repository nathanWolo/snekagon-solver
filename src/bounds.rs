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

/// E12: the reach test starts from the longest snake of the target snake (`--no-longest` turns it off).
pub static FROM_LONGEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

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

/// `upper_bound_cells` without clusters recorded, for a caller that only asks whether the bound is
/// at most `limit`: the exact bound when it is, else some number above `limit` (as soon as a cluster
/// is seen to exceed it, part-built: a cluster's bound only grows as regions and snakes join it).
#[inline]
pub fn upper_bound_cells_upto(t: &Tables, ctx: &BoundCtx, mut rest: BB, limit: i32) -> i32 {
    let lower = ctx.lower;
    let fill = &t.best_fill[ctx.left as usize];
    let joinable = ctx.left.count_ones() as usize + 1;
    let ns = ctx.ns;
    let snake_nb = &ctx.snake_nb;
    let snake_len = &ctx.snake_len;
    let mut upper = lower;
    let mut used = 0u32;
    while rest != 0 {
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
                    // the part built so far: the joinable longest snakes and the capacity
                    let mut own = 0;
                    if nl > joinable {
                        let mut l2 = lens;
                        l2[..nl].sort_unstable_by(|a, b| b.cmp(a));
                        for l in &l2[..joinable] {
                            own += l;
                        }
                    } else {
                        for l in &lens[..nl] {
                            own += l;
                        }
                    }
                    let reach = own + fill[capacity.min(GRID)] as i32;
                    if reach > limit {
                        return reach;
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
        if upper > limit {
            return upper;
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
    let mut wit = Witness::NONE;
    can_reach_touching_w(t, pos, player, target, work, &mut wit)
}

/// A sequence of placements that builds a snake of `len` cells for a player with the cells `own`
/// and the pieces `left`, the opponent frozen. `mask`: the cells the sequence needs empty (its cells,
/// and the border of a piece that starts a new snake). It proves "yes" for any target up to `len` in
/// every position where the player has the same cells and pieces and `mask` is empty: placing a piece
/// that grows a snake depends only on the own cells and on its cells being empty, and a new snake also
/// on its border being empty. len = 0: no witness.
#[derive(Clone, Copy)]
pub struct Witness {
    pub own: BB,
    pub mask: BB,
    pub left: u8,
    pub len: i32,
}

impl Witness {
    pub const NONE: Witness = Witness { own: 0, mask: 0, left: 0, len: 0 };

    /// Whether this witness proves that `player` reaches `target` in `pos`.
    #[inline(always)]
    pub fn proves(&self, pos: &Pos, player: usize, target: i32) -> bool {
        self.len >= target && self.own == pos.cells[player] && self.left == pos.left[player] && self.mask & pos.occupied() == 0
    }
}

/// The cells placement `p` needs empty when played with the own cells `own`.
#[inline(always)]
fn needs_empty(p: &Placement, own: BB) -> BB {
    if p.border & own != 0 {
        p.cells
    } else {
        p.cells | p.border
    }
}

/// `can_reach_touching`; when it answers yes from a sequence it found, the sequence goes to `wit`
/// (otherwise `wit` is left alone). The answer is exactly that of `can_reach_touching`.
pub fn can_reach_touching_w(t: &Tables, pos: &Pos, player: usize, target: i32, work: &mut u32, wit: &mut Witness) -> bool {
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
    let pieces = left.count_ones();
    if pieces == 0 {
        return false;
    }
    if pieces > 2 || !FROM_LONGEST.load(std::sync::atomic::Ordering::Relaxed) {
        return if target > rem {
            pos.cells[player] != 0 && reach_from(t, pos, player, target, pos.cells[player], ALL, work)
        } else {
            reach_from(t, pos, player, target, 0, ALL, work)
        };
    }
    // One or two pieces. A final snake that holds old snakes is built outward from its longest old
    // snake S: the first piece touches S, every other old snake in it is no longer than S, and each
    // piece brings in at most one more snake (a piece touches own cells only with its two ends).
    let ns = pos.nsnakes[player] as usize;
    let mut len_of = [0i32; MAX_SNAKES];
    for j in 0..ns {
        len_of[j] = count(pos.snakes[player][j]) as i32;
    }
    let mut start: BB = 0; // the snakes that can be S; 0 = no restriction
    if target > rem {
        for j in 0..ns {
            let (mut b1, mut b2) = (0i32, 0i32); // the two longest other snakes no longer than j
            for i in 0..ns {
                if i != j && len_of[i] <= len_of[j] {
                    if len_of[i] > b1 {
                        b2 = b1;
                        b1 = len_of[i];
                    } else if len_of[i] > b2 {
                        b2 = len_of[i];
                    }
                }
            }
            let others = if pieces == 1 { b1 } else { b1 + b2 };
            if len_of[j] + rem + others >= target {
                start |= pos.snakes[player][j];
            }
        }
        if start == 0 {
            return false;
        }
    }
    if pieces == 1 {
        return pos.find_move(t, player, left, start, |m| {
            let p = &t.placements[m as usize];
            let mut len = p.n as i32;
            for j in 0..ns {
                if pos.snakes[player][j] & p.border != 0 {
                    len += len_of[j];
                }
            }
            if len >= target {
                let own = pos.cells[player];
                *wit = Witness { own, mask: needs_empty(p, own), left, len };
                true
            } else {
                false
            }
        });
    }
    if *work == 0 {
        return true;
    }
    *work -= 1;
    let mut ends: BB = 0; // own cells that can still take a neighbour: computed when first needed
    let mut have_ends = false;
    pos.find_move(t, player, left, start, |m| {
        let p = &t.placements[m as usize];
        let mut len = p.n as i32;
        let mut top = 0i32; // the longest snake this piece joins
        let mut joined: u32 = 0;
        let mut joined_cells: BB = 0;
        for j in 0..ns {
            if pos.snakes[player][j] & p.border != 0 {
                len += len_of[j];
                top = top.max(len_of[j]);
                joined |= 1 << j;
                joined_cells |= pos.snakes[player][j];
            }
        }
        if len >= target {
            let own = pos.cells[player];
            *wit = Witness { own, mask: needs_empty(p, own), left, len };
            return true;
        }
        let last = rem - p.n as i32; // the size of the other piece
        let gap = target - len - last; // what the other piece cannot add by itself
        if gap > 0 {
            // It has to bring in one more snake: at least `gap` long, no longer than the longest
            // joined so far, and with an end within last + 1 steps of where the piece can attach.
            if !have_ends {
                ends = shape_of(pos.cells[player]).ends;
                have_ends = true;
            }
            let from = p.cells | (joined_cells & ends);
            let near = &t.geo.within[(last + 1) as usize];
            let mut found = false;
            'snakes: for j in 0..ns {
                if joined >> j & 1 == 0 && len_of[j] >= gap && len_of[j] <= top {
                    let mut e = pos.snakes[player][j] & ends;
                    while e != 0 {
                        if near[first(e)] & from != 0 {
                            found = true;
                            break 'snakes;
                        }
                        e &= e - 1;
                    }
                }
            }
            if !found {
                return false;
            }
        }
        // the last piece (reach_from with one piece left, keeping the sequence it finds), on the
        // position after the first piece without building it: the built snake is p and the
        // snakes it joined, `len` cells long
        if *work == 0 {
            return true;
        }
        *work -= 1;
        let built = p.cells | joined_cells;
        let own2 = pos.cells[player] | p.cells;
        let occ2 = pos.occupied() | p.cells;
        let left2 = left & !(1u8 << p.piece);
        let mut found = PASS;
        let mut flen = 0;
        let check = |m2: u16| {
            let p2 = &t.placements[m2 as usize];
            let mut len2 = p2.n as i32 + len; // it touches the built snake
            for j in 0..ns {
                if joined >> j & 1 == 0 && pos.snakes[player][j] & p2.border != 0 {
                    len2 += len_of[j];
                }
            }
            if len2 >= target {
                found = m2;
                flen = len2;
                true
            } else {
                false
            }
        };
        let ok = if gap > 0 {
            // The last piece touches the built snake and at most one other (a piece has at most two
            // outside own neighbours): only a snake of at least `gap` cells can be that one.
            let mut other: BB = 0;
            for j in 0..ns {
                if joined >> j & 1 == 0 && len_of[j] >= gap {
                    other |= pos.snakes[player][j];
                }
            }
            find_grow_both(t, own2, occ2, left2, built, other, check)
        } else {
            find_grow(t, own2, occ2, left2, built, check)
        };
        if ok {
            let own = pos.cells[player];
            let p2 = &t.placements[found as usize];
            *wit = Witness { own, mask: needs_empty(p, own) | needs_empty(p2, own | p.cells), left, len: flen };
        }
        ok
    })
}

/// The number of cells in the pieces `left`.
#[inline]
pub fn hand_cells(left: u8) -> i32 {
    let mut rem = 0i32;
    for p in 0..NPIECES {
        if left >> p & 1 == 1 {
            rem += PIECE_SIZE[p] as i32;
        }
    }
    rem
}

/// The cells at most `rem` steps from the cells `own`. When a player holding `rem` cells in hand
/// asks for a snake longer than `rem`, only these cells matter: the final snake holds an old snake,
/// and each of its new cells is joined to an old one by a path of at most `rem` new cells. Whether
/// the player can reach a target above `rem` therefore depends on its own cells, its pieces and
/// which of these cells are occupied, and more occupied cells can only make it harder (a placement
/// legal with more cells occupied is legal with fewer: emptiness is all a placement needs from the
/// other cells). `verify` checks this against every sequence of placements.
#[inline]
pub fn near_cells(own: BB, rem: i32) -> BB {
    let mut n = own;
    for _ in 0..rem {
        n |= neighbours(n);
    }
    n
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
        if later == 1 {
            // The last piece, on the position after p without building it (what reach_from on that
            // position does, with the same work): it grows the snake p built (p and the snakes it
            // joined, `len` cells), and adds the other snakes it touches.
            if *work == 0 {
                return true;
            }
            *work -= 1;
            let mut joined: u32 = 0;
            let mut joined_cells: BB = 0;
            for j in 0..ns {
                if pos.snakes[player][j] & p.border != 0 {
                    joined |= 1 << j;
                    joined_cells |= pos.snakes[player][j];
                }
            }
            let built = p.cells | joined_cells;
            let own2 = pos.cells[player] | p.cells;
            let occ2 = pos.occupied() | p.cells;
            let left2 = left & !(1u8 << p.piece);
            return find_grow(t, own2, occ2, left2, built, |m2| {
                let p2 = &t.placements[m2 as usize];
                let mut len2 = p2.n as i32 + len;
                for j in 0..ns {
                    if joined >> j & 1 == 0 && pos.snakes[player][j] & p2.border != 0 {
                        len2 += len_of[j];
                    }
                }
                len2 >= target
            });
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
