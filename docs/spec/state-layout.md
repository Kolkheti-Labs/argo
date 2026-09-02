# Argo account sharding and state layout (M0 draft)

Status: draft for review by rekt and adp. Runtime facts below are from the
LEZ v0.2.4 source (`lee/state_machine/core`), confirmed by spike S-B where
noted.

## 1. Goal

One immutable program id holds every market. LEZ compute and data cost scale
with the accounts a transaction touches, so the logical singleton is sharded
across PDA accounts and each instruction touches a fixed, small set. The
touched set must not grow with the number of markets or positions
(Performance P2).

## 2. Accounts

All accounts are PDAs derived from the Argo program id. Seeds are byte strings;
`marketId` is the 32-byte hash of the canonical encoding of the five market
parameters (F3).

| Account | Seeds | Owner | Holds | Mutated by |
| --- | --- | --- | --- | --- |
| Config | `"config"` | argo_lending | admin authority, enabled-LLTV set, enabled-IRM set, fee fraction, fee recipient | create_market (read), admin ops |
| Market | `"market" ‖ marketId` | argo_lending | loan token, collateral token, oracle program id, IRM program id, LLTV; `total_supply_assets`, `total_supply_shares`, `total_borrow_assets`, `total_borrow_shares`, `last_update`, `fee`, `rate_at_target` | every market op |
| Position | `"position" ‖ marketId ‖ owner` | argo_lending | `supply_shares`, `borrow_shares`, `collateral` | that owner's ops, liquidation of that owner |
| Loan vault | `"loan-vault" ‖ marketId` | token program, authorised by argo_lending PDA seed | supplied loan tokens | supply, withdraw, borrow, repay, liquidate (repay leg), flash loan |
| Collateral vault | `"coll-vault" ‖ marketId` | token program, authorised by argo_lending PDA seed | collateral | supply_collateral, withdraw_collateral, liquidate (seize leg) |
| Authorization | `"auth" ‖ authorizer ‖ manager` | argo_lending | `is_authorized`, `nonce` | set_authorization, set_authorization_with_sig |

### Runtime account model (v0.2.4)

`Account { program_owner: ProgramId, balance: u128, data: Data, nonce }`,
`data` at most 100 KiB. There is no ownership enum: `program_owner` is the
owner and is set exactly once, by a claim (`Claim::Authorized` needs a signer
or caller-authorised account; `Claim::Pda(seed)` needs
`account_id == for_public_pda(executing_program, seed)`). Only the owner may
decrease `balance` or change `data` (rules 5 and 6), which is what makes the
vault pattern safe:

- Argo's own accounts (Config, Market, Position, Authorization) are claimed
  with `Claim::Pda` and owned by `argo_lending`.
- Vaults are token holdings, so they are owned by the **token program**, not
  Argo. Argo creates one by chaining `token::InitializeAccount` with the vault
  marked authorised and its seed in `pda_seeds`; Argo debits it by chaining
  `token::Transfer` the same way. Anyone can credit it with a plain transfer
  once it exists. A plain transfer into a not-yet-claimed PDA is rejected, so
  `create_market` must claim both vaults before any deposit (S-B B2).
- PDA id = SHA-256 over a fixed 96-byte buffer: the 32-byte prefix
  `"/LEE/v0.2/AccountId/PDA/"` (24 ASCII bytes + 8 NUL padding) ‖ program_id
  (32) ‖ seed (32); a single 32-byte seed, no bump. Seeds above will be hashed
  into 32 bytes as `seed = SHA-256(domain ‖ fields)` by a function in
  `argo_lending_core` that M1 adds (M0 ships only the zero-padded `config`
  seed).
- Account size drives compute (about 33M cycles for a 100 KB owned account),
  so every Argo account stays under 1 KB; `Config`'s enabled sets are bounded
  (32 LLTVs, 8 IRMs) rather than open-ended.

## 3. Touched set per instruction

| Instruction | Writes | Reads | Chained calls |
| --- | --- | --- | --- |
| create_market | Market, LoanVault (claim), CollVault (claim) | Config | none |
| supply | Market, Position(owner), LoanVault(+), user holding(−) | — | token::Transfer user→LoanVault, signer-authorised |
| withdraw | Market, Position(owner), LoanVault(−), receiver holding(+) | — | token::Transfer LoanVault→receiver, PDA-authorised |
| supply_collateral | Position(owner), CollVault(+), user holding(−) | Market | token::Transfer user→CollVault |
| withdraw_collateral | Position(owner), CollVault(−), receiver(+) | Market, Oracle price | token::Transfer CollVault→receiver, PDA-authorised |
| borrow | Market, Position(owner), LoanVault(−), receiver(+) | Oracle price | token::Transfer LoanVault→receiver, PDA-authorised |
| repay | Market, Position(owner), LoanVault(+), payer(−) | — | token::Transfer payer→LoanVault |
| liquidate | Market, Position(borrower), LoanVault(+), CollVault(−), liquidator loan(−), liquidator coll(+) | Oracle price | token::Transfer liquidator→LoanVault; token::Transfer CollVault→liquidator, PDA-authorised |
| flash_loan | LoanVault(−, then +), borrower holding | — | token::Transfer LoanVault→borrower; chained call into the borrower program; chained self-call `repay_flash`; token::Transfer borrower→LoanVault. Constraint from S-A: every chained call's pre-states must be predicted exactly, so `repay_flash` can only name the vault and the borrower holding at their expected post-callback balances |
| accrue_interest (implicit) | Market, Position(fee recipient) when fee > 0 | — | none |
| set_authorization | Authorization(authorizer, manager) | — | none |
| admin: enable_lltv / enable_irm / set_fee / set_fee_recipient | Config | — | none |

Every row's account count is a constant. No instruction iterates over markets
or positions. The `accrue_interest` fee-share mint touches the fee recipient's
Position, which adds one fixed account to any instruction that accrues while a
fee is set.

## 4. Market id

```
marketId = H(domain ‖ loan_token ‖ collateral_token ‖ oracle ‖ irm ‖ lltv)
```

`H` will be SHA-256 via `risc0_zkvm::sha::Impl`, the hash LEZ itself uses for
PDA ids, so guest and client agree byte-for-byte; `domain = "argo/market/v1"`
(M1).
Fields are Borsh-encoded in the order above; `lltv` is a WAD-scaled u128.
The Market PDA seed is `SHA-256("argo/market" ‖ marketId)`, and `create_market`
fails if the PDA is already claimed.

## 5. Numeric layout

- Assets and shares: `u128`. Intermediates: `U256` (or widening `u128 × u128`
  via a checked helper) for `mul_div`.
- `WAD = 1e18`, `VIRTUAL_SHARES = 1e6`, `VIRTUAL_ASSETS = 1`,
  `ORACLE_PRICE_SCALE = 1e36`, `MAX_FEE = 0.25 WAD`.
- `rate_at_target`, `last_update` (seconds). Clock source is an open design
  point: v0.2.4 has a `clock` program (`CLOCK_01` account, ms timestamp) but
  the public testnet does not register it (evidence in S-B B5). Options:
  (a) require the clock account and ask Logos to deploy it, (b) pass the
  timestamp as an instruction arg bounded by `timestamp_validity_window`
  so the runtime rejects lies outside the window. Decide in M1 with rekt.

## 6. Sharding argument for P2

A transaction's cost is a function of (a) the guest cycles it executes and
(b) the accounts it reads and writes. Argo has no global list of markets or
positions, so neither (a) nor (b) depends on how many markets exist. The only
per-deployment bound is the number of *distinct* PDAs a single transaction
may include, which caps what a flash loan can span, not what the singleton can
hold. Spike S-D measures the cycles for the heaviest single-market instruction
(liquidate) and reports the margin against the per-tx envelope; the maximum
market count reported in M3/M7 derives from the flash-loan account cap and the
envelope, not from any per-market state cost.

## 7. Open questions for review

1. Should `Authorization` be a separate PDA or a small map inside `Position`?
   Separate keeps `Position` fixed-size; the manager path then touches one
   extra account.
2. Oracle price account: read-only input validated against `market.oracle` by
   program-owner check. Confirmed possible: every input arrives as
   `AccountWithMetadata { account.program_owner, account_id, is_authorized }`,
   so the guest asserts `price.account.program_owner == market.oracle`.
3. Config bound: data limit is 100 KiB per account, but compute cost argues
   for the fixed small bounds above. Confirm the numbers with rekt.
4. Clock source for accrual (section 5).
