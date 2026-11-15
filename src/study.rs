//! Measurement tools. Nothing here is used by the solver.

use crate::bounds::*;
use crate::geom::*;
use crate::pieces::*;
use crate::pos::*;
use crate::solver::*;

/// For every legal move of each position in a file: what a move ordering could know about the move,
/// and what the search then finds (does it win for the mover, and how many nodes that takes).
/// One CSV line a move. `cap`: node limit for one move (win is "?" when it is reached).
pub fn root_moves(t: &Tables, path: &str, from: usize, to: usize, cfg: Config, cap: u64) {
    let text = std::fs::read_to_string(path).expect("cannot read the position file");
    let mut solver = Solver::new(cfg);
    println!("pos,move,piece,size,grows,joined,gain,lead,front,touch,my_up,their_up,their_up_before,their_moves,my_moves,replies,my_ends,their_ends,static,win,nodes");
    let mut list = Vec::new();
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).enumerate() {
        if i < from || i >= to {
            continue;
        }
        let (_, rest) = line.trim().split_once(char::is_whitespace).expect("line needs an expected value and a position");
        let pos = Pos::parse(t, rest).unwrap_or_else(|| panic!("bad position on line {}", i));
        let me = pos.stm as usize;
        let opp = me ^ 1;
        let own = pos.cells[me];
        let theirs = pos.cells[opp];
        let front = neighbours(shape_of(theirs).ends) & pos.empty();
        let their_up_before = upper_bound(t, &pos, opp);
        pos.moves(t, &mut list);
        for &m in list.iter() {
            let p = &t.placements[m as usize];
            let grows = p.border & own != 0;
            let mut joined = p.n as u32;
            for j in 0..pos.nsnakes[me] as usize {
                if pos.snakes[me][j] & p.border != 0 {
                    joined += count(pos.snakes[me][j]);
                }
            }
            let after = pos.longest_after(me, p) as i32;
            let gain = after - pos.longest[me] as i32;
            let lead = after - pos.longest[opp] as i32;
            let is_front = p.cells & front != 0;
            let touch = p.border & theirs != 0;
            let mut score = p.n as u64 * 64;
            if grows {
                score += 256;
            }
            if is_front {
                score += 128;
            } else if touch {
                score += 32;
            }
            let mut child = pos;
            child.play(t, m);
            let my_up = upper_bound(t, &child, me);
            let their_up = upper_bound(t, &child, opp);
            let their_moves = child.count_moves(t);
            let mut my_moves = 0u32;
            child.for_each_move(t, me, child.left[me], |_| my_moves += 1);
            // the opponent's replies that leave no deciding placement of ours playable
            let mut th = Threats::new();
            let mut cl = Clusters::new();
            upper_bound_ex(t, &child, opp, 0, Some(&mut cl));
            if child.left[me] != 0 {
                solver.scan_threats(&child, &cl, -1, &mut th);
            }
            let mut replies = 0u32;
            child.for_each_move(t, opp, child.pieces_now(), |r| {
                if th.n == 0 || th.bound_for(t.placements[r as usize].cells) > -1 {
                    replies += 1;
                }
            });
            // free cells at the ends of each side's snakes: room to grow
            let empty = child.empty();
            let my_ends = count(neighbours(shape_of(child.cells[me]).ends) & empty & !neighbours(shape_of(child.cells[me]).body));
            let their_ends = count(neighbours(shape_of(child.cells[opp]).ends) & empty & !neighbours(shape_of(child.cells[opp]).body));
            solver.clear();
            let v = solver.solve(&child, -1, 0, cap);
            let win = if v == UNKNOWN {
                "?"
            } else if v <= -1 {
                "1"
            } else {
                "0"
            };
            println!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                i, m, p.piece, p.n, grows as u8, joined, gain, lead, is_front as u8, touch as u8, my_up, their_up, their_up_before, their_moves, my_moves, replies, my_ends, their_ends, score, win, solver.nodes
            );
        }
    }
}
