#!/usr/bin/env bash
# S-A: chained-call atomicity + re-entry, decided by the in-process state
# machine tests a1..a6 in spikes/integration_tests (RISC0_DEV_MODE=1), then
# one sequencer observation of a dropped tx (see harness) once the harness is up.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$R/.spike-S-A.log"
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"
cargo test --release -p spike_integration_tests --test spike_ab -- a1_ a2_ a3_ a4_ a5_ a6_ --nocapture 2>&1 | tee "$LOG"
if grep -q 'test result: ok' "$LOG" && ! grep -q 'FAILED' "$LOG"; then
  echo "VERDICT S-A: GO -- a1..a6 green: late-leg failure reverted everything; self re-entry works; top-level Internal, wrong seed, forged debit, 11-call chain all rejected"
else
  echo "VERDICT S-A: NO-GO -- see $LOG"
fi
