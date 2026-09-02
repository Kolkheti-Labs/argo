#!/usr/bin/env bash
# Argo M0 spike runner.
#   ./spikes/run.sh S-A        run one spike
#   ./spikes/run.sh all        run all five, in dependency order
#
# Each spike prints a final line of the form
#   VERDICT S-X: GO|NO-GO|PARTIAL -- <one-line observable>
# and exits 0 only if it produced a verdict (a NO-GO is still exit 0; the
# spike failing to run at all is non-zero). docs/m0/verdicts.md quotes these
# lines verbatim.
set -euo pipefail

R="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../harness/lib.sh
source "$R/harness/lib.sh"

run_one() {
  local s=$1
  local f="$R/spikes/$s/run.sh"
  [ -x "$f" ] || die "spike $s has no runnable at $f"
  echo "=== spike $s ==="
  "$f"
}

case "${1:-}" in
  all) for s in S-A S-B S-D S-C S-E; do run_one "$s"; done ;;
  S-A|S-B|S-C|S-D|S-E) run_one "$1" ;;
  *) echo "usage: $0 {S-A|S-B|S-C|S-D|S-E|all}" >&2; exit 2 ;;
esac
