#!/usr/bin/env bash
# S-D: per-execution cycle counts from the risc0 executor (spike_d test).
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$R/.spike-S-D.log"
cargo test --release -p spike_integration_tests --test spike_d -- --nocapture 2>&1 | tee "$LOG"
grep 'VERDICT S-D' "$LOG" || echo "VERDICT S-D: NO-GO -- test did not run, see $LOG"
