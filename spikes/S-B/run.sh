#!/usr/bin/env bash
# S-B: custody rule on the LEZ v0.2.4 builtin token program, decided by
# a1_b1_b3_*, a5_b4_* and b2_* in spikes/integration_tests/tests/spike_ab.rs.
# A foreign-program debit attempt was not built in M0.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
EV="$R/evidence/localnet"; mkdir -p "$EV"
export RISC0_BUILD_LOCKED=1
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"
cargo test --locked --release -p spike_integration_tests --test spike_ab -- a1_b1 a5_b4 b2_ --nocapture 2>&1 | tee "$R/.spike-S-B.log"
rc=${PIPESTATUS[0]}
grep -E "^test |^test result|^B[0-9] error|^A5 error" "$R/.spike-S-B.log" | sed -E 's/ finished in [0-9.]+s//' | sed -E 's/account_id: [1-9A-HJ-NP-Za-km-z]{32,44}/account_id: <pda>/g' | sort > "$EV/S-B.run.txt"
if [ "$rc" -eq 0 ]; then
  echo "VERDICT S-B: GO -- anyone credits a claimed vault; only the program debits it (by seed); credit before claim is rejected (ClaimedUnauthorizedAccount) so create_market claims first"
else
  echo "VERDICT S-B: NO-GO -- tests failed or did not run (exit $rc), see .spike-S-B.log"
fi
exit "$rc"
