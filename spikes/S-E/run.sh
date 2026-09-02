#!/usr/bin/env bash
# S-E: fee-funding-within-deshield on v0.2.4 (no fees exist; the live question
# is the ephemeral-account init rule and linkability). Wallet-driven:
#   E0b deshield Private P -> fresh Public A (never initialised)   expect: dropped
#   E0  A initialised by its own key (auth-transfer init)           expect: lands
#   E1  deshield P -> A                                             expect: lands
#   E2  A acts as a public signer (auth-transfer send A -> B)       expect: lands
#   E3  reshield A -> Private P2                                    expect: lands, A at 0
#   E4  accounts co-appearing with A in its blocks                  expect: only B/protocol
set -uo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
source "$R/harness/lib.sh"
export ARGO_PORT="${ARGO_PORT:-3048}"; export ARGO_RPC="http://127.0.0.1:$ARGO_PORT"
TARGET="${CARGO_TARGET_DIR:-$R/target}"; RUN="$TARGET/release"
W="$TARGET/lez-bin/bin/wallet"
export LEE_WALLET_HOME_DIR="$R/.localnet-argo/wallet"
EV="$R/evidence/localnet"; mkdir -p "$EV"
trap '"$R/harness/localnet.sh" down' EXIT
"$R/harness/localnet.sh" up >/dev/null || die "localnet up failed"
w() { RUST_LOG=error "$W" "$@"; }
# Run a wallet command, keep its full output as evidence, and snapshot the raw
# getTransaction payload of every tx hash it printed.
wev() {
  local tag=$1; shift
  local out; out=$("$W" "$@" 2>&1); local rc=$?
  printf '%s\n' "$out" > "$EV/S-E-$tag.wallet.txt"
  printf '%s\n' "$out" | sed -n 's/^Transaction hash is //p' | while read -r h; do
    wait_until 60 "tx $h in a block" rpc_tx_landed "$h" || true
    rpc_transaction "$h" > "$EV/S-E-$tag-$h.getTransaction.json"
    echo "  $tag tx $h -> $(python3 -c 'import json,sys; r=json.load(open(sys.argv[1]))["result"]; print("block", r[1] if r else "NOT LANDED")' "$EV/S-E-$tag-$h.getTransaction.json")"
  done
  return $rc
}
idof() { w account id --account-id "$1" 2>/dev/null | grep -o "[1-9A-HJ-NP-Za-km-z]\{32,44\}" | tail -1; }

echo "== funder: import genesis public account 0 =="
FUNDER=$("$RUN/argo_setup_funder" | sed -n 's/^FUNDER_ID=//p'); [ -n "$FUNDER" ] || die "no funder"
wait_until 30 "funder funded" rpc_account_funded "$FUNDER"
echo "funder $FUNDER balance $(rpc_field "$FUNDER" 'r["balance"]')"

echo "== P: new private account, shielded-fund it from the funder =="
w account new private -l P >/dev/null 2>&1 || die "account new private"
P=$(idof P); [ -n "$P" ] || die "no P id"
wev p-shield auth-transfer send --from "Public/$FUNDER" --to "Private/$P" --amount 1000 || die "shield funder->P"
echo "P=$P"

echo "== A: fresh public account (never touched on-chain) =="
w account new public -l A >/dev/null 2>&1 || die "account new public"
A=$(idof A); [ -n "$A" ] || die "no A id"
echo "A=$A pre-state: $(rpc_account "$A" | head -c 200)"

echo "== E0b: deshield P -> A while A is uninitialised =="
E0B_OK=0
if wev e0b auth-transfer send --from "Private/$P" --to "Public/$A" --amount 300; then
  sleep 5; if rpc_account_funded "$A"; then E0B="landed (A funded without init)"; else E0B="accepted by wallet but never landed (dropped)"; E0B_OK=1; fi
else
  E0B="rejected by wallet/prover"; grep -q "Cannot claim unauthorized account" "$EV/S-E-e0b.wallet.txt" && E0B_OK=1
fi
echo "E0b: $E0B"

echo "== E0: initialise A by its own key =="
wev e0-init-A auth-transfer init --account-id "Public/$A" || die "init A"
wait_until 60 "A initialised" rpc_account_initialized "$A"
echo "A program_owner: $(rpc_field "$A" 'r["program_owner"]')"

echo "== E1: deshield P -> A =="
wev e1-deshield auth-transfer send --from "Private/$P" --to "Public/$A" --amount 300 || die "deshield P->A"
wait_until 60 "A funded" rpc_account_funded "$A" || die "E1 deshield never landed"
A_AFTER_E1=$(rpc_field "$A" 'r["balance"]'); echo "A balance: $A_AFTER_E1"

echo "== E2: A signs a public op (transfer A -> B) =="
w account new public -l B >/dev/null 2>&1 || die "account new public B"
B=$(idof B)
wev e2-init-B auth-transfer init --account-id "Public/$B" || die "init B"
wait_until 60 "B initialised" rpc_account_initialized "$B"
wev e2-public-A-to-B auth-transfer send --from "Public/$A" --to "Public/$B" --amount 100 || die "public A->B"
wait_until 60 "B funded" rpc_account_funded "$B" || die "E2 public transfer never landed"

echo "== E3: reshield A -> new private P2 =="
w account new private -l P2 >/dev/null 2>&1 || die "account new private P2"
P2=$(idof P2)
wev e3-reshield auth-transfer send --from "Public/$A" --to "Private/$P2" --amount 200 || die "reshield A->P2"
a_drained() { [ "$(rpc_field "$A" 'r["balance"]')" = "0" ]; }
wait_until 60 "A drained by reshield" a_drained || die "E3 reshield never landed"
A_FINAL=$(rpc_field "$A" 'r["balance"]'); echo "A final balance: $A_FINAL"

echo "== E4: raw blocks saved; linkability is NOT evaluated in M0 (needs a borsh decoder) =="
TIP=$(rpc_tip); rpc getBlockRange "[1,$TIP]" > "$EV/S-E-blocks.json"

cat > "$EV/S-E-ids.txt" <<IDS
FUNDER=$FUNDER
P=$P
A=$A
B=$B
P2=$P2
IDS
if [ "$E0B_OK" = 1 ] && [ "$A_AFTER_E1" = 300 ] && [ "$A_FINAL" = 0 ]; then
  echo "VERDICT S-E: PARTIAL -- fee half not applicable (v0.2.4 has no tx fees); deshield into an uninitialised public account is $E0B; after an own-key init, deshield (A=300) -> public op signed by A -> reshield (A=0) all landed (wallet output + raw getTransaction per step in evidence/localnet/S-E-*); E4 linkability NOT evaluated in M0"
  exit 0
else
  echo "VERDICT S-E: NO-GO -- observation contradicts the expected sequence (E0b_ok=$E0B_OK, A after E1=$A_AFTER_E1, A final=$A_FINAL)"
  exit 1
fi
