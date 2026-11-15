# M1: core market primitives

**Part 1 of M1, on branch `m1`.** Part 2 comes from adp and lands on the same branch. We submit M1 for
acceptance on [logos-co/rfp issue 171](https://github.com/logos-co/rfp/issues/171) once both parts are in, and
the submission will name the exact commit to review.

Part 1 is Argo's lending program on LEZ v0.2.4: one program that holds every market, permissionless market
creation, and the six core operations (`supply`, `withdraw`, `supply_collateral`, `withdraw_collateral`,
`borrow`, `repay`). Tokens move by chained calls to the lez-programs v2.0.1 token program, a health check caps
borrowing and collateral withdrawals, and supply and debt are tracked in virtual shares. Nothing is deployed. It
runs in tests and on a local standalone sequencer.

## The eight deliverables

Deliverable wording is from issue 171.

| # | Deliverable | Where |
|---|---|---|
| 1 | Immutable singleton | One program, `argo_lending`, holds every market. It has no upgrade, pause or market-edit instruction: once `create_market` has run, nothing writes that market's parameters. Apart from `initialize`, the only admin actions enable an LLTV or an IRM in Config. Tests: `m1_core::two_markets_are_isolated`, `admin_operations_are_admin_only`. |
| 2 | Permissionless market creation (5-param id) | `create_market` in `programs/argo_lending/src/market.rs`, open to any signer. The market id is SHA-256 over the five parameters (loan token, collateral token, oracle, IRM, LLTV). The oracle parameter is a digest of the oracle program, the price account and the maximum price age ([D2](decisions.md)). Tests: `market_id_changes_with_every_param`, `m1_core::create_market_guards`. |
| 3 | Market / Position / vault PDAs | Config, Market, Position, LoanVault and CollateralVault are PDAs of Argo (seeds in `programs/argo_lending/core/src/lib.rs`). The two vaults are holdings of the pinned token program under Argo's seeds, so only Argo can debit them. Tests: `m1_core::vault_cannot_be_debited_directly`, `own_holding_as_vault_is_rejected`. |
| 4 | Virtual-share accounting + inflation test | Morpho's virtual shares (`argo_core/src/shares.rs`) plus a dead deposit at market creation ([D11](decisions.md)). Inflation tests: `m1_core::donation_does_not_inflate_shares`, `near_empty_market_attacker_cannot_dilute_victim`, `supply_withdraw_rounding_loop_never_profits`; at the math level `shares::tests::empty_market_first_deposit_is_not_100_percent` and two proptests. |
| 5 | `supply` / `withdraw` / `borrow` / `repay` / `supplyCollateral` / `withdrawCollateral` via chained calls | Handlers in `programs/argo_lending/src/ops.rs`. Every token transfer is a chained call to the token program, built in `programs/argo_lending/src/token.rs`. If a token leg fails, the whole transaction fails and Argo's accounting does not change. Tests: `m1_core::six_operations_end_to_end`, `failed_token_leg_reverts_accounting`, and the sequencer run below. |
| 6 | Health check | `argo_core/src/health.rs`. `borrow`, and `withdraw_collateral` while the position has debt, check the position against the market's LLTV, rounding against the borrower. Tests: `m1_core::borrow_past_lltv_is_rejected`, `withdraw_collateral_that_breaks_health_is_rejected`, `health::tests::*`. On the sequencer, an over-LLTV borrow and an unhealthy collateral withdrawal are both rejected. |
| 7 | Accrual scaffolding | Every core operation except `supply_collateral` (as in Morpho) first moves the market's `last_update_ms` forward to the LEZ clock (`CLOCK_01`), never back. The rate is 0 until the interest rate model lands in M2. Tests: `m1_core::accrual_moves_last_update_forward_only`, `clock_must_be_clock_01_and_started`. |
| 8 | ≥1 test per requirement covered by this milestone | [`traceability.md`](traceability.md): one row per RFP-008 requirement M1 covers, with the tests for each. |

## What was run

CI runs all of this on every push to `m1` (`.github/workflows/argo-ci.yml`):

- Lint: `cargo fmt --check`, and clippy with `-D warnings`.
- Host unit tests for `argo_core`, `irm_core`, `argo_lending_core` and `argo_lending_program`.
- The Argo guest built for riscv32 from the committed lockfiles, and the IDL in `idl/` regenerated and compared.
- 37 in-process tests (`programs/argo_lending/integration_tests/tests/m1_core.rs`) that run the Argo guest and
  the pinned v2.0.1 token in LEZ v0.2.4's state machine.
- The sequencer run, `./harness/localnet.sh e2e`. It boots a standalone LEZ v0.2.4 sequencer, deploys the token,
  `twap_oracle` and Argo, creates two tokens and a price feed, and runs all six operations, checking token balances
  and Market and Position accounting. An over-LLTV borrow and an unhealthy collateral withdrawal must not
  land, and the market must be unchanged after each rejection. It ends by printing `ARGO M1 E2E GREEN`.
- The five M0 spikes, which still pass.

The commands to run any of these locally are in the [repo README](../../README.md#running).

## Where M1 differs from the M0 spec

Each point, with its reason and source, is in [`decisions.md`](decisions.md). In short:

- Token: the lez-programs v2.0.1 token, which testnet-legacy uses, instead of the v0.2.4 builtin (D1).
- Oracle: a market commits to one exact price account and a maximum price age. Prices use the Q64.64 encoding
  Logos specified, and the core rejects stale, zero, wrong-pair and future-dated prices (D2 to D7).
- Time is in milliseconds, read from `CLOCK_01` (D8).
- The IRM and fee fields are in the layout now, so M2 does not change it (D9, D10).
- `create_market` makes a dead deposit (D11) and rejects a market whose two tokens are the same (D12).
- Every handler refuses inputs the runtime cannot check, so callers and the admin must be program-owned
  accounts (D15).
- Morpho's callbacks are not implemented (D16).

## Known limits

- The build pins a public test admin key (`ADMIN_GENESIS`, D14). It must not be deployed to a shared network as
  built.
- Built and tested on LEZ v0.2.4 only, following Logos's direction that delivery teams stay on 0.2.4 until 0.4
  ([logos-co/ecosystem issue 251](https://github.com/logos-co/ecosystem/issues/251)).
- Interest is 0 until M2. Liquidation and flash loans are M3. Real oracle feeds and per-feed maximum ages are M4.
- Until M3 adds managers (F17), only a position's owner can withdraw, borrow or withdraw collateral, and that owner
  must be a program-owned account (D15).
- RFP-008 F11 asks for LLTV removal; M1 can only enable an LLTV. Admin scope is M2.

## Notes for M4

Neither of these blocks M1.

1. [`decisions.md`](decisions.md#d5-and-f11-why-the-core-checks-staleness-and-how-to-choose-max_age_ms) gives our
   rule for choosing the maximum price age, `max_age_ms`, against the F11 stranded-collateral risk. We set per-feed
   defaults in M4, once we know how often each testnet price feed updates.
2. A TWAP feed gets a fresh timestamp on every publish, so the maximum-age check cannot see that the source behind
   the TWAP has stopped. It could if the price account carried the source's observation time.
