#!/usr/bin/env bash
# S-A: chained-call atomicity + re-entry, decided by the in-process state
# machine tests a1..a6 in spikes/integration_tests/tests/spike_ab.rs
# (RISC0_DEV_MODE=1, real guest ELF, real builtin token). No sequencer-layer
# observation is part of this spike in M0.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
EV="$R/evidence/localnet"; mkdir -p "$EV"
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"
cargo test --locked --release -p spike_integration_tests --test spike_ab -- a1_ a2_ a3_ a4_ a5_ a6_ --nocapture 2>&1 | tee "$R/.spike-S-A.log"
rc=${PIPESTATUS[0]}
# Committed evidence: the test list and results (stable across runs), not timings.
grep -E "^test |^test result|^A[0-9] error" "$R/.spike-S-A.log" | sed -E 's/ finished in [0-9.]+s//' | sort > "$EV/S-A.run.txt"
if [ "$rc" -eq 0 ]; then
  echo "VERDICT S-A: GO -- a1..a6 green: a failing last leg reverted the state write and the token transfer; self re-entry works and a top-level call into the internal entrypoint is rejected; wrong PDA seed, signer-less top-level debit, and an 11-call chain are rejected with the expected runtime errors"
else
  echo "VERDICT S-A: NO-GO -- tests failed or did not run (exit $rc), see .spike-S-A.log"
fi
exit "$rc"
