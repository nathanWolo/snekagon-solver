//! Shadow checks (feature `shadow`): every routine changed since f4e5efc also computes the answer of
//! the old code, and the program aborts with the inputs on any difference. A verification tool only.
#![allow(dead_code)]

use crate::bounds::*;
use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub const NAMES: [&str; 12] = [
    "grow_anchors_in (598ec2e)",
    "find_move touching (0f0a9fc)",
    "find_grow_both (2768e0c)",
    "upper_bound_cells_upto (0a78a08/79e5d8c)",
    "try_threat decision (79e5d8c)",
    "scan_threats decision (0a78a08)",
    "can_reach_touching answer (2768e0c/1d857c7)",
    "can_reach_touching work left",
    "witness yes vs full test, same work (938a220)",
    "witness yes vs exhaustive test (938a220)",
    "witness recorded: replays as legal (938a220)",
    "can_reach_touching vs exhaustive, decided no",
];
pub static COUNT: [AtomicU64; 12] = [const { AtomicU64::new(0) }; 12];
/// Witness answers where the full test with the same work would have said no (not a wrong value:
/// a yes only skips a cut; but it contradicts the claim "same nodes").
pub static WIT_DIFF_SAME_WORK: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn hit(i: usize) {
    COUNT[i].fetch_add(1, Relaxed);
}

pub fn fail(what: &str, detail: String) -> ! {
    eprintln!("SHADOW MISMATCH in {}: {}", what, detail);
    report();
    std::process::abort();
}

pub fn report() {
    eprintln!("# shadow comparisons:");
    for i in 0..NAMES.len() {
        eprintln!("#   {:48} {}", NAMES[i], COUNT[i].load(Relaxed));
    }
    eprintln!("#   witness yes where the same-work test said no: {}", WIT_DIFF_SAME_WORK.load(Relaxed));
}

/// The grow_anchors of f4e5efc.
#[inline]
pub fn old_grow_anchors(o: &Orientation, g: &Gen) -> BB {
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

#[inline]
fn old_legal_anchors(o: &Orientation, g: &Gen) -> BB {
    let a = new_anchors(o, g);
    if g.next_own == 0 {
        a
    } else {
        a | old_grow_anchors(o, g)
    }
}

/// Every placement the f4e5efc find_move offers with `touching` != 0, in its order.
pub fn old_find_move_list(t: &Tables, pos: &Pos, player: usize, pieces: u8, touching: BB) -> Vec<u16> {
    let mut out = Vec::new();
    if pieces == 0 {
        return out;
    }
    let mut g = pos.gen_ctx(player);
    if touching != 0 {
        g.free_new = 0;
        g.next_own &= neighbours(touching);
        if g.next_own == 0 {
            return out;
        }
    }
    for piece in (0..NPIECES).rev() {
        if pieces >> piece & 1 == 0 {
            continue;
        }
        for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
            let mut anchors = old_legal_anchors(o, &g);
            while anchors != 0 {
                out.push(o.placement_at[first(anchors)] as u16);
                anchors &= anchors - 1;
            }
        }
    }
    out
}

/// A position with the player's cells `own`, all occupied `occ` (opponent = occ minus own).
pub fn pos_from(own: BB, occ: BB, player: usize, pieces: u8) -> Pos {
    let mut p = Pos::initial();
    p.cells[player] = own;
    p.cells[player ^ 1] = occ & !own;
    p.left[player] = pieces;
    p.recompute();
    p
}

/// The can_reach_touching of f4e5efc.
pub fn old_can_reach_touching(t: &Tables, pos: &Pos, player: usize, target: i32, work: &mut u32) -> bool {
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
    let ns = pos.nsnakes[player] as usize;
    let mut len_of = [0i32; MAX_SNAKES];
    for j in 0..ns {
        len_of[j] = count(pos.snakes[player][j]) as i32;
    }
    let mut start: BB = 0;
    if target > rem {
        for j in 0..ns {
            let (mut b1, mut b2) = (0i32, 0i32);
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
        return old_find_move(t, pos, player, left, start, |m| {
            let p = &t.placements[m as usize];
            let mut len = p.n as i32;
            for j in 0..ns {
                if pos.snakes[player][j] & p.border != 0 {
                    len += len_of[j];
                }
            }
            len >= target
        });
    }
    if *work == 0 {
        return true;
    }
    *work -= 1;
    let mut ends: BB = 0;
    let mut have_ends = false;
    old_find_move(t, pos, player, left, start, |m| {
        let p = &t.placements[m as usize];
        let mut len = p.n as i32;
        let mut top = 0i32;
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
            return true;
        }
        let last = rem - p.n as i32;
        let gap = target - len - last;
        if gap > 0 {
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
        let mut child = *pos;
        let built = child.place_for(t, player, m);
        old_reach_from(t, &child, player, target, built, 0, work)
    })
}

/// The f4e5efc find_move (old generator throughout).
pub fn old_find_move<F: FnMut(u16) -> bool>(t: &Tables, pos: &Pos, player: usize, pieces: u8, touching: BB, mut f: F) -> bool {
    if pieces == 0 {
        return false;
    }
    let mut g = pos.gen_ctx(player);
    if touching != 0 {
        g.free_new = 0;
        g.next_own &= neighbours(touching);
        if g.next_own == 0 {
            return false;
        }
    }
    for piece in (0..NPIECES).rev() {
        if pieces >> piece & 1 == 0 {
            continue;
        }
        for o in &t.orientations[t.first_orientation[piece]..t.first_orientation[piece + 1]] {
            let mut anchors = old_legal_anchors(o, &g);
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

/// The f4e5efc reach_from (unchanged there), on the old generator.
fn old_reach_from(t: &Tables, pos: &Pos, player: usize, target: i32, snake: BB, good: BB, work: &mut u32) -> bool {
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
        return old_find_move(t, pos, player, left, snake, |m| {
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
    let mut sorted = len_of;
    sorted[..ns].sort_unstable_by(|a, b| b.cmp(a));
    let mut extra = [0i32; MAX_SNAKES + 1];
    for j in 0..ns {
        extra[j + 1] = extra[j] + sorted[j];
    }
    old_find_move(t, pos, player, left, snake, |m| {
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
        old_reach_from(t, &child, player, target, joined, 0, work)
    })
}

/// Whether the witness's own cells, pieces and mask describe a real sequence: some sequence of at
/// most two legal placements from the player's pieces, each needing only mask cells empty, reaching
/// `len` (checked by brute force on the position made of the own cells and the mask as the only empty
/// cells... approximated: on the actual position the witness was recorded for).
pub fn witness_replays(t: &Tables, pos: &Pos, player: usize, w: &Witness) -> bool {
    // the witness must reach len by placing pieces only on mask cells, legally, in pos
    let target = w.len;
    let mut best = pos.longest[player] as i32;
    let mut moves = Vec::new();
    pos.for_each_move(t, player, pos.left[player], |m| moves.push(m));
    for m in moves {
        let p = &t.placements[m as usize];
        if p.cells & !w.mask != 0 {
            continue;
        }
        let mut c = *pos;
        c.place_for(t, player, m);
        best = best.max(c.longest[player] as i32);
        let mut moves2 = Vec::new();
        c.for_each_move(t, player, c.left[player], |m2| moves2.push(m2));
        for m2 in moves2 {
            let p2 = &t.placements[m2 as usize];
            if p2.cells & !w.mask != 0 {
                continue;
            }
            let mut c2 = c;
            c2.place_for(t, player, m2);
            best = best.max(c2.longest[player] as i32);
        }
    }
    best >= target
}
