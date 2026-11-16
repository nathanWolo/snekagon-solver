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
    println!("pos,move,piece,size,grows,joined,gain,lead,front,touch,my_up,their_up,their_up_before,their_moves,my_moves,replies,my_ends,their_ends,their_best,my_next,my_long_ends,their_long_ends,their_reach,static,win,nodes");
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
            // one move ahead for each side: the longest snake each could have after its next placement
            let mut their_best = child.longest[opp] as u32;
            child.for_each_move(t, opp, child.left[opp], |r| their_best = their_best.max(child.longest_after(opp, &t.placements[r as usize])));
            let mut my_next = child.longest[me] as u32;
            child.for_each_move(t, me, child.left[me], |r| my_next = my_next.max(child.longest_after(me, &t.placements[r as usize])));
            // growth room of each side's longest snake alone
            let long_ends = |player: usize| -> u32 {
                let mut best = 0u128;
                for j in 0..child.nsnakes[player] as usize {
                    if count(child.snakes[player][j]) == child.longest[player] as u32 {
                        best = child.snakes[player][j];
                    }
                }
                let s = shape_of(child.cells[player]);
                count(neighbours(best & s.ends) & empty & !neighbours(s.body))
            };
            let my_long_ends = long_ends(me);
            let their_long_ends = long_ends(opp);
            // can they still reach the length we have now (a cheap look at how alive they are)?
            let mut work = 2000u32;
            let their_reach = can_reach_alone(t, &child, opp, child.longest[me] as i32 + 1, &mut work) as u8;
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
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                i, m, p.piece, p.n, grows as u8, joined, gain, lead, is_front as u8, touch as u8, my_up, their_up, their_up_before, their_moves, my_moves, replies, my_ends, their_ends, their_best, my_next, my_long_ends, their_long_ends, their_reach, score, win, solver.nodes
            );
        }
    }
}

/// E32 fit data: for every legal move of each position in a file, the terms of the evaluation of
/// the position after it (`evalorder.rs`), the static score, and (unless `cap` is 1) whether the
/// move wins for the mover and the nodes that takes when the child is solved alone with a fresh
/// table. One CSV line a move.
pub fn eval_moves(t: &Tables, path: &str, from: usize, to: usize, cfg: Config, cap: u64) {
    let text = std::fs::read_to_string(path).expect("cannot read the position file");
    let mut solver = Solver::new(cfg);
    let names: Vec<String> = crate::evalorder::NAMES.iter().map(|s| s.to_string()).collect();
    println!("pos,move,{},static,win,nodes", names.join(","));
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
        pos.moves(t, &mut list);
        for &m in list.iter() {
            let p = &t.placements[m as usize];
            let mut score = p.n as u64 * 64;
            if p.border & own != 0 {
                score += 256;
            }
            if p.cells & front != 0 {
                score += 128;
            } else if p.border & theirs != 0 {
                score += 32;
            }
            let mut child = pos;
            child.play(t, m);
            let f = crate::evalorder::features(t, &child);
            let fs: Vec<String> = f.iter().map(|x| x.to_string()).collect();
            let (win, nodes) = if cap == 1 {
                ("?", 0)
            } else {
                solver.clear();
                let v = solver.solve(&child, -1, 0, cap);
                (if v == UNKNOWN { "?" } else if v <= -1 { "1" } else { "0" }, solver.nodes)
            };
            println!("{},{},{},{},{},{}", i, m, fs.join(","), score, win, nodes);
        }
    }
}
