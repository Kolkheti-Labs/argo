# Argo account sharding and state layout (M0 draft)

Status: DRAFT, revised 2026-10-16 after the first Logos review. Written by
jeefxM; no review by rekt or adp is recorded yet. The review with Logos
follows theirs.

Target runtime: LEZ v0.2.4 (tag `v0.2.4`, commit 47eba256). Per Logos on
2026-10-12, Argo builds and tests on v0.2.4, delivers for testnet 0.4 and
re-adapts to it once 0.4 is published, and does not port to 0.3. If Logos
names another 0.2 tag for the relaunched 0.2 testnet (for example
`v0.2.4-lez-legacy`, published 2026-10-13, which is v0.2.4 plus 3 commits),
M1 moves to it.

Runtime facts below are from the LEZ v0.2.4 source unless marked otherwise
(the Token program note cites LEZ `dev`, and the 100 KB cycle figure is
lez-programs' benchmark via S-D). Spike results are cited where they back a
fact.

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
| Config | `"config"` | argo_lending | admin authority, enabled-LLTV set, enabled-IRM set, fee fraction, fee recipient | create_market (read), admin ops, accrual while a fee is set (read) |
| Market | `"market" ‖ marketId` | argo_lending | loan token, collateral token, oracle program id, IRM program id, LLTV; `total_supply_assets`, `total_supply_shares`, `total_borrow_assets`, `total_borrow_shares`, `last_update`, `fee`, `rate_at_target` | every market op except supply_collateral (reads it) and flash_loan |
| Position | `"position" ‖ marketId ‖ owner` | argo_lending | `supply_shares`, `borrow_shares`, `collateral` | that owner's ops, liquidation of that owner; the fee recipient's Position on any accrual while a fee is set |
| Loan vault | `"loan-vault" ‖ marketId` | token program, authorised by argo_lending PDA seed | supplied loan tokens | create_market (claim), supply, withdraw, borrow, repay, liquidate (repay leg), flash loan |
| Collateral vault | `"coll-vault" ‖ marketId` | token program, authorised by argo_lending PDA seed | collateral | create_market (claim), supply_collateral, withdraw_collateral, liquidate (seize leg) |
| Authorization | `"auth" ‖ authorizer ‖ manager` | argo_lending | `is_authorized`, `nonce` | set_authorization, set_authorization_with_sig |

### Runtime account model (v0.2.4)

`Account { program_owner: ProgramId, balance: u128, data: Data, nonce }`,
`data` at most 100 KiB. There is no ownership enum: `program_owner` is the
owner. For an account a program creates, it is set exactly once, by a claim
(`Claim::Authorized` needs a signer or caller-authorised account;
`Claim::Pda(seed)` needs `account_id == for_public_pda(executing_program, seed)`).
System accounts such as the clock accounts get it at genesis instead
(`lez/system_accounts/src/lib.rs:59-61`). Only the owner may
decrease `balance` or change `data` (rules 5 and 6), which is what makes the
vault pattern safe:

- Argo's own accounts (Config, Market, Position, Authorization) are claimed
  with `Claim::Pda` and owned by `argo_lending`.
- Vaults are token holdings, so they are owned by the **token program**, not
  Argo. Argo creates one by chaining `token::InitializeAccount` with the vault
  marked authorised and its seed in `pda_seeds`; Argo debits it by chaining
  `token::Transfer` the same way. Anyone can credit it once it exists (S-B B1
  credited it through a chained token `Transfer`; a top-level transfer into a
  claimed vault was not run in M0). A plain transfer into a not-yet-claimed
  PDA is rejected, so `create_market` must claim both vaults before any
  deposit (S-B B2).
- PDA id = SHA-256 over a fixed 96-byte buffer: the 32-byte prefix
  `"/LEE/v0.2/AccountId/PDA/"` (24 ASCII bytes + 8 NUL padding) ‖ program_id
  (32) ‖ seed (32); a single 32-byte seed, no bump. Seeds above will be hashed
  into 32 bytes as `seed = SHA-256(domain ‖ fields)` by a function in
  `argo_lending_core` that M1 adds (M0 ships only the zero-padded `config`
  seed).
- Account size drives compute (lez-programs' benchmark: about 33M extra
  cycles for a program-owned 100 KB account, via S-D), so every Argo account
  stays under 1 KB; `Config`'s enabled sets are bounded
  (32 LLTVs, 8 IRMs) rather than open-ended.

### Token program

M1 targets the v0.2.4 builtin token (`programs::token()`, types from
`token_core`) behind one adapter module, so switching to another token
program is a one-module change before any real value. M0 did not establish
which token program the public testnet runs (S-B B5 was withdrawn: v0.2.4's
`getProgramIds` is hardcoded). LEZ `dev` removed the `token`, `amm` and
`associated_token_account` programs from genesis after v0.3.0 (PR 926,
merged 2026-10-07), so which token program 0.4 ships is open.

## 3. Touched set per instruction

| Instruction | Writes | Reads | Chained calls |
| --- | --- | --- | --- |
| create_market | Market (sets `last_update`), LoanVault (claim), CollVault (claim) | Config, Clock, loan token definition, collateral token definition | token::InitializeAccount for LoanVault and for CollVault, PDA-authorised (S-B B1, B2) |
| supply | Market, Position(owner), LoanVault(+), user holding(−) | Clock | token::Transfer user→LoanVault, signer-authorised |
| withdraw | Market, Position(owner), LoanVault(−), receiver holding(+) | Clock | token::Transfer LoanVault→receiver, PDA-authorised |
| supply_collateral | Position(owner), CollVault(+), user holding(−) | Market | token::Transfer user→CollVault |
| withdraw_collateral | Market, Position(owner), CollVault(−), receiver(+) | Oracle price, Clock | token::Transfer CollVault→receiver, PDA-authorised |
| borrow | Market, Position(owner), LoanVault(−), receiver(+) | Oracle price, Clock | token::Transfer LoanVault→receiver, PDA-authorised |
| repay | Market, Position(owner), LoanVault(+), payer(−) | Clock | token::Transfer payer→LoanVault |
| liquidate | Market, Position(borrower), LoanVault(+), CollVault(−), liquidator loan(−), liquidator coll(+) | Oracle price, Clock | token::Transfer liquidator→LoanVault; token::Transfer CollVault→liquidator, PDA-authorised |
| flash_loan | LoanVault(−, then +), borrower holding | — | token::Transfer LoanVault→borrower; chained call into the borrower program; chained self-call `repay_flash`; token::Transfer borrower→LoanVault. Constraint from S-A: every chained call's pre-states must be predicted exactly, so `repay_flash` can only name the vault and the borrower holding at their expected post-callback balances |
| accrue_interest (implicit) | Market, Position(fee recipient) when fee > 0 | Clock; Config (the fee recipient, and the fee itself while it sits in Config; see section 7 item 5) | none |
| set_authorization | Authorization(authorizer, manager) | — | none |
| admin: enable_lltv / enable_irm / set_fee / set_fee_recipient | Config | — | none |

Every row's account count is a constant. No instruction iterates over markets
or positions. The `accrue_interest` fee-share mint writes the fee recipient's
Position and reads Config, which names the recipient, so it adds at most two
fixed accounts to any instruction that accrues while a fee is set. While the
fee fraction sits in Config (section 7 item 5), every accrual reads Config.
Reading the clock (section 5) adds one fixed read-only account, `CLOCK_01`, to
every instruction that accrues or sets `last_update`. As in Morpho Blue,
`withdraw_collateral` accrues (so it writes Market), while
`supply_collateral`, `flash_loan`, `set_authorization`, `enable_lltv`,
`enable_irm` and `set_fee_recipient` do not. Morpho's `setFee` accrues before
it changes the fee (`Morpho.sol:129-130` at commit 8e26ca6); the `set_fee` row
stays Config-only until section 7 item 5 is decided.

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
- `rate_at_target` (WAD per second), `last_update` (Unix seconds).

### Clock (proposed for M1 on v0.2.4; pending rekt's review and a live check on the relaunched 0.2 testnet)

Accrual reads the LEZ clock account `CLOCK_01` as a read-only input.

What the v0.2.4 source says (not yet observed on a public testnet):
- The sequencer ends every block after genesis with a clock-program
  transaction carrying its wall-clock time in Unix milliseconds: it builds the
  transaction from `timestamp_millis` (`lez/sequencer/core/src/lib.rs:845-849`)
  and appends it last (`:958-961`), and a node ingesting a block rejects it if
  its last transaction is not that clock invocation
  (`lez/chain_state/src/apply.rs:126-137`). The genesis it builds
  registers the clock program and inserts its accounts
  (`lez/testnet_initial_state/src/lib.rs:256-260, 269`). Our v0.2.4 localnet
  shows the clock accounts in every block (S-B, "B5 withdrawn").
- `CLOCK_01` (`/LEZ/ClockProgramAccount/0000001`,
  `lez/programs/clock/core/src/lib.rs:6-7`) holds
  `ClockAccountData { block_id: u64, timestamp: u64 }` and is rewritten every
  block (`lez/programs/clock/src/main.rs:70-84`). It is owned by the clock
  program from genesis, so no other program can change its data (rule 6).
  The clock program does not check who invokes it (`main.rs:51-68` checks
  only the account count, ids and owner). What stops a user transaction from
  rewriting a clock account is the sequencer's `validate_on_state`, which
  rejects any user transaction whose diff modifies one
  (`lez/common/src/transaction.rs:78-102`); nodes replaying blocks skip that
  check (`execute_on_state`, `transaction.rs:140-145`). So the time Argo reads
  is the producing sequencer's wall clock, and Argo trusts the sequencer for
  it.
- Upstream test programs read it the same way, as an input checked against
  `CLOCK_01_PROGRAM_ACCOUNT_ID`
  (`test_programs/guest/src/bin/time_locked_transfer.rs:34`,
  `test_programs/guest/src/bin/pinata_cooldown.rs:64`).

Argo's rules:
- The guest asserts the input's account id is `CLOCK_01` and decodes it. A
  default (empty) account fails the decode. The guest also rejects
  `timestamp == 0`, because genesis creates `CLOCK_01` with timestamp 0
  (`lez/system_accounts/src/lib.rs:59-71`) and the genesis block's clock call
  keeps that value (`lez/sequencer/core/src/lib.rs:1495`). In both cases the
  instruction fails instead of accruing from time zero.
- `now = timestamp / 1000`.
- The clock transaction runs last in its block (`lez/sequencer/core/src/lib.rs:958-961`),
  so an Argo transaction sees the previous block's time. The gap is accrued
  on the next touch.
- The clock program does not compare a new timestamp with the previous one
  (`main.rs:70-80`), and we found no other v0.2.4 check that block timestamps
  increase. So `elapsed = now.saturating_sub(last_update)`, and when
  `elapsed == 0` accrual returns without writing `last_update`, as Morpho does
  (`Morpho.sol:485`), so `last_update` never moves backwards.

Alternative considered: the caller passes the timestamp, and the guest bounds
it with a `timestamp_validity_window` on its `ProgramOutput`, which the
runtime checks against the block's timestamp
(`lee/state_machine/src/validated_state_diff/mod.rs:202-209`). That bounds the
value but does not fix it, trading accuracy against how long the transaction
stays valid, so we use it for deadlines (for example
`set_authorization_with_sig`) and not for accrual.

The clock is re-checked with the rest of the runtime when 0.4 ships.

## 6. Sharding argument for P2

A transaction's cost is a function of (a) the guest cycles it executes and
(b) the accounts it reads and writes. Argo has no global list of markets or
positions, so neither (a) nor (b) depends on how many markets exist. The only
per-deployment bound is how much one transaction can span. v0.2.4 has no cap
on the number of accounts in a transaction; it checks that the encoded
transaction fits in a block (`lez/sequencer/service/src/service.rs:55-66`).
The limit that binds is the chained-call cap, `MAX_NUMBER_CHAINED_CALLS = 10`
(`lee/state_machine/core/src/program/mod.rs:14`, checked at
`lee/state_machine/src/validated_state_diff/mod.rs:110`), so a public
transaction runs at most 11 program executions (S-A A6). `flash_loan` uses
four chained calls (transfer out, borrower callback, `repay_flash`, transfer
back), and any calls the borrower's callback makes count against the same
limit. This caps what a flash loan can span, not what the singleton can hold.
Spike S-D measured per-execution cycles for supply-shaped and
settlement-shaped legs and extrapolated a liquidation at about 0.6M cycles
(an estimate, not a measurement), against v0.2.4's 32M-cycle limit, which
applies to each program execution (S-D). M3 measures the real `liquidate`,
and the maximum market count reported in M3/M7 derives from these limits, not
from any per-market state cost.

## 7. Open questions for review

1. Should `Authorization` be a separate PDA or a small map inside `Position`?
   Separate keeps `Position` fixed-size; the manager path then touches one
   extra account.
2. Oracle price account: a read-only input. Every input arrives as
   `AccountWithMetadata { account, is_authorized, account_id }`
   (`lee/state_machine/core/src/account.rs:125-129`), so the guest
   can check both `price.account.program_owner == market.oracle` and the
   account id. A program-owner check alone is not enough, because it accepts
   any account the oracle program owns, including another pair's feed.
   Decide with rekt how a market binds its price account (for example its id
   in the market parameters, or a seed the guest derives).
3. Config bound: data limit is 100 KiB per account, but compute cost argues
   for the fixed small bounds above. Confirm the numbers with rekt.
4. Clock source for accrual: proposed in section 5 (read `CLOCK_01`), open
   for rekt's review.
5. Protocol fee placement: section 2 puts a fee fraction in Config and `fee`
   in Market, and the admin `set_fee` row writes Config. Morpho Blue keeps the
   fee per market and accrues before changing it. Decide with rekt; if per
   market, `set_fee` writes Market and reads the clock.
6. `set_authorization_with_sig` is listed under Authorization but has no row
   in section 3. Add one, with its deadline enforced by a
   `timestamp_validity_window`.
7. What we re-check when 0.4 ships: the account and claim model, chained-call
   authorization, fees and compute limits, clock access, the token program,
   and events.
