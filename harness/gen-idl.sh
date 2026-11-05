#!/usr/bin/env bash
# Regenerate the SPEL IDL JSON for every guest into idl/. CI fails if the
# committed files differ from the regenerated ones.
set -euo pipefail
R="$(cd "$(dirname "$0")/.." && pwd)"; cd "$R"
cargo build --release -p idl-gen >/dev/null
GEN="${CARGO_TARGET_DIR:-$R/target}/release/idl-gen"
"$GEN" programs/argo_lending/methods/guest/src/bin/argo_lending.rs > idl/argo_lending.json
"$GEN" spikes/vault/methods/guest/src/bin/spike_vault.rs > idl/spike_vault.json
ls -la idl/
