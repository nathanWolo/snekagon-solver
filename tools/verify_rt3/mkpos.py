# a red position where the final snake must take in four old snakes (S + 3) with three pieces
def ok(q, r): return abs(q) <= 5 and abs(r) <= 5 and abs(q + r) <= 5
def idx(q, r): return (r + 5) * 11 + (q + 5)
# boundary ring of the side-6 hexagon, in order: walk the six sides
corners = [(5, 0), (5, -5), (0, -5), (-5, 0), (-5, 5), (0, 5)]
ring = []
for i in range(6):
    a, b = corners[i], corners[(i + 1) % 6]
    dq, dr = (b[0] - a[0]) // 5, (b[1] - a[1]) // 5
    for k in range(5):
        ring.append((a[0] + k * dq, a[1] + k * dr))
# corners are at ring indices 0,5,10,...; we need a corner at path index 18: start at ring index 2
start = 2
path = [ring[(start + i) % 30] for i in range(21)]
assert all(ok(q, r) for q, r in path)
seg = {"A": range(0, 4), "B": range(6, 10), "C": range(13, 17), "D": [20]}
red = 0
for s, rg in seg.items():
    for i in rg:
        red |= 1 << idx(*path[i])
E = [(-1, 0), (0, 0), (1, 0), (2, 0)]
for c in E:
    red |= 1 << idx(*c)
print("gaps", [path[i] for i in (4, 5)], [path[i] for i in (10, 11, 12)], [path[i] for i in (17, 18, 19)])
print("corners on path at", [i for i, c in enumerate(path) if c in corners])
# red holds 2, 3I, 3V (bits 1, 2, 3); blue some pieces; red to move
print("%032x_%032x_%d_%d_0_12_0" % (red, 0, 2 + 4 + 8, 2 + 4 + 8))
