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
- So today the fee half of the question is vacuous, and the privacy half
  reduces to: a private→public transfer to a never-seen public account, then
  the public op, then a public→private transfer, with nothing else touching
  the ephemeral account.

- **A deshield to a fresh, never-initialised public account fails on v0.2.4.**
  The transfer program claims a default recipient with `Claim::Authorized`,
  and a public-account claim requires `pre.is_authorized`; a `PublicNoSign`
  recipient is never authorized (`validated_state_diff/mod.rs:225-231`, test
  `unauthorized_public_account_claiming_fails`). Both in-tree deshield tests
  target already-initialised accounts. So the ephemeral account must first be
  initialised by its own key (`wallet auth-transfer init`, a separate public
  tx, free today). Private recipients do not have this problem.
- Upstream fee design (PR 801, open on `dev`): `Message.fee: Option<FeeDeclaration
  { payer, gas_limit, tip, max_fee }>`, payer must be a signer with balance
  ≥ reserve, budget shared across the whole chain, private txs possibly
  fee-exempt. Not in v0.2.4.

## Experiment
On the standalone sequencer with the v0.2.4 wallet:
- E0 init: A's key submits `Initialize` (public tx, no funds needed today).
  Record that this tx exists on-chain and what it reveals (A's id only).
- E0b negative: skip E0 and deshield straight to a default A → expect the tx
  to be dropped (confirms the claim rule; decides whether the SDK must always
  do E0).
- E1 deshield: private account P → public account A (derived by the SDK
  from a nonce). Confirm A is funded and its `program_owner` after this step.
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
E0b rejected, E1–E3 landed with A's balances 300 then 0 (the script exits
non-zero otherwise). Because E4 is not evaluated, the best M0 verdict is
PARTIAL: the init-step design change is established, linkability is not. The fee sub-question is recorded as
"not applicable on v0.2.4, design reserved" with the E5 notes.

## If no-go
- If A is claimed by the transfer program on E1 such that Argo cannot later
  treat it as a plain signer: the SDK must have A's first tx be the Argo call
  (which claims it `Authorized` via SPEL's `ClaimedIfDefault`) and fund it in
  the same chain, or Argo's instruction must accept a token-program-owned
  signer. Decide in M5's SDK design; note in M1's instruction layout.
- If E4 leaks (e.g. nonces reveal ordering across A and P): document in the
  privacy properties doc as a protocol-level leak the SDK cannot close.
