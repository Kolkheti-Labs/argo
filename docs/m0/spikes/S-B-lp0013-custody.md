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
- This is the "anyone can credit, only the owner can debit" rule, enforced by
  the runtime today, without waiting on LP-0013.
- LP-0013 (lambda-prize) is closed with an accepted solution (PR 56, a
  mint-authority model + `lez-authority` crate). Its target PR into
  lez-programs (PR 125) was closed unmerged, yet lez-programs `main` today has
  `NewFungibleDefinition { mint_authority }`, `SetAuthority`, …, so authority
  landed by another route. Nothing of it is in the v0.2.4 builtin token, and
  the two `TokenDefinition` layouts are Borsh-incompatible. B5 settles which
  one the testnet actually runs.
- Watch upstream PR 817 (open): changes rule 5 to authorization-gated balance
  decrease and removes `Claim`; the author flags a squatting race. If it
  ships, the claim-first ordering in B2 changes.
- LP-0002 lesson: a fresh PDA funded by plain transfer gets claimed
  `Authorized` by the transfer program and can then never be PDA-debited.
  Claim first, fund second.

## Experiment
Reuse `spike_vault` from S-A, in-process (`spikes/integration_tests`), against
the v0.2.4 builtin token (`programs::token()`, the one the testnet registers).
The lez-programs authority token is exercised only if B5 shows the testnet
runs it:
- B1 claim-then-credit: `Init` (chained `InitializeAccount`, PDA seed), then a
  plain user `Transfer` into the vault → accepted.
- B2 credit-before-claim: plain `Transfer` into a never-initialised PDA →
  observe what owns it afterwards; then `Init` → expect rejection (already
  owned). This pins the ordering rule for `create_market`.
- B3 authorised debit: Argo `PayOut` → accepted.
- B4 unauthorised debit: user-signed `Transfer` out of the vault → rejected;
  foreign program chained `Transfer` out of the vault → rejected.
- B5 which token: does the live testnet's token program id match the builtin
  or lez-programs' token? Compare `get_program_ids` on
  `https://testnet.lez.logos.co` with both image ids.

## B5 evidence (captured 2026-09-02T18:15Z, tip block 34812)
`evidence/testnet/getProgramIds-20260902T181548Z.json`: the testnet registers
exactly `amm`, `authenticated_transfer`, `pinata`, `privacy_preserving_circuit`,
`token`. No `clock`, `ata`, or `vault` program, although v0.2.4's
`testnet_initial_state` lists them. Consequences: (a) the token id to compare
against the builtin is in that file; (b) **Argo's lazy accrual cannot read a
`CLOCK_01` account on today's testnet** — the guest must take elapsed time
from `timestamp_validity_window` / block-derived input instead, or Argo ships
its own clock reference. Carry this into the state-layout spec §5 and M2.

## Observable
B1, B3 accepted; B2's second step and B4 rejected, with `getAccount` snapshots.
B5 yields the program id Argo must target (compare `getProgramIds` on the
testnet with the builtin's id and lez-programs' `TOKEN_ID`). GO iff B1–B4 hold on the chosen
token program.

## If no-go
- If debit-by-seed fails on the chosen token: Argo cannot custody; fall back
  to holding tokens in Argo-owned accounts whose `data` Argo itself manages
  (rule 6 permits it), which means Argo would need its own transfer logic and
  the vaults would not be standard token holdings. Escalate, since this is the
  RFP's declared hard blocker.
- If the testnet token is the builtin without authorities: Argo targets the
  builtin for M1–M3 and keeps `token_core` behind one adapter module so the
  switch to the authority token is a one-file change before any real value.
