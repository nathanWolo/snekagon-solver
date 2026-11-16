//! Shadow checks of the reach-threat rule (feature `shadow`; a verification tool only).
//!
//! Every move a reach threat rules out (its reach bound is at most alpha) is, when sampled, solved
//! exactly by a second solver with reach threats switched off and with its own table, and the
//! program aborts if the move's exact value is above the bound the rule assumed for it. Nodes where a
//! reach threat ruled out a move are, when sampled, solved the same way, and the value returned and
//! the bounds stored must agree with the exact value. Every "cannot reach" answer of a reach-threat
//! test (from a search or from the cache) is checked against the reach test with unlimited work.
//!
//! The second solver (one per thread) is built with `reach_threat = false` on a `Table` of its own:
//! nothing the rule computed can answer it, and it never writes into the searching solver's table.
//!
//! Sampling: a check whose position has at most SHADOW_FULL pieces left (both hands; default 5) is
//! always made; above, one in 2^(SHADOW_SHIFT * (level - SHADOW_FULL)) (default shift 2), chosen by a
//! hash of the position (and the move): deterministic.
#![allow(dead_code)]

use crate::bounds::*;
use crate::pieces::*;
use crate::pos::*;
use crate::solver::*;
use std::cell::RefCell;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};

pub const NAMES: [&str; 12] = [
    "nodes where a reach threat ruled out a move",
    "moves ruled out with a reach bound <= alpha",
    "  of them not ruled out without reach threats",
    "ruled-out moves checked (exact solve)",
    "  of them not ruled out without reach threats",
    "node checks (value returned, bounds stored)",
    "no-reach answers checked (unlimited work)",
    "  of them from the cache",
    "no-reach answers checked by one-player optimum",
    "nodes dumped for the value check",
    "reach threats found",
    "ruled-out moves skipped by the sampling",
];
pub static C: [AtomicU64; 12] = [const { AtomicU64::new(0) }; 12];

#[inline]
pub fn hit(i: usize) {
    C[i].fetch_add(1, Relaxed);
}

pub fn report() {
    eprintln!("# shadow (reach threats): full level {} shift {}", params().0, params().1);
    for i in 0..NAMES.len() {
        eprintln!("#   {:52} {}", NAMES[i], C[i].load(Relaxed));
    }
    report_rq();
}

pub fn fail(what: &str, detail: String) -> ! {
    eprintln!("SHADOW MISMATCH in {}: {}", what, detail);
    report();
    std::process::abort();
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// (full level, shift, dump rate log2, dump cap per level and kind)
pub fn params() -> (u32, u32, u32, u64) {
    static P: OnceLock<(u32, u32, u32, u64)> = OnceLock::new();
    *P.get_or_init(|| (env_u32("SHADOW_FULL", 5), env_u32("SHADOW_SHIFT", 2), env_u32("SHADOW_DUMP_RATE", 6), env_u32("SHADOW_DUMP_CAP", 150) as u64))
}

#[inline]
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Whether a check on a position with `level` pieces left is made (deterministic in `key`).
pub fn sampled(key: u64, level: u32) -> bool {
    let (full, shift, _, _) = params();
    if level <= full {
        return true;
    }
    let k = ((level - full) * shift).min(40);
    mix(key) & ((1u64 << k) - 1) == 0
}

thread_local! {
    static SH: RefCell<Option<Solver>> = const { RefCell::new(None) };
}

/// The exact value of `pos` for the side to move, by a solver with reach threats off and a table of
/// its own (a new generation for every call).
pub fn exact(cfg: &Config, pos: &Pos) -> i32 {
    SH.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            let mut c = *cfg;
            c.reach_threat = false;
            c.reach_threat_cache = false;
            c.rt_study = false;
            c.tt_bits = 20;
            *s = Some(Solver::new(c));
        }
        let sv = s.as_mut().unwrap();
        sv.clear();
        let v = sv.solve(pos, -1, 1, u64::MAX);
        assert!(v != UNKNOWN);
        v
    })
}

/// A pruned move `m` at `pos`, bound `rb` assumed by the reach threats.
pub fn check_pruned(cfg: &Config, pos: &Pos, m: u16, rb: i32, only: bool, alpha: i32) {
    if layers() & 4 == 0 {
        return;
    }
    let t = tables();
    let mut child = *pos;
    child.play(t, m);
    let level = child.left[0].count_ones() + child.left[1].count_ones();
    if !sampled(child.hash ^ 0x5bd1e995, level) {
        hit(11);
        return;
    }
    hit(3);
    if only {
        hit(4);
    }
    let v = -exact(cfg, &child);
    if v > rb {
        fail("pruned move", format!("position {} move {} (alpha {}, reach bound {}, only by reach {}) exact value of the move {}", pos.to_text(), move_to_string(t, m), alpha, rb, only, v));
    }
}

/// A node where a reach threat ruled out a move: `best` returned for the window (a, b); [lo, hi] stored.
pub fn check_node(cfg: &Config, pos: &Pos, a: i32, b: i32, best: i32, lo: i32, hi: i32) {
    if layers() & 4 == 0 {
        return;
    }
    let level = pos.left[0].count_ones() + pos.left[1].count_ones();
    if !sampled(pos.hash ^ 0x2545F491, level) {
        return;
    }
    hit(5);
    let e = exact(cfg, pos);
    let ok_ret = if best <= a { e <= best } else if best >= b { e >= best } else { e == best };
    if !ok_ret || e < lo || e > hi {
        fail("node", format!("position {} window ({}, {}) returned {} stored [{}, {}] exact {}", pos.to_text(), a, b, best, lo, hi, e));
    }
}

/// A "cannot reach `need`" answer for `me` in `child` (the position with the threat placed).
pub fn check_no_reach(child: &Pos, me: usize, need: i32, from_cache: bool) {
    if layers() & 2 == 0 {
        return;
    }
    let t = tables();
    hit(6);
    if from_cache {
        hit(7);
    }
    // asked again with unlimited work (a cap far above the search's), start3 off, then on
    for s3 in [false, true] {
        match unlimited(child, me, need, s3) {
            Some(true) => fail("no-reach answer", format!("child {} player {} need {} cache {}: the unlimited test (start3 {}) reaches it", child.to_text(), me, need, from_cache, s3)),
            Some(false) => {}
            None => {
                NR_UND[s3 as usize].fetch_add(1, Relaxed); // undecided at the cap
            }
        };
    }
    if child.left[me].count_ones() <= 2 {
        hit(8);
        let opt = one_player_optimum(t, child, me);
        if opt >= need {
            fail("no-reach answer", format!("child {} player {} need {} cache {}: one-player optimum {}", child.to_text(), me, need, from_cache, opt));
        }
    }
}

static DUMP: Mutex<Option<std::fs::File>> = Mutex::new(None);
static DUMPED: [[AtomicU64; 2]; 16] = [const { [const { AtomicU64::new(0) }; 2] }; 16];

/// For the value check below the root: writes sampled nodes (level, whether a reach threat ruled out
/// a move, the window, the position) to the file named by SHADOW_DUMP.
pub fn dump(pos: &Pos, fired: bool, a: i32, b: i32) {
    static ON: OnceLock<bool> = OnceLock::new();
    let on = *ON.get_or_init(|| {
        if let Ok(p) = std::env::var("SHADOW_DUMP") {
            *DUMP.lock().unwrap() = Some(std::fs::OpenOptions::new().create(true).append(true).open(p).expect("cannot open SHADOW_DUMP"));
            true
        } else {
            false
        }
    });
    if !on {
        return;
    }
    let (_, _, rate, cap) = params();
    let level = (pos.left[0].count_ones() + pos.left[1].count_ones()) as usize;
    let rate = if fired { rate.saturating_sub(3) } else { rate };
    if mix(pos.hash ^ 0x77) & ((1u64 << rate) - 1) != 0 {
        return;
    }
    let slot = &DUMPED[level.min(15)][fired as usize];
    if slot.fetch_add(1, Relaxed) >= cap {
        return;
    }
    hit(9);
    let mut g = DUMP.lock().unwrap();
    if let Some(f) = g.as_mut() {
        writeln!(f, "{} {} {} {} {}", level, fired as u8, a, b, pos.to_text()).unwrap();
    }
}

/// The value check: every line "level fired a b position" is solved exactly (reach threats off,
/// fresh table), then with reach threats on in each configuration of `cfgs` (fresh table each time)
/// for the windows (-1,1), (-1,0), (0,1) and (a,b); every value returned and the bounds stored at the
/// root must agree with the exact value.
pub fn rtcheck(path: &str, base: Config, cfgs: &[(&str, Config, bool)], from: usize, to: usize) {
    let text = std::fs::read_to_string(path).expect("cannot read the dump");
    let (mut n, mut solves) = (0u64, 0u64);
    let mut by_level = [[0u64; 2]; 16];
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        if i < from || i >= to {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let (level, fired, a, b): (usize, usize, i32, i32) = (f[0].parse().unwrap(), f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap());
        let pos = Pos::from_text(f[4]).expect("bad position");
        let mut off = base;
        START3.store(false, Relaxed);
        off.reach_threat = false;
        off.tt_bits = 18;
        let mut s = Solver::new(off);
        let e = s.solve(&pos, -1, 1, u64::MAX);
        let mut windows = vec![(-1, 1), (-1, 0), (0, 1)];
        if !windows.contains(&(a, b)) && a < b {
            windows.push((a, b));
        }
        for (name, c, s3) in cfgs {
            START3.store(*s3, Relaxed);
            let mut c = *c;
            c.tt_bits = 18;
            for &(wa, wb) in &windows {
                let mut s = Solver::new(c);
                let v = s.solve(&pos, wa, wb, u64::MAX);
                solves += 1;
                let ok = if v <= wa { e <= v } else if v >= wb { e >= v } else { e == v };
                let (lo, hi) = s.root_bounds(&pos);
                if !ok || e < lo || e > hi {
                    fail("value check", format!("line {} position {} config {} window ({}, {}) returned {} stored [{}, {}] exact {}", i, f[4], name, wa, wb, v, lo, hi, e));
                }
            }
        }
        n += 1;
        by_level[level.min(15)][fired] += 1;
        println!("{} level {} fired {} window ({}, {}) exact {} ok", i, level, fired, a, b, e);
    }
    println!("rtcheck ok: {} positions, {} solves with reach threats on, mismatches 0", n, solves);
    for l in 0..16 {
        if by_level[l][0] + by_level[l][1] > 0 {
            println!("#   level {:2}: {} without a reach prune at the node, {} with", l, by_level[l][0], by_level[l][1]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// start3 (E29, third report): every reach test in start3's domain (3+ pieces, target above the
// cells in hand) is asked again in the other mode with the same work, and (sampled) in both modes
// with unlimited work, and (sampled, small hands) against a brute force over placements.

/// "Cannot reach" answers of the threat test that stayed undecided at the cap: [start3 off, on].
pub static NR_UND: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

pub const RQ_NAMES: [&str; 14] = [
    "queries in start3's domain",
    "limited: both can",
    "limited: start3 cannot, plain can",
    "limited: start3 can, plain cannot (theory says 0)",
    "limited: both cannot",
    "unlimited: checked in both modes",
    "unlimited: truth cannot",
    "unlimited: undecided at the cap",
    "brute force: checked",
    "brute force: over its state cap",
    "changed answers: unlimited agrees with start3",
    "changed answers: undecided",
    "work used, start3 (sum)",
    "work used, plain (sum)",
];
pub static RQ: [[AtomicU64; 14]; 8] = [const { [const { AtomicU64::new(0) }; 14] }; 8];

#[inline]
fn rq(h: usize, i: usize, by: u64) {
    RQ[h.min(7)][i].fetch_add(by, Relaxed);
}

/// (unlimited log2 rate, unlimited cap, brute-force max hand, brute-force log2 rate, brute-force state cap)
pub fn rq_params() -> (u32, u32, u32, u32, u32) {
    static P: OnceLock<(u32, u32, u32, u32, u32)> = OnceLock::new();
    *P.get_or_init(|| {
        let f = env_u32("SHADOW_FAULT", 0);
        crate::bounds::FAULT.store(f, Relaxed);
        (
            env_u32("SHADOW_RQ_SHIFT", 0),
            env_u32("SHADOW_RQ_CAP", 20_000_000),
            env_u32("SHADOW_BF_HAND", 4),
            env_u32("SHADOW_BF_SHIFT", 3),
            env_u32("SHADOW_BF_CAP", 300_000),
        )
    })
}

pub fn report_rq() {
    let (sh, cap, bfh, bfs, bfc) = rq_params();
    eprintln!("# shadow (start3): fault {} unlimited rate 1/2^{} cap {} brute force hands <= {} rate 1/2^{} cap {}", crate::bounds::FAULT.load(Relaxed), sh, cap, bfh, bfs, bfc);
    eprintln!("#   no-reach answers undecided at the cap: start3 off {} on {}", NR_UND[0].load(Relaxed), NR_UND[1].load(Relaxed));
    for i in 0..RQ_NAMES.len() {
        let mut line = format!("#   {:52}", RQ_NAMES[i]);
        for h in 3..8 {
            line += &format!(" {:>11}", RQ[h][i].load(Relaxed));
        }
        eprintln!("{}   (hands 3 4 5 6 7)", line);
    }
}

/// Which checks are made (SHADOW_LAYERS, default 7): 1 the start3 differential in every reach test,
/// 2 every "cannot reach" of a threat test asked again with unlimited work, 4 the exact solves of
/// pruned moves and of the nodes where they were pruned.
pub fn layers() -> u32 {
    static L: OnceLock<u32> = OnceLock::new();
    *L.get_or_init(|| {
        rq_params();
        env_u32("SHADOW_LAYERS", 7)
    })
}

/// The reach test with unlimited work (up to the cap); None when the cap ran out.
pub fn unlimited(pos: &Pos, player: usize, target: i32, start3: bool) -> Option<bool> {
    let t = tables();
    let cap = rq_params().1;
    let mut work = cap;
    let mut wit = Witness::NONE;
    let r = can_reach_touching_core(t, pos, player, target, &mut work, &mut wit, start3);
    if work == 0 {
        None
    } else {
        Some(r)
    }
}

/// Brute force: can `player` reach a snake of `target` cells, the opponent frozen, by any sequence of
/// legal placements (new snakes included)? Failed states (own cells, pieces) are remembered. None
/// when more than `cap` states were looked at.
pub fn bf_reach(pos: &Pos, player: usize, target: i32, cap: u32) -> Option<bool> {
    let mut seen: std::collections::HashSet<(u128, u8)> = std::collections::HashSet::new();
    let mut n = 0u32;
    fn go(pos: &Pos, player: usize, target: i32, seen: &mut std::collections::HashSet<(u128, u8)>, n: &mut u32, cap: u32) -> Option<bool> {
        if pos.longest[player] as i32 >= target {
            return Some(true);
        }
        if !seen.insert((pos.cells[player], pos.left[player])) {
            return Some(false);
        }
        *n += 1;
        if *n > cap {
            return None;
        }
        let t = tables();
        let mut moves = Vec::new();
        pos.for_each_move(t, player, pos.left[player], |m| moves.push(m));
        for m in moves {
            let mut child = *pos;
            child.place_for(t, player, m);
            match go(&child, player, target, seen, n, cap) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => return None,
            }
        }
        Some(false)
    }
    go(pos, player, target, &mut seen, &mut n, cap)
}

static NOTES: AtomicU64 = AtomicU64::new(0);

/// Called for every reach test of the search (`can_reach_touching_w`): answers it in the mode the
/// run asked for, with the work it was given, and checks it as described above.
pub fn reach_hook(t: &crate::pieces::Tables, pos: &Pos, player: usize, target: i32, work: &mut u32, wit: &mut Witness) -> bool {
    let (sh, _, bfh, bfs, bfc) = rq_params();
    let s3 = START3.load(Relaxed);
    let w0 = *work;
    let r = can_reach_touching_core(t, pos, player, target, work, wit, s3);
    let pieces = pos.left[player].count_ones() as usize;
    let rem = hand_cells(pos.left[player]);
    if pos.longest[player] as i32 >= target || pieces <= 2 || target <= rem || w0 == 0 || layers() & 1 == 0 {
        return r;
    }
    rq(pieces, 0, 1);
    // the other mode, the same work
    let mut w2 = w0;
    let mut dw = Witness::NONE;
    let r2 = can_reach_touching_core(t, pos, player, target, &mut w2, &mut dw, !s3);
    let (on, off, won, woff) = if s3 { (r, r2, w0 - *work, w0 - w2) } else { (r2, r, w0 - w2, w0 - *work) };
    rq(pieces, 12, won as u64);
    rq(pieces, 13, woff as u64);
    let cell = match (on, off) {
        (true, true) => 1,
        (false, true) => 2,
        (true, false) => 3,
        (false, false) => 4,
    };
    rq(pieces, cell, 1);
    let desc = || format!("pos {} player {} target {} (start3 limited {}, plain limited {}, work {})", pos.to_text(), player, target, on, off, w0);
    if cell == 2 && NOTES.fetch_add(1, Relaxed) < 200 {
        eprintln!("SHADOW CHANGED start3 cannot, plain can (limited): {}", desc());
    }
    if cell == 3 && NOTES.fetch_add(1, Relaxed) < 200 {
        eprintln!("SHADOW NOTE start3 can, plain cannot: {}", desc());
    }
    let key = mix((pos.cells[0] as u64) ^ ((pos.cells[0] >> 64) as u64).rotate_left(17) ^ (pos.cells[1] as u64).rotate_left(31) ^ ((pos.cells[1] >> 64) as u64).rotate_left(47) ^ ((pos.left[player] as u64) << 8) ^ (target as u64) << 20 ^ player as u64);
    let changed = on != off;
    if changed || sh == 0 || key & ((1u64 << sh) - 1) == 0 {
        rq(pieces, 5, 1);
        let u_on = unlimited(pos, player, target, true);
        let u_off = unlimited(pos, player, target, false);
        if let (Some(a), Some(b)) = (u_on, u_off) {
            if a != b {
                fail("start3 unlimited", format!("{}: unlimited start3 {} plain {}", desc(), a, b));
            }
        }
        let truth = u_off.or(u_on);
        match truth {
            None => {
                rq(pieces, 7, 1);
                if changed {
                    rq(pieces, 11, 1);
                }
            }
            Some(tr) => {
                if !tr {
                    rq(pieces, 6, 1);
                }
                if tr && (!on || !off) {
                    fail("start3 limited", format!("{}: a limited 'cannot' but the unlimited test reaches it", desc()));
                }
                if changed && tr == on {
                    rq(pieces, 10, 1);
                }
            }
        }
        if pieces <= bfh as usize && (bfs == 0 || (key >> 20) & ((1u64 << bfs) - 1) == 0) {
            match bf_reach(pos, player, target, bfc) {
                None => rq(pieces, 9, 1),
                Some(b) => {
                    rq(pieces, 8, 1);
                    if (truth.is_some() && truth != Some(b)) || (b && (!on || !off)) {
                        fail("start3 brute force", format!("{}: brute force {} unlimited {:?}", desc(), b, truth));
                    }
                }
            }
        }
    }
    r
}
