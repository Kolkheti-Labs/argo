#!/usr/bin/env bash
# Argo M0 spike runner.
#   ./spikes/run.sh S-A        run one spike
#   ./spikes/run.sh all        run all five, in dependency order
#
# Each spike prints a final line of the form
#   VERDICT S-X: GO|NO-GO|PARTIAL -- <one-line observable>
# Exit status is about the EXPERIMENT, not the verdict: a spike exits 0 only
# when its measurements ran to completion and its decisive assertions held
# (a designed NO-GO such as S-C is exit 0; a compile error, a failed test, a
# transaction that never landed, or a verdict contradicting the observation
# is non-zero and fails CI). docs/m0/verdicts.md quotes the lines verbatim.
set -euo pipefail

R="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../harness/lib.sh
source "$R/harness/lib.sh"

run_one() {
  local s=$1
  local f="$R/spikes/$s/run.sh"
  [ -x "$f" ] || die "spike $s has no runnable at $f"
  echo "=== spike $s ==="
  "$f" || { echo "spike $s FAILED (exit $?)" >&2; return 1; }
}

case "${1:-}" in
  all) rc=0; for s in S-A S-B S-D S-C S-E; do run_one "$s" || rc=1; done; exit $rc ;;
  S-A|S-B|S-C|S-D|S-E) run_one "$1" ;;
  *) echo "usage: $0 {S-A|S-B|S-C|S-D|S-E|all}" >&2; exit 2 ;;
esac
