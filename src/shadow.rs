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
    let t = tables();
    hit(6);
    if from_cache {
        hit(7);
    }
    let mut work = u32::MAX;
    if can_reach_touching(t, child, me, need, &mut work) {
        fail("no-reach answer", format!("child {} player {} need {} cache {}: the unlimited test reaches it", child.to_text(), me, need, from_cache));
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
pub fn rtcheck(path: &str, base: Config, cfgs: &[(&str, Config)], from: usize, to: usize) {
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
        off.reach_threat = false;
        off.tt_bits = 18;
        let mut s = Solver::new(off);
        let e = s.solve(&pos, -1, 1, u64::MAX);
        let mut windows = vec![(-1, 1), (-1, 0), (0, 1)];
        if !windows.contains(&(a, b)) && a < b {
            windows.push((a, b));
        }
        for (name, c) in cfgs {
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
    }
    println!("rtcheck ok: {} positions, {} solves with reach threats on, mismatches 0", n, solves);
    for l in 0..16 {
        if by_level[l][0] + by_level[l][1] > 0 {
            println!("#   level {:2}: {} without a reach prune at the node, {} with", l, by_level[l][0], by_level[l][1]);
        }
    }
}
