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

/// Prints the pot measurements of a study build (`--study`).
pub fn print_stats<W: std::io::Write>(out: &mut W, s: &Solver) {
    let st = &s.st;
    let total: u64 = s.by_left.iter().sum();
    writeln!(out, "# study: total nodes {}", total).unwrap();
    let w: [u64; 2] = [st.waste[0].iter().sum(), st.waste[1].iter().sum()];
    writeln!(out, "# waste (nodes under failed moves at nodes that cut, outermost): red-to-move nodes {} ({:.1}%), blue-to-move nodes {} ({:.1}%)", w[0], 100.0 * w[0] as f64 / total as f64, w[1], 100.0 * w[1] as f64 / total as f64).unwrap();
    writeln!(out, "# waste by pieces left of the wasted node: left nodes | under red's failed moves, under blue's failed moves (share of the level)").unwrap();
    for l in 2..12 {
        let n = s.by_left[l].max(1);
        writeln!(out, "#  {:2}: {:9} | {:9} {:5.1}% {:9} {:5.1}%", l, s.by_left[l], st.waste[0][l], 100.0 * st.waste[0][l] as f64 / n as f64, st.waste[1][l], 100.0 * st.waste[1][l] as f64 / n as f64).unwrap();
    }
    writeln!(out, "# all-fail nodes: left count | moves/node distinct-replies/node top-reply-share-of-moves top-reply-share-of-nodes nodes/node | kinds of children: bound, table, first-cut, later-cut, other").unwrap();
    for l in 4..12 {
        let a = &st.an[l];
        if a[0] == 0 {
            continue;
        }
        let c = a[0] as f64;
        writeln!(out, "#  {:2}: {:7} | {:6.1} {:5.2} {:5.1}% {:5.1}% {:8.1} | {} {} {} {} {}", l, a[0], a[1] as f64 / c, a[2] as f64 / c, 100.0 * a[3] as f64 / a[1].max(1) as f64, 100.0 * a[5] as f64 / a[4].max(1) as f64, a[4] as f64 / c, a[6], a[7], a[8], a[9], a[10]).unwrap();
    }
    writeln!(out, "# reach threats at all-fail nodes: left | tests threats work/test | in order: moves pruned, nodes under them | oracle: moves, nodes").unwrap();
    for l in 4..12 {
        let r = &st.rt[l];
        if r[0] == 0 {
            continue;
        }
        writeln!(out, "#  {:2}: {:7} {:7} {:8.1} | {:8} {:10} | {:8} {:10}", l, r[0], r[1], r[4] as f64 / r[0] as f64, r[2], r[3], r[5], r[6]).unwrap();
    }
    writeln!(out, "# reach threats, exclusive nodes saved: in order {} ({:.1}%), oracle {} ({:.1}%)", st.rt_saved[0], 100.0 * st.rt_saved[0] as f64 / total as f64, st.rt_saved[1], 100.0 * st.rt_saved[1] as f64 / total as f64).unwrap();
    writeln!(out, "# one-player optimum at open nodes: left | nodes tests decided work/test out-of-work | separated (whole) separated (relevant clusters)").unwrap();
    for l in 4..12 {
        let o = &st.opt[l];
        if o[0] == 0 {
            continue;
        }
        writeln!(out, "#  {:2}: {:8} {:8} {:8} {:8.1} {:6} | {:6} {:6}", l, o[0], o[1], o[2], o[3] as f64 / o[1].max(1) as f64, o[4], st.sep[l][1], o[5]).unwrap();
    }
    writeln!(out, "# exclusive nodes saved: one-player optimum {} ({:.1}%), separation {} ({:.1}%)", st.opt_saved, 100.0 * st.opt_saved as f64 / total as f64, st.sep_saved, 100.0 * st.sep_saved as f64 / total as f64).unwrap();
    writeln!(out, "# slack of the upper bound over the one-player optimum (sample), by pieces left and hand: counts of slack 0..7+").unwrap();
    for l in 4..12 {
        for h in 1..6 {
            let sl = &st.slack[l][h];
            if sl.iter().sum::<u64>() == 0 {
                continue;
            }
            writeln!(out, "#  {:2} hand {}: {:?}", l, h, sl).unwrap();
        }
    }
}
