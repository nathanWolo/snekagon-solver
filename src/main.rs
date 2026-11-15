mod bounds;
mod geom;
mod naive;
mod pieces;
mod pns;
mod pos;
mod solver;
mod study;

use geom::*;
use pieces::*;
use pos::*;
use pns::*;
use solver::*;
use std::io::Write;
use std::time::Instant;

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn perft(t: &Tables, pos: &Pos, depth: u32) -> u64 {
    if pos.is_over() {
        return 1;
    }
    if depth == 1 {
        return pos.count_moves(t).max(1);
    }
    let mut moves = Vec::new();
    pos.moves(t, &mut moves);
    if moves.is_empty() {
        moves.push(PASS);
    }
    let mut n = 0;
    for &m in &moves {
        let mut child = *pos;
        child.play(t, m);
        n += perft(t, &child, depth - 1);
    }
    n
}

/// Random games; at every position the fast code is compared with the naive rules.
fn verify(t: &Tables, games: usize, seed: u64) {
    let mut rng = Rng(seed);
    let (mut positions, mut placements_checked, mut threat_checks, mut reach_checks) = (0u64, 0u64, 0u64, 0u64);
    let mut moves = Vec::new();
    for _ in 0..games {
        let mut pos = Pos::initial();
        let mut prev_upper = [i32::MAX; 2];
        while !pos.is_over() {
            pos.moves(t, &mut moves);
            let reference = naive::moves(t, &pos);
            assert_eq!(moves, reference, "move lists differ at {}", pos.to_text());
            assert_eq!(pos.count_moves(t), moves.len() as u64);
            assert_eq!(pos.has_move(t, pos.stm as usize, pos.pieces_now()), !moves.is_empty());
            for player in 0..2 {
                for p in t.placements.iter() {
                    assert_eq!(pos.legal_placement(t, player, p), naive::legal(&pos, player, p), "placement check differs at {}", pos.to_text());
                    placements_checked += 1;
                }
                assert_eq!(pos.longest[player] as u32, naive::longest(pos.cells[player]));
                let total: u32 = (0..pos.nsnakes[player] as usize).map(|j| count(pos.snakes[player][j])).sum();
                assert_eq!(total, count(pos.cells[player]));
                let upper = bounds::upper_bound(t, &pos, player);
                assert_eq!(upper, bounds::upper_bound_reference(t, &pos, player), "upper bound differs at {}", pos.to_text());
                assert!(upper <= prev_upper[player], "upper bound rose at {}", pos.to_text());
                assert!(upper >= pos.longest[player] as i32);
                prev_upper[player] = upper;
            }
            let mut fresh = pos;
            fresh.recompute();
            assert_eq!(fresh.hash, pos.hash, "hash differs at {}", pos.to_text());
            assert_eq!(fresh.nsnakes, pos.nsnakes);
            let round = Pos::from_text(&pos.to_text()).unwrap();
            assert_eq!(round.hash, pos.hash);
            // what the threat zones rely on: if the opponent could play b now, our upper bound after b
            // is no lower than after any move of ours that leaves b playable, followed by b
            let (me, opp) = (pos.stm as usize, pos.stm as usize ^ 1);
            for _ in 0..40 {
                if moves.is_empty() || pos.left[opp] == 0 {
                    break;
                }
                let b = &t.placements[rng.below(t.placements.len())];
                if pos.left[opp] >> b.piece & 1 == 0 || !pos.legal_placement(t, opp, b) {
                    continue;
                }
                let zone = if b.border & pos.cells[opp] != 0 { b.cells } else { b.cells | b.border };
                let before = bounds::upper_bound_ex(t, &pos, me, b.cells, None);
                for _ in 0..8 {
                    let m = moves[rng.below(moves.len())];
                    if t.placements[m as usize].cells & zone != 0 {
                        continue;
                    }
                    let mut child = pos;
                    child.play(t, m);
                    assert!(child.legal_placement(t, opp, b), "threat no longer playable at {}", pos.to_text());
                    let after = bounds::upper_bound_ex(t, &child, me, b.cells, None);
                    assert!(after <= before, "upper bound rose after our own move at {}", pos.to_text());
                    threat_checks += 1;
                }
            }
            // the reach test against every sequence of placements, where that is affordable
            for player in 0..2 {
                let k = pos.left[player].count_ones();
                if k == 0 || k > 3 || (k == 3 && rng.below(40) != 0) || (k == 2 && rng.below(3) != 0) {
                    continue;
                }
                let optimum = bounds::one_player_optimum(t, &pos, player);
                for target in pos.longest[player] as i32 + 1..=optimum + 2 {
                    let mut work = u32::MAX;
                    assert_eq!(bounds::can_reach_alone(t, &pos, player, target, &mut work), target <= optimum, "reach test wrong for {} at {}", target, pos.to_text());
                    let mut work = u32::MAX;
                    assert_eq!(bounds::can_reach_touching(t, &pos, player, target, &mut work), target <= optimum, "touching reach test wrong for {} at {}", target, pos.to_text());
                    reach_checks += 1;
                }
            }
            positions += 1;
            let m = if moves.is_empty() { PASS } else { moves[rng.below(moves.len())] };
            pos.play(t, m);
        }
        for player in 0..2 {
            assert!(pos.longest[player] as i32 <= prev_upper[player], "final snake above the bound");
        }
    }
    println!("verify ok: {} games, {} positions, {} placement checks, {} threat-bound checks, {} reach checks", games, positions, placements_checked, threat_checks, reach_checks);
}

struct Opts {
    cfg: Config,
    nodes: u64,
    from: usize,
    to: usize,
    quiet: bool,
    profile: bool,
    pn: bool,
    oracle: bool,
    pn_cfg: PnConfig,
    rest: Vec<String>,
}

fn parse_opts(args: &[String]) -> Opts {
    let mut o = Opts { cfg: Config::default(), nodes: u64::MAX, from: 0, to: usize::MAX, quiet: false, profile: false, pn: false, oracle: false, pn_cfg: PnConfig::default(), rest: Vec::new() };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || {
            i += 1;
            args[i].clone()
        };
        match a {
            "--no-bounds" => o.cfg.bounds = false,
            "--no-last" => o.cfg.last_pieces = false,
            "--no-threats" => o.cfg.threats = false,
            "--no-reach" => o.cfg.reach = false,
            "--no-staged" => o.cfg.staged = false,
            "--no-lazy" => o.cfg.lazy = false,
            "--no-narrow" => o.cfg.narrow = false,
            "--answer-bonus" => o.cfg.answer_bonus = val().parse().unwrap(),
            "--no-longest" => bounds::FROM_LONGEST.store(false, std::sync::atomic::Ordering::Relaxed),
            "--no-model" => o.cfg.model_level = 99,
            "--model-level" => o.cfg.model_level = val().parse().unwrap(),
            "--model" => {
                for (i, x) in val().split(',').enumerate() {
                    o.cfg.model[i] = x.parse().unwrap();
                }
            }
            "--reach-hand" => o.cfg.reach_hand = val().parse().unwrap(),
            "--reach-work" => o.cfg.reach_work = val().parse().unwrap(),
            "--pn" => o.pn = true,
            "--oracle" => o.oracle = true,
            "--pn-slack" => o.pn_cfg.slack = val().parse().unwrap(),
            "--pn-rank" => o.pn_cfg.rank_div = val().parse().unwrap(),
            "--pn-tt" => o.pn_cfg.tt_bits = val().parse().unwrap(),
            "--tt" => o.cfg.tt_bits = val().parse().unwrap(),
            "--nodes" => o.nodes = val().parse().unwrap(),
            "--from" => o.from = val().parse().unwrap(),
            "--to" => o.to = val().parse().unwrap(),
            "--quiet" => o.quiet = true,
            "--profile" => o.profile = true,
            _ => o.rest.push(a.to_string()),
        }
        i += 1;
    }
    o
}

/// Solves every position of a file (lines: "<expected or ?> <position text or moves>") and prints one
/// line each: index, ply, expected, value, nodes, microseconds. Exits 1 on any wrong value.
fn bench(t: &Tables, o: &Opts) {
    let path = &o.rest[0];
    let text = std::fs::read_to_string(path).expect("cannot read the position file");
    let mut solver = Solver::new(if o.pn { Config { tt_bits: 4, ..o.cfg } } else { o.cfg });
    let mut pn = if o.pn { Some(PnSolver::new(o.pn_cfg, o.cfg)) } else { None };
    let (mut total_nodes, mut total_us, mut wrong, mut unknown, mut n) = (0u64, 0u128, 0u32, 0u32, 0u32);
    let mut oracle_first = 0u64;
    let out = std::io::stdout();
    let mut out = out.lock();
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).enumerate() {
        if i < o.from || i >= o.to {
            continue;
        }
        let (expected, rest) = line.trim().split_once(char::is_whitespace).expect("line needs an expected value and a position");
        let pos = Pos::parse(t, rest).unwrap_or_else(|| panic!("bad position on line {}", i));
        solver.clear();
        if let Some(p) = pn.as_mut() {
            p.clear();
        }
        let start = Instant::now();
        let v = match pn.as_mut() {
            Some(p) => p.solve(&pos, o.nodes),
            None => solver.solve(&pos, -1, 1, o.nodes),
        };
        if o.oracle && pn.is_none() {
            // measurement: solve again with the first solve's best moves as the first moves
            let first = solver.nodes;
            solver.clear();
            solver.hint = true;
            let v2 = solver.solve(&pos, -1, 1, o.nodes);
            solver.hint = false;
            assert!(v2 == v);
            oracle_first += first;
        }
        let us = start.elapsed().as_micros();
        let nodes = match pn.as_ref() {
            Some(p) => p.nodes,
            None => solver.nodes,
        };
        let ok = if v == UNKNOWN {
            unknown += 1;
            "unknown"
        } else if expected == "?" || expected.parse::<i32>().ok() == Some(v) {
            "ok"
        } else {
            wrong += 1;
            "WRONG"
        };
        total_nodes += nodes;
        total_us += us;
        n += 1;
        if !o.quiet {
            writeln!(out, "{} {} {} {} {} {} {}", i, pos.ply, expected, v, nodes, us, ok).unwrap();
        }
    }
    writeln!(out, "# positions {} nodes {} seconds {:.3} wrong {} unknown {}", n, total_nodes, total_us as f64 / 1e6, wrong, unknown).unwrap();
    if o.oracle {
        writeln!(out, "# oracle: first solve {} nodes, second solve (reported above) with its best moves first", oracle_first).unwrap();
    }
    if o.profile {
        writeln!(out, "# nodes by pieces left: {:?}", solver.by_left).unwrap();
        writeln!(out, "# left: nodes | table cuts, bound cuts, searched nodes, moves searched, first-move cuts, no cut, moves left unsearched, scans").unwrap();
        for l in 2..12 {
            writeln!(out, "# {:2}: {:9} | {:?}", l, solver.by_left[l], solver.prof[l]).unwrap();
        }
        writeln!(out, "# mover tests by pieces left [tests, decided]: {:?}", &solver.reach_by[0][..8]).unwrap();
        writeln!(out, "# opponent tests by pieces left [tests, decided]: {:?}", &solver.reach_by[1][..8]).unwrap();
    }
    if wrong > 0 {
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let t = tables();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    match cmd {
        "info" => {
            println!("placements {}", t.placements.len());
            println!("orientations {}", t.orientations.len());
            for p in 0..NPIECES {
                println!("  {:3} {} orientations, {} placements", PIECE_NAME[p], t.first_orientation[p + 1] - t.first_orientation[p], t.first_placement[p + 1] - t.first_placement[p]);
            }
        }
        "perft" => {
            let depth: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
            for d in 1..=depth {
                let start = Instant::now();
                let n = perft(t, &Pos::initial(), d);
                println!("perft {} = {}   ({:.3} s)", d, n, start.elapsed().as_secs_f64());
            }
        }
        "verify" => {
            let games: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
            let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
            verify(t, games, seed);
        }
        "solve" => {
            let o = parse_opts(&args[2..]);
            let pos = Pos::parse(t, &o.rest.join(" ")).expect("bad position");
            let mut solver = Solver::new(o.cfg);
            let start = Instant::now();
            let v = solver.solve(&pos, -1, 1, o.nodes);
            println!("value {} nodes {} seconds {:.3}", v, solver.nodes, start.elapsed().as_secs_f64());
        }
        "rootmoves" => {
            let o = parse_opts(&args[2..]);
            study::root_moves(t, &o.rest[0], o.from, o.to, o.cfg, o.nodes);
        }
        "bench" => {
            let o = parse_opts(&args[2..]);
            bench(t, &o);
        }
        _ => {
            println!("commands: info | perft <depth> | verify <games> <seed> | solve [opts] <position> | bench [opts] <file>");
            println!("options: --pn --pn-slack N --pn-rank N --pn-tt BITS --no-bounds --no-last --no-threats --no-reach --no-model --model-level N --model W,W,... --reach-hand N --reach-work N --tt <log2 entries> --nodes <limit> --from <i> --to <j> --quiet");
        }
    }
}
