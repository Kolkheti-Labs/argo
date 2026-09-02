# S-D — per-transaction compute budget

## Question
Do a supply-shaped and a liquidate-shaped Argo instruction fit inside one
program execution's cycle limit with a known margin, and what does account
size do to that margin?

## What the source says
- `MAX_NUM_CYCLES_PUBLIC_EXECUTION = 32 * 1024 * 1024` (32M cycles) applied
  as `session_limit` per program execution, so each chained call gets its own
  32M (`lez/lee/state_machine/src/program/mod.rs`). Comment says it becomes
  variable "when fees are implemented".
- Account size drives cost: lez-programs' benchmark notes ~33M extra cycles for
  a program-owned 100 KB account (paging/hashing). `DATA_MAX_LENGTH` is
  100 KiB. Keep every Argo account small.
- Other limits: 10 chained calls (public path allows 11 executions, private
  path 10), block 1 MiB, RPC body 10 MiB, no cap on accounts per tx (open
  issue 622). No fee, no `Message.fee` field.
- Upstream baselines (`docs/benchmarks/cycle_bench.md`): token Transfer
  127,726 cycles; AMM SwapExactInput 508,904; AddLiquidity 643,464; proving
  ≈100 µs/cycle po2-padded on an M2 Pro; ≈53 s per extra chained call.
- Upstream fee PR 801 (open) meters cycles with a budget shared across the
  WHOLE chain, not per leg. Argo's M3 benchmark must report both the per-leg
  max (v0.2.4 rule) and the chain total (future rule).
- LP-0002 measured on v0.2.4: a small guest ~1M cycles; the outer privacy
  circuit 4.7–5.2M cycles and >6 GB RAM to prove. Public txs are proven by
  the sequencer, private ones client-side.

## Experiment
Guest `spike_math` linking `argo_core` + `irm_core` stubs with real-shaped
math (u128 mul_div with U256 intermediates, Taylor-compounded accrual, health
check, LIF derivation) over realistic account layouts:
- D1 supply-shaped: accrue + share math + 1 chained Transfer. Record executor
  cycles at `RISC0_DEV_MODE=1` (r0vm session stats) for the Argo execution and
  for the token execution separately.
- D2 liquidate-shaped: accrue + health + LIF + seize/repay derivation +
  bad-debt branch + 2 chained Transfers, plus an oracle price account read.
- D3 account-size sweep: same D2 with Market account padded to 1 KB, 10 KB,
  50 KB to plot cycles vs data size and fix Argo's size budget.
- D4 flash-loan shape: 10-call chain (the max) to see the per-call overhead.
- D5 one real proof of D2 on hetzner2 (`RISC0_DEV_MODE=0`) for wall-clock
  and RAM, so M3's benchmark has a baseline.

## Observable
Cycles per execution for D1–D4; margin = 32M − max(D2). GO iff D2 < 16M
(50% headroom) with Market ≤ 1 KB, AND the full liquidate chain total
(Argo leg + 2 token legs) < 32M so it survives a tx-wide budget. PARTIAL if 16M–32M (fits, but M3 must
budget carefully). NO-GO if D2 > 32M.

## If no-go
- Two-phase liquidation via self-call (phase 1: price + reserve; phase 2:
  settle + bad debt), each phase under its own 32M. Already in the proposal
  as the funded fallback (§7).
- If even the split does not fit: shrink accounts (split Market totals from
  Market params), move LIF/accrual to fixed-point tables, and report the
  reduced max-market figure honestly in M3/M7.
