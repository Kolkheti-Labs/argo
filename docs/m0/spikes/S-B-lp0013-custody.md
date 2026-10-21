# S-B — LP-0013 custody rule

## Question
On the token program Argo will actually use, can anyone credit a program-owned
vault, and can only Argo (by PDA seed) debit it? And which token program is
that: the LEZ v0.2.4 builtin or lez-programs' authority-bearing token?

## What the source says
- LEZ rules: balance decrease only if `program_owner == executing program`
  (rule 5); data change only if owner is the executing program or the account
  is default (rule 6). Runtime: `Claim::Pda(seed)` requires
  `account_id == for_public_pda(callee_program, seed)`; only default-owned
  accounts can be claimed; `program_owner` never changes afterwards.
- Vault pattern used by Logos (`lez-programs/programs/stablecoin/src/open_position.rs`,
  `ata/src/transfer.rs`): vault = Argo PDA, `program_owner` = token program,
  claimed by a chained `InitializeAccount` with `pda_seeds=[seed]`; credit =
  anyone chains `Transfer` to it; debit = Argo chains `Transfer` with
  `sender = vault, is_authorized = true` and `.with_pda_seeds(vec![seed])`.
- This is the "anyone can credit, only the owner can debit" rule. On v0.2.4 it
  rests on two checks, without waiting on LP-0013: the runtime accepts a PDA
  as authorised only under the seed of the program that derives it
  (`InvalidAccountAuthorization`, test a4), and the builtin token's `Transfer`
  requires an authorised sender (test a5_b4).
- LP-0013 (lambda-prize) is closed with an accepted solution (PR 56, a
  mint-authority model + `lez-authority` crate). Its target PR into
  lez-programs (PR 125) was closed unmerged, yet lez-programs `main` today has
  `NewFungibleDefinition { mint_authority }`, `SetAuthority`, …, so authority
  landed by another route. Nothing of it is in the v0.2.4 builtin token, and
  the two `TokenDefinition` layouts are Borsh-incompatible. M0 did not
  establish which one a deployed chain runs (B5 was withdrawn, see below).
- Upstream PR 817 (open when this spike was written on 2026-09-02; merged to
  `dev` 2026-09-04, in v0.2.5-rc1 and v0.3.0, not in v0.2.4) makes balance
  decreases authorization-gated and removes `Claim`; the author flags a
  squatting race. B2's claim-first ordering is established for v0.2.4 only
  and is re-tested on the version Argo ships on.
- LP-0002 lesson: claim first, fund second. On v0.2.4 a plain transfer into
  an unclaimed PDA does not capture it: the transfer asks to claim the
  recipient `Authorized`, a PDA cannot be authorised at top level, so the
  transfer is rejected (`ClaimedUnauthorizedAccount`) and the PDA stays default
  (B2). Whether an early credit can capture a PDA on a version without `Claim`
  (PR 817) is re-tested on the version Argo ships on.

## Experiment
Reuse `spike_vault` from S-A, in-process (`spikes/integration_tests`), against
the v0.2.4 builtin token (`programs::token()`). The lez-programs authority
token was not exercised in M0:
- B1 claim-then-credit: `Init` (chained `InitializeAccount`, PDA seed), then a
  user-signed credit through the spike's `PayIn`, which chains a token
  `Transfer` from the user's holding into the vault → accepted. A top-level
  token `Transfer` into the claimed vault was not run in M0.
- B2 credit-before-claim: plain `Transfer` into a never-initialised PDA →
  observed outcome: the transfer itself is REJECTED (`ClaimedUnauthorizedAccount`,
  a PDA cannot be claimed `Authorized` at top level), the PDA stays default,
  and a subsequent `Init` succeeds. So the ordering rule for `create_market`
  is "claim first", and a stray early credit cannot poison the PDA.
- B3 authorised debit: Argo `PayOut` → accepted.
- B4 unauthorised debit: user-signed `Transfer` out of the vault → rejected.
  A foreign program chaining a `Transfer` out of the vault was NOT built in
  M0 (see S-A A5).
- B5 which token the testnet runs: withdrawn on 2026-10-16, see below.

## B5 withdrawn (2026-10-16)
B5 compared `programs::token().id()` with the `token` entry of the public
testnet's `getProgramIds` reply, captured 2026-09-02
(`evidence/testnet/getProgramIds-20260902T181548Z.json`). On v0.2.4 that RPC
does not read chain state. It returns a fixed map of five ids compiled into
the sequencer (`lez/sequencer/service/src/service.rs:199-214`,
`// TODO: Get programs from state`): four builtin programs and the
privacy-preserving circuit. The 2026-09-02 reply has exactly those five keys.
The comparison could only show that the answering sequencer was built with the same token program image. It said
nothing about which programs the chain registered or which program owns token
holdings, so the test was removed, and both conclusions drawn from the
capture are withdrawn:
- "the testnet runs the builtin token": not established by M0.
- "no clock on the testnet": unsupported. The hardcoded map never lists
  `clock`, so its absence from the reply means nothing. On v0.2.4 the
  sequencer's genesis (`initial_state_testnet`, `lez/testnet_initial_state/src/lib.rs`)
  registers `clock`, `ata` and `vault` (lines 264-285) and inserts the
  `CLOCK_01/10/50` accounts (lines 256-260), and the sequencer ends every
  block with a clock transaction (built at `lez/sequencer/core/src/lib.rs:845-849`,
  appended last at `:958-961`). Our own v0.2.4 localnet shows the clock part:
  all 162 blocks in `evidence/localnet/S-E-blocks.json` contain the three
  clock account ids (the `ata` and `vault` registrations were not observed).
  M0 did not read `CLOCK_01` on the public testnet, which Logos reported
  reset (Discord, 2026-10-10).

A real check needs chain state: `getAccount` on a token holding created on
the target chain (its `program_owner`), and `getAccount` on `CLOCK_01` twice
(its `block_id` advancing). We run it on the relaunched 0.2 testnet once Logos
publishes it. The clock proposal (pending rekt's review and that live check)
is in `docs/spec/state-layout.md` §5.

## Observable
B1, B3 accepted; B2's credit and B4 rejected, with balances and account state
asserted before and after in `spike_ab.rs` (no state snapshots are saved to
`evidence/`).
GO iff B1–B4 hold on the v0.2.4 builtin token.

## If no-go
- If debit-by-seed fails on the chosen token: Argo cannot custody; fall back
  to holding tokens in Argo-owned accounts whose `data` Argo itself manages
  (rule 6 permits it), which means Argo would need its own transfer logic and
  the vaults would not be standard token holdings. Escalate, since this is the
  RFP's declared hard blocker.
- If the chain Argo ships on runs a token program other than the v0.2.4
  builtin (for example lez-programs' authority token): M1 keeps `token_core`
  behind one adapter module that it adds, so the switch is planned as a
  one-module change before any real value (spec §2, "Token program"). Which
  token program the relaunched 0.2 testnet and 0.4 run is to be confirmed
  with Logos.
