# Frozen versions

Newest first. Each row is a base the experiments ran against. To go back to one, copy its file from
`frozen/` over `frozen/base.exe` and check out its commit.

| Version | Id | Commit | Date | What it adds | Reads it was frozen on |
| --- | --- | --- | --- | --- | --- |
| v9 | `2deb69fb855b` | `84e8f3b` | 2026-11-15 | E14: after a failed move, the moves that cover the refuting reply go first | gate `E14` pass (+24.7% [+14.6%, +35.7%] where it stopped, ply 7); tune: nodes 0.77x at ply 7, 0.78x at ply 6, 0.59x on 30 ply-5 positions |
| v8 | `a5a28e371f18` | `2adb793` | 2026-11-15 | E12: the reach test starts from the longest snake of the target snake; joins pruned by size and distance | gate `E12` pass (+28.0% [+22.2%, +34.2%] where it stopped, ply 7; node counts identical to v7) |
| v7 | `d9bb32dbac68` | `5fd3fee` | 2026-11-15 | E11: a threat scan recomputes only the clusters a placement covers | gate `E11-P7` pass (+4.8% [+4.2%, +5.4%], ply 7; node counts identical to v6); the first gate, `E11` at ply 6, was void (24 units left, the test needs 30) and read +5.3% |
| v6 | `ab0ef6e42b6d` | `5c09458` | 2026-11-15 | E10: the exact reach question first; no cluster bound for a side that has it | gate `E10` pass (+16.7% [+13.3%, +20.3%] where it stopped, ply 6) |
| v5 | `c01a8bd36dee` | `68d5e9e` | 2026-11-15 | E9: the table move or a killer is searched before the move list is generated | gate `E9` pass (+11.4% [+9.8%, +13.0%], ply 6; node counts identical to v4) |
| v4 | `66c7f2a9e5ee` | `a1000a0` | 2026-11-15 | E4: upper moves ordered by a fitted model | gate `E4` pass (+45.6% where it stopped, ply 6); `E4-UP` 0.92x at ply 5 |
| v3 | `ae34f6c8332c` | `b67a0c4` | 2026-11-14 | E3: reach test for hands of one or two pieces | gate `E3` pass (+31.4% where it stopped, ply 6); `E3-UP` 0.66x at ply 5 |
| v2 | `a146108981b9` | `44d720c` | 2026-11-14 | E2: threat zones | gate `E2` pass (+59.5% where it stopped, ply 7); `E2-UP` 0.54x at ply 6 |
| v1 | `46104e473a4f` | `072ada9` | 2026-11-14 | E1: the last pieces in closed form | gate `E1` pass (+60.6% where it stopped, ply 7); `E1-UP` 0.62x at ply 6 |
| v0 | `917e1b93969f` | `44a10bb` | 2026-11-14 | alpha-beta, table, length bounds, static ordering | labels equal on 8,000 positions (plies 9-11); A/A zero |
