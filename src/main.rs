mod geom;
mod naive;
mod pieces;
mod pos;

use geom::*;
use pieces::*;
use pos::*;
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
    let (mut positions, mut placements_checked) = (0u64, 0u64);
    let mut moves = Vec::new();
    for _ in 0..games {
        let mut pos = Pos::initial();
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
            }
            let mut fresh = pos;
            fresh.recompute();
            assert_eq!(fresh.hash, pos.hash, "hash differs at {}", pos.to_text());
            assert_eq!(fresh.nsnakes, pos.nsnakes);
            let round = Pos::from_text(&pos.to_text()).unwrap();
            assert_eq!(round.hash, pos.hash);
            positions += 1;
            let m = if moves.is_empty() { PASS } else { moves[rng.below(moves.len())] };
            pos.play(t, m);
        }
    }
    println!("verify ok: {} games, {} positions, {} placement checks", games, positions, placements_checked);
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
        _ => {
            println!("commands: info | perft <depth> | verify <games> <seed>");
        }
    }
}
