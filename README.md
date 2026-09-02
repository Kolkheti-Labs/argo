# Argo

Isolated-market lending for the Logos Execution Zone (LEZ). A Morpho Blue
equivalent: one immutable program, permissionless isolated markets keyed by
`(loan token, collateral token, oracle, IRM, LLTV)`, lazy AdaptiveCurveIRM
accrual, permissionless liquidation, zero-fee flash loans over LEZ chained
calls.

Built by Kolkheti Labs under Logos RFP-008
(https://github.com/logos-co/rfp/issues/141). Status: **M0, foundations and
de-risking spikes.** Nothing here is deployed and nothing custodies value.

## Layout

```
argo_core/       no_std shared crate: Instruction enum, account layouts, PDA seeds, pure math
irm_core/        no_std AdaptiveCurveIRM math
methods/guest/   RISC0 guest binaries (argo_lending, argo_irm)
harness/         standalone-sequencer test harness + evidence capture
spikes/          M0 go/no-go experiments (one runnable per spike)
docs/m0/         M0 plan, spike plan, verdicts, acceptance gate, verification log
docs/spec/       account-sharding + state-layout spec
tasks/           working task list
```

Later milestones add `app/core`, `app/cli`, `app/gui`, `services/liquidator`,
`services/risk_monitor` as described in the proposal.

## Toolchain

See `rust-toolchain.toml` and `.github/docker/ci.Dockerfile` for the exact
pinned versions. System packages needed on Debian/Ubuntu:

```
build-essential clang libclang-dev libssl-dev pkg-config libpcsclite-dev curl git jq python3
```

`libclang-dev` is required by the RocksDB bindings pulled in by the sequencer;
without it a workspace build fails late. A full workspace build needs roughly
15 GB of disk.

## Running the harness

```
./harness/localnet.sh smoke
```

Boots a throwaway standalone sequencer on a scratch directory, initialises a
wallet, deploys the placeholder program and reads it back over JSON-RPC.
Honours `CARGO_TARGET_DIR` and `RISC0_DEV_MODE` (default `1`).

## Licence

Dual-licensed under MIT and Apache-2.0, at your option. See `LICENSE-MIT`,
`LICENSE-APACHE`, and `NOTICE`.
