//! Check build of E33, the tight question (feature `tqcheck`; a verification tool, not the solver).
//!
//! At every node the rule decides, the node is solved again exactly (full window) by a second
//! solver that has the rule switched off (`tight_q = 0`) and a table of its own, so nothing the rule
//! computed can answer it and it never writes into the searching solver's table. The program aborts
//! when the exact value breaks the rule's claim (at most 0 for alpha >= 0, a loss for alpha = -1).
//! At the same node G is compared with the brute force (`sure_next_length_brute`: every move of
//! ours, then every placement of the opponent): the G used must not be above it. The "cannot reach"
//! answer is asked again with unlimited work, and for a hand of at most two (sampled for three)
//! compared with the one-player optimum by brute force.
//!
//! At nodes where the rule is asked and does not decide, G is compared with the brute force on a
//! sample (one in 2^TQCHECK_GSHIFT, default 3, by a hash of the position): a G at or above the floor
//! must not be above the brute force; a G found below the floor while the brute force is at or above
//! it is counted (a lost decision, not an error).
//!
//! The searching solver's node counts are those of the plain build: the checks only read.
#![allow(dead_code)]

use crate::bounds::*;
use crate::pieces::*;
use crate::pos::*;
use crate::solver::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::OnceLock;

pub const NAMES: [&str; 14] = [
    "G checked against the brute force",
    "  G at or above the floor, equal to the brute force",
    "  G at or above the floor, below the brute force (kept extensions)",
    "  G below the floor, brute force at or above it (lost)",
    "  G below the floor, brute force below it",
    "nodes decided (all re-searched)",
    "  decided by the cluster bound",
    "  decided by a reach question",
    "  exact value -1",
    "  exact value 0",
    "  exact value +1 (would be a mismatch)",
    "cannot-reach answers asked again with unlimited work",
    "cannot-reach answers checked by the one-player optimum",
    "mismatches",
];
pub static C: [AtomicU64; 14] = [const { AtomicU64::new(0) }; 14];

#[inline]
fn hit(i: usize) {
    C[i].fetch_add(1, Relaxed);
}

pub fn report() {
    eprintln!("# tqcheck: G sample 1/2^{}, optimum sample for hand 3 1/2^{}", gshift(), oshift());
    for i in 0..NAMES.len() {
        eprintln!("#   {:70} {}", NAMES[i], C[i].load(Relaxed));
    }
}

fn fail(what: &str, detail: String) -> ! {
    hit(13);
    eprintln!("TQCHECK MISMATCH in {}: {}", what, detail);
    report();
    std::process::abort();
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn gshift() -> u32 {
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| env_u32("TQCHECK_GSHIFT", 3))
}

fn oshift() -> u32 {
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| env_u32("TQCHECK_OSHIFT", 4))
}

#[inline]
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

thread_local! {
    static SH: RefCell<Option<Solver>> = const { RefCell::new(None) };
}

/// The exact value of `pos` for the side to move, by a solver with the rule off and its own table.
pub fn exact(cfg: &Config, pos: &Pos) -> i32 {
    SH.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            let mut c = *cfg;
            c.tight_q = 0;
            c.tq_study = false;
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

fn compare_g(pos: &Pos, floor: i32, g: i32) {
    let t = tables();
    hit(0);
    let bf = sure_next_length_brute(t, pos);
    let floor = floor.max(pos.longest[pos.stm as usize ^ 1] as i32 + 1);
    if g >= floor {
        if g > bf {
            fail("G", format!("position {} floor {} G {} brute force {}", pos.to_text(), floor, g, bf));
        }
        hit(if g == bf { 1 } else { 2 });
    } else {
        hit(if bf >= floor { 3 } else { 4 });
    }
}

/// G as the rule computed it at an asked node (sampled).
pub fn check_g(pos: &Pos, floor: i32, g: i32) {
    if mix(pos.hash ^ 0x6a09e667) & ((1u64 << gshift()) - 1) != 0 {
        return;
    }
    compare_g(pos, floor, g);
}

/// A node the rule decided: G, the "cannot reach" answer and the value.
pub fn check_decided(cfg: &Config, pos: &Pos, alpha: i32, g: i32, target: i32, by_reach: bool) {
    let t = tables();
    let me = pos.stm as usize;
    hit(5);
    hit(if by_reach { 7 } else { 6 });
    compare_g(pos, i32::MIN, g);
    if by_reach {
        hit(11);
        let mut work = u32::MAX;
        if can_reach_touching(t, pos, me, target, &mut work) {
            fail("cannot-reach answer", format!("position {} target {}: the unlimited test reaches it", pos.to_text(), target));
        }
        let hand = pos.left[me].count_ones();
        if hand <= 2 || (hand == 3 && mix(pos.hash ^ 0xbb67ae85) & ((1u64 << oshift()) - 1) == 0) {
            hit(12);
            let opt = one_player_optimum(t, pos, me);
            if opt >= target {
                fail("cannot-reach answer", format!("position {} target {}: one-player optimum {}", pos.to_text(), target, opt));
            }
        }
    }
    let e = exact(cfg, pos);
    hit(8 + (e + 1) as usize);
    let claim = if alpha >= 0 { 0 } else { -1 };
    if e > claim {
        fail("value", format!("position {} alpha {} G {} target {} by reach {}: exact {} above the claim {}", pos.to_text(), alpha, g, target, by_reach, e, claim));
    }
}
