# S-C — LP-0012 event surface

## Question
Does LEZ v0.2.4 give programs a structured event emission that clients can
read, so the reference liquidator and risk monitor can observe state changes
without polling every PDA?

## What the source says
- `ProgramOutput` in v0.2.4 has no events field; the guest only does
  `env::commit`. SPEL has no event macro.
- LP-0012 was awarded to a fork (`bristinWild/logos-execution-zone`, an
  `lez-events` crate and a `getTransactionReceipt` RPC). It is NOT merged
  upstream. The RFP lists LP-0012 as "closed", which is true of the prize, not
  of the runtime.
- Sequencer RPC surface: `send_transaction`, `get_transaction`,
  `get_account`, `get_block`, `get_block_range`, `get_accounts_nonces`,
  `get_program_ids` (`lez/sequencer/service/rpc/src/lib.rs`). A failed tx
  surfaces only as a sequencer log line.

- Upstream `dev` merged events after v0.2.4 (PR 705 `ProgramOutput.events`,
  PR 707 `ProgramEvent { selector: [u8;8], data }`, indexer `getEvents` in
  PR 713/785/709). Events exist only for successful public txs and are
  dropped for privacy-preserving txs. No tag ships them yet.
- Failed txs are dropped from the block entirely: no receipt, no error.
  `getTransaction` returns `Option<(LeeTransaction, BlockId)>`.

## Experiment
- C1 confirm on the standalone sequencer that `getTransaction` for an executed
  Argo-spike tx returns no event/log payload beyond the message and status.
- C2 measure the polling baseline: 200 sequential `getAccount` calls for one
  account against the local sequencer, giving a per-call latency (the
  multi-account sweep and the block-tail decoder are M4 work, not M0).
- C3 (not done in M0) ask upstream whether the events API lands in the next
  tag; the `dev` branch already has it.

## Observable
C1: `getTransaction` returns only the submitted transaction and its block
id; there is no receipt object to carry events, and `ProgramOutput` in the
source has no event field. C2: per-call `getAccount` latency.
GO means "events exist"; the expected verdict is NO-GO with the polling
design below adopted.

## If no-go (expected)
- Observation layer = block-tail follower: decode every tx in each new block,
  map `message.account_ids` to known Argo PDAs, refresh only those. Full
  sweep on start and after any gap. This keeps the liquidator O(txs per
  block), not O(positions).
- Market/Position PDAs are deterministic, so discovering positions needs an
  index: the monitor records every `(marketId, owner)` it sees in a tx.
- Design the observation layer behind a trait with two impls: `PdaPoller`
  (M4 default) and `EventFollower` (wired when the first LEZ tag ships
  `getEvents`). Event selectors are `sha256("<program>::<Event>")[..8]`, so
  Argo's event names can be fixed in M1 even before the runtime supports them.
- Re-check at the start of M4 which tag the testnet runs.
