p='src/evalorder.rs'
s=open(p,encoding='utf-8').read()
old="""    // first arrival; the side to move takes the ties
    let mut mine = [0u128; 2];"""
new="""    // the same race where a side cannot walk through the cells the other has taken: the side to
    // move takes a layer, then the other, by turns
    let mut held = [wave[s] & 0, 0u128];
    {
        let mut front = [wave[s], wave[m]];
        let mut taken = 0u128;
        for _ in 0..40 {
            let a = front[0] & !taken;
            taken |= a;
            let b = front[1] & !taken;
            taken |= b;
            held[0] |= a;
            held[1] |= b;
            if a | b == 0 {
                break;
            }
            front = [neighbours(a) & room[s] & !taken, neighbours(b) & room[m] & !taken];
        }
    }
    let terrb = (count(held[1]) as i32).min(40) - (count(held[0]) as i32).min(40);
    // first arrival; the side to move takes the ties
    let mut mine = [0u128; 2];"""
assert s.count(old)==1; s=s.replace(old,new,1)
s=s.replace("""        child.nsnakes[m] as i32 - child.nsnakes[s] as i32,
    ]""","""        child.nsnakes[m] as i32 - child.nsnakes[s] as i32,
        terrb,
    ]""")
s=s.replace('pub const NFEAT: usize = 9;\npub const NAMES: [&str; NFEAT] = ["est", "ub", "long", "terr", "hand", "ends", "short", "fresh", "snakes"];',
 'pub const NFEAT: usize = 10;\npub const NAMES: [&str; NFEAT] = ["est", "ub", "long", "terr", "hand", "ends", "short", "fresh", "snakes", "terrb"];')
s=s.replace("""//!   snakes the number of snakes
""","""//!   snakes the number of snakes
//!   terrb  cells taken (at most 40) when the two walk by turns, layer by layer, and neither may
//!         pass through the other's cells (the side to move first)
""")
s=s.replace("pub const WEIGHTS: [i32; NFEAT + 2] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0];","pub const WEIGHTS: [i32; NFEAT + 2] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0];")
open(p,'w',encoding='utf-8',newline='\n').write(s)
