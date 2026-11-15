//! The eight pieces, their orientations and every placement on the board.
//!
//! A placement is a piece on concrete cells. Placements are numbered once (piece by piece, orientation
//! by orientation, anchor cells ascending) and a move is a placement id.

use crate::geom::*;
use std::sync::OnceLock;

pub const NPIECES: usize = 8;
pub const PIECE_NAME: [&str; NPIECES] = ["1", "2", "3I", "3V", "4I", "4A", "4B", "4C"];
pub const PIECE_SIZE: [u8; NPIECES] = [1, 2, 3, 3, 4, 4, 4, 4];
pub const PIECE_SHAPE: [&[(i32, i32)]; NPIECES] = [
    &[(0, 0)],
    &[(0, 0), (1, 0)],
    &[(0, 0), (0, 1), (0, 2)],
    &[(0, 0), (1, 0), (1, 1)],
    &[(0, 0), (-1, 1), (-2, 2), (-3, 3)],
    &[(0, 0), (1, 0), (1, 1), (1, 2)],
    &[(0, 0), (0, 1), (1, 1), (1, 2)],
    &[(0, 0), (-1, 1), (-1, 2), (0, 2)],
];
pub const ALL_PIECES: u8 = 0xFF;
pub const ALL_BUT_ONE: u8 = 0xFE;
pub const PASS: u16 = u16::MAX;

#[derive(Clone, Copy)]
pub struct Placement {
    pub cells: BB,
    pub border: BB,
    pub piece: u8,
    pub n: u8,
    pub cell_list: [u8; 4],
}

/// One orientation of a piece, prepared for the bit-parallel move generator. Offsets are relative to
/// the anchor, the first cell in (r, q) order, so every cell's index offset is non-negative.
pub struct Orientation {
    pub piece: u8,
    pub size: u8,
    pub dq: [i32; 4],
    pub dr: [i32; 4],
    pub shift: [u32; 4],
    pub win: [BB; 4],
    /// For each piece cell: how many other cells of the piece it touches.
    pub inner: [u8; 4],
    /// Outside cells touching two piece cells that touch each other, or three piece cells: an own cell
    /// there would close a triangle or get a third neighbour.
    pub wedge: Vec<(i32, i32)>,
    /// Outside cells touching exactly two piece cells that do not touch each other: an own cell there
    /// is fine only if it has no own neighbour yet.
    pub bridge: Vec<(i32, i32)>,
    pub placement_at: [i16; GRID],
}

pub struct Tables {
    pub geo: Geometry,
    pub placements: Vec<Placement>,
    pub first_placement: [usize; NPIECES + 1],
    pub orientations: Vec<Orientation>,
    pub first_orientation: [usize; NPIECES + 1],
    /// sym_placement[id][t]: the placement id after symmetry t.
    pub sym_placement: Vec<[u16; NSYM]>,
    /// best_fill[pieces][n]: the most cells the given pieces can cover using at most n cells.
    pub best_fill: Vec<[u8; GRID + 1]>,
    pub cells_in: [u8; 256],
    pub zob_cell: [[u64; GRID]; 2],
    pub zob_piece: [[u64; NPIECES]; 2],
    pub zob_stm: u64,
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn distinct_orientations(piece: usize) -> Vec<Vec<(i32, i32)>> {
    let mut result: Vec<Vec<(i32, i32)>> = Vec::new();
    for t in 0..NSYM {
        let mut cells: Vec<(i32, i32)> = PIECE_SHAPE[piece].iter().map(|&(q, r)| apply_symmetry(t, q, r)).collect();
        cells.sort_by_key(|&(q, r)| (r, q));
        let (aq, ar) = cells[0];
        for c in cells.iter_mut() {
            c.0 -= aq;
            c.1 -= ar;
        }
        if !result.contains(&cells) {
            result.push(cells);
        }
    }
    result
}

fn build() -> Tables {
    let geo = build_geometry();
    let mut placements: Vec<Placement> = Vec::new();
    let mut orientations: Vec<Orientation> = Vec::new();
    let mut first_placement = [0usize; NPIECES + 1];
    let mut first_orientation = [0usize; NPIECES + 1];
    for piece in 0..NPIECES {
        first_orientation[piece] = orientations.len();
        first_placement[piece] = placements.len();
        for cells in distinct_orientations(piece) {
            let size = cells.len();
            let mut o = Orientation {
                piece: piece as u8,
                size: size as u8,
                dq: [0; 4],
                dr: [0; 4],
                shift: [0; 4],
                win: [0; 4],
                inner: [0; 4],
                wedge: Vec::new(),
                bridge: Vec::new(),
                placement_at: [-1; GRID],
            };
            for j in 0..size {
                o.dq[j] = cells[j].0;
                o.dr[j] = cells[j].1;
                let s = cells[j].1 * WIDTH + cells[j].0;
                assert!(s >= 0);
                o.shift[j] = s as u32;
                o.win[j] = COLWIN[(cells[j].0 + 8) as usize];
                o.inner[j] = (0..size).filter(|&l| l != j && adjacent(cells[j].0, cells[j].1, cells[l].0, cells[l].1)).count() as u8;
            }
            // classify the outside cells that touch two or more piece cells
            let mut seen: Vec<(i32, i32)> = Vec::new();
            for j in 0..size {
                for d in 0..6 {
                    let out = (cells[j].0 + DIR_Q[d], cells[j].1 + DIR_R[d]);
                    if cells.contains(&out) || seen.contains(&out) {
                        continue;
                    }
                    seen.push(out);
                    let touching: Vec<usize> = (0..size).filter(|&l| adjacent(out.0, out.1, cells[l].0, cells[l].1)).collect();
                    if touching.len() < 2 {
                        continue;
                    }
                    let mut wedge = touching.len() >= 3;
                    for x in 0..touching.len() {
                        for y in x + 1..touching.len() {
                            let (a, b) = (cells[touching[x]], cells[touching[y]]);
                            if adjacent(a.0, a.1, b.0, b.1) {
                                wedge = true;
                            }
                        }
                    }
                    if wedge {
                        o.wedge.push(out);
                    } else {
                        o.bridge.push(out);
                    }
                }
            }
            for &anchor in geo.cells.iter() {
                let anchor = anchor as usize;
                let mut p = Placement { cells: 0, border: 0, piece: piece as u8, n: 0, cell_list: [0; 4] };
                let mut fits = true;
                for &(dq, dr) in cells.iter() {
                    let (q, r) = (cell_q(anchor) + dq, cell_r(anchor) + dr);
                    if !on_board(q, r) {
                        fits = false;
                        break;
                    }
                    let c = cell_index(q, r);
                    p.cell_list[p.n as usize] = c as u8;
                    p.n += 1;
                    p.cells |= bit(c);
                }
                if !fits {
                    continue;
                }
                p.border = neighbours(p.cells) & !p.cells;
                o.placement_at[anchor] = placements.len() as i16;
                placements.push(p);
            }
            orientations.push(o);
        }
    }
    first_orientation[NPIECES] = orientations.len();
    first_placement[NPIECES] = placements.len();

    let mut sym_placement = vec![[0u16; NSYM]; placements.len()];
    for (id, p) in placements.iter().enumerate() {
        for t in 0..NSYM {
            let mut image: BB = 0;
            for j in 0..p.n as usize {
                image |= bit(geo.sym[t][p.cell_list[j] as usize] as usize);
            }
            let piece = p.piece as usize;
            let found = (first_placement[piece]..first_placement[piece + 1]).find(|&k| placements[k].cells == image).expect("symmetric placement");
            sym_placement[id][t] = found as u16;
        }
    }

    let mut best_fill = vec![[0u8; GRID + 1]; 256];
    let mut cells_in = [0u8; 256];
    for set in 0..256usize {
        let mut reachable = [false; GRID + 1];
        reachable[0] = true;
        for p in 0..NPIECES {
            if set >> p & 1 == 1 {
                cells_in[set] += PIECE_SIZE[p];
                let sz = PIECE_SIZE[p] as usize;
                for sum in (sz..=GRID).rev() {
                    if reachable[sum - sz] {
                        reachable[sum] = true;
                    }
                }
            }
        }
        let mut best = 0u8;
        for n in 0..=GRID {
            if reachable[n] {
                best = n as u8;
            }
            best_fill[set][n] = best;
        }
    }

    let mut seed = 0x5eca9011u64;
    let mut zob_cell = [[0u64; GRID]; 2];
    let mut zob_piece = [[0u64; NPIECES]; 2];
    for player in 0..2 {
        for i in 0..GRID {
            zob_cell[player][i] = splitmix64(&mut seed);
        }
        for p in 0..NPIECES {
            zob_piece[player][p] = splitmix64(&mut seed);
        }
    }
    let zob_stm = splitmix64(&mut seed);

    Tables { geo, placements, first_placement, orientations, first_orientation, sym_placement, best_fill, cells_in, zob_cell, zob_piece, zob_stm }
}

static TABLES: OnceLock<Tables> = OnceLock::new();

#[inline]
pub fn tables() -> &'static Tables {
    TABLES.get_or_init(build)
}

pub fn move_to_string(t: &Tables, m: u16) -> String {
    if m == PASS {
        return "pass".to_string();
    }
    let p = &t.placements[m as usize];
    let mut cells: Vec<u8> = p.cell_list[..p.n as usize].to_vec();
    cells.sort();
    format!("{}:{}", PIECE_NAME[p.piece as usize], cells.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","))
}

pub fn parse_move(t: &Tables, text: &str) -> Option<u16> {
    if text == "pass" {
        return Some(PASS);
    }
    let (name, rest) = text.split_once(':')?;
    let piece = PIECE_NAME.iter().position(|&n| n == name)?;
    let mut cells: BB = 0;
    for part in rest.split(',') {
        let c: usize = part.trim().parse().ok()?;
        if c >= GRID {
            return None;
        }
        cells |= bit(c);
    }
    (t.first_placement[piece]..t.first_placement[piece + 1]).find(|&k| t.placements[k].cells == cells).map(|k| k as u16)
}
