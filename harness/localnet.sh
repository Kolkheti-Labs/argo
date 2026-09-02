#!/usr/bin/env bash
# Argo standalone-sequencer harness.
#
#   ./harness/localnet.sh build   build sequencer + wallet from LEZ v0.2.4 (once)
#   ./harness/localnet.sh up      boot a throwaway sequencer on a scratch dir
#   ./harness/localnet.sh down    stop it
#   ./harness/localnet.sh smoke   build + up + deploy argo_lending + initialize Config
#                                 + read it back over JSON-RPC, then down. Exit 0 = green.
#
# Never touches testnet.lez.logos.co. Honours CARGO_TARGET_DIR and RISC0_DEV_MODE
# (default 1). Assumes a minimal container: curl + python3 only, no ss/pkill.
set -uo pipefail

R="${ARGO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
# shellcheck source=lib.sh
source "$R/harness/lib.sh"

D="$R/.localnet-argo"
export ARGO_PORT="${ARGO_PORT:-3048}"
export ARGO_RPC="http://127.0.0.1:$ARGO_PORT"
TARGET="${CARGO_TARGET_DIR:-$R/target}"
BIN="$TARGET/lez-bin"                 # cargo install --root for the LEZ binaries
export RISC0_DEV_MODE="${RISC0_DEV_MODE:-1}"
export LEE_WALLET_HOME_DIR="$D/wallet"
LEZ_GIT="https://github.com/logos-blockchain/logos-execution-zone.git"
LEZ_TAG="v0.2.4"

cd "$R" || exit 1

cmd_build() {
  echo "=== build LEZ $LEZ_TAG standalone sequencer + wallet (cargo install, cached in $BIN) ==="
  mkdir -p "$BIN"
  # `--features standalone` swaps in the mock Bedrock/Indexer clients; without
  # it the binary boots and then dies trying to reach those services.
  [ -x "$BIN/bin/sequencer_service" ] || cargo install --locked --git "$LEZ_GIT" --tag "$LEZ_TAG" \
      --root "$BIN" --target-dir "$TARGET" --features standalone sequencer_service \
      || die "sequencer_service install failed"
  [ -x "$BIN/bin/wallet" ] || cargo install --locked --git "$LEZ_GIT" --tag "$LEZ_TAG" \
      --root "$BIN" --target-dir "$TARGET" wallet \
      || die "wallet install failed"
  echo "=== build Argo guests (embedded via risc0_build) + harness runner ==="
  cargo build --release -p argo_lending_methods -p spike_vault_methods -p harness_runner \
      || die "argo build failed"
}

write_configs() {
  rm -rf "$D"; mkdir -p "$D/wallet"
  # The debug sequencer config ships inside the LEZ source; cargo keeps a
  # checkout under the registry's git cache, which we locate by tag.
  local src
  src=$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -path '*logos-execution-zone*' \
          -name sequencer_config.json -path '*configs/debug*' 2>/dev/null | head -1)
  [ -n "$src" ] || die "could not find LEZ debug sequencer_config.json in the cargo git cache"
  python3 - "$src" "$D/sequencer_config.json" "$D" <<'PY' || exit 1
import json, sys
src, dst, home = sys.argv[1], sys.argv[2], sys.argv[3]
c = json.load(open(src))
c["home"] = home
c["block_create_timeout"] = "1s"
json.dump(c, open(dst, "w"), indent=2)
PY
  cat > "$D/wallet/wallet_config.json" <<JSON
{
  "sequencers": [ { "sequencer_addr": "$ARGO_RPC" } ],
  "seq_poll_timeout": "30s",
  "seq_tx_poll_max_blocks": 25,
  "seq_poll_max_retries": 25,
  "seq_block_poll_max_amount": 300,
  "multi_sequencer_client_config": { "distribution_limit": 1, "calibration_limit": 100 }
}
JSON
}

cmd_up() {
  # A sequencer left over from an earlier run would keep the port and its
  # stale state; stop it first (pid file, then by command line).
  [ -f "$D/seq.pid" ] && kill "$(cat "$D/seq.pid")" 2>/dev/null || true
  command -v pkill >/dev/null && pkill -f "sequencer_service .*--port $ARGO_PORT" 2>/dev/null || true
  sleep 1
  write_configs
  echo "=== boot standalone sequencer :$ARGO_PORT (RISC0_DEV_MODE=$RISC0_DEV_MODE) ==="
  RUST_LOG=info nohup "$BIN/bin/sequencer_service" "$D/sequencer_config.json" --port "$ARGO_PORT" \
    > "$D/seq.log" 2>&1 &
  echo $! > "$D/seq.pid"
  wait_until 60 "sequencer RPC on :$ARGO_PORT" seq_up || { tail -30 "$D/seq.log" >&2; exit 1; }
  echo "sequencer up, pid $(cat "$D/seq.pid"), tip block $(rpc_tip)"
  echo "=== init wallet storage ==="
  printf 'argo\n' | RUST_LOG=error "$BIN/bin/wallet" account list >/dev/null 2>&1 || true
  [ -f "$D/wallet/storage.json" ] || die "wallet storage was not initialised"
}

cmd_down() {
  # The throwaway sequencer mints 1s blocks forever and its RocksDB grows
  # unbounded, so it must not outlive the run.
  if [ -f "$D/seq.pid" ]; then kill "$(cat "$D/seq.pid")" 2>/dev/null || true; rm -f "$D/seq.pid"; fi
}

cmd_smoke() {
  trap cmd_down EXIT
  cmd_build
  cmd_up
  local RUN="$TARGET/release"
  echo "=== deploy argo_lending ==="
  DEPLOY_OUT=$("$RUN/argo_deploy" argo_lending) || die "deploy failed"
  echo "$DEPLOY_OUT"
  PROGRAM_ID=$(printf '%s\n' "$DEPLOY_OUT" | sed -n 's/^PROGRAM_ID=//p' | head -1)
  DEPLOY_TX=$(printf '%s\n' "$DEPLOY_OUT" | sed -n 's/^TX=//p' | head -1)
  [ -n "$PROGRAM_ID" ] && [ -n "$DEPLOY_TX" ] || die "could not parse deploy output"
  wait_until 60 "deploy tx $DEPLOY_TX in a block" rpc_tx_landed "$DEPLOY_TX"
  echo "=== initialize Config ==="
  INIT_OUT=$("$RUN/argo_initialize" "$PROGRAM_ID") || die "initialize failed"
  echo "$INIT_OUT"
  CONFIG_ID=$(printf '%s\n' "$INIT_OUT" | sed -n 's/^CONFIG_ID=//p' | head -1)
  wait_until 60 "Config PDA $CONFIG_ID initialised" rpc_account_initialized "$CONFIG_ID"
  echo "=== read back over JSON-RPC ==="
  rpc_account "$CONFIG_ID" | python3 -c '
import json,sys
r=json.load(sys.stdin)["result"]
assert any(r["program_owner"]), "config not owned"
print("config program_owner =", r["program_owner"])' || die "read-back failed"
  echo "=== ARGO LOCALNET SMOKE GREEN (program $PROGRAM_ID) ==="
}

case "${1:-}" in
  build) cmd_build ;;
  up) cmd_up ;;
  down) cmd_down ;;
  smoke) cmd_smoke ;;
  *) echo "usage: $0 {build|up|down|smoke}" >&2; exit 2 ;;
esac
