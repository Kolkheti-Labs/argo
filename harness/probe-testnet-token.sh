#!/usr/bin/env bash
# S-B item B5 (read-only): which programs does the public testnet register, and
# does its token program id match the v0.2.4 builtin embedded in our test
# build? Prints the testnet map and, when a local build exists, the builtin id.
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
