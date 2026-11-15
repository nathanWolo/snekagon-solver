//! Board geometry and bitboards.
//!
//! The board is a hexagon of side 6 (91 cells) in axial coordinates (q, r) with |q|, |r|, |q+r| <= 5.
//! A cell's index is (r+5)*11 + (q+5), so any set of cells fits in one u128 and the six neighbours of
//! a cell are at index offsets +1, -1, +11, -11, -10 and +10.

pub type BB = u128;

pub const RADIUS: i32 = 5;
pub const WIDTH: i32 = 11;
pub const GRID: usize = 121;
pub const NCELLS: usize = 91;
pub const NSYM: usize = 12;

/// The six directions in ring order: directions d and d+1 lead to cells that touch each other.
pub const DIR_Q: [i32; 6] = [1, 1, 0, -1, -1, 0];
pub const DIR_R: [i32; 6] = [0, -1, -1, 0, 1, 1];

#[inline(always)]
pub const fn cell_index(q: i32, r: i32) -> usize {
    ((r + RADIUS) * WIDTH + (q + RADIUS)) as usize
}
#[inline(always)]
pub const fn cell_q(i: usize) -> i32 {
    (i as i32) % WIDTH - RADIUS
}
#[inline(always)]
pub const fn cell_r(i: usize) -> i32 {
    (i as i32) / WIDTH - RADIUS
}
pub const fn on_board(q: i32, r: i32) -> bool {
    q.abs() <= RADIUS && r.abs() <= RADIUS && (q + r).abs() <= RADIUS
}
pub fn adjacent(q1: i32, r1: i32, q2: i32, r2: i32) -> bool {
    (0..6).any(|d| q1 + DIR_Q[d] == q2 && r1 + DIR_R[d] == r2)
}

/// The 91 cells of the board.
pub const ALL: BB = {
    let mut m = 0u128;
    let mut i = 0;
    while i < GRID {
        if on_board(cell_q(i), cell_r(i)) {
            m |= 1u128 << i;
        }
        i += 1;
    }
    m
};
/// Cells with q < 5: a step in +q stays in the same grid row.
pub const NOT_EAST: BB = {
    let mut m = 0u128;
    let mut i = 0;
    while i < GRID {
        if on_board(cell_q(i), cell_r(i)) && cell_q(i) < RADIUS {
            m |= 1u128 << i;
        }
        i += 1;
    }
    m
};
/// Cells with q > -5.
pub const NOT_WEST: BB = {
    let mut m = 0u128;
    let mut i = 0;
    while i < GRID {
        if on_board(cell_q(i), cell_r(i)) && cell_q(i) > -RADIUS {
            m |= 1u128 << i;
        }
        i += 1;
    }
    m
};
/// COLWIN[dq + 8]: grid cells whose q lies in [-5 + dq, 5 + dq]. Shifting only these by dq columns
/// cannot wrap into a neighbouring grid row.
pub const COLWIN: [BB; 17] = {
    let mut t = [0u128; 17];
    let mut d = 0;
    while d < 17 {
        let dq = d as i32 - 8;
        let mut i = 0;
        while i < GRID {
            let q = cell_q(i);
            if q >= -RADIUS + dq && q <= RADIUS + dq {
                t[d] |= 1u128 << i;
            }
            i += 1;
        }
        d += 1;
    }
    t
};

#[inline(always)]
pub fn bit(i: usize) -> BB {
    1u128 << i
}
#[inline(always)]
pub fn count(b: BB) -> u32 {
    b.count_ones()
}
#[inline(always)]
pub fn first(b: BB) -> usize {
    b.trailing_zeros() as usize
}

/// All cells adjacent to at least one cell of b.
#[inline(always)]
pub fn neighbours(b: BB) -> BB {
    let e = b & NOT_EAST;
    let w = b & NOT_WEST;
    ((e << 1) | (w >> 1) | (b << 11) | (b >> 11) | (e >> 10) | (w << 10)) & ALL
}

/// The cells x for which x + (dq, dr) is in b.
#[inline(always)]
pub fn shift_in(b: BB, dq: i32, dr: i32) -> BB {
    let src = b & COLWIN[(dq + 8) as usize];
    let s = dr * WIDTH + dq;
    (if s >= 0 { src >> s } else { src << (-s) }) & ALL
}

/// `seed` plus all cells of `allowed` connected to it.
#[inline]
pub fn flood(seed: BB, allowed: BB) -> BB {
    let mut region = seed;
    loop {
        let next = region | (neighbours(region) & allowed);
        if next == region {
            return region;
        }
        region = next;
    }
}

/// Symmetry t (0..12): mirror (swap q and r) when t >= 6, then rotate t % 6 times by 60 degrees.
pub fn apply_symmetry(t: usize, mut q: i32, mut r: i32) -> (i32, i32) {
    if t >= 6 {
        std::mem::swap(&mut q, &mut r);
    }
    for _ in 0..t % 6 {
        let nq = -r;
        let nr = q + r;
        q = nq;
        r = nr;
    }
    (q, r)
}

pub struct Geometry {
    pub neighbours: [BB; GRID],
    pub cells: [u8; NCELLS],
    pub sym: [[u8; GRID]; NSYM],
    pub inv_sym: [u8; NSYM],
}

pub fn build_geometry() -> Geometry {
    let mut g = Geometry { neighbours: [0; GRID], cells: [0; NCELLS], sym: [[0; GRID]; NSYM], inv_sym: [0; NSYM] };
    let mut n = 0;
    for i in 0..GRID {
        if ALL >> i & 1 == 1 {
            g.cells[n] = i as u8;
            n += 1;
            g.neighbours[i] = neighbours(bit(i));
        }
    }
    assert_eq!(n, NCELLS);
    for t in 0..NSYM {
        for i in 0..GRID {
            g.sym[t][i] = i as u8;
            if ALL >> i & 1 == 1 {
                let (q, r) = apply_symmetry(t, cell_q(i), cell_r(i));
                g.sym[t][i] = cell_index(q, r) as u8;
            }
        }
    }
    for t in 0..NSYM {
        for u in 0..NSYM {
            if g.cells.iter().all(|&c| g.sym[u][g.sym[t][c as usize] as usize] == c) {
                g.inv_sym[t] = u as u8;
                break;
            }
        }
    }
    g
}
