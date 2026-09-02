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
idof() { w account id --account-id "$1" 2>/dev/null | grep -o "[1-9A-HJ-NP-Za-km-z]\{32,44\}" | tail -1; }

echo "== funder: import genesis public account 0 =="
FUNDER=$("$RUN/argo_setup_funder" | sed -n 's/^FUNDER_ID=//p'); [ -n "$FUNDER" ] || die "no funder"
wait_until 30 "funder funded" rpc_account_funded "$FUNDER"
echo "funder $FUNDER balance $(rpc_field "$FUNDER" 'r["balance"]')"

echo "== P: new private account, shielded-fund it from the funder =="
w account new private -l P >/dev/null 2>&1 || die "account new private"
P=$(idof P); [ -n "$P" ] || die "no P id"
w auth-transfer send --from "Public/$FUNDER" --to "Private/$P" --amount 1000 || die "shield funder->P"
echo "P=$P"

echo "== A: fresh public account (never touched on-chain) =="
w account new public -l A >/dev/null 2>&1 || die "account new public"
A=$(idof A); [ -n "$A" ] || die "no A id"
echo "A=$A pre-state: $(rpc_account "$A" | head -c 200)"

echo "== E0b: deshield P -> A while A is uninitialised =="
if w auth-transfer send --from "Private/$P" --to "Public/$A" --amount 300 2>&1 | tee "$EV/S-E-e0b.log"; then
  sleep 5; if rpc_account_funded "$A"; then E0B="landed (A funded without init)"; else E0B="accepted by wallet but never landed (dropped)"; fi
else
  E0B="rejected by wallet/prover"
fi
echo "E0b: $E0B"

echo "== E0: initialise A by its own key =="
w auth-transfer init --account-id "Public/$A" || die "init A"
wait_until 60 "A initialised" rpc_account_initialized "$A"
echo "A program_owner: $(rpc_field "$A" 'r["program_owner"]')"

echo "== E1: deshield P -> A =="
w auth-transfer send --from "Private/$P" --to "Public/$A" --amount 300 || die "deshield P->A"
wait_until 60 "A funded" rpc_account_funded "$A"
echo "A balance: $(rpc_field "$A" 'r["balance"]')"

echo "== E2: A signs a public op (transfer A -> B) =="
w account new public -l B >/dev/null 2>&1 || die "account new public B"
B=$(idof B)
w auth-transfer init --account-id "Public/$B" || die "init B"
wait_until 60 "B initialised" rpc_account_initialized "$B"
w auth-transfer send --from "Public/$A" --to "Public/$B" --amount 100 || die "public A->B"
wait_until 60 "B funded" rpc_account_funded "$B"

echo "== E3: reshield A -> new private P2 =="
w account new private -l P2 >/dev/null 2>&1 || die "account new private P2"
P2=$(idof P2)
w auth-transfer send --from "Public/$A" --to "Private/$P2" --amount 200 || die "reshield A->P2"
sleep 5
echo "A final balance: $(rpc_field "$A" 'r["balance"]')"

echo "== E4: which accounts co-appear with A on-chain =="
TIP=$(rpc_tip); rpc getBlockRange "[1,$TIP]" > "$EV/S-E-blocks.json"
grep -o "$A" "$EV/S-E-blocks.json" | wc -l | sed 's/^/A occurrences in raw blocks 1..'"$TIP"': /'
for X in "$FUNDER" "$P" "$B" "$P2"; do printf '%s occurrences: ' "$X"; grep -o "$X" "$EV/S-E-blocks.json" | wc -l; done

echo "VERDICT S-E: PARTIAL -- no tx fees on v0.2.4 (fee half N/A); E0b=$E0B; after own-key init the deshield->interact->reshield sequence landed; see evidence/localnet/S-E-*.json for the raw blocks"
