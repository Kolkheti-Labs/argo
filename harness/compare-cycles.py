#!/usr/bin/env python3
"""Compare a fresh S-D cycle table with the committed one.

The guest ELF embeds absolute source paths in panic-location strings, so the
image id and the exact cycle counts depend on the checkout directory (see
docs/m0/riscv32-build.md). Rows, labels and the verdict word must match exactly; segment counts may
differ by one; cycle counts may differ by at most TOLERANCE.
Usage: compare-cycles.py <committed> <fresh>
"""
import re, sys

TOLERANCE = 0.005  # 0.5 %

def parse(path):
    rows, verdict = {}, None
    for line in open(path):
        m = re.match(r"^(D\S+ .*?)\s{2,}(\d+) cycles(?:\s+(\d+) seg)?", line)
        if m:
            rows[m.group(1).strip()] = (int(m.group(2)), m.group(3))
        if line.startswith("VERDICT S-D:"):
            verdict = line.split("--")[0].strip()
    return rows, verdict

a, va = parse(sys.argv[1]); b, vb = parse(sys.argv[2])
bad = []
if set(a) != set(b):
    bad.append(f"row sets differ: only committed={sorted(set(a)-set(b))} only fresh={sorted(set(b)-set(a))}")
for k in sorted(set(a) & set(b)):
    (ca, sa), (cb, sb) = a[k], b[k]
    # Segment count is the po2-padded split of the same trace; a one-segment
    # difference is the boundary moving with the ELF layout, not a change in work.
    if sa is not None and sb is not None and abs(int(sa) - int(sb)) > 1:
        bad.append(f"{k}: segments {sa} vs {sb}")
    if abs(ca - cb) > TOLERANCE * max(ca, 1):
        bad.append(f"{k}: {ca} vs {cb} cycles ({abs(ca-cb)/max(ca,1):.2%} > {TOLERANCE:.1%})")
if va != vb:
    bad.append(f"verdict differs: {va!r} vs {vb!r}")
if bad:
    print("CYCLE TABLE MISMATCH:\n  " + "\n  ".join(bad)); sys.exit(1)
worst = max((abs(a[k][0]-b[k][0])/max(a[k][0],1) for k in a), default=0)
print(f"cycle table matches ({len(a)} rows, worst drift {worst:.3%}, verdict {va!r})")
