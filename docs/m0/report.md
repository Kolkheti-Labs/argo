# M0 milestone report — Argo (RFP-008)

Status: DRAFT. Submitted as https://github.com/Kolkheti-Labs/argo/pull/1
(milestone PR, the review surface Logos uses); tracker
https://github.com/logos-co/rfp/issues/170.

## Deliverables

| # | Deliverable | Where | Verified by |
| --- | --- | --- | --- |
| D1 | Repo, MIT + Apache-2.0, CI | `LICENSE-*`, `NOTICE`, `.github/workflows/` | green at the submitted head `566fb61` (tag `m0-submit`): https://github.com/Kolkheti-Labs/argo/actions/runs/33729403009 |
| D2 | SPEL scaffolding | `argo_core/`, `irm_core/`, `programs/argo_lending/`, `spikes/vault/` | guests build, `docs/m0/verification-log.md` |
| D3 | Standalone-sequencer harness | `harness/` | smoke green on dev box and from a fresh toolchain (verification log) |
| D4 | ring/riscv32 build | `docs/m0/riscv32-build.md` | guest ELFs built from a fresh toolchain home (verification log) |
| D5 | Sharding + state-layout spec | `docs/spec/state-layout.md` | review by rekt, adp, then with Logos on the tracker: _pending_ |
| D6 | Go/no-go spikes | `docs/m0/verdicts.md`, `spikes/`, `evidence/` | S-A GO, S-B GO, S-C NO-GO (polling), S-D GO, S-E PARTIAL (init step established for the stock v0.2.4 wallet deshield; linkability not evaluated) |

## What the spikes changed in the M1–M3 design

1. **Settlement shape confirmed.** State write + chained token legs + optional
   internal self-call is atomic on v0.2.4; a failing last leg reverts
   everything. `repay_flash` is an internal entrypoint gated on
   `caller_program_id`. A transaction may run 11 executions (top level + 10
   chained calls).
2. **Custody confirmed on the v0.2.4 builtin token.** Vaults are token-owned
   PDAs claimed under Argo's seed; only Argo debits them. `create_market` must
   claim both vaults before any deposit. M0 did not establish which token
   program the public testnet runs (S-B B5 was withdrawn); Argo targets the
   v0.2.4 builtin behind a one-module adapter that M1 adds.
3. **No events on v0.2.4.** The liquidator and risk monitor poll PDAs via a
   block-tail follower; the observation layer is a trait so `getEvents` slots
   in on a LEZ version that has it (v0.2.5-rc1 and v0.3.0 do, v0.2.4 does
   not).
4. **Compute is not the constraint, account size is.** The heaviest measured
   leg is 271k cycles and a liquidation extrapolates to ~0.6M against a 32M
   per-leg budget, but carrying a 50 KB account through one execution costs
   23.7M cycles. Every Argo account stays under 1 KB and `Config`'s enabled
   sets will be fixed-size. Arithmetic costs ~5.3k cycles per
   accrual/health/LIF-shaped iteration with the bitwise 256/128 division, so
   M1 uses a cheaper widening divide.
5. **The stock wallet's private path needs an init step on v0.2.4.** The
   stock v0.2.4 wallet deshield into a never-initialised public account is
   rejected, so the SDK initialises the ephemeral account with its own key
   first (v0.2.4 has no fees, so this is free but visible). A deshield that
   the ephemeral account co-signs may avoid that step; untested (S-E, E5
   notes). Linkability of the ephemeral account in public block data was not
   evaluated in M0 (needs a block decoder, M5).
6. **Clock.** In the v0.2.4 source, and on our v0.2.4 localnet, the sequencer
   updates the clock program's `CLOCK_01` account every block (not yet read
   on a public testnet). The earlier "no clock on the testnet" finding rested
   on the hardcoded `getProgramIds` reply and is withdrawn. The spec proposes
   reading `CLOCK_01` for accrual (`docs/spec/state-layout.md` §5).

## Evidence

- `docs/m0/verification-log.md` — hosts, commands, outcomes, executor errors.
- `evidence/testnet/` — raw `getProgramIds` reply from the public testnet
  (2026-09-02). On v0.2.4 that RPC returns builtin ids hardcoded in the
  sequencer, so it does not show the chain's programs; kept for the record
  (see its README).
- `evidence/localnet/` — raw RPC payloads from the standalone sequencer runs.

## Open items handed to M1

- Token program on the relaunched 0.2 testnet and on 0.4, to confirm with
  Logos.
- Clock: rekt's review of spec §5, and a live check of `CLOCK_01` on the
  relaunched 0.2 testnet.
- `Authorization` as its own PDA vs inside `Position`; protocol fee placement
  (spec §7).
- S-E re-run (E0b, E1–E3, E5) on the LEZ version Argo ships on.

## Review response (2026-10-16)

Answers to Logos's first M0 review (on Discord, 2026-10-10) and its guidance
of 2026-10-12 (Discord).

1. **Spec review.** Not yet. The spec was written by jeefxM, and no review by
   rekt or adp is recorded yet. They review it next on PR #1, then the review
   with Logos follows, so D5 stays open.
2. **Clock source, LEZ version, token program.**
   - Version: M1 builds on LEZ v0.2.4 and the deliverable targets 0.4, as
     Logos asked on 2026-10-12. No 0.3 port (spec, "Target runtime").
   - Clock: read `CLOCK_01`, proposed for rekt's review (spec §5). The
     "no clock on the testnet" finding is withdrawn (S-B, "B5 withdrawn").
   - Token: the v0.2.4 builtin behind one adapter (spec §2, "Token
     program"). Which token program the relaunched testnet and 0.4 run is
     open.
3. **S-E E5 notes.** Added to `docs/m0/spikes/S-E-fee-within-deshield.md`.
4. **Re-run on 0.3.** Replaced by the 2026-10-12 guidance, so not done. After
   removing b5, all five spikes were re-run on v0.2.4 on hetzner2 at
   `134d353` (verification log, 2026-10-17).
5. **0.4 adaptation.** Not started, since 0.4 is not published. Spec §7 item 7
   lists what we re-check when it ships. Milestone impact follows once Logos
   publishes the 0.4 dates.

The spike corrections made in this round are listed in `docs/m0/verdicts.md`
("Corrections").
