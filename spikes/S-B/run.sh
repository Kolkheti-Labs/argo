#!/usr/bin/env bash
# S-B: custody rule on the builtin token program, decided by b1..b4 in
# spikes/integration_tests. B5 (which token the testnet runs) is a separate
# read-only probe: harness/probe-testnet-token.sh.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$R/.spike-S-B.log"
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"
cargo test --release -p spike_integration_tests --test spike_ab -- a1_b1 a5_b4 b2_ b5_ --nocapture 2>&1 | tee "$LOG"
if grep -q 'test result: ok' "$LOG" && ! grep -q 'FAILED' "$LOG"; then
  echo "VERDICT S-B: GO -- anyone credits a claimed vault; only the program debits it (by seed); credit-before-claim rejected, claim-first ordering confirmed; builtin token id == testnet capture"
else
  echo "VERDICT S-B: NO-GO -- see $LOG"
fi
