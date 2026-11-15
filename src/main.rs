mod bounds;
mod geom;
mod naive;
mod pieces;
mod pos;
mod solver;

use geom::*;
use pieces::*;
use pos::*;
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
    let (mut positions, mut placements_checked, mut threat_checks) = (0u64, 0u64, 0u64);
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
            positions += 1;
            let m = if moves.is_empty() { PASS } else { moves[rng.below(moves.len())] };
            pos.play(t, m);
        }
        for player in 0..2 {
            assert!(pos.longest[player] as i32 <= prev_upper[player], "final snake above the bound");
        }
    }
    println!("verify ok: {} games, {} positions, {} placement checks, {} threat-bound checks", games, positions, placements_checked, threat_checks);
}

struct Opts {
    cfg: Config,
    nodes: u64,
    from: usize,
    to: usize,
    quiet: bool,
    profile: bool,
    rest: Vec<String>,
}

fn parse_opts(args: &[String]) -> Opts {
    let mut o = Opts { cfg: Config::default(), nodes: u64::MAX, from: 0, to: usize::MAX, quiet: false, profile: false, rest: Vec::new() };
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
    let mut solver = Solver::new(o.cfg);
    let (mut total_nodes, mut total_us, mut wrong, mut unknown, mut n) = (0u64, 0u128, 0u32, 0u32, 0u32);
    let out = std::io::stdout();
    let mut out = out.lock();
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).enumerate() {
        if i < o.from || i >= o.to {
            continue;
        }
        let (expected, rest) = line.trim().split_once(char::is_whitespace).expect("line needs an expected value and a position");
        let pos = Pos::parse(t, rest).unwrap_or_else(|| panic!("bad position on line {}", i));
        solver.clear();
        let start = Instant::now();
        let v = solver.solve(&pos, -1, 1, o.nodes);
        let us = start.elapsed().as_micros();
        let ok = if v == UNKNOWN {
            unknown += 1;
            "unknown"
        } else if expected == "?" || expected.parse::<i32>().ok() == Some(v) {
            "ok"
        } else {
            wrong += 1;
            "WRONG"
        };
        total_nodes += solver.nodes;
        total_us += us;
        n += 1;
        if !o.quiet {
            writeln!(out, "{} {} {} {} {} {} {}", i, pos.ply, expected, v, solver.nodes, us, ok).unwrap();
        }
    }
    writeln!(out, "# positions {} nodes {} seconds {:.3} wrong {} unknown {}", n, total_nodes, total_us as f64 / 1e6, wrong, unknown).unwrap();
    if o.profile {
        writeln!(out, "# nodes by pieces left: {:?}", solver.by_left).unwrap();
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
        "bench" => {
            let o = parse_opts(&args[2..]);
            bench(t, &o);
        }
        _ => {
            println!("commands: info | perft <depth> | verify <games> <seed> | solve [opts] <position> | bench [opts] <file>");
            println!("options: --no-bounds --no-last --no-threats --tt <log2 entries> --nodes <limit> --from <i> --to <j> --quiet");
        }
    }
}
