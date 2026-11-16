import re
p='src/solver.rs'
s=open(p,encoding='utf-8').read()
s=s.replace("""    pub rt_refuter: u8,
}
""","""    pub rt_refuter: u8,
    /// E32: at nodes with at least this many pieces left, the listed moves are ordered by the
    /// evaluation of the position after each (`evalorder.rs`) instead of the model (99 = never).
    pub eval_level: u32,
    /// E32: the weights of that evaluation.
    pub eval_w: [i32; crate::evalorder::NFEAT],
    /// E32: the answer bonus at those nodes (added to the score of each move that covers the reply
    /// that refuted the move before; the scores of the evaluation lie below 1 << 21).
    pub eval_bonus: u64,
}
""",1)
s=s.replace("rt_refuter: 0 }","rt_refuter: 0, eval_level: 99, eval_w: crate::evalorder::WEIGHTS, eval_bonus: 1 << 21 }",1)
old="""            let use_model = level as u32 >= self.cfg.model_level;
            let w = self.cfg.model;
            let empty = pos.empty();
            let their_shape = shape_of(theirs);
            let their_free = neighbours(their_shape.ends) & empty & !neighbours(their_shape.body);
            pos.for_each_move(t, me, pos.pieces_now(), |m| {
                let p = &t.placements[m as usize];
                if th.n > 0 {
                    let bound = th.bound_for_move(p.cells, p.border & pos.cells[me] != 0);
                    if bound <= alpha {
                        skipped = skipped.max(bound);
                        return;
                    }
                }
"""
new="""            let use_model = level as u32 >= self.cfg.model_level;
            let use_eval = level as u32 >= self.cfg.eval_level;
            let w = self.cfg.model;
            let ew = self.cfg.eval_w;
            let empty = pos.empty();
            let their_shape = shape_of(theirs);
            let their_free = neighbours(their_shape.ends) & empty & !neighbours(their_shape.body);
            pos.for_each_move(t, me, pos.pieces_now(), |m| {
                let p = &t.placements[m as usize];
                if th.n > 0 {
                    let bound = th.bound_for_move(p.cells, p.border & pos.cells[me] != 0);
                    if bound <= alpha {
                        skipped = skipped.max(bound);
                        return;
                    }
                }
                if use_eval {
                    // E32: the evaluation of the position after the move
                    let mut child = *pos;
                    child.play(t, m);
                    let v = if child.is_over() { -game_result(&child) << 19 } else { crate::evalorder::score(t, &child, &ew) };
                    let mut score = (v + (1 << 20)).clamp(0, (1 << 21) - 1) as u64;
                    if m == tt_move {
                        score = TT_MOVE;
                    } else if m == k1 {
                        score = KILLER1;
                    } else if m == k2 {
                        score = KILLER2;
                    }
                    buf.push(score << 16 | m as u64);
                    return;
                }
"""
assert s.count(old)==1
s=s.replace(old,new,1)
old="""                        let bonus = self.cfg.answer_bonus << 16;
                        for e in buf[i..].iter_mut() {"""
new="""                        let bonus = if level as u32 >= self.cfg.eval_level { self.cfg.eval_bonus } else { self.cfg.answer_bonus } << 16;
                        for e in buf[i..].iter_mut() {"""
assert s.count(old)==1
s=s.replace(old,new,1)
open(p,'w',encoding='utf-8',newline='\n').write(s)
p='src/main.rs'
s=open(p,encoding='utf-8').read()
s=s.replace("mod bounds;\n","mod bounds;\nmod evalorder;\n",1)
old='            "--no-model" => o.cfg.model_level = 99,\n'
new=old+'''            "--eval-order" => o.cfg.eval_level = val().parse().unwrap(),
            "--eval-bonus" => o.cfg.eval_bonus = val().parse().unwrap(),
            "--eval-w" => {
                let v: Vec<i32> = val().split(',').map(|x| x.parse().unwrap()).collect();
                assert!(v.len() == crate::evalorder::NFEAT, "--eval-w needs {} weights", crate::evalorder::NFEAT);
                o.cfg.eval_w.copy_from_slice(&v);
            }
'''
assert s.count(old)==1
s=s.replace(old,new,1)
open(p,'w',encoding='utf-8',newline='\n').write(s)
