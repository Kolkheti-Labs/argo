# Argo

Isolated-market lending for the Logos Execution Zone (LEZ). A Morpho Blue
equivalent: one immutable program, permissionless isolated markets keyed by
`(loan token, collateral token, oracle, IRM, LLTV)`, lazy AdaptiveCurveIRM
accrual, permissionless liquidation, zero-fee flash loans over LEZ chained
calls.

Built by Kolkheti Labs under Logos RFP-008
(https://github.com/logos-co/rfp/issues/141). Status: **M1, core market
primitives: done on branch `m1`**, see
`docs/m1/README.md` (tracker: https://github.com/logos-co/rfp/issues/171;
M0: https://github.com/logos-co/rfp/issues/170, code merged to `main` in PR #1). Nothing here is
deployed and nothing custodies value. The M1 build pins a public test admin key
(`ADMIN_GENESIS`, `docs/m1/decisions.md` D14) and must not be deployed to a
shared network as built.

## Pinned dependencies

| Dependency | Pin | Where |
| --- | --- | --- |
| Logos Execution Zone | tag `v0.2.4` (commit 47eba256) | `Cargo.toml` workspace deps, both guest `Cargo.toml` |
| SPEL framework | rev `1ef0500f8fc8ce3ddf95523726ae159db83f744f` (the revision logos-blockchain/lez-programs builds against) | same |
| risc0 (`risc0-zkvm`, `risc0-build`, r0vm, cargo-risczero) | `=3.0.5` | `Cargo.toml`, guest manifests, `.github/docker/ci.Dockerfile` |
| Rust | `1.94.0` (`rust-toolchain.toml`); risc0 guest toolchain `1.94.1` via rzup | `rust-toolchain.toml`, CI image |
| Token program (M1) | logos-blockchain/lez-programs tag `v2.0.1`, image id `14b3bc6c…` (testnet-legacy) | `programs/argo_lending/core/src/external.rs` (`TOKEN_PROGRAM_ID`), `vendor/lez-programs-v2.0.1/` |
| Vendored files | `tools/idl-gen/src/main.rs`, `vendor/lez-programs-v2.0.1/*.bin`, types in `external.rs` | `NOTICE` |

All three Cargo workspaces (root, `programs/argo_lending/methods/guest`,
`spikes/vault/methods/guest`) commit their `Cargo.lock`; CI builds with
`--locked` and `RISC0_BUILD_LOCKED=1`, so guest image ids are a function of
the committed tree.

## Layout

```
argo_core/                     no_std shared crate: constants, widening mul_div, virtual-share math
irm_core/                      no_std AdaptiveCurveIRM constants and utilisation (rate function lands in M2)
programs/argo_lending/         the lending program: core/ (types, ids, PDA seeds, copied external types), src/ (logic), methods/ + methods/guest/ (RISC0 guest)
programs/argo_lending/integration_tests/  M1 tests: the guest plus the pinned v2.0.1 token in the in-process LEZ state machine
vendor/lez-programs-v2.0.1/    the pinned lez-programs token and twap_oracle binaries (provenance in its README)
spikes/vault/                  the M0 spike program (vault custody, chained calls, stress math), same layout
spikes/integration_tests/      S-A, S-B, S-D tests against the in-process LEZ state machine and the risc0 executor
spikes/S-*/run.sh              one runnable per spike; prints a VERDICT line; non-zero exit if the experiment did not run
harness/                       standalone-sequencer harness: localnet.sh, lib.sh (JSON-RPC probes), runner/ (deploy, initialize, read)
tools/idl-gen/                 SPEL IDL generator (vendored); idl/ holds the generated IDLs, checked in CI
evidence/                      raw JSON-RPC captures and spike run outputs referenced by docs/m0/verdicts.md
docs/m0/                       acceptance gate, spike plan and per-spike docs, verdicts, verification log, milestone report
docs/spec/                     account-sharding + state-layout spec (reviewed in M0, tag m0-r1)
docs/m1/                       M1 summary (README), decisions that differ from the spec, and the requirement -> test table
```

Later milestones add `app/core`, `app/cli`, `app/gui`, `services/liquidator`,
`services/risk_monitor`, and the standalone `argo_irm` program (M2; RFP-008 F8).
M1 decisions that differ from the M0 spec are in `docs/m1/decisions.md`.

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
cargo test --locked -p argo_lending_core -p argo_lending_program   # types, ids, forged-input guards
RISC0_DEV_MODE=1 cargo test --locked --release -p argo_integration_tests   # M1 operations, in-process
./harness/localnet.sh smoke                           # boot a local sequencer, deploy, initialize, read back
./harness/localnet.sh e2e                             # M1 done gate: all six operations on a local sequencer
./spikes/run.sh all                                   # all five spikes, each printing a VERDICT line
./harness/gen-idl.sh                                  # regenerate idl/ (CI fails if it drifts)
./harness/clean-host.sh <repo-url-or-bundle> <empty-dir> <commit>   # the same from empty toolchain homes
```

Apart from the read-only `harness/probe-testnet-token.sh`, which CI does not
run, the harness never touches the public testnet. It honours `CARGO_TARGET_DIR`,
`ARGO_PORT` (default 3048) and `RISC0_DEV_MODE` (default 1).

## Licence

Dual-licensed under MIT and Apache-2.0, at your option. See `LICENSE-MIT`,
`LICENSE-APACHE`, and `NOTICE`.
