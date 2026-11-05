#!/usr/bin/env bash
# Shared helpers for the Argo standalone-sequencer harness.
#
# Rules (learned on LP-0002, see docs/m0/spike-plan.md):
#   * read chain state over JSON-RPC, never via wallet CLI output (it can stall
#     against a fresh sequencer);
#   * gate dependent steps by polling state, never by sleeping a fixed time;
#   * assume a minimal container: no `ss`, no `pkill`, only curl + python3.
#
# Source this file; do not execute it.

: "${ARGO_RPC:=http://127.0.0.1:${ARGO_PORT:-3048}}"

# rpc <method> <params-json>  -> raw JSON-RPC 2.0 response
rpc() {
  curl -sS -m 10 -X POST "$ARGO_RPC" -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" 2>/dev/null
}

rpc_account()     { rpc getAccount "[\"$1\"]"; }
rpc_transaction() { rpc getTransaction "[\"$1\"]"; }
rpc_block()       { rpc getBlock "[$1]"; }
rpc_balance()     { rpc getAccountBalance "[\"$1\"]"; }
rpc_program_ids() { rpc getProgramIds; }
rpc_health()      { rpc checkHealth; }

# A tx that fails execution is DROPPED from the block on v0.2.4: no receipt,
# no error code. "Rejected" therefore means getTransaction stays null for N
# blocks after submission; the sequencer log has the panic text.
rpc_tx_landed() { rpc_transaction "$1" | grep -q '"result":\['; }

# Sequencer answers RPC at all.
seq_up() { rpc getChannelId | grep -q '"result"'; }

# Account exists with a non-zero program_owner (initialised).
rpc_account_initialized() {
  rpc_account "$1" | python3 -c '
import json,sys
r=json.load(sys.stdin).get("result")
sys.exit(0 if r and any(r["program_owner"]) else 1)' 2>/dev/null
}

# Account balance > 0.
rpc_account_funded() {
  rpc_account "$1" | python3 -c '
import json,sys
r=json.load(sys.stdin).get("result")
sys.exit(0 if r and r["balance"]>0 else 1)' 2>/dev/null
}

# Field extractor: rpc_field <account-id> <python-expr-over-r>
rpc_field() {
  rpc_account "$1" | python3 -c "
import json,sys
r=json.load(sys.stdin).get('result')
print($2)" 2>/dev/null
}

# Chain tip. v0.2.4 exposes getLastBlockId; fall back to binary-searching
# getBlock for older sequencers that do not.
rpc_tip() {
  local t
  t=$(rpc getLastBlockId | python3 -c 'import json,sys; r=json.load(sys.stdin).get("result"); print(r if isinstance(r,int) else "")' 2>/dev/null)
  if [ -n "$t" ]; then echo "$t"; return 0; fi
  local lo=0 hi=1
  while rpc_block "$hi" | grep -q '"result":{'; do lo=$hi; hi=$((hi*2)); done
  while [ $((hi-lo)) -gt 1 ]; do
    local mid=$(((lo+hi)/2))
    if rpc_block "$mid" | grep -q '"result":{'; then lo=$mid; else hi=$mid; fi
  done
  echo "$lo"
}

# wait_until <seconds> <description> <cmd...> : poll until cmd exits 0.
wait_until() {
  local secs=$1 what=$2; shift 2
  local i
  for ((i=0; i<secs; i++)); do "$@" && return 0; sleep 1; done
  echo "FATAL: timed out after ${secs}s waiting for: $what" >&2
  return 1
}

die() { echo "FATAL: $*" >&2; exit 1; }
