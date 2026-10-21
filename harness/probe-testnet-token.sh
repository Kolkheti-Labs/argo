#!/usr/bin/env bash
# Read-only: saves the public testnet's getProgramIds reply and prints its tip.
# On LEZ v0.2.4 getProgramIds returns a map of ids hardcoded in the sequencer,
# four builtin programs and the privacy-preserving circuit
# (lez/sequencer/service/src/service.rs:199-214, "TODO: Get programs from
# state"), not the chain's registered programs, so this cannot show which
# programs the testnet runs. Kept for the record; S-B B5 was
# withdrawn on 2026-10-16 (docs/m0/spikes/S-B-lp0013-custody.md).
set -uo pipefail
R="$(cd "$(dirname "$0")/.." && pwd)"
ARGO_RPC="${TESTNET_RPC:-https://testnet.lez.logos.co}"
export ARGO_RPC
# shellcheck source=lib.sh
source "$R/harness/lib.sh"
mkdir -p "$R/evidence/testnet"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$R/evidence/testnet/getProgramIds-$STAMP.json"
rpc getProgramIds | tee "$OUT"
echo
echo "saved $OUT"
rpc getLastBlockId
echo
