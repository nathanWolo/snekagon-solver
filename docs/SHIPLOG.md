# Frozen versions

Newest first. Each row is a base the experiments ran against. To go back to one, copy its file from
`frozen/` over `frozen/base.exe` and check out its commit.

| Version | Id | Commit | Date | What it adds | Reads it was frozen on |
| --- | --- | --- | --- | --- | --- |
| v2 | `a146108981b9` | `44d720c` | 2026-11-14 | E2: threat zones | gate `E2` pass (+59.5% where it stopped, ply 7); `E2-UP` 0.54x at ply 6 |
| v1 | `46104e473a4f` | `072ada9` | 2026-11-14 | E1: the last pieces in closed form | gate `E1` pass (+60.6% where it stopped, ply 7); `E1-UP` 0.62x at ply 6 |
| v0 | `917e1b93969f` | `44a10bb` | 2026-11-14 | alpha-beta, table, length bounds, static ordering | labels equal on 8,000 positions (plies 9-11); A/A zero |
