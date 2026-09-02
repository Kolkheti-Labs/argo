# M0 milestone report — Argo (RFP-008)

Status: DRAFT. Submitted as https://github.com/Kolkheti-Labs/argo/pull/1
(milestone PR, the review surface Logos uses); tracker
https://github.com/logos-co/rfp/issues/170.

## Deliverables

| # | Deliverable | Where | Verified by |
| --- | --- | --- | --- |
| D1 | Repo, MIT + Apache-2.0, CI | `LICENSE-*`, `NOTICE`, `.github/workflows/` | green: https://github.com/Kolkheti-Labs/argo/actions/runs/33674571127 |
| D2 | SPEL scaffolding | `argo_core/`, `irm_core/`, `programs/argo_lending/`, `spikes/vault/` | guests build, `docs/m0/verification-log.md` |
| D3 | Standalone-sequencer harness | `harness/` | smoke green on dev box and from a fresh toolchain (verification log) |
| D4 | ring/riscv32 build | `docs/m0/riscv32-build.md` | guest ELFs built from a fresh toolchain home (verification log) |
| D5 | Sharding + state-layout spec | `docs/spec/state-layout.md` | review by rekt, adp, then with Logos on the tracker: _pending_ |
| D6 | Go/no-go spikes | `docs/m0/verdicts.md`, `spikes/` | S-A GO, S-B GO, S-C NO-GO (polling), S-D GO, S-E GO with init-step change |

## What the spikes changed in the M1–M3 design

1. **Settlement shape confirmed.** State write + chained token legs + optional
   internal self-call is atomic on v0.2.4; a failing last leg reverts
   everything. `repay_flash` is an internal entrypoint gated on
   `caller_program_id`. Chains are capped at 10 executions.
2. **Custody confirmed on the builtin token.** Vaults are token-owned PDAs
   claimed under Argo's seed; only Argo debits them. `create_market` must
   claim both vaults before any deposit. LP-0013's authority token is not on
   the testnet; Argo targets the builtin behind a one-module adapter.
3. **No events.** The liquidator and risk monitor poll PDAs via a block-tail
   follower; the observation layer is a trait so `getEvents` slots in when a
   LEZ tag ships it.
4. **Compute is not the constraint, account size is.** A liquidate-shaped
   chain is ~0.6M cycles against a 32M per-leg budget, but a 50 KB account
   costs 23.7M cycles. Every Argo account stays under 1 KB and `Config`'s
   enabled sets are fixed-size. Widening division is ~2.5k cycles a call, so
   M1 uses a cheaper 256/128 divide.
5. **Private path needs an init step.** A deshield into a never-initialised
   public account is rejected; the SDK initialises the ephemeral account with
   its own key first (no fees exist yet, so this is free but visible).
6. **No clock on the testnet.** Accrual's time source is an M1 decision
   (clock program if Logos deploys it, else a windowed timestamp argument).

## Evidence

- `docs/m0/verification-log.md` — hosts, commands, outcomes, executor errors.
- `evidence/testnet/` — raw `getProgramIds` capture from the public testnet.
- `evidence/localnet/` — raw RPC payloads from the standalone sequencer runs.

## Open items handed to M1

- Token program choice sign-off (builtin vs authority token).
- Clock source for accrual.
- `Authorization` as its own PDA vs inside `Position`.
