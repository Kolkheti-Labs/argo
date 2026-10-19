# Argo account sharding and state layout (M0 draft)

Status: DRAFT, revised 2026-10-19 after adp's review. Written by jeefxM.
adp reviewed §2, §3, §4, §6, §7.1 and §7.6 at `2beec8c`
(https://github.com/Kolkheti-Labs/argo/pull/1#issuecomment-6265447369). This
revision addresses adp's six change requests (two with corrections: borrow
is never a Position's first use, and v0.2.4 serializes every transaction,
not only one market's) and records adp's two decisions. Everything that
differs from `2beec8c` postdates adp's review and is not yet reviewed. That
includes:
- issues we found while checking adp's points against the LEZ v0.2.4 source
  (the main ones marked "(found 2026-10-19)");
- proposals taken from Kamino klend in §7 items 2, 5, 11 and 12.

Still pending:
- rekt's review of §5 and §7 items 2–5;
- a review of the new material.

The review with Logos follows theirs.

Target runtime: LEZ v0.2.4 (tag `v0.2.4`, commit 47eba256). Per Logos on
2026-10-12, Argo builds and tests on v0.2.4, delivers for testnet 0.4 and
re-adapts to it once 0.4 is published, and does not port to 0.3. If Logos
names another 0.2 tag for the relaunched 0.2 testnet (for example
`v0.2.4-lez-legacy`, published 2026-10-13, which is v0.2.4 plus 3 commits),
M1 moves to it.

Runtime facts below are from the LEZ v0.2.4 source unless marked otherwise
(the Token program note cites LEZ `dev`, and the 100 KB cycle figure is
lez-programs' benchmark via S-D). Paths starting `lee/`, `lez/` or
`test_programs/` are in the LEZ v0.2.4 repository. Spike results are cited where they back a fact.

## 1. Goal

One immutable program id holds every market. LEZ compute and data cost scale
with the accounts a transaction touches, so the logical singleton is sharded
across PDA accounts and each instruction touches a fixed, small set. The
touched set must not grow with the number of markets or positions
(Performance P2).

## 2. Accounts

All accounts are PDAs derived from the Argo program id. Seeds are byte strings;
`marketId` is the 32-byte hash of the canonical encoding of the five market
parameters (F3, section 4).

An `owner`, `authorizer` or `manager` is an `AccountId` (found 2026-10-19).
Where an operation needs that party's consent, the id must be an authorised
input. On v0.2.4 an account is authorised for Argo in three cases:
- It signs the transaction. Signer ids are derived from public keys
  (`lee/state_machine/src/public_transaction/transaction.rs:34-40`).
- It is a PDA that the program calling Argo authorised through `pda_seeds`
  (`lee/state_machine/src/validated_state_diff/mod.rs:133-141`).
- It was authorised earlier in the chain, and each program in between listed
  it as an authorised input (`:264-283`). So, as in
Morpho, a program can own a Position by calling Argo. One account holds at
most one token holding, so a user who holds both of a market's tokens has at
least two accounts. The owner id need not be any holding the instruction
moves.

| Account | Seeds | Owner | Holds | Mutated by |
| --- | --- | --- | --- | --- |
| Config | `"config"` (zero-padded, not hashed) | argo_lending | admin authority, enabled-LLTV set, enabled-IRM set, fee fraction, fee recipient | initialize (claim), create_market (read), admin ops, every accrual while the fee fraction sits in Config (read; section 7 item 5) |
| Market | `SHA-256("market" ‖ marketId)` | argo_lending | loan token, collateral token, oracle program id, IRM id, LLTV; `total_supply_assets`, `total_supply_shares`, `total_borrow_assets`, `total_borrow_shares`, `last_update`, `fee`, `rate_at_target` | create_market (claim), every market op except supply_collateral (reads it) and flash_loan |
| Position | `SHA-256("position" ‖ marketId ‖ owner)` | argo_lending | `supply_shares`, `borrow_shares`, `collateral` | that owner's ops (claimed on first supply or supply_collateral), liquidation of that owner; the fee recipient's Position on any accrual while a fee is set |
| Loan vault | `SHA-256("loan-vault" ‖ marketId)` | token program, authorised by argo_lending PDA seed | supplied loan tokens | create_market (claim), supply, withdraw, borrow, repay, liquidate (repay leg), flash loan |
| Collateral vault | `SHA-256("coll-vault" ‖ marketId)` | token program, authorised by argo_lending PDA seed | collateral | create_market (claim), supply_collateral, withdraw_collateral, liquidate (seize leg) |
| Authorization | `SHA-256("auth" ‖ authorizer ‖ manager)` | argo_lending | `is_authorized` (and a nonce; see section 7 item 6) | set_authorization, set_authorization_with_sig (claimed on first use) |

### Runtime account model (v0.2.4)

`Account { program_owner: ProgramId, balance: u128, data: Data, nonce }`,
`data` at most 100 KiB. There is no ownership enum: `program_owner` is the
owner. For an account a program creates, it is set exactly once, by a claim
(`Claim::Authorized` needs a signer or caller-authorised account;
`Claim::Pda(seed)` needs `account_id == for_public_pda(executing_program, seed)`).
System accounts such as the clock accounts get it at genesis instead
(`lez/system_accounts/src/lib.rs:59-61`). Only the owner may decrease
`balance` (rule 5) or change `data` (rule 6). Rule 6 exempts an account
whose pre-state is the default account
(`lee/state_machine/core/src/program/mod.rs:718-722`). But any default
account a transaction changes must end that transaction claimed, or it fails
with `DefaultAccountModifiedWithoutClaim` (public path,
`lee/state_machine/src/validated_state_diff/mod.rs:293-307`). Argo claims
each of its PDAs in the output that first writes it, which satisfies that
check. No program may
change an account's `nonce` (`lee/state_machine/core/src/program/mod.rs:691-696`),
so any nonce Argo keeps lives in `data`. These rules are what make the vault
pattern safe:

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
  (32) ‖ seed (32); a single 32-byte seed, no bump
  (`lee/state_machine/core/src/program/mod.rs:128-138`). The program id is
  already in the derivation, so seed domains need no `argo/` prefix. Config's
  seed is the zero-padded bytes `config`, not hashed, as M0 ships it
  (`programs/argo_lending/core/src/lib.rs:47`). Every other seed is
  `SHA-256(domain ‖ fields)` over the fixed-length fields shown in the table,
  computed by a function in `argo_lending_core` that M1 adds.
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

The adapter pins the token program id as a constant (found 2026-10-19).
Argo's token calls only ever target that constant, never a program id taken
from an input account, whether from its `program_owner` or its `data`. The
M0 spike takes it from the input accounts' `program_owner`
(`spikes/vault/src/lib.rs:54, 91, 126`). If M1 did the same, a market created
for a definition owned by some other program P would chain into P with
Argo's vault authority, so P could claim that market's vaults, and every
later vault leg of that market would call P. Each such call also carries
the transaction signers' authority, because every chained call inherits it
(`lee/state_machine/src/validated_state_diff/mod.rs:264-283`). The harm is
limited to markets created for P's own definition; Morpho, too, lets anyone
create a market for any token. The builtin `InitializeAccount` does not
check that the definition belongs to the token program
(`lez/programs/token/src/initialize.rs:18`, an upstream TODO). With the id
pinned, a definition the token program does not own only yields an unusable
market. `create_market` also asserts that both definitions are owned by the
pinned token program and decode as `TokenDefinition::Fungible`
(`lez/programs/token/core/src/lib.rs:78-84`), so a vault is never a
non-fungible holding.

Argo debits a user's holding only when that holding is authorised for
Argo: its key signs the transaction, or the program calling Argo authorised
it, as above (found 2026-10-19). An associated token account is a PDA of the
ATA program (`lez/programs/associated_token_account/core/src/lib.rs:64-66`).
In a transaction that calls Argo directly it is not authorised, and Argo's
own `pda_seeds` authorise only PDAs of Argo, so a debit from it fails.
Whether to support ATAs is section 7 item 8.

## 3. Touched set per instruction

`onBehalf` is the owner whose Position the instruction changes. "Caller
authority" means one of two things:
- `onBehalf` itself, as an authorised input (section 2);
- an authorised manager, plus a read of `Authorization(onBehalf, manager)`.

| Instruction | Writes | Reads | Chained calls |
| --- | --- | --- | --- |
| initialize | Config (claim); admin account (claimed if default, as M0 does; section 7 item 11 proposes dropping this) | admin (authorised) | none |
| create_market | Market (claim; sets `last_update`), LoanVault (claim), CollVault (claim) | Config, Clock, loan token definition, collateral token definition (both owned by the pinned token program) | token::InitializeAccount for LoanVault and for CollVault, PDA-authorised (S-B B1, B2) |
| supply | Market, Position(onBehalf) (claim if default), LoanVault(+), payer holding(−) | Clock | token::Transfer payer→LoanVault, payer-authorised |
| withdraw | Market, Position(onBehalf), LoanVault(−), receiver holding(+) | Clock, caller authority | token::Transfer LoanVault→receiver, PDA-authorised |
| supply_collateral | Position(onBehalf) (claim if default), CollVault(+), payer holding(−) | Market | token::Transfer payer→CollVault, payer-authorised |
| withdraw_collateral | Market, Position(onBehalf), CollVault(−), receiver(+) | Oracle price (checked only when the position has debt, as `Morpho.sol:516`), Clock, caller authority | token::Transfer CollVault→receiver, PDA-authorised |
| borrow | Market, Position(onBehalf), LoanVault(−), receiver(+) | Oracle price, Clock, caller authority | token::Transfer LoanVault→receiver, PDA-authorised |
| repay | Market, Position(onBehalf), LoanVault(+), payer(−) | Clock | token::Transfer payer→LoanVault, payer-authorised |
| liquidate | Market, Position(borrower), LoanVault(+), CollVault(−), liquidator loan(−), liquidator coll(+) | Oracle price, Clock | token::Transfer liquidator→LoanVault, liquidator-authorised; token::Transfer CollVault→liquidator, PDA-authorised |
| flash_loan | LoanVault(−, then +), borrower holding, plus the accounts the borrower's callback needs | Market (to form `repay_flash`'s pre-states) | token::Transfer LoanVault→borrower, PDA-authorised; chained call into the borrower program, which itself chains token::Transfer borrower→LoanVault for exactly the amount; chained self-call `repay_flash` |
| accrue_interest (implicit) | Market, Position(fee recipient) when fee > 0 | Clock; Config (the fee recipient, and the fee itself while it sits in Config; see section 7 item 5) | none |
| set_authorization | Authorization(authorizer, manager) (claim if default) | authorizer (authorised) | none |
| set_authorization_with_sig | Authorization(authorizer, manager) (claim if default) | — (the signature is checked in the guest; see section 7 item 6) | none |
| admin: enable_lltv / enable_irm / set_fee / set_fee_recipient | Config | admin (authorised) | none |

Every row's account count is bounded by a constant, except that `flash_loan` also
forwards the accounts the borrower's callback names; a program sees only the
pre-states its caller passes it. No count grows with the number of markets
or positions, and no instruction iterates over them. The `accrue_interest`
fee-share mint writes the fee recipient's Position and reads Config, which
names the recipient, so it adds at most two fixed accounts to any
instruction that accrues while a fee is set. While the fee fraction sits in
Config (section 7 item 5), every accrual reads Config. Reading the clock
(section 5) adds one fixed read-only account, `CLOCK_01`, to every
instruction that accrues or sets `last_update`. As in Morpho Blue,
`withdraw_collateral` accrues (so it writes Market), while
`supply_collateral`, `flash_loan`, `set_authorization`, `enable_lltv`,
`enable_irm` and `set_fee_recipient` do not. Morpho's `setFee` accrues before
it changes the fee (`Morpho.sol:129-130` at commit 8e26ca6); the `set_fee` row
stays Config-only until section 7 item 5 is decided.

**First-use claims.** Argo claims with `Claim::Pda`:
- Config in `initialize`.
- Market in `create_market`. It uses a claim that fails if the account is
  already claimed, so a second create of the same market is rejected
  (`ClaimedNonDefaultAccount`,
  `lee/state_machine/src/validated_state_diff/mod.rs:219-222`).
- Position(onBehalf) in `supply` and `supply_collateral`, when it is still a
  default account.
- The fee recipient's Position on an accrual that mints fee shares.
- Authorization on the first `set_authorization` or
  `set_authorization_with_sig` for that pair.

`borrow` can never be a Position's first use: a non-zero borrow passes the
health check only with collateral, and collateral lives in the Position. So
`withdraw`, `withdraw_collateral`, `borrow`, `repay` and `liquidate` require
an Argo-owned Position and fail on a default one. The vaults are claimed by
the token program (`Claim::Authorized`), using the authorisation Argo grants
through `pda_seeds`.

**Caller authority (found 2026-10-19).** At top level only the
transaction's signers are authorised
(`lee/state_machine/src/validated_state_diff/mod.rs:99-102`). A PDA cannot
sign, because signer ids are derived from public keys, and with no calling
program no PDA is authorised (`lee/state_machine/core/src/program/mod.rs:632-643`).
Inside a chained call, the PDAs that the calling program names in
`pda_seeds` are authorised as well
(`lee/state_machine/src/validated_state_diff/mod.rs:133-141`), and so is
anything authorised earlier in the chain (section 2, third case).
A Position or a vault is never authorised by the user, so no row can prove
consent through one. The guest checks that the caller-authority input is
authorised: `onBehalf` itself, or the manager together with the derived
`Authorization(onBehalf, manager)` id and its flag.

**Account aliasing.** A transaction may not list an account id twice
(`lee/state_machine/src/validated_state_diff/mod.rs:71-75` for public
transactions, `:350-354` for private ones). No program execution, chained
calls included, may return two pre-states with the same id
(`lee/state_machine/core/src/program/mod.rs:675-678`). The runtime forbids
listing an id twice, not one account serving two roles. So when two roles in
a row are the same account, the client lists it once, and the guest derives
both ids, expects one input and returns one post-state. The cases:
- The fee recipient's Position equals Position(onBehalf), or
  Position(borrower) in `liquidate`. The guest applies the fee-share mint
  first and then the instruction's own update, in Morpho's order (accrue
  first: `Morpho.sol:214, 219` in withdraw, `:358, 384` in liquidate). Without this, a borrower who is also the fee
  recipient could not be liquidated while a fee is set.
- `loan_token == collateral_token`: section 7 item 10.
- A payer, receiver or liquidator holding equal to LoanVault or CollVault is
  not supported. The guest rejects it.

**Credited holdings must exist (found 2026-10-19).** The builtin
`Transfer` claims a default recipient with `Claim::Authorized`
(`lez/programs/token/src/transfer.rs:18, 109`). That claim needs the
recipient to be authorised
(`lee/state_machine/src/validated_state_diff/mod.rs:225-229`). So every
credited account must already be an initialised holding of the market's
token, or a fresh account that is authorised for Argo (section 2). That
covers the
withdraw and borrow receivers, the `withdraw_collateral` receiver, the
liquidator's collateral holding and the flash-loan borrower holding.
Creating a holding needs the same authorisation, because
`InitializeAccount` claims with `Claim::Authorized` as well
(`lez/programs/token/src/initialize.rs:33`). So a third-party receiver must
create its own holding first. This is a difference from Morpho, where any
receiver address works. The guest checks early that a receiver is either a
holding of the market's token or an authorised default account, so the
failure is clear.

**Flash loan.** Chained calls run depth-first, each after its caller has
finished, in the order emitted
(`lee/state_machine/src/validated_state_diff/mod.rs:278-285`). The caller
supplies each callee's input pre-states. The runtime checks the callee's
output pre-states against the current state when the callee runs
(`:143-158`). A well-behaved program, such as the token program, echoes those
from its inputs. So:
- When a user calls `flash_loan` directly and the borrower holding is a PDA
  of the borrower program, the repayment has to come from the borrower's
  callback. Argo's own calls carry only the authority Argo
  had when it emitted them
  (`lee/state_machine/src/validated_state_diff/mod.rs:264-285`). That does
  not cover a holding authorised by the borrower program, and a debit the
  holding did not authorise fails (S-A A5, S-B B4).
- When the borrower program itself calls `flash_loan` and authorises its
  holding through `pda_seeds`, that authority does carry into Argo's calls.
  Argo can then pull the repayment, as Morpho does (`Morpho.sol:427-431`).
- `repay_flash` runs after the callback. If Argo forms its pre-states
  itself, `repay_flash` names both LoanVault and Market at exactly their
  pre-loan states, and `flash_loan` reads Market to form them (found
  2026-10-19). LoanVault alone does not prove repayment. The pre-state check
  is only an equality test
  (`lee/state_machine/src/validated_state_diff/mod.rs:147-152`), and a
  credit into LoanVault looks the same whatever caused it. So a same-market
  `supply`, `repay` or `liquidate` inside the callback could refill
  LoanVault with the loaned tokens and also be credited for them, leaving
  the market short by the loan. Pinning Market as well makes any same-market
  operation that changes Market fail. `supply_collateral`, which only reads
  Market and does not touch LoanVault, is unaffected and harmless.
  Repayment must be exact.
- Morpho's `flashLoan` has no such limit: it pulls the amount back after the
  callback whatever the callback did (`Morpho.sol:422-431`). Allowing
  same-market use on LEZ would need more than client-simulated predictions.
  `repay_flash` would also need a rule that reconciles LoanVault's balance
  with Market's totals, and this spec does not give one. Section 7 item 9
  decides.
- `repay_flash` asserts `caller_program_id == self_program_id`, so nothing
  else can call it.

If Argo can debit the borrower holding itself (its key signs, or the
calling borrower program authorised it), it can chain the repayment and
drop `repay_flash`, for three chained calls. The borrower holding and
LoanVault must then end the callback at exactly their predicted states,
because Argo's repayment transfer names both. A refilled LoanVault fails that
transfer's pre-state.

**Interest rate models (found 2026-10-19).** A program cannot get a value
back from a chained call. `ProgramOutput` has no return-data field
(`lee/state_machine/core/src/program/mod.rs:430-448`), and a caller's
post-states are written before its chained calls run
(`lee/state_machine/src/validated_state_diff/mod.rs:250-257`). So, unlike in
Morpho, accrual cannot read a rate back from an external IRM in the same
execution. An external IRM could deliver a rate most directly by calling
back into an Argo continuation that checks its caller, at about two more
executions per accrual. Proposed: compile IRMs into `argo_lending`, so a Market's IRM id
selects a built-in model and `enable_irm` enables only built-ins. That
deviates from Morpho and replaces the standalone `argo_irm` program the
README planned. Section 7 item 13 decides.

**Morpho callbacks (found 2026-10-19).** Morpho's optional callbacks on supply, repay,
supply_collateral and liquidate are not in these rows. Without them a
liquidator needs the loan tokens up front. Section 7 item 9 decides whether
M1 drops them.

## 4. Market id

```
marketId = SHA-256(DOMAIN ‖ borsh(MarketParams))
```

The hash is computed with `risc0_zkvm::sha::Impl`, the hash LEZ itself uses
for PDA ids, so guest and client agree byte-for-byte. `DOMAIN` is the 14
ASCII bytes `argo/market/v1`, written raw: it is not Borsh-encoded, so it
carries no length prefix. `MarketParams` has these fields, in this order:
- `loan_token`: `AccountId`, the id of the loan token's definition account.
- `collateral_token`: `AccountId`, the id of the collateral token's
  definition account.
- `oracle`: `ProgramId`.
- `irm`: `ProgramId` (as M0's Config already types IRMs): the IRM program
  id, or the id that selects a built-in model if section 7 item 13 adopts
  compiled-in IRMs.
- `lltv`: `u128`, WAD-scaled.

Borsh writes these with no length prefixes or tags:
- An `AccountId` is its 32 raw bytes: it derives Borsh over
  `value: [u8; 32]` (`lee/state_machine/core/src/account.rs:142-157`).
- A `ProgramId` is `[u32; 8]` (`lee/state_machine/core/src/program/mod.rs:16`),
  written as its eight words in order, each little-endian, so 32 bytes.
- `lltv` is 16 little-endian bytes.

The hash input is therefore exactly 158 bytes (190 if section 7 item 2 adds
the price-account id). The Market PDA seed is the
section 2 seed, `SHA-256("market" ‖ marketId)`, and `create_market` fails if
the PDA is already claimed. If section 7 item 2 binds a market to a specific
price account, that account's id has to join these inputs. Otherwise two
markets that differ only in their price account get the same id, and the
first to be created fixes the feed for good.

## 5. Numeric layout

- Assets and shares: `u128`. Intermediates: `U256` (or widening `u128 × u128`
  via a checked helper) for `mul_div`.
- `WAD = 1e18`, `VIRTUAL_SHARES = 1e6`, `VIRTUAL_ASSETS = 1`,
  `ORACLE_PRICE_SCALE = 1e36`, `MAX_FEE = 0.25 WAD`.
- Oracle price: `U256`, as Morpho's `IOracle.price()` returns `uint256`. A
  `u128` price overflows for some common pairs. For example, an 8-decimal
  collateral worth 60,000 whole tokens of a 6-decimal loan token (a
  WBTC/USDC-like pair) has price 6e4 × 10^(36 + 6 − 8) = 6e38, above the
  `u128` maximum of about 3.4e38. So the health check's `collateral × price`
  is a checked `U256` multiply that reverts on overflow, as Morpho's does,
  not the `u128 × u128` helper above.
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

Private transactions (found 2026-10-19). When the sequencer verifies a
private transaction, it rebuilds the pre-state of every public account the
transaction names from the current state
(`lee/state_machine/src/validated_state_diff/mod.rs:401-411`). It then checks
the proof against exactly those states (`:515-532`).

Read-only accounts are included in that set. The circuit emits a public
action for every public input, modified or not
(`lee/privacy_preserving_circuit/src/output.rs:36-42`), and the message's
public account ids come from those actions
(`lee/state_machine/src/privacy_preserving_transaction/message.rs:96-101`).

`CLOCK_01` changes every block. So a private Argo transaction that reads it
has to be included in the block right after the state it was proved
against, or it fails. The options for the private path (M5):
- Read `CLOCK_10` or `CLOCK_50`. These change only every 10 and 50 blocks
  (`lez/programs/clock/src/main.rs:82-84`;
  `lez/programs/clock/core/src/lib.rs:6-13`), trading time resolution for a
  longer proving window.
- Use the caller-passed timestamp below, which avoids naming a clock account
  at all.

In every case, a private proof still fails if the Market changed after it
was made (section 6, Ordering). This is part of section 7 item 4.

Alternative considered: the caller passes the timestamp, and the guest bounds
it with a `timestamp_validity_window` on its `ProgramOutput`. The runtime
checks the window against the timestamp of the block being built
(`lee/state_machine/src/validated_state_diff/mod.rs:202-209`). That timestamp
is in Unix milliseconds: the sequencer takes `timestamp_millis()`
(`lez/sequencer/core/src/lib.rs:845-846`) and passes it in (`:928`). The
upper bound is exclusive
(`lee/state_machine/core/src/program/mod.rs:351-352`). Private messages carry
the same window (`lee/state_machine/src/validated_state_diff/mod.rs:396`). It bounds the value but
does not fix it, trading accuracy against how long the transaction stays
valid. So we use it for deadlines (for example `set_authorization_with_sig`)
and, for now, not for public accrual.

The clock is re-checked with the rest of the runtime when 0.4 ships.

## 6. Sharding argument for P2

A transaction's cost is a function of (a) the guest cycles it executes and
(b) the accounts it reads and writes. Argo has no global list of markets or
positions, so neither (a) nor (b) depends on how many markets exist. The only
per-deployment bound is how much one transaction can span. v0.2.4 has no cap
on the number of accounts in a transaction; it checks that the encoded
transaction fits in a block (`lez/sequencer/service/src/service.rs:55-66`).
The limit that binds is the chained-call cap. The public path enforces
`MAX_NUMBER_CHAINED_CALLS = 10` from `lee/state_machine/src/state/mod.rs:21`,
imported at `lee/state_machine/src/validated_state_diff/mod.rs:25`. A
second constant with the same value sits at
`lee/state_machine/core/src/program/mod.rs:14`. One counter covers every
execution in the transaction, nested or not, the top-level call included
(`lee/state_machine/src/validated_state_diff/mod.rs:106-112, 288-290`). So a public transaction
runs at most 11 program executions, 10 of them chained calls (S-A A6).
The stock private prover stops one call earlier (`>=` at
`lee/state_machine/src/privacy_preserving_transaction/circuit/mod.rs:107`),
so the private path (M5) gets 10 executions.
`flash_loan` uses four chained calls: transfer out, the borrower callback,
the borrower's repayment transfer (issued from inside the callback), and
`repay_flash` (section 3). Any other calls the callback makes count against
the same limit. This caps what a flash loan can span, not what the singleton
can hold. Spike S-D measured per-execution cycles for supply-shaped and
settlement-shaped legs and extrapolated a liquidation at about 0.6M cycles
(an estimate, not a measurement), against v0.2.4's 32M-cycle limit, which
applies to each program execution (S-D). M3 measures the real `liquidate`,
and the maximum market count reported in M3/M7 derives from these limits, not
from any per-market state cost.

Ordering (found 2026-10-19). v0.2.4 does not run transactions in parallel.
The sequencer first takes the system transactions it has pending, then
mempool transactions in order, and validates and applies each one before
the next (`lez/sequencer/core/src/lib.rs:856-930`). A node replaying a block
applies its transactions in block order (`lez/chain_state/src/apply.rs:140-160`).
So every transaction in the zone is serialized, not only one market's, and
sharding does not let two markets' transactions run concurrently. A public
transaction executes against the state at its position in the block
(`lee/state_machine/src/validated_state_diff/mod.rs:77-87`), so two public
transactions on the same Market do not invalidate each other: each runs its
checks on the state the one before left. A later one can still fail its own
checks; a liquidation after a repay is one example. Private transactions do conflict: a proof made against a Market state
that has since changed fails (section 5, Private transactions).

## 7. Open questions for review

1. Authorization: decided by adp, it stays a separate PDA. Morpho's
   `isAuthorized` is keyed by authorizer and manager only, with no market
   (`Morpho.sol:66`), while Position is per market. So putting it inside
   Position would change its meaning.
2. Oracle price account (rekt): a read-only input. Every input arrives as
   `AccountWithMetadata { account, is_authorized, account_id }`
   (`lee/state_machine/core/src/account.rs:125-129`), so the guest
   can check both `price.account.program_owner == market.oracle` and the
   account id. A program-owner check alone is not enough, because it accepts
   any account the oracle program owns, including another pair's feed.
   Proposed, from Kamino klend (commit 77e8185; BUSL-1.1, so ideas only, no
   code): bind each market to its exact price-account id, hash that id into
   `marketId` (section 4), and in the guest check the account id, the owner
   and a layout tag. Check prices per operation:
   - `borrow`, and `withdraw_collateral` with debt: staleness, non-zero and
     any confidence field.
   - `liquidate`: staleness and non-zero only, so a liquidation is not
     blocked by a secondary check.
   - `supply`, `withdraw`, `repay`, `supply_collateral`: no price read.

   Staleness is measured against `CLOCK_01` in milliseconds. It tolerates a
   price published later than the previous block's time, up to a bounded
   skew.
3. Config bound (rekt): data limit is 100 KiB per account, but compute cost
   argues for the fixed small bounds above. Confirm the numbers.
4. Clock source for accrual (rekt): proposed in section 5 (read `CLOCK_01`).
   Section 5 also explains why the private path may need `CLOCK_10` or
   `CLOCK_50`, which give a proving window of up to 10 or 50 blocks depending
   on where in the cycle the proof is made, or the caller-passed timestamp
   instead.
5. Protocol fee placement (rekt): section 2 puts a fee fraction in Config and
   `fee` in Market, and the admin `set_fee` row writes Config. Morpho Blue
   keeps the fee per market and accrues before changing it.

   Proposed, from klend's fee counter: keep the fee per market. Mint
   Morpho's fee shares at accrual, but credit them to a `pending_fee_shares`
   field in Market. A permissionless `claim_fee` then moves them to the
   recipient's Position. This takes Config and the fee recipient's Position
   out of every accruing instruction. It also removes the fee-recipient case
   from Account aliasing (section 3), and means an admin change of the
   recipient can no longer fail in-flight transactions.

   Either way, specify what happens when a fee is set and no recipient is
   (M0's Config allows `fee_recipient: None`).
6. `set_authorization_with_sig`: adp agreed to add it (the section 3 row).
   Open details:
   - LEZ's window is in Unix milliseconds with an exclusive upper bound
     (`lee/state_machine/core/src/program/mod.rs:351-352`). Morpho's
     deadline is in seconds and inclusive (`Morpho.sol:448`). Proposed: sign
     the deadline in milliseconds, and keep it inclusive with
     `to = deadline_ms + 1`.
   - The nonce is per authorizer in Morpho (`Morpho.sol:68`) but per pair if
     kept in the Authorization PDA. Decide which.
   - We found no chain id in v0.2.4's transaction format (found 2026-10-19). A program id is
     the image id of its bytecode (`lee/state_machine/src/program/mod.rs:23-31`),
     so the same build has the same id on every network. So the signed
     message needs a network tag, fixed in the guest or in Config; a tag
     the caller supplies proves nothing.
   - Measure the in-guest signature check.

   Question for adp: since an authorizer can sign `set_authorization`
   directly on v0.2.4, the signed variant matters only for relayed flows.
   Keep it anyway?
7. What we re-check when 0.4 ships: the account and claim model, chained-call
   authorization, fees and compute limits, clock access, the token program,
   and events.
8. Token holdings Argo supports (found 2026-10-19): holdings with their own
   key only, or also ATAs. ATA support means debiting through the ATA
   program with the owner signing, which adds one execution per debit leg to
   section 6's count.
9. Morpho callbacks and flash-loan predictions (found 2026-10-19). Drop the
   supply, repay, supply_collateral and liquidate callbacks in M1 and record
   the deviation, or add them. Adding them puts a callback, a continuation
   and a repayment pull into each row, with flash_loan's prediction and
   authorization constraints. For `flash_loan`, choose between two options:
   - Argo forms the post-callback predictions itself and pins both
     LoanVault and Market. This is simpler, and rules out same-market use
     inside the callback, apart from `supply_collateral`.
   - Argo takes client-simulated predictions in the instruction data, which
     the runtime checks. This also needs a repayment rule that reconciles
     LoanVault with Market's totals.
10. `loan_token == collateral_token` (found 2026-10-19): forbid it in
    `create_market` and record the deviation from Morpho, or support it. Supporting it
    means listing the shared definition once and, in `liquidate`, predicting
    the second transfer's pre-state from the first.
11. Admin (found 2026-10-19). M0's `initialize` accepts any authorised
    account, and
    M0 has no other instruction, so whoever lands the first successful
    `initialize` after deployment is the admin for good. If the admin's own
    account is still unowned, it also claims that account for
    `argo_lending` (`programs/argo_lending/src/lib.rs:10-44`), and then the
    account can never become a token holding. A deployment carries only
    bytecode, so there is no constructor
    (`lee/state_machine/src/program_deployment_transaction/message.rs:4-6`).
    Redeploying the same image fails
    (`lee/state_machine/src/validated_state_diff/mod.rs:446-448`), so
    recovering needs a new image and a new program id. We have not
    established whether a third party can get in ahead of the deployer;
    that depends on sequencer ordering. Proposed for M1: compile the initial
    admin into the guest, and stop claiming the admin's account. Proposed
    with the M2 RFP-001 admin library, from klend: two-step admin transfer,
    and a one-way freeze. klend's `immutable` flag, once set, cannot be unset
    and blocks the market owner's market and reserve config updates; klend's
    global admin keeps a few fee fields and emergency mode
    (klend `programs/klend/src/handlers/handler_update_reserve_config.rs:42-47`).
12. Proposed from klend, for review:
    - Optional `min_seized` / `max_repaid` bounds on `liquidate`.
    - `overflow-checks = true` in the guest's release profile.
    - A per-operation rounding table in section 5 using Morpho's directions
      (debt rounds up).
13. Interest rate models (found 2026-10-19): compile IRMs into
    `argo_lending` (proposed in section 3), or support external IRMs through
    a callback into an Argo continuation, which costs about two executions
    per accrual. Either way it is a deviation from Morpho, which calls the
    IRM for a return value.
