# S-A — ChainedCall re-entry and revert/atomicity

## Question
Is a transaction that emits chained calls all-or-nothing on LEZ v0.2.4, and may
a chained call target the caller program again (re-entry) so a flash-loan
repayment continuation and a two-phase liquidation are expressible?

Stated so it can be false: "a failed later leg leaves earlier legs' state
writes applied" or "a program cannot chain into itself / be chained back into".

## What the source says (to be confirmed by running it)
- `ChainedCall { program_id, pre_states, instruction_data, pda_seeds }`,
  `ChainedCall::new(program_id, pre_states, &instruction)` encodes with risc0
  serde of the callee's `Instruction` enum. `MAX_NUMBER_CHAINED_CALLS = 10`.
  (`lez/lee/state_machine/core/src/program/mod.rs`)
- Execution is depth-first in declared order; each execution has its own
  32M-cycle session. The whole tx builds one `ValidatedStateDiff`; any error
  or panic anywhere in the chain returns `Err` and nothing is applied.
- Re-entry is allowed: LEZ's own `flash_swap_initiator` example does
  token → callback → self `InvariantCheck`. There is no reentrancy guard.
- The caller must supply the callee's `pre_states` byte-exactly, including the
  effects of earlier calls in the chain, or the runtime rejects with
  `InconsistentAccountPreState`.
- `ProgramContext.caller_program_id` is available to a handler and is verified
  by the runtime, so an internal entrypoint can assert
  `caller_program_id == self_program_id`.
- LP-0015 is closed as "delivered": the mechanism is self-call plus
  `caller_program_id`. There is no capability, ticket, or return-value API.

- In-tree proof of rollback: `flash_swap_callback_keeps_funds_rollback`
  (`lee/state_machine/src/state/tests/flash_swap.rs:55-108`). Depth test:
  `execution_fails_if_chained_calls_exceeds_depth` (`tests/claiming.rs:143`).
  A1–A7 below re-run these facts against a *sequencer*, not the in-process
  state machine, because a failed tx is dropped from the block with no
  receipt (open issue 160: no error propagation), which is what the client
  will actually observe.
- Flash-loan constraint: because the caller must precompute the repay leg's
  pre-states, the `repay_flash` self-call can only reference accounts whose
  post-callback state is deterministic (the vault and the borrower's holding
  at exact expected balances). A borrower callback that touches those
  accounts differently makes the whole tx fail, which is the desired outcome.

## Experiment
Two layers. (1) The rule verdicts come from the in-process LEZ state machine
(`lee::V03State`, `RISC0_DEV_MODE=1`) in `spikes/integration_tests/tests/spike_ab.rs`,
which executes the real guest ELF and the real builtin token program and
returns the executor's error type, so every negative case is exact and
repeatable in seconds. (2) One run of the happy path and one of the A2
failure against the standalone sequencer, to record what a *client* sees
(the failed tx never appears in a block; `getTransaction` stays null).

Program `spike_vault` (SPEL guest) with instructions:
1. `Init` — claim a vault PDA under the token program via chained
   `token::InitializeAccount` with `pda_seeds=[vault_seed]`.
2. `PayOut { amount, then_fail: bool }` — write a counter into its own state
   account, chain `token::Transfer` vault → recipient (PDA-authorised), then
   chain a self-call `Internal { must_fail: then_fail }` that panics when asked.
3. `Internal` — asserts `caller_program_id == self_program_id`, panics if
   `must_fail`.

Runs on the standalone sequencer at `RISC0_DEV_MODE=1`:
- A1 happy path: `PayOut { then_fail: false }` → counter incremented, tokens
  moved.
- A2 late failure: `PayOut { then_fail: true }` → assert counter unchanged
  AND vault balance unchanged (read over `getAccount`, not wallet).
- A3 re-entry: `Internal` reached through the chain succeeds; `Internal`
  submitted top-level (caller = default id) is rejected.
- A4 wrong seed: `PayOut` with a wrong `pda_seeds` entry is rejected.
- A5 forged debit: a second program tries to chain a `Transfer` out of the
  vault with the vault marked authorized but no matching seed → rejected.
- A6 chain length: 11 chained calls → rejected; 10 → accepted.
- A7 pre-state prediction: chain Transfer twice from the same vault with the
  second call's pre_state reflecting the first → accepted; without → rejected
  with `InconsistentAccountPreState`.

## Observable
For each of A1–A7: sequencer accept/reject plus `getAccount` snapshots of
counter, vault, recipient before and after. GO iff A2 shows zero state change
and A3 shows the top-level call rejected.

## If no-go
- Partial commit on late failure: settlement must be re-ordered so the token
  legs precede any Argo state write, and every instruction becomes
  "state write as the last leg via self-call". Cost: one extra chained call per
  instruction, ~+1 execution's overhead.
- No re-entry: flash loans (F14) cannot be built as specified; escalate to
  Logos, since the RFP text presumes LP-0015 semantics. Two-phase liquidation
  would need to be two transactions, which breaks R5 atomicity, so liquidation
  must fit one execution (see S-D).
