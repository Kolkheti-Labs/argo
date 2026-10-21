# S-E — fee-funding-within-deshield

## Question
Can the SDK move value from a private account to a fresh single-use public
account and have that same output pay for the public operation's fee, as one
indivisible user action, so no external linkable source ever funds the
ephemeral account (U9/U10, Privacy P1)?

## What the source says
- v0.2.4 has NO transaction fees. `Message { program_id, account_ids, nonces,
  instruction_data }` + `WitnessSet`; nothing is debited for gas. The
  sequencer pays Bedrock from its own funding key. Native `balance: u128`
  exists (genesis supply, `wallet pinata` faucet) but is unused for fees.
- "shield/deshield" in the wallet are the private↔public token modes
  (`auth-transfer send --from Private/… --to Public/…` and back), not fee
  flows.
- So on v0.2.4 the fee half of the question is vacuous, and the privacy half
  reduces to: a private→public transfer to a never-seen public account, then
  the public op, then a public→private transfer, with nothing else touching
  the ephemeral account.

- **The stock v0.2.4 wallet deshield into a fresh, never-initialised public
  account fails.** `authenticated_transfer` claims a default recipient with
  `Claim::Authorized` (`lez/programs/authenticated_transfer/src/main.rs:50-52`),
  the privacy circuit allows that claim on a public account only if the
  account is authorized (`lee/privacy_preserving_circuit/src/execution_state.rs:397-405`,
  test `unauthorized_public_account_claiming_fails_when_executed_privately`,
  `lee/state_machine/src/state/tests/claiming.rs:281`), and the wallet's
  deshield always passes the recipient as `PublicNoSign`
  (`lez/wallet/src/program_facades/native_token_transfer/deshielded.rs:22`),
  so it is never authorized. With the stock wallet the ephemeral account is
  therefore initialised by its own key first (`wallet auth-transfer init`, a
  separate public tx, free on v0.2.4). A deshield that the recipient co-signs
  is a different case, see the E5 notes. Private recipients do not have this
  problem.
- Upstream fee design (PR 801: open on `dev` when this spike was written on
  2026-09-02; merged 2026-09-03, in v0.2.5-rc1 and v0.3.0, not in v0.2.4):
  `Message.fee: Option<FeeDeclaration { payer, gas_limit, tip, max_fee }>`,
  the payer must sign and hold at least the fee reserve. See the E5 notes.

## Experiment
On the standalone sequencer with the v0.2.4 wallet:
- E0 init: A's key submits `Initialize` (public tx, no funds needed on v0.2.4).
  Record that this tx exists on-chain and what it reveals (A's id only).
- E0b negative: skip E0 and deshield straight to a default A with the stock
  wallet → expect it to fail (the 2026-09-02 plan said "dropped"; the run
  showed a client-side rejection while proving, see Observable). This
  confirms the claim rule for the stock wallet deshield and decides whether
  the SDK must do E0 when it uses that path.
- E1 deshield: private account P → public account A (in M0 a fresh wallet key
  from `wallet account new public`; M5's SDK derives A from a nonce). Confirm
  A is funded.
- E2 interact: A signs a public operation. In M0 that operation is a native
  transfer A → B (B initialised first), not an Argo instruction; what it
  proves is that the deshield output can act as a public signer.
- E3 reshield: A → private P' (a different note), A left at zero.
- E4 linkability check (NOT evaluated in M0): from the public chain data
  alone, list every account that co-appears with A. Raw blocks are saved
  (`evidence/localnet/S-E-blocks.json`) but decoding them needs a borsh
  decoder that M5's SDK work provides.
- E5 record for the design doc how a future fee would have to be paid:
  which account the sequencer would debit if fees land on `balance`, and
  whether the deshield output can carry native balance plus token holding in
  one tx (the "deposit token + native gas" atomic action the RFP describes).

## Observable
E0b was rejected while the wallet was proving (circuit panic `Cannot claim
unauthorized account`, no tx submitted; `evidence/localnet/S-E-e0b.wallet.txt`),
and E1–E3 landed with A's balances 300 then 0 (the script exits non-zero
otherwise). Because E4 is not evaluated, the best M0 verdict is PARTIAL: the
init step is established for the stock wallet deshield, linkability is not.
The fee sub-question is "not applicable on v0.2.4"; see the E5 notes.

## E5 notes (added 2026-10-16, review round 1)

E5 asks which account a fee would be debited from, and whether one deshield
can carry native balance and a token holding together. The review also asks
how the ephemeral account's own init is paid once fees exist.

**On v0.2.4, the version M0 ran and M1 builds on.** No v0.2.4 message kind
has a fee field (public `Message` is `program_id, account_ids, nonces,
instruction_data`, `lee/state_machine/src/public_transaction/message.rs:14-19`;
the privacy-preserving and program-deployment messages have none either), so
nothing pays a fee and E0, A's own-key init, is free. What E0b measured is
narrower than "a fresh account must be initialised first": it is the stock
wallet deshield, which passes A as `PublicNoSign`. The circuit does accept
the claim when the fresh public recipient is authorized: the in-tree test
`authorized_public_account_claiming_succeeds_when_executed_privately`
(`lee/state_machine/src/state/tests/claiming.rs:297`) claims and funds an
`Account::default()` recipient that signs, in one privacy-preserving tx
(with a test program, not `authenticated_transfer`). A deshield that A
co-signs may therefore remove the separate init tx. We have not run it. Its
privacy cost is not evaluated: A's key signs the private tx, and upstream
treats a recipient signature on a deshield as a link leak (LEZ v0.3.0
`integration_tests/tests/native_transfer/private.rs:219-224`,
`deshielded_transfer_does_not_sign_with_recipient_key`).

**Once fees exist (conditional, not measured).** The init question only
arises on a chain that has both a claim step for fresh public accounts and a
fee on the tx that claims. If the init is a separate public tx signed only by
A, A has nothing to pay with, so another account would have to be the fee
payer and would appear next to A on the public chain. Three routes avoid an
outside payer, none tested in M0:
1. a fee-exempt deshield that claims and funds A in one tx (the co-signed
   deshield above);
2. no claim step at all, so the deshield credits A and A pays its own next
   fee;
3. the deshield's fee paid from the private side, inside the proof.

Which one applies depends on the LEZ version Argo ships on. S-E (E0b, E1–E3,
E5) is re-run on that version; for now that means 0.4 when it is published.

**Upstream context (read from LEZ v0.3.0 source, not run; v0.3.0 is not an
Argo target).**
- Which account is debited: a charged public tx carries
  `FeeDeclaration { payer, gas_limit, tip, max_fee }`
  (`lee/state_machine/src/fees.rs:13`); the payer must sign it
  (`is_fee_authorized`, `fees.rs:49`) and hold at least the fee reserve in its
  native balance at admission (`UnauthorizedPayer` / `PayerCannotFund`,
  `lez/sequencer/core/src/fees.rs:66,76`). A user public tx that is not an
  exempt shape (system injection, cross-zone lock, sequencer-stake call;
  `lez/chain_state/src/classify.rs:61-77`) and carries no declaration is
  rejected (`MissingFeeDeclaration`, `classify.rs:81`); an Argo transaction
  is not an exempt shape.
- Privacy-preserving txs, deshields included, are fee-exempt under an interim
  policy "pending the private-payer (Q3) decision"
  (`lez/chain_state/src/classify.rs:3-4`).
- Accounts are `Account { nonce, data }` with no owner field
  (`lee/state_machine/core/src/account.rs:106-109`), and upstream notes that
  "`Claim` no longer exists" (`integration_tests/tests/bridge.rs:398-400`),
  which suggests route 2.
  No in-tree test asserts a native deshield into a never-used account.
- LEZ PR 949 (open on `dev` as of 2026-10-16) would make every
  privacy-preserving tx pay its fee inside its proof, with the wallet picking
  the first authorized private account that has a native balance, else the
  public fee payer. When a private account pays, that is route 3; the
  public-payer fallback is the outside-payer case.

**Native balance plus a token holding in one deshield.** Not checked in M0.
M5's SDK work decides it on the version it targets.

## If no-go
- If A, once claimed by `authenticated_transfer` (at the E0 init, or at E1 if
  a co-signed deshield claims it), cannot be used by Argo as a plain signer: the SDK must have A's first tx be the Argo call
  (which claims it `Authorized` via SPEL's `ClaimedIfDefault`) and fund it in
  the same chain, or Argo's instruction must accept a token-program-owned
  signer. Decide in M5's SDK design; note in M1's instruction layout.
- If E4 leaks (e.g. nonces reveal ordering across A and P): document in the
  privacy properties doc as a protocol-level leak the SDK cannot close.
