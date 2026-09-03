# Argo

Isolated-market lending for the Logos Execution Zone (LEZ). A Morpho Blue
equivalent: one immutable program, permissionless isolated markets keyed by
`(loan token, collateral token, oracle, IRM, LLTV)`, lazy AdaptiveCurveIRM
accrual, permissionless liquidation, zero-fee flash loans over LEZ chained
calls.

Built by Kolkheti Labs under Logos RFP-008
(https://github.com/logos-co/rfp/issues/141). Status: **M0, foundations and
de-risking spikes** (tracker: https://github.com/logos-co/rfp/issues/170).
Nothing here is deployed and nothing custodies value.

## Pinned dependencies

| Dependency | Pin | Where |
| --- | --- | --- |
| Logos Execution Zone | tag `v0.2.4` (commit 47eba256) | `Cargo.toml` workspace deps, both guest `Cargo.toml` |
| SPEL framework | rev `1ef0500f8fc8ce3ddf95523726ae159db83f744f` (the revision logos-blockchain/lez-programs builds against) | same |
| risc0 (`risc0-zkvm`, `risc0-build`, r0vm, cargo-risczero) | `=3.0.5` | `Cargo.toml`, guest manifests, `.github/docker/ci.Dockerfile` |
| Rust | `1.94.0` (`rust-toolchain.toml`); risc0 guest toolchain `1.94.1` via rzup | `rust-toolchain.toml`, CI image |
| Vendored file | `tools/idl-gen/src/main.rs` from logos-blockchain/lez-programs | `NOTICE` |

All three Cargo workspaces (root, `programs/argo_lending/methods/guest`,
`spikes/vault/methods/guest`) commit their `Cargo.lock`; CI builds with
`--locked` and `RISC0_BUILD_LOCKED=1`, so guest image ids are a function of
the committed tree.

## Layout

```
argo_core/                     no_std shared crate: constants, widening mul_div, virtual-share math
irm_core/                      no_std AdaptiveCurveIRM constants and utilisation (rate function lands in M2)
programs/argo_lending/         the lending program: core/ (types, PDA seeds), src/ (logic), methods/ + methods/guest/ (RISC0 guest)
spikes/vault/                  the M0 spike program (vault custody, chained calls, stress math), same layout
spikes/integration_tests/      S-A, S-B, S-D tests against the in-process LEZ state machine and the risc0 executor
spikes/S-*/run.sh              one runnable per spike; prints a VERDICT line; non-zero exit if the experiment did not run
harness/                       standalone-sequencer harness: localnet.sh, lib.sh (JSON-RPC probes), runner/ (deploy, initialize, read)
tools/idl-gen/                 SPEL IDL generator (vendored); idl/ holds the generated IDLs, checked in CI
evidence/                      raw JSON-RPC captures and spike run outputs referenced by docs/m0/verdicts.md
docs/m0/                       acceptance gate, spike plan and per-spike docs, verdicts, verification log, milestone report
docs/spec/                     account-sharding + state-layout spec (draft for review)
```

Later milestones add `app/core`, `app/cli`, `app/gui`, `services/liquidator`,
`services/risk_monitor`, and the standalone `argo_irm` program.

## Prerequisites

Debian/Ubuntu packages:

```
build-essential clang libclang-dev libssl-dev pkg-config libpcsclite-dev curl git jq python3
```

`libclang-dev` is needed by the RocksDB bindings the sequencer pulls in.
Rust from `rust-toolchain.toml`, plus `rzup install rust 1.94.1`, `rzup install
cpp 2024.1.5`, `rzup install r0vm 3.0.5`, `rzup install cargo-risczero 3.0.5`.
A full build (sequencer, wallet, guests) needs about 15 GB of disk and took
27 minutes on 8 vCPU from an empty cache; 4 GB of RAM is enough at
`RISC0_DEV_MODE=1` (no proving).

## Running

```
cargo test --locked -p argo_core -p irm_core          # pure math
cargo test --locked -p spike_integration_tests        # S-A, S-B, S-D (in-process LEZ state machine)
./harness/localnet.sh smoke                           # boot a local sequencer, deploy, initialize, read back
./spikes/run.sh all                                   # all five spikes, each printing a VERDICT line
./harness/gen-idl.sh                                  # regenerate idl/ (CI fails if it drifts)
./harness/clean-host.sh <repo-url-or-bundle> <empty-dir> <commit>   # the same from empty toolchain homes
```

The harness never touches the public testnet. It honours `CARGO_TARGET_DIR`,
`ARGO_PORT` (default 3048) and `RISC0_DEV_MODE` (default 1).

## Licence

Dual-licensed under MIT and Apache-2.0, at your option. See `LICENSE-MIT`,
`LICENSE-APACHE`, and `NOTICE`.
