#!/usr/bin/env bash
# S-D: per-execution cycle counts from the risc0 executor (spike_d test). The
# test itself asserts the GO condition; this wrapper only records the table.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
EV="$R/evidence/localnet"; mkdir -p "$EV"
cargo test --locked --release -p spike_integration_tests --test spike_d -- --nocapture 2>&1 | tee "$R/.spike-S-D.log"
rc=${PIPESTATUS[0]}
# The cycle table is deterministic for a given guest image + lockfiles, so it is committed as evidence.
grep -E "^(D[0-9]|VERDICT|== S-D)" "$R/.spike-S-D.log" > "$EV/S-D.run.txt"
grep 'VERDICT S-D' "$R/.spike-S-D.log" || echo "VERDICT S-D: NO-GO -- test did not run or its GO assertion failed (exit $rc), see .spike-S-D.log"
exit "$rc"
