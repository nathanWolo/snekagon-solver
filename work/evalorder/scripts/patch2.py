p='src/solver.rs'
s=open(p,encoding='utf-8').read()
old="""                if use_eval {
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
s=s.replace(old,"",1)
old="""                    score = (s + (1 << 20)).max(0) as u64;
                }
                if m == tt_move {
                    score = TT_MOVE;"""
new="""                    score = (s + (1 << 20)).max(0) as u64;
                    model_s = s;
                }
                if use_eval {
                    // E32: the evaluation of the position after the move, with the model's score as one term
                    let mut child = *pos;
                    child.play(t, m);
                    let v = if child.is_over() { -game_result(&child) << 19 } else { crate::evalorder::score(t, &child, &ew) + ew[crate::evalorder::NFEAT] * model_s };
                    score = (v + (1 << 20)).clamp(0, (1 << 21) - 1) as u64;
                }
                if m == tt_move {
                    score = TT_MOVE;"""
assert s.count(old)==1, s.count(old)
s=s.replace(old,new,1)
old="""                let mut score = p.n as u64 * 64;
                if p.border & own != 0 {
                    score += 256;
                }
                if p.cells & front != 0 {
                    score += 128;
                } else if p.border & theirs != 0 {
                    score += 32;
                }
                if use_model {
                    let mut joined = p.n as i32;
                    for j in 0..pos.nsnakes[me] as usize {
                        if pos.snakes[me][j] & p.border != 0 {
                            joined += count(pos.snakes[me][j]) as i32;
                        }
                    }
                    let after = joined.max(pos.longest[me] as i32);
                    let mine = shape_of(own | p.cells);
                    let my_ends = count(neighbours(mine.ends) & empty & !p.cells & !neighbours(mine.body)) as i32;
                    let their_ends = count(their_free & !p.cells) as i32;
                    let mut s ="""
new="""                let mut score = p.n as u64 * 64;
                if p.border & own != 0 {
                    score += 256;
                }
                if p.cells & front != 0 {
                    score += 128;
                } else if p.border & theirs != 0 {
                    score += 32;
                }
                let mut model_s = 0i32;
                if use_model {
                    let mut joined = p.n as i32;
                    for j in 0..pos.nsnakes[me] as usize {
                        if pos.snakes[me][j] & p.border != 0 {
                            joined += count(pos.snakes[me][j]) as i32;
                        }
                    }
                    let after = joined.max(pos.longest[me] as i32);
                    let mine = shape_of(own | p.cells);
                    let my_ends = count(neighbours(mine.ends) & empty & !p.cells & !neighbours(mine.body)) as i32;
                    let their_ends = count(their_free & !p.cells) as i32;
                    let mut s ="""
assert s.count(old)==1, s.count(old)
s=s.replace(old,new,1)
s=s.replace("pub eval_w: [i32; crate::evalorder::NFEAT],","pub eval_w: [i32; crate::evalorder::NFEAT + 1],")
open(p,'w',encoding='utf-8',newline='\n').write(s)
p='src/main.rs'
s=open(p,encoding='utf-8').read()
s=s.replace('assert!(v.len() == crate::evalorder::NFEAT, "--eval-w needs {} weights", crate::evalorder::NFEAT);','assert!(v.len() == crate::evalorder::NFEAT + 1, "--eval-w needs {} weights", crate::evalorder::NFEAT + 1);')
open(p,'w',encoding='utf-8',newline='\n').write(s)
p='src/evalorder.rs'
s=open(p,encoding='utf-8').read()
s=s.replace("""/// The fitted weights (scaled by 64), in the order of `NAMES`.
pub const WEIGHTS: [i32; NFEAT] = [64, 0, 0, 0, 0, 0, 0, 0];""","""/// The fitted weights, in the order of `NAMES`, then the weight of the model's score (`MODEL`, the
/// order below these levels; 64 with every other weight 0 is today's order).
pub const WEIGHTS: [i32; NFEAT + 1] = [0, 0, 0, 0, 0, 0, 0, 0, 64];""")
s=s.replace("pub fn score(t: &Tables, child: &Pos, w: &[i32; NFEAT]) -> i32 {","pub fn score(t: &Tables, child: &Pos, w: &[i32; NFEAT + 1]) -> i32 {")
s=s.replace("//! The weights are fitted on our own solved positions (LOG, E32).","//! The order score is the weighted sum of these terms and of the model's score of the move.\n//! The weights are fitted on our own solved positions (LOG, E32).")
open(p,'w',encoding='utf-8',newline='\n').write(s)
