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
/// Three or more pieces, target above the cells in hand: the first piece must touch an old snake that
/// can be the longest old snake of the final snake (`--reach-start3`).
pub static START3: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

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
    if pieces > 2 && target > rem && START3.load(std::sync::atomic::Ordering::Relaxed) {
        // The final snake holds old snakes; built outward from its longest old snake S, the first
        // piece touches S, every other old snake in it is no longer than S, and each piece brings in
        // at most one more snake (a piece touches own cells only with its two ends): so S can only
        // be a snake whose length, the cells in hand and the `pieces` longest other snakes no
        // longer than it reach the target.
        let ns = pos.nsnakes[player] as usize;
        let mut len_of = [0i32; MAX_SNAKES];
        for j in 0..ns {
            len_of[j] = count(pos.snakes[player][j]) as i32;
        }
        // `top` keeps 4 lengths and at most min(pieces, 4) of them are summed. That is enough: every
        // snake was started by a piece already placed, and a player has 8 pieces in all (blue 7), so
        // snakes + pieces in hand <= 8. With 4 or fewer pieces in hand the sum takes `pieces` lengths;
        // with 5 or more there are at most 3 snakes, so at most 2 others to sum.
        debug_assert!(ns + pieces as usize <= 8, "more snakes and pieces than a player has");
        let mut start: BB = 0;
        for j in 0..ns {
            let mut top = [0i32; 4]; // the longest other snakes no longer than j, longest first
            for i in 0..ns {
                if i != j && len_of[i] <= len_of[j] {
                    let mut v = len_of[i];
                    for k in 0..4 {
                        if v > top[k] {
                            std::mem::swap(&mut v, &mut top[k]);
                        }
                    }
                }
            }
            let mut others = 0;
            for k in 0..(pieces as usize).min(4) {
                others += top[k];
            }
            if len_of[j] + rem + others >= target {
                start |= pos.snakes[player][j];
            }
        }
        return start != 0 && reach_from(t, pos, player, target, start, ALL, work);
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

/// E33: the most extensions `sure_next_length` keeps (the longest; dropping some only lowers G).
pub const MAX_EXT: usize = 48;

/// E33, the opponent's sure next length G: the side to move is `me`, the opponent `opp`. G is the
/// largest length L such that after every legal move of `me` the opponent still has a legal
/// placement that makes its longest snake at least L. Returns G when G >= `floor`; otherwise some
/// number below `floor` (the opponent's longest snake now, or more if `floor` is above it).
///
/// Why G is a lower bound on the opponent's final longest snake. Let m be the move `me` makes.
///  - If `me` has a legal placement it must place one (a player passes only without one). m adds
///    cells of `me` and nothing else: the opponent's cells, snakes and pieces stay. An opponent
///    placement e that grows one of its snakes is legal exactly when its cells are empty and the
///    opponent's own cells allow it, so it stays legal unless m covers a cell of e. A placement
///    that starts a new snake also needs its border free of every tile, so it stays legal unless m
///    covers a cell of e or of its border. That set is e's `block` here.
///  - After m (a placement) the passes are 0 and the opponent holds a piece (e uses one), so the
///    game goes on and the opponent may play e. Its snakes are those of now, so the snake e makes
///    has the length `longest_after` gives now; snakes never shrink, so its final longest is at
///    least that. Taking the best e that m leaves legal, and the worst m for the opponent, gives G.
///  - If `me` has no legal placement it passes. With passes = 0 now the game goes on (one pass)
///    and every e stays legal: G is the longest extension. With a pass already made the game ends
///    at our pass: G is the opponent's longest snake now. (A player without pieces has no legal
///    placement; that case is the same.)
///  - A placement joins at most two snakes (only a piece's two ends can touch own cells, and the
///    1-cell piece has at most two own neighbours), so no extension is longer than the largest
///    piece in hand plus the two longest snakes: when that is below `floor` nothing is generated.
///  - Only extensions of at least `floor` cells are collected, at most `MAX_EXT` of them, the
///    longest kept. Leaving extensions out can only make "every move blocks them all" easier to
///    meet, so the length found is never above the true G (and is exact when nothing is left out).
///
/// `cand` is scratch space (the moves of `me` that block every extension seen so far).
pub fn sure_next_length(t: &Tables, pos: &Pos, floor: i32, cand: &mut Vec<BB>) -> i32 {
    let me = pos.stm as usize;
    let opp = me ^ 1;
    let now = pos.longest[opp] as i32;
    let floor = floor.max(now + 1);
    let left = pos.left[opp];
    if left == 0 {
        return now;
    }
    let ns = pos.nsnakes[opp] as usize;
    let mut len_of = [0i32; MAX_SNAKES];
    let (mut l1, mut l2) = (0i32, 0i32);
    for j in 0..ns {
        let l = count(pos.snakes[opp][j]) as i32;
        len_of[j] = l;
        if l > l1 {
            l2 = l1;
            l1 = l;
        } else if l > l2 {
            l2 = l;
        }
    }
    let mut maxn = 0i32;
    for p in 0..NPIECES {
        if left >> p & 1 == 1 {
            maxn = maxn.max(PIECE_SIZE[p] as i32);
        }
    }
    if maxn + l1 + l2 < floor {
        return now;
    }
    // the extensions of at least `floor` cells, longest first: (length, block)
    let mut ext = [(0i32, 0 as BB); MAX_EXT];
    let mut ne = 0usize;
    let mut push = |len: i32, block: BB| {
        let mut i = if ne < MAX_EXT {
            ne += 1;
            ne - 1
        } else if len > ext[MAX_EXT - 1].0 {
            MAX_EXT - 1
        } else {
            return;
        };
        while i > 0 && ext[i - 1].0 < len {
            ext[i] = ext[i - 1];
            i -= 1;
        }
        ext[i] = (len, block);
    };
    let own = pos.cells[opp];
    if floor > maxn {
        // a new snake is at most `maxn` long: only placements that grow a snake can reach `floor`,
        // and only by touching a snake j with n + len(j) + (the longest snake) >= floor
        let g = gen_grow(own, pos.occupied());
        for piece in 0..NPIECES {
            if left >> piece & 1 == 0 {
                continue;
            }
            let n = PIECE_SIZE[piece] as i32;
            let mut a: BB = 0;
            for j in 0..ns {
                if n + len_of[j] + l1 >= floor {
                    a |= pos.snakes[opp][j];
                }
            }
            if a == 0 {
                continue;
            }
            let g2 = Gen { free_new: 0, allowed: g.allowed, next_own: g.next_own & neighbours(a), own: g.own, own_with_nb: g.own_with_nb };
            if g2.next_own == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                let mut anchors = grow_anchors(o, &g2);
                while anchors != 0 {
                    let p = &t.placements[o.placement_at[first(anchors)] as usize];
                    anchors &= anchors - 1;
                    let len = pos.longest_after(opp, p) as i32;
                    if len >= floor {
                        push(len, p.cells);
                    }
                }
            }
        }
    } else {
        pos.for_each_move(t, opp, left, |m| {
            let p = &t.placements[m as usize];
            let len = pos.longest_after(opp, p) as i32;
            if len >= floor {
                push(len, if p.border & own != 0 { p.cells } else { p.cells | p.border });
            }
        });
    }
    if ne == 0 {
        return now;
    }
    // the longest length: the moves of ours that block all its extensions, found among the moves
    // that block the one with the fewest cells to cover
    let top = ext[0].0;
    let mut e = 1;
    while e < ne && ext[e].0 == top {
        e += 1;
    }
    let mut star = 0;
    for i in 1..e {
        if count(ext[i].1) < count(ext[star].1) {
            star = i;
        }
    }
    let b = ext[star].1;
    let pieces = pos.pieces_now();
    cand.clear();
    if pieces != 0 {
        let g = pos.gen_ctx(me);
        for piece in 0..NPIECES {
            if pieces >> piece & 1 == 0 {
                continue;
            }
            for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
                // the anchors at which some cell of the orientation lies in b (cell k sits at
                // anchor + shift[k]); a superset, checked exactly below
                let mut hit = b;
                for k in 1..o.size as usize {
                    hit |= b >> o.shift[k];
                }
                let mut anchors = legal_anchors(o, &g) & hit;
                while anchors != 0 {
                    let c = t.placements[o.placement_at[first(anchors)] as usize].cells;
                    anchors &= anchors - 1;
                    if c & b != 0 && ext[..e].iter().all(|x| c & x.1 != 0) {
                        cand.push(c);
                    }
                }
            }
        }
    }
    if cand.is_empty() {
        // no legal move of ours blocks them all (or we have none and pass)
        if pos.passes > 0 && !pos.has_move(t, me, pieces) {
            return now; // our pass ends the game
        }
        return top;
    }
    // each shorter length in turn: the moves that also block all of its extensions
    let mut s = e;
    while s < ne {
        let len = ext[s].0;
        let mut e2 = s + 1;
        while e2 < ne && ext[e2].0 == len {
            e2 += 1;
        }
        cand.retain(|&c| ext[s..e2].iter().all(|x| c & x.1 != 0));
        if cand.is_empty() {
            return len; // every move of ours leaves one of length len or more (we have one)
        }
        s = e2;
    }
    now
}

/// G by brute force (tests and the check build): the minimum over the legal moves of the side to
/// move of the opponent's longest snake after its best next placement, or after its pass; with no
/// legal move of ours, after our pass.
pub fn sure_next_length_brute(t: &Tables, pos: &Pos) -> i32 {
    let me = pos.stm as usize;
    let opp = me ^ 1;
    let best_after = |child: &Pos| -> i32 {
        if child.is_over() {
            return child.longest[opp] as i32;
        }
        let mut best = child.longest[opp] as i32;
        child.for_each_move(t, opp, child.pieces_now(), |r| best = best.max(child.longest_after(opp, &t.placements[r as usize]) as i32));
        best
    };
    let mut moves = Vec::new();
    pos.moves(t, &mut moves);
    if moves.is_empty() {
        let mut child = *pos;
        child.play(t, PASS);
        return best_after(&child);
    }
    let mut g = i32::MAX;
    for m in moves {
        let mut child = *pos;
        child.play(t, m);
        g = g.min(best_after(&child));
    }
    g
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::tables;
    use crate::pos::Pos;
    use std::sync::atomic::Ordering::Relaxed;

    /// The reach test against every sequence of placements, for every target from one above the
    /// longest snake to two above the optimum, with the three-piece test from the longest snake
    /// (START3) off and on.
    fn agrees_with_brute_force(text: &str, player: usize) -> i32 {
        let t = tables();
        let pos = Pos::from_text(text).expect("bad position text");
        let opt = one_player_optimum(t, &pos, player);
        for s3 in [false, true] {
            START3.store(s3, Relaxed);
            for target in pos.longest[player] as i32 + 1..=opt + 2 {
                let mut work = u32::MAX;
                assert_eq!(can_reach_touching(t, &pos, player, target, &mut work), target <= opt, "start3 {} target {} optimum {} at {}", s3, target, opt, text);
            }
        }
        START3.store(true, Relaxed);
        opt
    }

    /// The k term of START3: the final snake must take in S and three other old snakes with a hand
    /// of three pieces (the verifier's position; red snakes of 4, 4, 4, 1 and a bar of 4, hand
    /// 2 + 3 + 3). With k - 1 other snakes counted the test says "cannot" for 21.
    #[test]
    fn start3_counts_one_other_snake_per_piece() {
        let t = tables();
        let text = "000100000100200478800801002084e0_00000000000000000000000000000000_14_14_0_12_0";
        let pos = Pos::from_text(text).unwrap();
        for s3 in [false, true] {
            START3.store(s3, Relaxed);
            let mut work = u32::MAX;
            assert!(can_reach_touching(t, &pos, 0, 21, &mut work), "start3 {}: red can reach 21", s3);
        }
        START3.store(true, Relaxed);
        let opt = agrees_with_brute_force(text, 0);
        assert!(opt >= 21, "optimum {}", opt);
        // a neighbour: a fifth snake of 5 next to the bar (S is now longer than the snakes it
        // must take in; the snakes of 4 tie with each other)
        agrees_with_brute_force("0001000001002004f8800801002084e0_00000000000000000000000000000000_14_14_0_12_0", 0);
        // a neighbour: a blue tile in the gap between the first two snakes of the ring
        agrees_with_brute_force("000100000100200478800801002084e0_00000000000000000000000000000100_14_14_0_12_0", 0);
    }
}
