#!/usr/bin/env bash
# S-C: event surface on v0.2.4. C1: does getTransaction/getBlock carry any
# event/log/receipt payload for an executed tx? C2: polling baseline latency.
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
source "$R/harness/lib.sh"
export ARGO_PORT="${ARGO_PORT:-3048}"; export ARGO_RPC="http://127.0.0.1:$ARGO_PORT"
TARGET="${CARGO_TARGET_DIR:-$R/target}"; RUN="$TARGET/release"
export LEE_WALLET_HOME_DIR="$R/.localnet-argo/wallet"
EV="$R/evidence/localnet"; mkdir -p "$EV"
trap '"$R/harness/localnet.sh" down' EXIT
"$R/harness/localnet.sh" up >/dev/null || die "localnet up failed"

OUT=$("$RUN/argo_deploy" argo_lending) || die deploy
PID=$(sed -n 's/^PROGRAM_ID=//p' <<<"$OUT"); DTX=$(sed -n 's/^TX=//p' <<<"$OUT")
wait_until 60 "deploy landed" rpc_tx_landed "$DTX"
OUT=$("$RUN/argo_initialize" "$PID") || die initialize
CFG=$(sed -n 's/^CONFIG_ID=//p' <<<"$OUT"); ITX=$(sed -n 's/^TX=//p' <<<"$OUT")
wait_until 60 "initialize landed" rpc_tx_landed "$ITX"

# C1: raw payloads for the executed public tx and its block
rpc_transaction "$ITX" > "$EV/S-C-getTransaction.json"
BLK=$(python3 -c 'import json,sys; r=json.load(open(sys.argv[1]))["result"]; print(r[1])' "$EV/S-C-getTransaction.json")
rpc_block "$BLK" > "$EV/S-C-getBlock.json"
KEYS=$(python3 - "$EV/S-C-getTransaction.json" "$EV/S-C-getBlock.json" <<'PY'
import json,sys
def keys(o, acc, path=""):
    if isinstance(o, dict):
        for k,v in o.items(): acc.add(k); keys(v, acc, path+"/"+k)
    elif isinstance(o, list):
        for v in o: keys(v, acc, path)
    return acc
ks=set()
for f in sys.argv[1:]: keys(json.load(open(f)), ks)
hits=[k for k in ks if any(w in k.lower() for w in ("event","log","receipt","status","error"))]
print("ALLKEYS="+",".join(sorted(ks)))
print("EVENTKEYS="+",".join(sorted(hits)))
PY
)
echo "$KEYS"
EVENTKEYS=$(sed -n 's/^EVENTKEYS=//p' <<<"$KEYS")

# C2: polling baseline. 200 sequential getAccount calls against the local sequencer.
S=$(python3 -c 'import time; print(time.time())')
for _ in $(seq 1 200); do rpc_account "$CFG" >/dev/null; done
E=$(python3 -c 'import time; print(time.time())')
MS=$(python3 -c "print(round(($E-$S)*1000/200,2))")
echo "C2 getAccount mean latency: ${MS} ms/call (200 calls, localhost)"

if [ -z "$EVENTKEYS" ]; then
  echo "VERDICT S-C: NO-GO -- v0.2.4 getTransaction/getBlock carry no event/log/receipt field (keys: $(sed -n 's/^ALLKEYS=//p' <<<"$KEYS")); observation layer = PDA polling (${MS} ms/getAccount local); switch to getEvents when a tag ships PR 705/707"
else
  echo "VERDICT S-C: GO -- event-ish keys present: $EVENTKEYS"
fi
