//! A parallel solve: every thread runs the same search on one shared table. A thread that meets a
//! move another thread is already searching sets it aside (see `Busy`), so the moves of a node that
//! must all be refuted are divided between the threads. The first thread to finish has the answer.
//! Every value any thread stores is a proof, so the answer is exact whatever the timing.

use crate::pos::*;
use crate::solver::*;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64};
use std::sync::Arc;

pub struct ParSolver {
    cfg: Config,
    threads: usize,
    table: Arc<Table>,
    /// Nodes of all threads in the last solve.
    pub nodes: u64,
}

impl ParSolver {
    pub fn new(cfg: Config, threads: usize) -> ParSolver {
        ParSolver { cfg, threads, table: Arc::new(Table::new(cfg.tt_bits)), nodes: 0 }
    }

    /// The exact value for the side to move.
    pub fn solve(&mut self, pos: &Pos) -> i32 {
        self.table.next_generation();
        let stop = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(Busy::new());
        let answer = AtomicI32::new(UNKNOWN);
        let nodes = AtomicU64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..self.threads {
                let mut solver = Solver::with_table(self.cfg, self.table.clone());
                solver.stop = Some(stop.clone());
                solver.busy = Some(busy.clone());
                solver.sync();
                let (stop, answer, nodes) = (&stop, &answer, &nodes);
                scope.spawn(move || {
                    let v = solver.solve(pos, -1, 1, u64::MAX);
                    if v != UNKNOWN {
                        answer.store(v, Relaxed);
                        stop.store(true, Relaxed);
                    }
                    nodes.fetch_add(solver.nodes, Relaxed);
                });
            }
        });
        self.nodes = nodes.load(Relaxed);
        answer.load(Relaxed)
    }
}
